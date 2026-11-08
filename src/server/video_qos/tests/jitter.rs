use super::*;

fn abr_session() -> VideoQoS {
    let mut qos = stable_qos();
    qos.new_display("test".to_owned());
    qos.set_support_changing_quality("test", true);
    qos.store_bitrate(4000);
    // Linux skips the first-reply adjustment; exercise it on every platform.
    qos.first_reply_adjusts_ratio = true;
    qos.advance_ms(4000);
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
    qos.adjust_ratio_instant = qos.now();
    qos.user_network_delay(1, 1200);
    assert_eq!(qos.fps(), 15);
    qos.user_delay_response_elapsed(1, 2500);
    assert_eq!(qos.fps(), 7);
}

#[test]
fn a_single_stall_does_not_cut_bitrate() {
    // One probe out for 2.5 s, then its late reply: jitter, not congestion.
    let mut qos = abr_session();
    let ratio = qos.ratio();
    qos.user_delay_response_elapsed(1, 2500);
    qos.advance_ms(1000);
    qos.update_display_data("test", 30);
    assert_eq!(qos.ratio(), ratio, "the timeout tick alone");
    qos.user_network_delay(1, 2600);
    assert_eq!(qos.ratio(), ratio, "the late reply alone");
    qos.user_network_delay(1, 400);
    assert!(qos.ratio() < ratio, "a second bad reply confirms");
}

#[test]
fn a_stall_beyond_three_seconds_cuts_bitrate() {
    let mut qos = abr_session();
    let ratio = qos.ratio();
    qos.user_delay_response_elapsed(1, 2001);
    qos.update_display_data("test", 30);
    assert_eq!(qos.ratio(), ratio);
    qos.advance_ms(1000);
    qos.user_delay_response_elapsed(1, 3001);
    qos.update_display_data("test", 30);
    assert!(qos.ratio() < ratio, "still out at the next tick");
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
fn a_congested_viewer_does_not_lower_another_viewers_target() {
    let mut qos = stable_qos();
    qos.users.insert(2, UserData::default());
    for _ in 0..30 {
        qos.user_network_delay(2, 10);
        qos.user_network_delay(1, 10);
    }
    assert_eq!(qos.fps(), FPS);
    // Viewer 1 congests; the stream follows the slowest viewer.
    for _ in 0..2 {
        qos.user_network_delay(1, 1200);
    }
    assert_eq!(qos.fps(), 8);
    // Viewer 2 is fine and keeps its own target rather than inheriting viewer 1's.
    qos.user_network_delay(2, 10);
    assert_eq!(qos.users[&2].delay.fps, Some(FPS));
    // Once viewer 1 restores, the stream is back at once.
    for _ in 0..3 {
        qos.user_network_delay(1, 10);
    }
    assert_eq!(qos.fps(), FPS);
}
