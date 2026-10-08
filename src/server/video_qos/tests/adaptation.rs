//! Regression tests for sustained capacity drops and path-delay/content changes.
//! Capacity drops use the closed-loop model; delay and activity fixtures are open-loop.
use super::*;

fn percentile(values: &[f64], p: f64) -> f64 {
    assert!(!values.is_empty());
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

// Count a fall and subsequent rise of at least `amplitude`, ignoring smaller
// reversals. A one-way reduction or recovery is not an oscillation.
fn round_trips(values: &[f64], amplitude: f64) -> usize {
    let mut peak = values[0];
    let mut trough = peak;
    let mut falling = false;
    let mut count = 0;
    for &value in &values[1..] {
        if falling {
            trough = trough.min(value);
            if value - trough >= amplitude {
                count += 1;
                peak = value;
                falling = false;
            }
        } else {
            peak = peak.max(value);
            if peak - value >= amplitude {
                trough = value;
                falling = true;
            }
        }
    }
    count
}

#[test]
fn oscillation_metric_distinguishes_recovery_and_small_jitter() {
    assert_eq!(round_trips(&[30.0, 29.0, 30.0, 28.0, 30.0], 7.5), 0);
    assert_eq!(round_trips(&[30.0, 20.0, 10.0], 7.5), 0);
    assert_eq!(round_trips(&[10.0, 20.0, 30.0], 7.5), 0);
    assert_eq!(round_trips(&[30.0, 10.0, 30.0, 20.0, 30.0], 7.5), 2);
    assert_eq!(round_trips(&[0.49, 0.17, 0.49], 0.67 * 0.25), 1);
}

struct Oscillation {
    mean_fps: f64,
    mean_ratio: f64,
    fps_span: f64,
    ratio_span: f64,
    fps_cycles_per_min: f64,
    ratio_cycles_per_min: f64,
    queue_p95_ms: f64,
}

fn permanent_drop(seeds: std::ops::RangeInclusive<u64>) {
    use super::sim::{self, Summary};

    const STEADY_START_MS: u32 = 120_000;
    const END_MS: u32 = 600_000;
    let cases = [
        ("bandwidth_halved_30", 3000.0),
        ("bandwidth_halved_fixed_rate_30", 2000.0),
        ("bandwidth_halved_fixed_rate_no_abr_30", 3000.0),
    ];
    println!("Permanent drop: 8 -> 2.5 Mbps at 60 s; duration 600 s; seeds {seeds:?}.");
    println!("Oscillation/queue window: 120-600 s. Spans are p95-p5; round trips require 25% of the requested FPS/ratio in each direction. Delivered FPS/frame age cover 15-600 s.");
    println!("| scenario | capacity wobble | steady mean FPS (median) | mean ratio (median) | FPS span (p90) | ratio span (p90) | FPS cycles/min (p90) | ratio cycles/min (p90) | queue p95 (p90) | delivered FPS (median) | frame age p95 (p90) |");
    println!("|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|");
    let mut cases_run = 0;
    for mut sc in sim::scenarios() {
        let Some((_, queue_bound_ms)) = cases.iter().find(|(name, _)| *name == sc.name) else {
            continue;
        };
        cases_run += 1;
        sc.seconds = END_MS / 1000;
        sc.link.capacity_kbps = vec![(0, 8000.0), (60_000, 2500.0)];
        let original_wobble = sc.link.wobble;
        for wobble in [0.0, original_wobble] {
            sc.link.wobble = wobble;
            let mut reports = Vec::new();
            let mut oscillations = Vec::new();
            for seed in seeds.clone() {
                sc.seed = seed;
                let report = sim::run(&sc);
                assert!(
                    report
                        .trace
                        .iter()
                        .filter(|(t, ..)| (30_000..60_000).contains(t))
                        .all(|(_, fps, _, _)| *fps == sc.limit),
                    "healthy pre-drop phase: {} seed {seed}",
                    sc.name
                );
                let steady: Vec<_> = report
                    .trace
                    .iter()
                    .filter(|(t, ..)| *t >= STEADY_START_MS)
                    .collect();
                let fps: Vec<_> = steady.iter().map(|(_, fps, _, _)| *fps as f64).collect();
                let ratios: Vec<_> = steady
                    .iter()
                    .map(|(_, _, _, ratio)| *ratio as f64)
                    .collect();
                let queues: Vec<_> = steady
                    .iter()
                    .map(|(_, _, queue, _)| *queue as f64)
                    .collect();
                let minutes = (END_MS - STEADY_START_MS) as f64 / 60_000.0;
                oscillations.push(Oscillation {
                    mean_fps: fps.iter().sum::<f64>() / fps.len() as f64,
                    mean_ratio: ratios.iter().sum::<f64>() / ratios.len() as f64,
                    fps_span: percentile(&fps, 0.95) - percentile(&fps, 0.05),
                    ratio_span: percentile(&ratios, 0.95) - percentile(&ratios, 0.05),
                    fps_cycles_per_min: round_trips(&fps, sc.limit as f64 * 0.25) as f64 / minutes,
                    ratio_cycles_per_min: round_trips(&ratios, sc.quality.ratio() as f64 * 0.25)
                        as f64
                        / minutes,
                    queue_p95_ms: percentile(&queues, 0.95),
                });
                reports.push(report);
            }
            let metric = |field: fn(&Oscillation) -> f64, p| {
                percentile(&oscillations.iter().map(field).collect::<Vec<_>>(), p)
            };
            let summary = Summary::of(&reports);
            let queue_p95 = metric(|o| o.queue_p95_ms, 0.9);
            println!("| {} | {:.0}% | {:.1} | {:.3} | {:.1} | {:.3} | {:.2} | {:.2} | {:.0} ms | {:.1} | {} ms |",
                sc.name, wobble * 100.0, metric(|o| o.mean_fps, 0.5),
                metric(|o| o.mean_ratio, 0.5),
                metric(|o| o.fps_span, 0.9), metric(|o| o.ratio_span, 0.9),
                metric(|o| o.fps_cycles_per_min, 0.9), metric(|o| o.ratio_cycles_per_min, 0.9),
                queue_p95, summary.delivered_median, summary.frame_age_p95_p90);
            // Queue and frame-age limits reuse the transient-drop budgets;
            // oscillation statistics are diagnostic.
            assert!(
                queue_p95 < *queue_bound_ms,
                "{}: steady queue {queue_p95} ms",
                sc.name
            );
            assert!(
                (summary.frame_age_p95_p90 as f64) < *queue_bound_ms,
                "{summary:?}"
            );
            assert!(
                summary.delivered_median >= sc.limit as f64 * 0.4,
                "{summary:?}"
            );
        }
    }
    assert_eq!(cases_run, cases.len());
}

#[test]
fn permanent_capacity_drop_600s() {
    permanent_drop(super::sim::SEEDS);
}

#[test]
#[ignore = "extended permanent-drop coverage over 100 held-out seeds"]
fn permanent_capacity_drop_held_out_seeds() {
    permanent_drop(21..=120);
}

#[test]
fn low_capacity_preserves_auto_floor_and_recovers() {
    let mut sc = super::sim::scenarios()
        .into_iter()
        .find(|s| s.name == "bandwidth_halved_fixed_rate_no_abr_30")
        .unwrap();
    sc.link.capacity_kbps = vec![(0, 8000.0), (60_000, 700.0), (120_000, 8000.0)];
    for seed in super::sim::SEEDS {
        sc.seed = seed;
        let report = super::sim::run(&sc);
        assert!(
            report.trace.iter().all(|(_, fps, ..)| *fps >= 5),
            "seed {seed}: automatic reductions went below 5 FPS"
        );
        let congested: Vec<_> = report
            .trace
            .iter()
            .filter(|(t, ..)| (80_000..120_000).contains(t))
            .collect();
        let min_fps = congested.iter().map(|(_, fps, ..)| *fps).min().unwrap();
        let min_queue = congested
            .iter()
            .map(|(_, _, queue, _)| *queue)
            .min()
            .unwrap();
        assert_eq!(
            min_fps, 5,
            "seed {seed}: severe congestion must reach the floor"
        );
        // At 5 FPS this model sends about 670 kbps, leaving little room to drain
        // existing backlog at 700 kbps. Require drainage after capacity returns.
        assert!(
            report.recovery_ms.is_some_and(|ms| ms <= 20_000),
            "seed {seed}: recovery took {:?}",
            report.recovery_ms
        );
        println!("700 kbps fixed-rate, seed {seed}: min FPS={min_fps}, min queue={min_queue} ms, queue p95={} ms, frame age p95={} ms, recovery={:?}", report.queue_p95_ms, report.frame_age_p95_ms, report.recovery_ms);
    }
}

const DISPLAY: &str = "adaptation";

fn session(abr: bool) -> VideoQoS {
    let mut qos = super::smoke::session(FPS, Quality::Balanced);
    qos.abr_config = abr;
    qos.new_display(DISPLAY.to_owned());
    qos.set_support_changing_quality(DISPLAY, true);
    sync_bitrate(&mut qos);
    qos
}

fn sync_bitrate(qos: &mut VideoQoS) {
    let bitrate = (6000.0 * qos.ratio()) as u32;
    qos.store_bitrate(bitrate);
}

fn second(qos: &mut VideoQoS, delay: u32, dynamic: bool) {
    let encoded = if dynamic { qos.fps() as usize } else { 0 };
    qos.advance_ms(1000);
    sync_bitrate(qos);
    qos.user_network_delay(1, delay);
    sync_bitrate(qos);
    qos.update_display_data(DISPLAY, encoded);
    sync_bitrate(qos);
}

fn baseline_steps() -> Vec<String> {
    let mut unmet = Vec::new();
    println!("Baseline step: 90 s at 10 ms, 180 s at new RTT, 90 s at 10 ms; one fresh reply per second. Relearning requires FPS=30 and excess<150 ms throughout the final 60 s at the new RTT.");
    println!("| ABR | new RTT | cold-start final FPS | learned baseline | final excess | final FPS | final ratio | relearned | returned-path FPS |");
    println!("|---|---:|---:|---:|---:|---:|---:|---|---:|");
    for abr in [false, true] {
        for rtt in [310, 410] {
            let mut cold = session(abr);
            for _ in 0..90 {
                second(&mut cold, rtt, true);
                assert!(cold.fps() >= INIT_FPS, "stable cold-start RTT {rtt}");
            }
            assert_eq!(cold.fps(), FPS);
            let mut qos = session(abr);
            for _ in 0..90 {
                second(&mut qos, 10, true);
            }
            assert_eq!(qos.fps(), FPS);
            let mut relearned = true;
            for s in 0..180 {
                second(&mut qos, rtt, true);
                if s >= 120 {
                    let base = qos.users[&1].delay.rtt_calculator.get_rtt().unwrap();
                    relearned &= qos.fps() == FPS && rtt.saturating_sub(base) < 150;
                }
            }
            let base = qos.users[&1].delay.rtt_calculator.get_rtt().unwrap();
            let high_fps = qos.fps();
            let high_ratio = qos.ratio();
            for _ in 0..90 {
                second(&mut qos, 10, true);
            }
            assert_eq!(
                qos.fps(),
                FPS,
                "return to the original path: ABR={abr} RTT={rtt}"
            );
            assert!(qos.ratio() >= BR_BALANCED * 0.95);
            println!("| {abr} | {rtt} ms | {} | {base} ms | {} ms | {high_fps} | {high_ratio:.3} | {relearned} | {} |",
                cold.fps(), rtt.saturating_sub(base), qos.fps());
            if !relearned {
                unmet.push(format!(
                    "ABR={abr}, RTT 10 -> {rtt} ms: base={base}, FPS={high_fps}"
                ));
            }
        }
    }
    unmet
}

#[test]
fn baseline_step_relearns_higher_rtt() {
    let unmet = baseline_steps();
    assert!(
        unmet.is_empty(),
        "higher baseline was not relearned: {unmet:?}"
    );
}

#[test]
fn static_to_dynamic_ratio_recovery() {
    println!("Static recovery: 90 s healthy video, 12 s confirmed 800 ms delay, 60 s healthy static screen, then 90 s video. Bitrate is modeled as ratio * 6000 kbps.");
    println!("| restart profile | ratio after cut | ratio after static | time to 90% | time to 95% | final ratio | final FPS | modeled bitrate |");
    println!("|---|---:|---:|---|---|---:|---:|---:|");
    for (profile, restart_delay, growing_queue) in [
        ("healthy 10 ms", 10, false),
        ("stable path 800 ms", 800, false),
        ("growing queue 800 + 10 ms/s", 800, true),
    ] {
        let mut qos = session(true);
        for _ in 0..90 {
            second(&mut qos, 10, true);
        }
        let target = qos.latest_quality().ratio();
        assert_eq!(qos.ratio(), target);
        for _ in 0..12 {
            second(&mut qos, 800, true);
        }
        let after_cut = qos.ratio();
        assert!(
            after_cut < target * 0.5,
            "fixture must confirm congestion and cut bitrate"
        );
        for _ in 0..60 {
            second(&mut qos, 10, false);
        }
        let after_static = qos.ratio();
        let mut t90 = (after_static >= target * 0.90).then_some(0);
        let mut t95 = (after_static >= target * 0.95).then_some(0);
        for s in 1..=90 {
            let delay = restart_delay + if growing_queue { s * 10 } else { 0 };
            second(&mut qos, delay, true);
            let ratio = qos.ratio();
            if ratio >= target * 0.90 {
                t90.get_or_insert(s);
            }
            if ratio >= target * 0.95 {
                t95.get_or_insert(s);
            }
            if growing_queue {
                assert!(
                    ratio <= after_static * 1.02,
                    "activity must not restore quality into congestion"
                );
            }
        }
        let seconds = |time: Option<u32>| {
            time.map(|s| format!("{s} s"))
                .unwrap_or_else(|| "never".to_owned())
        };
        let ratio = qos.ratio();
        println!("| {profile} | {after_cut:.3} | {after_static:.3} | {} | {} | {ratio:.3} | {} | {} kbps |",
            seconds(t90), seconds(t95), qos.fps(), qos.bitrate());
        if !growing_queue {
            assert!(
                t95.is_some(),
                "{profile}: video did not regain 95% quality within 90 s"
            );
            assert_eq!(qos.fps(), FPS);
        }
    }
}
