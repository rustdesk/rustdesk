//! Closed-loop network simulation for the QoS controller.
//!
//! The controller is driven the way `Connection` drives it: one TestDelay probe per
//! second, a single probe outstanding, `user_delay_response_elapsed` on every timer
//! tick, `update_display_data` once per second.  Video frames and probes share one
//! FIFO link, so a probe measures the queue that the frames built up in front of it.
//!
//! The link is deliberately richer than a fixed-rate pipe: variable frame sizes with
//! scene changes, slowly wobbling capacity, heavy-tailed jitter, loss events that
//! behave like a reliable stream's retransmission (a short stall plus a temporary
//! rate reduction) and independent link stalls.  It still is a model, not a network:
//! it does not reproduce a real transport's congestion control or a real encoder.
//! Its job is to show how the controller reacts to the *kind* of behaviour a home
//! Wi-Fi, a stable relay or a saturated uplink produce, deterministically.
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
    /// Hardware encoders are configured for a fixed 30 fps, so every frame carries
    /// a thirtieth of the bitrate and fewer frames do mean fewer bytes.
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
const TICK_MS: u32 = 10;
/// Samples taken before this instant only warm the controller up.
const WARM_UP_MS: u32 = 15_000;

struct Packet {
    bits: f64,
    probe_sent_ms: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct Report {
    pub name: String,
    pub limit: u32,
    pub mean_fps: f64,
    pub p10_fps: u32,
    pub min_fps: u32,
    /// Share of the measured time spent below half of the FPS limit.
    pub below_half_pct: f64,
    pub queue_p95_ms: u32,
    pub max_delay_ms: u32,
    /// Time from the capacity restore until the FPS limit was reached again.
    pub recovery_ms: Option<u32>,
    pub final_fps: u32,
    pub final_ratio: f32,
    pub trace: Vec<(u32, u32, u32, f32)>, // (time_ms, fps, queue_ms, ratio)
}

impl Report {
    pub fn row(&self) -> String {
        format!(
            "| {} | {} | {:.1} | {} | {} | {:.1}% | {} | {} | {} | {} | {:.2} |",
            self.name,
            self.limit,
            self.mean_fps,
            self.p10_fps,
            self.min_fps,
            self.below_half_pct,
            self.queue_p95_ms,
            self.max_delay_ms,
            self.recovery_ms
                .map(|ms| format!("{:.1}s", ms as f64 / 1000.0))
                .unwrap_or_else(|| "-".to_owned()),
            self.final_fps,
            self.final_ratio
        )
    }

    pub const HEADER: &'static str = "| scenario | limit | mean fps | p10 fps | min fps | < limit/2 | queue p95 | max probe | recovery | final fps | final ratio |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|";
}

fn percentile(sorted: &[u32], p: f64) -> u32 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

pub fn run(sc: &Scenario) -> Report {
    let mut rng = Rng::new(sc.seed);
    let mut qos = super::smoke::session(sc.limit, sc.quality);
    qos.abr_config = sc.abr;
    if sc.abr {
        qos.new_display("sim".to_owned());
        qos.set_support_changing_quality("sim", true);
    }

    let mut queue: VecDeque<Packet> = VecDeque::new();
    let mut queued_bits = 0.0_f64;
    let mut encode_phase = 0.0_f64;
    let mut frames_encoded = 0_u64;
    let mut encoded_this_second = 0_usize;
    let mut probe_sent: Option<u32> = None;
    let mut replies: Vec<(u32, u32)> = Vec::new(); // (arrive_ms, delay_ms)
    let mut stall_until = 0_u32;
    let mut backoff_until = 0_u32;
    let mut next_stall_ms = if sc.link.stall_mean_interval_s > 0.0 {
        (rng.exponential(sc.link.stall_mean_interval_s) * 1000.0) as u32
    } else {
        u32::MAX
    };
    let mut wobble = 0.0_f64;
    let restore_ms = sc.link.restore_ms();

    let mut fps_samples = Vec::new();
    let mut queue_samples = Vec::new();
    let mut trace = Vec::new();
    let mut max_delay = 0_u32;
    let mut recovery_ms = None;

    let total_ms = sc.seconds * 1000;
    let mut now = 0_u32;
    while now < total_ms {
        qos.advance_ms(TICK_MS as u64);

        // Capacity: nominal schedule, slow wobble, retransmission backoff.
        if now % 100 == 0 {
            wobble = (wobble + rng.normal() * 0.03).clamp(-sc.link.wobble, sc.link.wobble);
        }
        let mut capacity_kbps = sc.link.capacity_at(now) * (1.0 + wobble);
        if now < backoff_until {
            capacity_kbps *= 0.5;
        }

        // Link events.
        if now >= next_stall_ms {
            let len = rng.range(sc.link.stall_ms.0, sc.link.stall_ms.1) as u32;
            stall_until = stall_until.max(now + len);
            next_stall_ms = now + (rng.exponential(sc.link.stall_mean_interval_s) * 1000.0) as u32;
        }
        if sc.link.loss_per_s > 0.0 && rng.uniform() < sc.link.loss_per_s * TICK_MS as f64 / 1000.0
        {
            let len = rng.range(200.0, 400.0) as u32;
            stall_until = stall_until.max(now + len);
            backoff_until = stall_until + 1000;
        }

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
            frames_encoded += 1;
            encoded_this_second += 1;
            let target_bits = match (sc.content, sc.encoder) {
                // A changed region of a static screen is small whatever the rate control does.
                (Content::Office, _) => bitrate_kbps * 1000.0 / ENCODER_CONFIGURED_FPS * 0.3,
                (Content::Video, EncoderModel::Cbr) => bitrate_kbps * 1000.0 / produce_rate,
                (Content::Video, EncoderModel::FixedRate) => {
                    bitrate_kbps * 1000.0 / ENCODER_CONFIGURED_FPS
                }
            };
            let mut bits = target_bits * rng.log_normal(1.0, 0.35);
            // A scene change every five seconds of video costs a few frames' worth of data.
            if sc.content == Content::Video
                && frames_encoded % (5 * produce_rate.max(1.0) as u64).max(1) == 0
            {
                bits *= 3.0;
            }
            queue.push_back(Packet {
                bits,
                probe_sent_ms: None,
            });
            queued_bits += bits;
        }

        // Link drain: probes are tiny and leave as soon as they reach the head.
        if now >= stall_until {
            let mut budget = capacity_kbps * TICK_MS as f64;
            while budget > 0.0 {
                let Some(head) = queue.front_mut() else { break };
                if let Some(sent) = head.probe_sent_ms {
                    let round_trip = sc.link.base_rtt_ms
                        + rng.log_normal(sc.link.jitter_median_ms, sc.link.jitter_sigma);
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
            if now >= WARM_UP_MS {
                fps_samples.push(fps);
                queue_samples.push(queue_ms);
            }
            if let Some(restore) = restore_ms {
                if now >= restore && fps >= sc.limit && recovery_ms.is_none() {
                    recovery_ms = Some(now - restore);
                }
            }
        }
        now += TICK_MS;
    }

    let mut sorted_fps = fps_samples.clone();
    sorted_fps.sort_unstable();
    let mut sorted_queue = queue_samples.clone();
    sorted_queue.sort_unstable();
    let below_half = fps_samples.iter().filter(|f| **f * 2 < sc.limit).count();
    Report {
        name: sc.name.to_owned(),
        limit: sc.limit,
        mean_fps: fps_samples.iter().map(|f| *f as f64).sum::<f64>()
            / fps_samples.len().max(1) as f64,
        p10_fps: percentile(&sorted_fps, 0.10),
        min_fps: sorted_fps.first().copied().unwrap_or(0),
        below_half_pct: 100.0 * below_half as f64 / fps_samples.len().max(1) as f64,
        queue_p95_ms: percentile(&sorted_queue, 0.95),
        max_delay_ms: max_delay,
        recovery_ms,
        final_fps: qos.fps(),
        final_ratio: qos.ratio(),
        trace,
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

/// Weak-signal home Wi-Fi: plenty of capacity on average, but heavy-tailed jitter,
/// retransmissions, and a link stall of up to 2.5 s every twenty seconds or so.
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
        seed: 7,
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

fn write_traces(reports: &[Report]) {
    use std::fmt::Write;
    if let Ok(path) = std::env::var("RUSTDESK_QOS_SIM_CSV") {
        let mut csv = String::from("scenario,time_ms,fps,queue_ms,ratio\n");
        for report in reports {
            for (t, fps, queue, ratio) in &report.trace {
                writeln!(csv, "{},{t},{fps},{queue},{ratio:.3}", report.name).unwrap();
            }
        }
        std::fs::write(path, csv).unwrap();
    }
}

#[test]
fn sim_scenarios() {
    let reports: Vec<Report> = scenarios().iter().map(run).collect();
    println!("{}", Report::HEADER);
    for report in &reports {
        println!("{}", report.row());
    }
    write_traces(&reports);
    let get = |name: &str| reports.iter().find(|r| r.name == name).unwrap();

    // A jittery but healthy link must stay fast: the whole point of the change.
    for name in [
        "home_wifi_30",
        "home_wifi_60",
        "home_wifi_fixed_rate_30",
        "home_wifi_no_abr_30",
        "office_home_wifi_30",
    ] {
        let r = get(name);
        assert!(r.mean_fps >= 0.85 * r.limit as f64, "{name}: {r:?}");
        assert!(r.below_half_pct <= 5.0, "{name}: {r:?}");
        assert!(r.queue_p95_ms < 500, "{name}: {r:?}");
    }
    // A clean link is where the developers test; it must sit at the limit.
    for name in ["city_relay_30", "city_relay_60"] {
        let r = get(name);
        assert_eq!(r.min_fps, r.limit, "{name}: {r:?}");
    }
    // High but stable RTT is not congestion.
    let r = get("intercontinental_30");
    assert!(
        r.mean_fps >= 0.9 * r.limit as f64,
        "intercontinental: {r:?}"
    );
    assert!(
        r.final_ratio >= Quality::Balanced.ratio() * 0.9,
        "intercontinental: {r:?}"
    );
    // Real congestion must be detected, drained, and recovered from.  With a CBR
    // encoder only the bitrate drains the queue, and three probe replies at one
    // second cadence are needed before a confirmed cut, so about two seconds of
    // queue are inherent there.  Without ABR nothing drains a CBR queue at all, so
    // that combination is reported but not asserted.
    for (name, queue_p95_bound_ms, below_half_bound_pct) in [
        ("bandwidth_halved_30", 2500, 10.0),
        ("bandwidth_halved_fixed_rate_30", 1500, 5.0),
        ("bandwidth_halved_fixed_rate_no_abr_30", 2000, 20.0),
    ] {
        let r = get(name);
        assert!(r.queue_p95_ms < queue_p95_bound_ms, "{name}: {r:?}");
        assert!(r.below_half_pct <= below_half_bound_pct, "{name}: {r:?}");
        assert!(
            r.recovery_ms.is_some_and(|ms| ms <= 15_000),
            "{name}: {r:?}"
        );
        assert_eq!(r.final_fps, r.limit, "{name}: {r:?}");
    }
    let r = get("mobile_bufferbloat_30");
    assert!(r.queue_p95_ms < 2000, "mobile: {r:?}");
}

/// Replays a `qos_trace` log captured on a real host (see `user_network_delay`),
/// so the controller's decisions on a recorded delay sequence can be inspected.
/// Set `RUSTDESK_QOS_TRACE` to the log file; the replay is open loop: the recorded
/// delays do not react to the replayed decisions.
#[test]
fn replay_recorded_trace() {
    let Ok(path) = std::env::var("RUSTDESK_QOS_TRACE") else {
        return;
    };
    let text = std::fs::read_to_string(&path).unwrap();
    // A present but malformed value is a corrupt trace, not a missing field.
    let field = |line: &str, key: &str| -> Option<u32> {
        line.split_whitespace()
            .find_map(|kv| kv.strip_prefix(key).and_then(|v| v.strip_prefix('=')))
            .map(|v| {
                v.parse()
                    .unwrap_or_else(|e| panic!("bad {key}={v:?} in {line:?}: {e}"))
            })
    };
    let mut qos = super::smoke::session(30, Quality::Balanced);
    let mut now = 0;
    let mut trace = Vec::new();
    for line in text.lines().filter(|l| l.contains("qos_trace")) {
        now += 1000;
        qos.advance_ms(1000);
        if let Some(elapsed) = field(line, "timeout") {
            qos.user_delay_response_elapsed(1, elapsed as u128);
        } else if let Some(delay) = field(line, "delay") {
            qos.user_delay_response_elapsed(1, 0);
            qos.user_network_delay(1, delay);
        }
        let recorded = field(line, "fps").unwrap_or(0);
        trace.push((now, recorded, qos.fps()));
    }
    println!("time_ms,recorded_fps,replayed_fps");
    for (t, recorded, replayed) in &trace {
        println!("{t},{recorded},{replayed}");
    }
    let mean = trace.iter().map(|t| t.2 as f64).sum::<f64>() / trace.len().max(1) as f64;
    println!("replayed mean fps: {mean:.1} over {} samples", trace.len());
}
