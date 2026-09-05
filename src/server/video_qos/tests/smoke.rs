use super::*;

fn session(fps: u32, quality: Quality) -> VideoQoS {
    let mut qos = VideoQoS {
        fps: INIT_FPS.min(fps),
        abr_config: false,
        new_user_instant: Instant::now() - Duration::from_secs(2),
        ..Default::default()
    };
    qos.users.insert(
        1,
        UserData {
            custom_fps: Some(fps),
            quality: Some((0, quality)),
            ..Default::default()
        },
    );
    qos
}

fn profiles() -> Vec<(&'static str, Vec<u32>, bool)> {
    vec![
        ("stable_10", vec![10; 120], false),
        ("stable_80", vec![80; 120], false),
        ("stable_180", vec![180; 120], false),
        ("stable_300", vec![300; 120], false),
        (
            "lan_jitter",
            (0..120).map(|i| 10 + (i * 37 % 70)).collect(),
            true,
        ),
        (
            "isolated_spikes",
            (0..120)
                .map(|i| if i % 15 == 0 { 800 } else { 10 })
                .collect(),
            true,
        ),
        ("alternating_10_350", [10, 350].repeat(60), true),
        (
            "two_sample_bursts",
            (0..120)
                .map(|i| if i % 12 < 2 { 700 } else { 10 })
                .collect(),
            true,
        ),
        (
            "threshold_jitter",
            [140, 180, 150, 190, 130, 170].repeat(20),
            true,
        ),
        (
            "congestion_200_recovery",
            [vec![200; 20], vec![10; 60]].concat(),
            true,
        ),
        (
            "congestion_800_recovery",
            [vec![800; 20], vec![10; 60]].concat(),
            true,
        ),
        (
            "rising_then_falling",
            (0..80).map(|i| 10 + i.min(79 - i) * 20).collect(),
            true,
        ),
    ]
}

#[test]
fn smoke_latency_profiles() {
    use std::fmt::Write;

    let mut csv = String::from("profile,limit,quality,sample,delay_ms,fps\n");
    for (quality_name, quality) in [
        ("balanced", Quality::Balanced),
        ("best", Quality::Best),
        ("low", Quality::Low),
    ] {
        for limit in [1, 5, 15, 30, 60, 120] {
            for (name, delays, warm_up) in profiles() {
                let mut qos = session(limit, quality);
                if warm_up {
                    for _ in 0..90 {
                        qos.user_network_delay(1, 10);
                    }
                    assert_eq!(qos.fps(), limit);
                }
                let mut trace = Vec::new();
                for (i, delay) in delays.into_iter().enumerate() {
                    qos.user_network_delay(1, delay);
                    let fps = qos.fps();
                    assert!((MIN_FPS..=limit).contains(&fps), "{name}: {fps}");
                    trace.push(fps);
                    writeln!(csv, "{name},{limit},{quality_name},{i},{delay},{fps}").unwrap();
                }
                if limit == 30 && quality_name == "balanced" {
                    println!(
                        "{name}: first20={:?}, last={}",
                        &trace[..20],
                        trace.last().unwrap()
                    );
                }
                if name.starts_with("stable_") {
                    assert_eq!(trace.last(), Some(&limit), "{name}, {quality_name}");
                }
                if matches!(name, "lan_jitter" | "isolated_spikes") {
                    assert!(
                        trace.iter().all(|fps| *fps == limit),
                        "{name}, {quality_name}"
                    );
                }
                if name == "alternating_10_350" && limit > 12 {
                    assert!(trace[29] < limit, "recurring delay must reduce FPS");
                    let tail = &trace[90..];
                    assert!(tail.iter().max().unwrap() - tail.iter().min().unwrap() <= 2);
                }
                if name == "congestion_800_recovery" {
                    assert!(
                        trace[4] <= (limit / 8).max(3),
                        "severe congestion: {trace:?}"
                    );
                    assert!(
                        trace[22] <= 3,
                        "recovery must not jump to the quality floor"
                    );
                }
                if name.ends_with("_recovery") && limit <= 60 {
                    assert_eq!(trace.last(), Some(&limit), "{name}, {quality_name}");
                }
            }
        }
    }
    if let Ok(path) = std::env::var("RUSTDESK_QOS_SMOKE_CSV") {
        std::fs::write(path, csv).unwrap();
    }
}

#[test]
fn smoke_bandwidth_drop_and_recovery() {
    use std::fmt::Write;

    let mut qos = session(30, Quality::Balanced);
    for _ in 0..90 {
        qos.user_network_delay(1, 10);
    }
    // Fixed-size frames, FIFO link and one outstanding TestDelay, with a 10 ms base RTT.
    // Encoding and ABR are intentionally absent so that FPS alone controls offered load.
    let mut queue = 0.0_f64;
    let mut probe: Option<(u32, f64, Option<u32>)> = None;
    let mut last_delay = 10;
    let mut csv = String::from("time_ms,capacity_fps,queue_ms,delay_ms,fps\n");
    let mut max_queue_ms = 0;
    let mut drained = false;
    let mut recovered = false;
    for now in (0..120_000).step_by(10) {
        let capacity = if (10_000..70_000).contains(&now) {
            15.0
        } else {
            40.0
        };
        queue = (queue + (qos.fps() as f64 - capacity) * 0.01).max(0.0);
        if let Some((_, remaining, reply_at)) = probe.as_mut() {
            *remaining -= capacity * 0.01;
            if *remaining <= 0.0 && reply_at.is_none() {
                *reply_at = Some(now + 10);
            }
        }
        if let Some((sent, _, Some(reply_at))) = probe {
            if now >= reply_at {
                last_delay = now - sent;
                qos.user_network_delay(1, last_delay);
                probe = None;
            }
        }
        if now % 1000 == 0 {
            if probe.is_none() {
                probe = Some((now, queue, None));
            }
            qos.user_delay_response_elapsed(1, (now - probe.unwrap().0) as u128);
            let queue_ms = (queue / capacity * 1000.0) as u32;
            max_queue_ms = max_queue_ms.max(queue_ms);
            if (20_000..40_000).contains(&now) && queue_ms < 100 {
                drained = true;
            }
            if now >= 70_000 && qos.fps() == 30 {
                recovered = true;
            }
            writeln!(
                csv,
                "{now},{capacity},{queue_ms},{last_delay},{}",
                qos.fps()
            )
            .unwrap();
        }
    }
    println!(
        "bandwidth 40 -> 15 -> 40 fps: max_queue_ms={max_queue_ms}, final_fps={}",
        qos.fps()
    );
    if let Ok(path) = std::env::var("RUSTDESK_QOS_SMOKE_CSV") {
        std::fs::write(std::path::Path::new(&path).with_extension("queue.csv"), csv).unwrap();
    }
    assert!(drained, "congestion must drain after the capacity drop");
    assert!(
        max_queue_ms < 2500,
        "queue must not grow while awaiting confirmation"
    );
    assert!(
        recovered && qos.fps() == 30,
        "FPS must recover when capacity returns"
    );
}
