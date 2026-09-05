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
                if matches!(
                    name,
                    "lan_jitter"
                        | "isolated_spikes"
                        | "alternating_10_350"
                        | "two_sample_bursts"
                        | "threshold_jitter"
                ) {
                    assert!(
                        trace.iter().all(|fps| *fps == limit),
                        "{name}, {quality_name}"
                    );
                }
                if name == "congestion_800_recovery" {
                    assert!(
                        trace[5] <= (limit / 8).max(3),
                        "severe congestion: {trace:?}"
                    );
                    assert!(
                        trace[20] <= 3,
                        "a single good reply must not restore the full frame rate"
                    );
                    assert!(
                        trace[22] >= limit.min(8),
                        "fresh replies must permit recovery"
                    );
                }
                if name.ends_with("_recovery") {
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
    let mut recovered_at = None;
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
            if now >= 70_000 && qos.fps() == 30 && recovered_at.is_none() {
                recovered_at = Some(now - 70_000);
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
        "bandwidth 40 -> 15 -> 40 fps: max_queue_ms={max_queue_ms}, recovery_ms={recovered_at:?}, final_fps={}",
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
        recovered_at.is_some_and(|ms| ms <= 15_000) && qos.fps() == 30,
        "FPS must recover when capacity returns"
    );
}

#[test]
fn smoke_capacity_with_short_stalls() {
    for limit in [30, 60] {
        for phase in [0, 500, 900] {
            let mut qos = session(limit, Quality::Balanced);
            for _ in 0..90 {
                qos.user_network_delay(1, 10);
            }
            let capacity = limit as f64 * 2.0;
            let mut queue = 0.0_f64;
            let mut probe: Option<(u32, f64, Option<u32>)> = None;
            let mut max_delay = 0;
            for now in (0..120_000).step_by(10) {
                // A 700 ms pause affects video and probes on the same FIFO link.
                let available = if (now + phase) % 6000 < 700 {
                    0.0
                } else {
                    capacity
                };
                queue = (queue + (qos.fps() as f64 - available) * 0.01).max(0.0);
                if let Some((_, remaining, reply_at)) = probe.as_mut() {
                    *remaining -= available * 0.01;
                    if available > 0.0 && *remaining <= 0.0 && reply_at.is_none() {
                        *reply_at = Some(now + 10);
                    }
                }
                if let Some((sent, _, Some(reply_at))) = probe {
                    if now >= reply_at {
                        max_delay = max_delay.max(now - sent);
                        qos.user_network_delay(1, now - sent);
                        probe = None;
                    }
                }
                if now % 1000 == 0 {
                    if probe.is_none() {
                        probe = Some((now, queue, None));
                    }
                    qos.user_delay_response_elapsed(1, (now - probe.unwrap().0) as u128);
                }
                assert_eq!(qos.fps(), limit, "limit={limit}, phase={phase}, time={now}");
            }
            println!("healthy FIFO link: limit={limit}, phase={phase}, max_delay_ms={max_delay}, final_fps={}", qos.fps());
        }
    }
}

#[test]
fn smoke_abr_bandwidth_drop_and_recovery() {
    for reduced_capacity in [27, 24, 15] {
        let mut qos = session(30, Quality::Balanced);
        for _ in 0..90 {
            qos.user_network_delay(1, 10);
        }
        qos.abr_config = true;
        qos.new_display("test".to_owned());
        qos.set_support_changing_quality("test", true);
        qos.store_bitrate(4000);
        let initial_ratio = qos.ratio();
        let mut queue = 0.0_f64;
        let mut probe: Option<(u32, f64, Option<u32>)> = None;
        let mut ratio_changed_at = 0;
        let mut first_ratio_drop = None;
        let mut first_fps_drop = None;
        let mut first_fps_drop_delay = None;
        let mut last_delay = 10;
        let mut max_queue_ms = 0;
        let mut drained = false;
        let mut recovered_at = None;
        for now in (0..120_000).step_by(10) {
            let capacity = if (10_000..70_000).contains(&now) {
                reduced_capacity as f64
            } else {
                40.0
            };
            // Frame size scales with the requested ratio; video and probes share one FIFO.
            let frame_size = (qos.ratio() / initial_ratio) as f64;
            queue = (queue + (qos.fps() as f64 * frame_size - capacity) * 0.01).max(0.0);
            qos.store_bitrate((4000.0 * frame_size) as u32);
            let simulated_instant =
                Instant::now() - Duration::from_millis((now - ratio_changed_at) as u64);
            qos.adjust_ratio_instant = simulated_instant;
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
                qos.update_display_data("test", qos.fps() as usize);
                let queue_ms = (queue / capacity * 1000.0) as u32;
                max_queue_ms = max_queue_ms.max(queue_ms);
                if (20_000..40_000).contains(&now) && queue_ms < 100 {
                    drained = true;
                }
            }
            if qos.adjust_ratio_instant != simulated_instant {
                ratio_changed_at = now;
            }
            if qos.ratio() < initial_ratio && first_ratio_drop.is_none() {
                first_ratio_drop = Some(now);
            }
            if qos.fps() < 30 && first_fps_drop.is_none() {
                first_fps_drop = Some(now);
                first_fps_drop_delay = Some(last_delay);
            }
            if now >= 70_000 && qos.fps() == 30 && recovered_at.is_none() {
                recovered_at = Some(now - 70_000);
            }
        }
        println!("ABR bandwidth 40 -> {reduced_capacity} -> 40: max_queue_ms={max_queue_ms}, first_ratio_drop_ms={first_ratio_drop:?}, first_fps_drop_ms={first_fps_drop:?}, first_fps_drop_delay_ms={first_fps_drop_delay:?}, recovery_ms={recovered_at:?}, final_fps={}, final_ratio={:.3}", qos.fps(), qos.ratio());
        assert!(drained && max_queue_ms < 2500);
        assert!(recovered_at.is_some_and(|ms| ms <= 15_000));
        assert_eq!(qos.fps(), 30);
        assert!(qos.ratio() >= initial_ratio * 0.8);
        if reduced_capacity >= 24 {
            assert!(first_ratio_drop.is_some_and(|ratio_time| {
                first_fps_drop.map_or(true, |fps_time| ratio_time < fps_time)
            }));
        }
    }
}
