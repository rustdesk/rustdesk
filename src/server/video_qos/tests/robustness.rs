//! Guards against tuning the controller to the simulator: the CI bounds applied
//! to seeds that never took part in setting them, and a sweep of the scenario
//! parameters.  Both are `#[ignore]`d: they take a few seconds and are meant for
//! anyone changing a controller constant or a bound.
//!
//! ```text
//! cargo test --lib video_qos::tests::robustness -- --ignored --nocapture
//! ```
use super::sim::{bound_violations, run, scenarios, Report, Scenario, Summary};
use super::*;

fn summarise(sc: &Scenario, seeds: impl Iterator<Item = u64>) -> Summary {
    let reports: Vec<Report> = seeds
        .map(|seed| run(&Scenario { seed, ..sc.clone() }))
        .collect();
    Summary::of(&reports)
}

/// Seeds 21 to 120 in five blocks of twenty, each block held to the CI bounds.
#[test]
#[ignore]
fn held_out_seeds() {
    println!("| scenario | blocks violating | which | median target | worst p10 | below limit/2 (p90) | queue p95 (p90) |");
    println!("|---|---:|---|---:|---:|---:|---:|");
    let mut failures = Vec::new();
    for sc in scenarios() {
        let mut failing_blocks = 0;
        let mut which: Vec<&str> = Vec::new();
        let mut reports: Vec<Report> = Vec::new();
        for block in 0..5u64 {
            let first = 21 + block * 20;
            let block_reports: Vec<Report> = (first..first + 20)
                .map(|seed| run(&Scenario { seed, ..sc.clone() }))
                .collect();
            let s = Summary::of(&block_reports);
            reports.extend(block_reports);
            let violations = bound_violations(&s);
            if !violations.is_empty() {
                failing_blocks += 1;
                for v in violations {
                    if !which.contains(&v) {
                        which.push(v);
                    }
                }
            }
        }
        let all = Summary::of(&reports);
        println!(
            "| {} | {}/5 | {} | {:.1} | {} | {:.1}% | {} ms |",
            sc.name,
            failing_blocks,
            which.join("; "),
            all.mean_target_median,
            all.p10_target_worst,
            all.below_half_p90,
            all.queue_p95_p90
        );
        if failing_blocks > 0 {
            failures.push(format!(
                "{} ({} of 5 blocks: {})",
                sc.name,
                failing_blocks,
                which.join("; ")
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "bounds fail on held-out seeds, so they were fitted to the CI seeds: {failures:?}"
    );
}

/// One scenario parameter at a time, halved and doubled, twenty seeds each.  No
/// bounds: the point is to see that nothing falls off a cliff, and to compare with
/// master by running the same test there.
#[test]
#[ignore]
fn sensitivity() {
    let base = scenarios();
    let pick = |name: &str| base.iter().find(|s| s.name == name).unwrap().clone();
    let home = pick("home_wifi_30");
    let halved = pick("bandwidth_halved_30");
    let with_link = |sc: &Scenario, edit: &dyn Fn(&mut super::sim::Link)| {
        let mut link = sc.link.clone();
        edit(&mut link);
        Scenario { link, ..sc.clone() }
    };
    let variants: Vec<(&str, Scenario)> = vec![
        ("home base", home.clone()),
        (
            "home stalls half as long",
            with_link(&home, &|l| l.stall_ms = (150.0, 1250.0)),
        ),
        (
            "home stalls twice as long",
            with_link(&home, &|l| l.stall_ms = (600.0, 5000.0)),
        ),
        (
            "home stalls twice as often",
            with_link(&home, &|l| l.stall_mean_interval_s = 10.0),
        ),
        (
            "home stalls half as often",
            with_link(&home, &|l| l.stall_mean_interval_s = 40.0),
        ),
        (
            "home loss doubled",
            with_link(&home, &|l| l.loss_per_s = 0.4),
        ),
        (
            "home capacity halved",
            with_link(&home, &|l| l.capacity_kbps = vec![(0, 10_000.0)]),
        ),
        (
            "home capacity 6 Mbps",
            with_link(&home, &|l| l.capacity_kbps = vec![(0, 6_000.0)]),
        ),
        (
            "home jitter heavier",
            with_link(&home, &|l| {
                l.jitter_sigma = 1.5;
                l.jitter_median_ms = 30.0;
            }),
        ),
        (
            "home base rtt 80 ms",
            with_link(&home, &|l| l.base_rtt_ms = 80.0),
        ),
        (
            "home 60 fps limit",
            Scenario {
                limit: 60,
                ..home.clone()
            },
        ),
        ("bandwidth halved base", halved.clone()),
        (
            "bandwidth to 4 Mbps",
            with_link(&halved, &|l| {
                l.capacity_kbps = vec![(0, 8_000.0), (60_000, 4_000.0), (120_000, 8_000.0)]
            }),
        ),
        (
            "bandwidth to 1.5 Mbps",
            with_link(&halved, &|l| {
                l.capacity_kbps = vec![(0, 8_000.0), (60_000, 1_500.0), (120_000, 8_000.0)]
            }),
        ),
        (
            "bandwidth halved, never restored, 300 s",
            Scenario {
                seconds: 300,
                ..with_link(&halved, &|l| {
                    l.capacity_kbps = vec![(0, 8_000.0), (60_000, 2_500.0)]
                })
            },
        ),
    ];
    println!("| variant | median target | worst p10 | below limit/2 (p90) | queue p95 (p90) | delivered | sustained recovery (worst) |");
    println!("|---|---:|---:|---:|---:|---:|---:|");
    for (label, sc) in &variants {
        let s = summarise(sc, super::sim::SEEDS);
        println!(
            "| {} | {:.1} | {} | {:.1}% | {} ms | {:.1} | {} |",
            label,
            s.mean_target_median,
            s.p10_target_worst,
            s.below_half_p90,
            s.queue_p95_p90,
            s.delivered_median,
            if s.has_restore {
                s.recovery_worst_ms
                    .map(|ms| format!("{:.1}s", ms as f64 / 1000.0))
                    .unwrap_or_else(|| "never".to_owned())
            } else {
                "-".to_owned()
            }
        );
    }
}
