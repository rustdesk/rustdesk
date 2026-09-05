use super::*;

fn abr_session() -> VideoQoS {
    let mut qos = stable_qos();
    qos.new_display("test".to_owned());
    qos.set_support_changing_quality("test", true);
    qos.store_bitrate(4000);
    qos.adjust_ratio_instant = Instant::now() - Duration::from_secs(4);
    qos
}

#[test]
fn bitrate_reduction_precedes_ordinary_fps_reduction() {
    let mut qos = abr_session();
    let ratio = qos.ratio();
    qos.user_network_delay(1, 400);
    assert_eq!(qos.fps(), FPS);
    assert_eq!(qos.ratio(), ratio);
    qos.user_network_delay(1, 400);
    assert_eq!(qos.fps(), FPS);
    assert!(qos.ratio() < ratio);
    qos.user_network_delay(1, 400);
    assert_eq!(qos.fps(), FPS);
    qos.user_network_delay(1, 400);
    assert!(qos.fps() < FPS);
}

#[test]
fn bitrate_cooldown_defers_ordinary_fps_reduction() {
    let mut qos = abr_session();
    qos.adjust_ratio_instant = Instant::now();
    let ratio = qos.ratio();
    for _ in 0..3 {
        qos.user_network_delay(1, 400);
        assert_eq!(qos.fps(), FPS);
        assert_eq!(qos.ratio(), ratio);
    }
    qos.adjust_ratio_instant = Instant::now() - Duration::from_secs(4);
    qos.user_network_delay(1, 400);
    assert_eq!(qos.fps(), FPS);
    assert!(qos.ratio() < ratio);
    qos.user_network_delay(1, 400);
    assert_eq!(qos.fps(), FPS);
    qos.user_network_delay(1, 400);
    assert!(qos.fps() < FPS);
}

#[test]
fn unavailable_abr_or_minimum_bitrate_does_not_prevent_fps_reduction() {
    for mode in ["disabled", "unsupported", "minimum"] {
        let mut qos = abr_session();
        match mode {
            "disabled" => qos.abr_config = false,
            "unsupported" => qos.set_support_changing_quality("test", false),
            "minimum" => qos.ratio = BR_MIN_HIGH_RESOLUTION,
            _ => unreachable!(),
        }
        for _ in 0..3 {
            qos.user_network_delay(1, 400);
        }
        assert!(qos.fps() < FPS, "{mode}");
    }
}

#[test]
fn severe_delay_and_timeout_bypass_bitrate_cooldown() {
    let mut qos = abr_session();
    qos.adjust_ratio_instant = Instant::now();
    qos.user_network_delay(1, 1200);
    assert_eq!(qos.fps(), 15);
    qos.user_delay_response_elapsed(1, 2500);
    assert!(qos.fps() <= 2);
}

#[test]
fn bitrate_timer_does_not_punish_unconfirmed_spikes_or_stale_averages() {
    let mut qos = abr_session();
    let ratio = qos.ratio();
    for delay in [800, 10, 350, 10, 350, 10].repeat(10) {
        qos.user_network_delay(1, delay);
        qos.adjust_ratio(false);
        assert_eq!(qos.fps(), FPS);
        assert_eq!(qos.ratio(), ratio);
    }
}

#[test]
fn viewers_confirm_congestion_independently() {
    let mut qos = stable_qos();
    qos.users.insert(2, UserData::default());
    for _ in 0..30 {
        qos.user_network_delay(2, 10);
        qos.user_network_delay(1, 10);
    }
    for id in [1, 2, 1, 2] {
        qos.user_network_delay(id, 400);
        assert_eq!(qos.fps(), FPS);
    }
    qos.user_network_delay(2, 10);
    qos.user_network_delay(1, 400);
    assert!(qos.fps() < FPS);
    qos.user_custom_fps(2, 12);
    qos.user_network_delay(1, 10);
    assert_eq!(qos.fps(), 12);
}

#[test]
fn pending_probe_checks_do_not_count_as_fresh_bad_replies() {
    let mut qos = stable_qos();
    qos.user_network_delay(1, 400);
    for elapsed in [1000, 1500, 1900] {
        qos.user_delay_response_elapsed(1, elapsed);
        assert_eq!(qos.fps(), FPS);
    }
    qos.user_network_delay(1, 400);
    assert_eq!(qos.fps(), FPS);
    qos.user_network_delay(1, 10);
    qos.user_network_delay(1, 400);
    qos.user_network_delay(1, 400);
    assert_eq!(qos.fps(), FPS);
}

#[test]
fn recovery_continues_with_intermittent_jitter() {
    let mut qos = stable_qos();
    for _ in 0..10 {
        qos.user_network_delay(1, 800);
    }
    assert!(qos.fps() <= 3);
    for delay in [10, 350].repeat(30) {
        qos.user_network_delay(1, delay);
    }
    assert_eq!(qos.fps(), FPS);
}
