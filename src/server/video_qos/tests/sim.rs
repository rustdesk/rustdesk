//! Closed-loop network simulation for the QoS controller.
//!
//! The controller is driven the way `Connection` drives it: one TestDelay probe per
//! second, a single probe outstanding, `user_delay_response_elapsed` on every timer
//! tick, `update_display_data` once per second.  Video frames and probes share one
//! FIFO, which stands for the downstream shared path (stream, transport, link): the
//! probe measures the bytes that were handed to that path in front of it.  It is not
//! the server's `tx_video` channel, which the probe does not pass through, and the
//! model does not stall the timer while a send is blocked, as the real loop does.
//!
//! Three independent random streams keep an A/B comparison paired: the network
//! trace (capacity wobble, stalls, loss events) is generated before the run from the
//! network stream alone, probe jitter is a per-second table from its own stream,
//! and scene changes follow the wall clock, so two controllers with the same seed
//! face the same link, the same jitter and the same content timeline whatever they
//! decide.  Only the frame size noise depends on how many frames were produced.
//!
//! The encoder model conserves its bitrate budget: a scene change costs three
//! frames' worth of data and the surplus is repaid by the following frames, so the
//! long-term offered load does not depend on the frame rate under CBR.
//!
//! It still is a model, not a network: it does not reproduce a real transport's
//! congestion control or a real encoder.  Its job is to show how the controller
//! reacts to the *kind* of behaviour a home Wi-Fi, a stable relay or a saturated
//! uplink produce, deterministically and over many seeds.
//!
//! Against overfitting: the CI run uses seeds 1 to 20; `robustness.rs` applies the
//! same bounds to seeds 21 to 120 and sweeps the scenario parameters.  Scenario
//! parameters are educated guesses until a recorded `qos_trace` calibrates them.
use super::*;

/// xorshift64* generator, so the tests need no external crate and stay reproducible.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng((seed ^ 0x9E37_79B9_7F4A_7C15).max(1))
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform in `[0, 1)`.
    fn uniform(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.uniform()
    }

    fn normal(&mut self) -> f64 {
        let u1 = (1.0 - self.uniform()).max(1e-12);
        let u2 = self.uniform();
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }

    fn log_normal(&mut self, median: f64, sigma: f64) -> f64 {
        median * (sigma * self.normal()).exp()
    }

    fn exponential(&mut self, mean: f64) -> f64 {
        -mean * (1.0 - self.uniform()).max(1e-12).ln()
    }
}

#[derive(Clone)]
pub struct Link {
    /// Step schedule `(from_ms, kbps)`, sorted by time.
    pub capacity_kbps: Vec<(u32, f64)>,
    /// Slow random walk of the capacity, as a fraction of the nominal value.
    pub wobble: f64,
    pub base_rtt_ms: f64,
    /// Log-normal jitter added to every probe round trip.
    pub jitter_median_ms: f64,
    pub jitter_sigma: f64,
    /// Loss events per second.  A reliable stream turns a loss into a 200-400 ms
    /// retransmission stall followed by a second at half rate.
    pub loss_per_s: f64,
    /// Mean interval between link stalls in seconds, `0` for none.
    pub stall_mean_interval_s: f64,
    /// Uniform stall duration range in milliseconds.
    pub stall_ms: (f64, f64),
}

impl Link {
    fn capacity_at(&self, now_ms: u32) -> f64 {
        self.capacity_kbps
            .iter()
            .rev()
            .find(|(from, _)| *from <= now_ms)
            .map(|(_, kbps)| *kbps)
            .unwrap_or(self.capacity_kbps[0].1)
    }

    /// Time at which the capacity was last restored to its initial value, if it ever dropped.
    fn restore_ms(&self) -> Option<u32> {
        let initial = self.capacity_kbps[0].1;
        let mut dropped = false;
        for (from, kbps) in &self.capacity_kbps {
            if *kbps < initial {
                dropped = true;
            } else if dropped && *kbps >= initial {
                return Some(*from);
            }
        }
        None
    }
}

/// Everything the link does during a run, decided before the run starts.
struct LinkTrace {
    capacity_kbps: Vec<f64>, // per tick, wobble and retransmission backoff applied
    stalled: Vec<bool>,      // per tick
}

fn mark(flags: &mut [bool], from_ms: f64, to_ms: f64) {
    let from = (from_ms / TICK_MS as f64).max(0.0) as usize;
    let to = ((to_ms / TICK_MS as f64).ceil() as usize).min(flags.len());
    for flag in flags.iter_mut().take(to).skip(from) {
        *flag = true;
    }
}

fn link_trace(link: &Link, ticks: usize, rng: &mut Rng) -> LinkTrace {
    let mut capacity_kbps = vec![0.0; ticks];
    let mut stalled = vec![false; ticks];
    let mut backoff = vec![false; ticks];
    let mut wobble = 0.0_f64;
    for (i, capacity) in capacity_kbps.iter_mut().enumerate() {
        let now = i as u32 * TICK_MS;
        if now % 100 == 0 {
            wobble = (wobble + rng.normal() * 0.03).clamp(-link.wobble, link.wobble);
        }
        *capacity = link.capacity_at(now) * (1.0 + wobble);
    }
    if link.stall_mean_interval_s > 0.0 {
        let mut start = rng.exponential(link.stall_mean_interval_s) * 1000.0;
        while start < (ticks as f64) * TICK_MS as f64 {
            let len = rng.range(link.stall_ms.0, link.stall_ms.1);
            mark(&mut stalled, start, start + len);
            start += rng.exponential(link.stall_mean_interval_s) * 1000.0;
        }
    }
    if link.loss_per_s > 0.0 {
        let per_tick = link.loss_per_s * TICK_MS as f64 / 1000.0;
        for i in 0..ticks {
            if rng.uniform() < per_tick {
                let start = (i as u32 * TICK_MS) as f64;
                let len = rng.range(200.0, 400.0);
                mark(&mut stalled, start, start + len);
                mark(&mut backoff, start + len, start + len + 1000.0);
            }
        }
    }
    for (capacity, backoff) in capacity_kbps.iter_mut().zip(&backoff) {
        if *backoff {
            *capacity *= 0.5;
        }
    }
    LinkTrace {
        capacity_kbps,
        stalled,
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Content {
    /// Every frame changes: a video call or a movie.
    Video,
    /// Mostly static: a couple of changed frames per second.
    Office,
}

/// How the encoder turns a bitrate into frame sizes.
#[derive(Clone, Copy, PartialEq)]
pub enum EncoderModel {
    /// VP8, VP9 and AV1 run CBR against millisecond timestamps: fewer frames per
    /// second means bigger frames, the bitrate stays.  Only the ratio moves bytes.
    Cbr,
    /// Hardware encoders configured for a fixed 30 fps rate-control assumption:
    /// every frame carries a thirtieth of the bitrate, so fewer frames mean fewer
    /// bytes.  Actual hardware behaviour is backend dependent (Android's MediaCodec
    /// path runs VBR).
    FixedRate,
}

#[derive(Clone)]
pub struct Scenario {
    pub name: &'static str,
    pub seconds: u32,
    pub limit: u32,
    pub quality: Quality,
    pub abr: bool,
    pub content: Content,
    pub encoder: EncoderModel,
    pub link: Link,
    pub seed: u64,
}

/// Bitrate at ratio 1.0; balanced quality (0.67) then encodes at about 4 Mbps.
const BASE_KBPS: f64 = 6000.0;
/// The frame rate hardware encoders are configured for.
const ENCODER_CONFIGURED_FPS: f64 = 30.0;
/// Log-normal spread of frame sizes around their budget.
const FRAME_SIZE_SIGMA: f64 = 0.35;
const TICK_MS: u32 = 10;
/// Samples taken before this instant belong to the cold start, not the steady state.
const WARM_UP_MS: u32 = 15_000;
/// A recovery counts once target and queue have held for this long.
const SUSTAINED_MS: u32 = 5_000;
/// Seeds every scenario is run with.
pub const SEEDS: std::ops::RangeInclusive<u64> = 1..=20;

/// Frame sizes with a conserved bitrate budget.
struct Encoder {
    model: EncoderModel,
    content: Content,
    rng: Rng,
    next_scene_ms: u32,
    debt_bits: f64,
}

/// A scene change every five seconds of video.
const SCENE_INTERVAL_MS: u32 = 5_000;

impl Encoder {
    fn frame_bits(&mut self, now_ms: u32, bitrate_kbps: f64, produce_rate: f64) -> f64 {
        let target = match (self.content, self.model) {
            // A changed region of a static screen is small whatever the rate control does.
            (Content::Office, _) => bitrate_kbps * 1000.0 / ENCODER_CONFIGURED_FPS * 0.3,
            (Content::Video, EncoderModel::Cbr) => bitrate_kbps * 1000.0 / produce_rate,
            (Content::Video, EncoderModel::FixedRate) => {
                bitrate_kbps * 1000.0 / ENCODER_CONFIGURED_FPS
            }
        };
        // Mean one: the spread must not change the offered load.
        let noise = self.rng.log_normal(1.0, FRAME_SIZE_SIGMA)
            * (-FRAME_SIZE_SIGMA * FRAME_SIZE_SIGMA / 2.0).exp();
        let mut bits = target * noise;
        // Scene changes follow the wall clock, not the frame count, so every
        // controller meets the same content timeline.
        let scene_change = self.content == Content::Video && now_ms >= self.next_scene_ms;
        if scene_change {
            self.next_scene_ms += SCENE_INTERVAL_MS;
            // A scene change costs a few frames' worth of data; rate control claws
            // it back from the frames that follow.
            bits *= 3.0;
            self.debt_bits += bits - target;
        } else if self.debt_bits > 0.0 {
            let repay = self.debt_bits.min(target * 0.5).min(bits * 0.5);
            bits -= repay;
            self.debt_bits -= repay;
        }
        bits
    }
}

struct Packet {
    bits: f64,
    enqueued_ms: u32,
    probe_sent_ms: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct Report {
    pub name: String,
    pub seed: u64,
    pub limit: u32,
    /// Controller target, sampled every 100 ms after the warm-up.
    pub mean_target_fps: f64,
    pub p10_target_fps: u32,
    pub min_target_fps: u32,
    /// Share of the measured time the target spent below half of the limit.
    pub below_half_pct: f64,
    /// Frames the encoder produced per second.
    pub produced_fps: f64,
    /// Frames that left the shared path per second.
    pub delivered_fps: f64,
    /// Time a delivered frame spent in the shared path, 95th percentile.
    pub frame_age_p95_ms: u32,
    pub queue_p95_ms: u32,
    pub max_delay_ms: u32,
    /// Whether the link drops and restores its capacity at all.
    pub has_restore: bool,
    /// Time from the capacity restore until target at the limit and queue below
    /// 200 ms held for `SUSTAINED_MS`.
    pub recovery_ms: Option<u32>,
    /// Lowest target during the first `WARM_UP_MS`.
    pub cold_start_min_fps: u32,
    /// First time the target reached 90% of the limit.
    pub time_to_90pct_ms: Option<u32>,
    pub final_fps: u32,
    pub final_ratio: f32,
    pub trace: Vec<(u32, u32, u32, f32)>, // (time_ms, target fps, queue_ms, ratio)
}

fn percentile_u32(values: &[u32], p: f64) -> u32 {
    if values.is_empty() {
        return 0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    sorted[(((sorted.len() - 1) as f64) * p).round() as usize]
}

fn percentile_f64(values: &[f64], p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    sorted[(((sorted.len() - 1) as f64) * p).round() as usize]
}

pub fn run(sc: &Scenario) -> Report {
    let mut network_rng = Rng::new(sc.seed);
    let mut probe_rng = Rng::new(sc.seed ^ 0x5052_4F42_45);
    let mut encoder = Encoder {
        model: sc.encoder,
        content: sc.content,
        rng: Rng::new(sc.seed ^ 0x454E_434F_4445),
        next_scene_ms: SCENE_INTERVAL_MS,
        debt_bits: 0.0,
    };
    let total_ms = sc.seconds * 1000;
    let ticks = (total_ms / TICK_MS) as usize;
    let link = link_trace(&sc.link, ticks, &mut network_rng);
    // Probe jitter indexed by the probe's send second, so the number of probes a
    // controller manages to send does not change the jitter the next one meets.
    let probe_jitter_ms: Vec<f64> = (0..=sc.seconds)
        .map(|_| probe_rng.log_normal(sc.link.jitter_median_ms, sc.link.jitter_sigma))
        .collect();

    let mut qos = super::smoke::session(sc.limit, sc.quality);
    qos.abr_config = sc.abr;
    if sc.abr {
        qos.new_display("sim".to_owned());
        qos.set_support_changing_quality("sim", true);
    }

    let mut queue: VecDeque<Packet> = VecDeque::new();
    let mut queued_bits = 0.0_f64;
    let mut encode_phase = 0.0_f64;
    let mut encoded_this_second = 0_usize;
    let mut probe_sent: Option<u32> = None;
    let mut replies: Vec<(u32, u32)> = Vec::new(); // (arrive_ms, delay_ms)
    let restore_ms = sc.link.restore_ms();

    let mut fps_samples = Vec::new();
    let mut queue_samples = Vec::new();
    let mut produced = 0_u64;
    let mut delivered = 0_u64;
    let mut frame_ages = Vec::new();
    let mut trace = Vec::new();
    let mut max_delay = 0_u32;
    let mut recovery_ms = None;
    let mut good_since: Option<u32> = None;
    let mut cold_start_min_fps = u32::MAX;
    let mut time_to_90pct_ms = None;

    for tick in 0..ticks {
        let now = tick as u32 * TICK_MS;
        qos.advance_ms(TICK_MS as u64);
        let capacity_kbps = link.capacity_kbps[tick];

        // Encoder: frames at the controller's rate, sized by the controller's ratio.
        // The video loop reports the bitrate as soon as it applies a new ratio.
        let fps = qos.fps();
        let ratio = qos.ratio();
        let bitrate_kbps = BASE_KBPS * ratio as f64;
        qos.store_bitrate(bitrate_kbps as u32);
        let produce_rate = match sc.content {
            Content::Video => fps as f64,
            Content::Office => (fps as f64).min(2.0),
        };
        encode_phase += produce_rate * TICK_MS as f64 / 1000.0;
        while encode_phase >= 1.0 {
            encode_phase -= 1.0;
            encoded_this_second += 1;
            if now >= WARM_UP_MS {
                produced += 1;
            }
            let bits = encoder.frame_bits(now, bitrate_kbps, produce_rate);
            queue.push_back(Packet {
                bits,
                enqueued_ms: now,
                probe_sent_ms: None,
            });
            queued_bits += bits;
        }

        // Shared path drain: probes are tiny and leave as soon as they reach the head.
        if !link.stalled[tick] {
            let mut budget = capacity_kbps * TICK_MS as f64;
            while budget > 0.0 {
                let Some(head) = queue.front_mut() else { break };
                if let Some(sent) = head.probe_sent_ms {
                    let round_trip = sc.link.base_rtt_ms + probe_jitter_ms[(sent / 1000) as usize];
                    let arrive = now + round_trip as u32;
                    replies.push((arrive, arrive - sent));
                    queue.pop_front();
                    continue;
                }
                let take = budget.min(head.bits);
                head.bits -= take;
                queued_bits -= take;
                budget -= take;
                if head.bits <= 1e-9 {
                    if now >= WARM_UP_MS {
                        delivered += 1;
                        frame_ages.push(now - head.enqueued_ms);
                    }
                    queue.pop_front();
                }
            }
        }

        // Probe replies reach the controller.
        replies.sort_by_key(|r| r.0);
        while replies.first().is_some_and(|r| r.0 <= now) {
            let (_, delay) = replies.remove(0);
            max_delay = max_delay.max(delay);
            probe_sent = None;
            qos.user_network_delay(1, delay);
        }

        // The connection's one second timer.
        if now % 1000 == 0 {
            if probe_sent.is_none() {
                probe_sent = Some(now);
                queue.push_back(Packet {
                    bits: 0.0,
                    enqueued_ms: now,
                    probe_sent_ms: Some(now),
                });
            }
            qos.user_delay_response_elapsed(1, (now - probe_sent.unwrap()) as u128);
            if sc.abr {
                qos.update_display_data("sim", encoded_this_second);
            }
            encoded_this_second = 0;
        }

        if now % 100 == 0 {
            let queue_ms = (queued_bits / capacity_kbps.max(1.0)) as u32;
            let fps = qos.fps();
            trace.push((now, fps, queue_ms, qos.ratio()));
            if now < WARM_UP_MS {
                cold_start_min_fps = cold_start_min_fps.min(fps);
            } else {
                fps_samples.push(fps);
                queue_samples.push(queue_ms);
            }
            if time_to_90pct_ms.is_none() && fps * 10 >= sc.limit * 9 {
                time_to_90pct_ms = Some(now);
            }
            if let Some(restore) = restore_ms {
                if now >= restore && recovery_ms.is_none() {
                    if fps >= sc.limit && queue_ms < 200 {
                        let since = *good_since.get_or_insert(now);
                        if now - since >= SUSTAINED_MS {
                            recovery_ms = Some(since - restore);
                        }
                    } else {
                        good_since = None;
                    }
                }
            }
        }
    }

    let measured_s = (total_ms - WARM_UP_MS) as f64 / 1000.0;
    let below_half = fps_samples.iter().filter(|f| **f * 2 < sc.limit).count();
    Report {
        name: sc.name.to_owned(),
        seed: sc.seed,
        limit: sc.limit,
        mean_target_fps: fps_samples.iter().map(|f| *f as f64).sum::<f64>()
            / fps_samples.len().max(1) as f64,
        p10_target_fps: percentile_u32(&fps_samples, 0.10),
        min_target_fps: fps_samples.iter().copied().min().unwrap_or(0),
        below_half_pct: 100.0 * below_half as f64 / fps_samples.len().max(1) as f64,
        produced_fps: produced as f64 / measured_s,
        delivered_fps: delivered as f64 / measured_s,
        frame_age_p95_ms: percentile_u32(&frame_ages, 0.95),
        queue_p95_ms: percentile_u32(&queue_samples, 0.95),
        max_delay_ms: max_delay,
        has_restore: restore_ms.is_some(),
        recovery_ms,
        cold_start_min_fps,
        time_to_90pct_ms,
        final_fps: qos.fps(),
        final_ratio: qos.ratio(),
        trace,
    }
}

/// One scenario over all seeds, summarised by the statistics the assertions use.
#[derive(Debug)]
pub struct Summary {
    pub name: String,
    pub limit: u32,
    pub mean_target_median: f64,
    pub p10_target_worst: u32,
    pub below_half_p90: f64,
    pub queue_p95_p90: u32,
    pub delivered_median: f64,
    pub frame_age_p95_p90: u32,
    pub has_restore: bool,
    /// Slowest sustained recovery, `None` when any seed never recovered.
    pub recovery_worst_ms: Option<u32>,
    pub cold_start_min_median: u32,
    /// Slowest time to 90% of the limit, `None` when any seed never got there.
    pub time_to_90pct_worst_ms: Option<u32>,
}

impl Summary {
    pub fn of(reports: &[Report]) -> Self {
        let f = |g: fn(&Report) -> f64| reports.iter().map(g).collect::<Vec<_>>();
        let u = |g: fn(&Report) -> u32| reports.iter().map(g).collect::<Vec<_>>();
        let all = |g: fn(&Report) -> Option<u32>| {
            reports
                .iter()
                .map(g)
                .try_fold(0, |worst, ms| ms.map(|ms| worst.max(ms)))
        };
        Summary {
            name: reports[0].name.clone(),
            limit: reports[0].limit,
            mean_target_median: percentile_f64(&f(|r| r.mean_target_fps), 0.5),
            p10_target_worst: percentile_u32(&u(|r| r.p10_target_fps), 0.0),
            below_half_p90: percentile_f64(&f(|r| r.below_half_pct), 0.9),
            queue_p95_p90: percentile_u32(&u(|r| r.queue_p95_ms), 0.9),
            delivered_median: percentile_f64(&f(|r| r.delivered_fps), 0.5),
            frame_age_p95_p90: percentile_u32(&u(|r| r.frame_age_p95_ms), 0.9),
            has_restore: reports[0].has_restore,
            recovery_worst_ms: all(|r| r.recovery_ms),
            cold_start_min_median: percentile_u32(&u(|r| r.cold_start_min_fps), 0.5),
            time_to_90pct_worst_ms: all(|r| r.time_to_90pct_ms),
        }
    }

    pub const HEADER: &'static str = "| scenario | limit | target fps (median of means) | worst p10 | below limit/2 (p90) | queue p95 (p90) | delivered fps (median) | frame age p95 (p90) | sustained recovery (worst) | cold-start min (median) | time to 90% (worst) |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|";

    pub fn row(&self) -> String {
        let secs = |ms: Option<u32>| {
            ms.map(|ms| format!("{:.1}s", ms as f64 / 1000.0))
                .unwrap_or_else(|| "never".to_owned())
        };
        format!(
            "| {} | {} | {:.1} | {} | {:.1}% | {} ms | {:.1} | {} ms | {} | {} | {} |",
            self.name,
            self.limit,
            self.mean_target_median,
            self.p10_target_worst,
            self.below_half_p90,
            self.queue_p95_p90,
            self.delivered_median,
            self.frame_age_p95_p90,
            if self.has_restore {
                secs(self.recovery_worst_ms)
            } else {
                "-".to_owned()
            },
            self.cold_start_min_median,
            secs(self.time_to_90pct_worst_ms),
        )
    }
}

fn clean_link(capacity_kbps: f64) -> Link {
    Link {
        capacity_kbps: vec![(0, capacity_kbps)],
        wobble: 0.05,
        base_rtt_ms: 15.0,
        jitter_median_ms: 3.0,
        jitter_sigma: 0.5,
        loss_per_s: 0.0,
        stall_mean_interval_s: 0.0,
        stall_ms: (0.0, 0.0),
    }
}

/// Weak-signal home Wi-Fi with ample average capacity: heavy-tailed jitter,
/// retransmissions, and a link stall of up to 2.5 s every twenty seconds or so.
/// Deliberately nasty; it isolates "capacity is fine, timing is not".
fn home_wifi_link() -> Link {
    Link {
        capacity_kbps: vec![(0, 20_000.0)],
        wobble: 0.5,
        base_rtt_ms: 8.0,
        jitter_median_ms: 15.0,
        jitter_sigma: 1.0,
        loss_per_s: 0.2,
        stall_mean_interval_s: 20.0,
        stall_ms: (300.0, 2500.0),
    }
}

fn intercontinental_link() -> Link {
    Link {
        capacity_kbps: vec![(0, 20_000.0)],
        wobble: 0.1,
        base_rtt_ms: 250.0,
        jitter_median_ms: 5.0,
        jitter_sigma: 0.5,
        loss_per_s: 0.05,
        stall_mean_interval_s: 0.0,
        stall_ms: (0.0, 0.0),
    }
}

/// 8 Mbps for a minute, 2.5 Mbps for the next, 8 Mbps again.
fn halved_link() -> Link {
    Link {
        capacity_kbps: vec![(0, 8_000.0), (60_000, 2_500.0), (120_000, 8_000.0)],
        ..clean_link(8_000.0)
    }
}

fn mobile_link() -> Link {
    Link {
        capacity_kbps: vec![(0, 6_000.0)],
        wobble: 0.4,
        base_rtt_ms: 40.0,
        jitter_median_ms: 30.0,
        jitter_sigma: 0.8,
        loss_per_s: 0.02,
        stall_mean_interval_s: 0.0,
        stall_ms: (0.0, 0.0),
    }
}

pub fn scenarios() -> Vec<Scenario> {
    let base = |name, limit, link, abr, encoder| Scenario {
        name,
        seconds: 180,
        limit,
        quality: Quality::Balanced,
        abr,
        content: Content::Video,
        encoder,
        link,
        seed: 1,
    };
    use EncoderModel::*;
    vec![
        base("home_wifi_30", 30, home_wifi_link(), true, Cbr),
        base("home_wifi_60", 60, home_wifi_link(), true, Cbr),
        base(
            "home_wifi_fixed_rate_30",
            30,
            home_wifi_link(),
            true,
            FixedRate,
        ),
        base("home_wifi_no_abr_30", 30, home_wifi_link(), false, Cbr),
        Scenario {
            content: Content::Office,
            ..base("office_home_wifi_30", 30, home_wifi_link(), true, Cbr)
        },
        base("city_relay_30", 30, clean_link(50_000.0), true, Cbr),
        base("city_relay_60", 60, clean_link(50_000.0), true, Cbr),
        base(
            "intercontinental_30",
            30,
            intercontinental_link(),
            true,
            Cbr,
        ),
        base("bandwidth_halved_30", 30, halved_link(), true, Cbr),
        base(
            "bandwidth_halved_fixed_rate_30",
            30,
            halved_link(),
            true,
            FixedRate,
        ),
        base(
            "bandwidth_halved_fixed_rate_no_abr_30",
            30,
            halved_link(),
            false,
            FixedRate,
        ),
        base("bandwidth_halved_no_abr_30", 30, halved_link(), false, Cbr),
        base("mobile_bufferbloat_30", 30, mobile_link(), true, Cbr),
    ]
}

/// Runs every scenario over `SEEDS` and returns the per-scenario summaries.
pub fn run_all() -> Vec<(Summary, Vec<Report>)> {
    scenarios()
        .iter()
        .map(|sc| {
            let reports: Vec<Report> = SEEDS
                .map(|seed| run(&Scenario { seed, ..sc.clone() }))
                .collect();
            (Summary::of(&reports), reports)
        })
        .collect()
}

fn write_traces(results: &[(Summary, Vec<Report>)]) {
    use std::fmt::Write;
    if let Ok(path) = std::env::var("RUSTDESK_QOS_SIM_CSV") {
        let mut csv = String::from("scenario,seed,time_ms,target_fps,queue_ms,ratio\n");
        for (_, reports) in results {
            for report in reports {
                for (t, fps, queue, ratio) in &report.trace {
                    writeln!(
                        csv,
                        "{},{},{t},{fps},{queue},{ratio:.3}",
                        report.name, report.seed
                    )
                    .unwrap();
                }
            }
        }
        std::fs::write(path, csv).unwrap();
    }
}

#[test]
fn sim_scenarios() {
    let results = run_all();
    println!("{}", Summary::HEADER);
    for (summary, _) in &results {
        println!("{}", summary.row());
    }
    if std::env::var("RUSTDESK_QOS_SIM_VERBOSE").is_ok() {
        for (_, reports) in &results {
            for r in reports {
                println!(
                    "{} seed {}: target mean {:.1} p10 {} min {} below-half {:.1}% delivered {:.1} age p95 {} queue p95 {} max probe {} recovery {:?} cold-start min {} t90 {:?}",
                    r.name, r.seed, r.mean_target_fps, r.p10_target_fps, r.min_target_fps,
                    r.below_half_pct, r.delivered_fps, r.frame_age_p95_ms, r.queue_p95_ms,
                    r.max_delay_ms, r.recovery_ms, r.cold_start_min_fps, r.time_to_90pct_ms
                );
            }
        }
    }
    write_traces(&results);
    for (summary, _) in &results {
        let violations = bound_violations(summary);
        assert!(
            violations.is_empty(),
            "{}: {violations:?}\n{summary:?}",
            summary.name
        );
    }
}

/// The bounds every scenario summary has to meet, shared by the CI run over `SEEDS`
/// and by the held-out run in `robustness.rs`.  They state what the product needs,
/// not what one seed produced.  If a new seed or a new scenario violates a bound,
/// change the design or loosen the bound with a written reason; never tune a
/// controller constant until the bound passes.
pub fn bound_violations(s: &Summary) -> Vec<&'static str> {
    let name = s.name.as_str();
    let limit = s.limit as f64;
    let mut v = Vec::new();
    let mut check = |ok: bool, what: &'static str| {
        if !ok {
            v.push(what);
        }
    };
    if name.starts_with("home_wifi") || name.starts_with("office_home_wifi") {
        // A jittery but healthy link must stay fast: the whole point of the change.
        // The target rarely leaves the limit, never collapses, and stalls of up to
        // 2.5 s leave about a second of queue at worst.
        check(
            s.mean_target_median >= 0.85 * limit,
            "median target below 85%",
        );
        check(s.p10_target_worst * 3 >= s.limit, "worst p10 below a third");
        check(s.below_half_p90 <= 10.0, "below half the limit over 10%");
        check(s.queue_p95_p90 < 1000, "queue p95 p90 over 1 s");
        if name != "office_home_wifi_30" {
            check(s.delivered_median >= 0.8 * limit, "delivered below 80%");
        }
        // Frame age is the time a delivered frame spent in the shared path: what a
        // viewer waits for on top of the round trip.  A jittery high-capacity link
        // contains isolated stalls of up to 2.5 s, and the bound is on the p90 of
        // per-seed p95 frame age: isolated stalls are tolerated, but they must not
        // turn into a sustained multi-second backlog.  A regression bound, not a
        // latency target; set from the scenario, not from a run.
        check(s.frame_age_p95_p90 < 1500, "frame age p95 p90 over 1.5 s");
    } else if name.starts_with("city_relay") {
        // A clean link is where the developers test; every seed sits at the limit,
        // and a fresh connection reaches 90% of it within ten seconds.
        check(s.p10_target_worst == s.limit, "left the limit");
        check(s.queue_p95_p90 < 50, "queue on a clean link");
        check(s.frame_age_p95_p90 < 100, "frame age on a clean link");
        check(
            s.time_to_90pct_worst_ms.is_some_and(|ms| ms <= 10_000),
            "cold start over 10 s",
        );
    } else if name == "intercontinental_30" {
        // High but stable RTT is not congestion, not even during the cold start.
        check(
            s.mean_target_median >= 0.9 * limit,
            "median target below 90%",
        );
        check(
            s.cold_start_min_median >= INIT_FPS,
            "cold start below INIT_FPS",
        );
        // Frame age excludes the round trip, so a high RTT earns no allowance.
        check(s.frame_age_p95_p90 < 150, "frame age over 150 ms");
        check(
            s.time_to_90pct_worst_ms.is_some_and(|ms| ms <= 10_000),
            "cold start over 10 s",
        );
    } else if let Some((queue_p95_bound_ms, below_half_bound_pct)) = match name {
        // Real congestion must be detected, drained and recovered from.  With a CBR
        // encoder only the bitrate drains the queue, and three probe replies at one
        // second cadence plus a three second ratio cooldown are needed before a
        // confirmed cut, so a few seconds of queue are inherent there.  Without
        // ABR nothing drains a CBR queue at all, so that combination is reported
        // but not asserted.
        "bandwidth_halved_30" => Some((3000, 40.0)),
        "bandwidth_halved_fixed_rate_30" => Some((2000, 10.0)),
        "bandwidth_halved_fixed_rate_no_abr_30" => Some((3000, 30.0)),
        _ => None,
    } {
        check(s.queue_p95_p90 < queue_p95_bound_ms, "queue p95 p90 bound");
        check(
            s.frame_age_p95_p90 < queue_p95_bound_ms,
            "frame age p95 p90 bound",
        );
        check(s.below_half_p90 <= below_half_bound_pct, "below half bound");
        check(
            s.recovery_worst_ms.is_some_and(|ms| ms <= 20_000),
            "sustained recovery over 20 s",
        );
    } else if name == "mobile_bufferbloat_30" {
        check(s.queue_p95_p90 < 2000, "queue p95 p90 over 2 s");
        check(s.frame_age_p95_p90 < 2000, "frame age p95 p90 over 2 s");
        check(s.below_half_p90 <= 15.0, "below half the limit over 15%");
    }
    v
}

/// Replays `qos_trace` lines through a fresh controller and returns
/// `(time_ms, id, recorded_fps, replayed_fps)` per line.  Open loop, FPS only:
/// the recorded delays do not react to the replayed decisions, the session runs
/// with ABR off, and connections are replayed as separate viewers of one 30 fps
/// balanced session.  Time advances by the wall-clock delta between consecutive
/// lines whatever their connection, or by one second per line when a trace
/// predates the `t=` field.
pub fn replay(text: &str) -> Vec<(u64, i32, u64, u32)> {
    // A present but malformed value is a corrupt trace, not a missing field.
    let field = |line: &str, key: &str| -> Option<u64> {
        line.split_whitespace()
            .find_map(|kv| kv.strip_prefix(key).and_then(|v| v.strip_prefix('=')))
            .map(|v| {
                v.parse()
                    .unwrap_or_else(|e| panic!("bad {key}={v:?} in {line:?}: {e}"))
            })
    };
    let mut qos = super::smoke::session(30, Quality::Balanced);
    qos.users.clear();
    let mut last_t: Option<u64> = None;
    let mut now = 0_u64;
    let mut trace = Vec::new();
    for line in text.lines().filter(|l| l.contains("qos_trace")) {
        let id = field(line, "id").unwrap_or(1) as i32;
        qos.users.entry(id).or_default();
        let t = field(line, "t");
        let step = match (t, last_t) {
            (Some(t), Some(prev)) => t.saturating_sub(prev).clamp(1, 10_000),
            _ => 1000,
        };
        if t.is_some() {
            last_t = t;
        }
        now += step;
        qos.advance_ms(step);
        if let Some(elapsed) = field(line, "timeout") {
            qos.user_delay_response_elapsed(id, elapsed as u128);
        } else if let Some(delay) = field(line, "delay") {
            qos.user_delay_response_elapsed(id, 0);
            qos.user_network_delay(id, delay as u32);
        }
        let recorded = field(line, "fps").unwrap_or(0);
        trace.push((now, id, recorded, qos.fps()));
    }
    trace
}

/// Replays the log named by `RUSTDESK_QOS_TRACE` and prints the result.
#[test]
fn replay_recorded_trace() {
    let Ok(path) = std::env::var("RUSTDESK_QOS_TRACE") else {
        return;
    };
    let trace = replay(&std::fs::read_to_string(&path).unwrap());
    println!("time_ms,id,recorded_fps,replayed_fps");
    for (t, id, recorded, replayed) in &trace {
        println!("{t},{id},{recorded},{replayed}");
    }
    let mean = trace.iter().map(|t| t.3 as f64).sum::<f64>() / trace.len().max(1) as f64;
    println!(
        "replayed mean target fps: {mean:.1} over {} lines",
        trace.len()
    );
}

#[test]
fn replay_time_axis_is_shared_across_connections() {
    // Two viewers each log once a second for twenty seconds: twenty seconds of
    // wall clock, not forty.
    let text: String = (0..20)
        .flat_map(|i| {
            [
                format!("qos_trace t={} id=1 delay=10 fps=30\n", 100_000 + i * 1000),
                format!("qos_trace t={} id=2 delay=10 fps=30\n", 100_001 + i * 1000),
            ]
        })
        .collect();
    let trace = replay(&text);
    let elapsed = trace.last().unwrap().0 - trace.first().unwrap().0;
    assert!(
        (19_000..=19_100).contains(&elapsed),
        "replayed {elapsed} ms for 19 s of wall clock"
    );
}

#[test]
fn replay_recorded_trace_is_independent_of_connection_id() {
    let replay = |id: i32| {
        let text: String = (0..20)
            .map(|i| {
                format!(
                    "qos_trace t={} id={id} delay=10 fps=30\n",
                    100_000 + i * 1000
                )
            })
            .collect();
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "rustdesk-qos-replay-{}-{nonce}-{id}.log",
            std::process::id()
        ));
        std::fs::write(&path, text).unwrap();
        // Exercise the real replay entry point without changing other tests' environment.
        let test = format!(
            "{}::replay_recorded_trace",
            module_path!().split_once("::").unwrap().1
        );
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &test, "--nocapture", "--test-threads=1"])
            .env("RUSTDESK_QOS_TRACE", &path)
            .output();
        std::fs::remove_file(&path).unwrap();
        let output = output.unwrap();
        assert!(output.status.success(), "replay failed: {output:?}");
        let stdout = String::from_utf8(output.stdout).unwrap();
        let id = id.to_string();
        let fps: Vec<u32> = stdout
            .lines()
            .filter_map(|line| {
                let fields: Vec<_> = line.split(',').collect();
                if fields.len() == 4 && fields[1] == id {
                    Some(fields[3].parse().unwrap())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(fps.len(), 20, "missing replay samples: {stdout}");
        fps
    };
    let expected = replay(1);
    assert_eq!(expected.last(), Some(&30));
    assert_eq!(replay(1652), expected);
}
