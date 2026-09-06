use super::*;

const DISPLAY: &str = "relay";

fn session(limit: u32, quality: Quality, abr: bool) -> VideoQoS {
    let mut qos = super::smoke::session(limit, quality);
    qos.abr_config = abr;
    qos.new_display(DISPLAY.to_owned());
    qos.set_support_changing_quality(DISPLAY, true);
    qos
}

fn sync_bitrate(qos: &mut VideoQoS) {
    // The recorded session used 1080p H265: its balanced-quality bitrate budget
    // is 2073 * 1.5 * ratio kbps, rather than the simulator's 6000 * ratio.
    let bitrate = (2073.0 * 1.5 * qos.ratio()) as u32;
    qos.store_bitrate(bitrate);
}

#[test]
fn moderate_delay_floor_respects_quality_and_custom_limit() {
    for (quality, floor) in [
        (Quality::Best, 8),
        (Quality::Balanced, 10),
        (Quality::Low, 12),
    ] {
        for limit in [1, 5, 15, 30, 60, 120] {
            for mode in ["minimum", "disabled", "unsupported"] {
                let mut qos = session(limit, quality, mode != "disabled");
                qos.set_support_changing_quality(DISPLAY, mode != "unsupported");
                for _ in 0..90 {
                    qos.user_network_delay(1, 159);
                }
                sync_bitrate(&mut qos);
                qos.ratio = qos.min_ratio();
                sync_bitrate(&mut qos);
                assert!(!qos.can_reduce_bitrate());
                for delay in [319, 359, 559, 959].repeat(30) {
                    qos.advance_ms(1000);
                    qos.user_network_delay(1, delay);
                    assert!(
                        (floor.min(limit)..=limit).contains(&qos.fps()),
                        "{quality:?}, {mode}, limit={limit}, delay={delay}, fps={}",
                        qos.fps()
                    );
                }
            }
        }
    }
}

#[test]
fn severe_delay_and_timeout_can_go_below_the_usable_floor() {
    for abr in [false, true] {
        for timeout in [false, true] {
            let mut qos = session(FPS, Quality::Balanced, abr);
            for _ in 0..90 {
                qos.user_network_delay(1, 159);
            }
            sync_bitrate(&mut qos);
            qos.ratio = qos.min_ratio();
            sync_bitrate(&mut qos);
            if timeout {
                qos.user_delay_response_elapsed(1, 5001);
            } else {
                for _ in 0..4 {
                    qos.user_network_delay(1, 1500);
                }
            }
            assert!(
                qos.fps() <= 3,
                "ABR={abr}, timeout={timeout}: {}",
                qos.fps()
            );
            let braked_fps = qos.fps();
            qos.user_network_delay(1, 400);
            assert!(
                qos.fps() <= braked_fps,
                "the floor must not undo a severe cut"
            );
            for _ in 0..3 {
                qos.user_network_delay(1, 159);
            }
            assert_eq!(qos.fps(), FPS);
        }
    }
}

#[test]
fn capacity_below_the_usable_floor_still_drains() {
    let mut sc = super::sim::scenarios()
        .into_iter()
        .find(|s| s.name == "bandwidth_halved_fixed_rate_no_abr_30")
        .unwrap();
    // Without ABR, 700 kbps carries only about five of the modeled frames per
    // second. The reply/timeout emergency paths must be able to break the floor.
    sc.link.capacity_kbps = vec![(0, 8000.0), (60_000, 700.0), (120_000, 8000.0)];
    for seed in super::sim::SEEDS {
        sc.seed = seed;
        let report = super::sim::run(&sc);
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
        assert!(min_fps < 10, "seed {seed}: FPS never went below the floor");
        assert!(
            min_queue < 200,
            "seed {seed}: queue never drained ({min_queue} ms)"
        );
        assert!(
            report.recovery_ms.is_some_and(|ms| ms <= 20_000),
            "seed {seed}: recovery took {:?}",
            report.recovery_ms
        );
        println!("700 kbps fixed-rate, seed {seed}: min FPS={min_fps}, min queue={min_queue} ms, queue p95={} ms, frame age p95={} ms, recovery={:?}", report.queue_p95_ms, report.frame_age_p95_ms, report.recovery_ms);
    }
}
