use super::*;

#[test]
fn ordinary_congestion_waits_between_bounded_cuts() {
    for delay in [400, 800] {
        let mut qos = stable_qos();
        for _ in 0..2 {
            qos.user_network_delay(1, delay);
            assert_eq!(qos.fps(), FPS);
        }
        qos.user_network_delay(1, delay);
        let first_cut = qos.fps();
        assert!((24..FPS).contains(&first_cut), "delay={delay}: {first_cut}");
        for _ in 0..2 {
            qos.user_network_delay(1, delay);
            assert_eq!(qos.fps(), first_cut, "wait for new evidence after a cut");
        }
        qos.user_network_delay(1, delay);
        assert!(qos.fps() < first_cut);
        assert!(qos.fps() >= first_cut - first_cut / 5);
    }
}

#[test]
fn automatic_floor_preserves_lower_custom_limits() {
    for limit in [1, 3, 5, 30, 60, 120] {
        for abr in [false, true] {
            for timeout in [false, true] {
                let mut qos = super::smoke::session(limit, Quality::Balanced);
                qos.abr_config = abr;
                qos.new_display("test".to_owned());
                qos.set_support_changing_quality("test", true);
                for _ in 0..90 {
                    qos.user_network_delay(1, 10);
                }
                assert_eq!(qos.fps(), limit);
                qos.ratio = BR_MIN_HIGH_RESOLUTION;
                qos.store_bitrate(600);
                let floor = 5.min(limit);
                if timeout {
                    for elapsed in [2001, 3001, 4001, 5001, 10_000, 30_000] {
                        qos.user_delay_response_elapsed(1, elapsed);
                        assert!(
                            (floor..=limit).contains(&qos.fps()),
                            "timeout={elapsed}, limit={limit}, ABR={abr}"
                        );
                    }
                } else {
                    for _ in 0..8 {
                        qos.user_network_delay(1, 1500);
                        assert!(
                            (floor..=limit).contains(&qos.fps()),
                            "limit={limit}, ABR={abr}"
                        );
                    }
                }
                assert_eq!(
                    qos.fps(),
                    floor,
                    "timeout={timeout}, limit={limit}, ABR={abr}"
                );
                if timeout {
                    qos.user_network_delay(1, 30_100);
                    assert_eq!(qos.fps(), floor, "a late reply must preserve the floor");
                }
                for _ in 0..2 {
                    qos.user_network_delay(1, 10);
                }
                assert_eq!(qos.fps(), limit, "recover: timeout={timeout}, ABR={abr}");
            }
        }
    }
}

#[test]
fn two_good_replies_restore_after_a_severe_stall() {
    let mut qos = stable_qos();
    qos.user_delay_response_elapsed(1, 5001);
    assert_eq!(qos.fps(), 5);
    qos.user_network_delay(1, 5100);
    assert_eq!(qos.fps(), 5, "the late reply must not brake twice");
    qos.user_network_delay(1, 10);
    assert!(
        (5..FPS).contains(&qos.fps()),
        "one good reply is not enough"
    );
    qos.user_network_delay(1, 10);
    assert_eq!(qos.fps(), FPS);
}

#[test]
fn jitter_during_recovery_does_not_discard_the_restore_target() {
    let mut qos = stable_qos();
    for _ in 0..3 {
        qos.user_network_delay(1, 1200);
    }
    for delay in [10, 350, 10] {
        qos.user_network_delay(1, delay);
    }
    assert_eq!(qos.fps(), FPS);
}

#[test]
fn a_failed_fast_restore_rolls_back_before_the_queue_grows() {
    let mut qos = stable_qos();
    qos.user_delay_response_elapsed(1, 5001);
    for _ in 0..2 {
        qos.user_network_delay(1, 10);
    }
    assert_eq!(qos.fps(), FPS);
    qos.user_network_delay(1, 800);
    assert_eq!(qos.fps(), FPS / 2);
    for _ in 0..2 {
        qos.user_network_delay(1, 10);
    }
    assert!(
        qos.fps() < FPS,
        "a failed restore must lower the next probe"
    );
}
