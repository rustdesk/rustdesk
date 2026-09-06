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
fn bitrate_cooldown_defers_ordinary_fps_reduction() {
    let mut qos = abr_session();
    qos.adjust_ratio_instant = qos.now();
    let ratio = qos.ratio();
    for _ in 0..3 {
        qos.user_network_delay(1, 400);
        assert_eq!(qos.fps(), FPS);
        assert_eq!(qos.ratio(), ratio);
    }
    qos.advance_ms(4000);
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
fn minimum_bitrate_during_cooldown_does_not_block_fps_reduction() {
    // ABR on, ratio at its floor, a good reply cleared the post-reduction counter,
    // and the adjustment cooldown has just restarted: bitrate cannot help here.
    let mut qos = abr_session();
    qos.ratio = BR_MIN_HIGH_RESOLUTION;
    qos.user_network_delay(1, 10);
    qos.adjust_ratio_instant = qos.now();
    for _ in 0..3 {
        qos.user_network_delay(1, 400);
    }
    assert!(qos.fps() < FPS);
}

fn abr_session_from_scratch() -> VideoQoS {
    let mut qos = VideoQoS::default();
    qos.advance_ms(2000);
    qos.users.insert(1, UserData::default());
    qos.new_display("test".to_owned());
    qos.set_support_changing_quality("test", true);
    qos.store_bitrate(4000);
    qos
}

/// The video loop reports the encoder's bitrate as soon as it applies a new ratio.
fn sync_bitrate(qos: &mut VideoQoS) {
    let target = qos.latest_quality().ratio();
    let ratio = qos.ratio();
    qos.store_bitrate((4000.0 * ratio / target) as u32);
}

/// One second of wall clock, one probe reply, one display update: what a
/// connection does every second.
fn second(qos: &mut VideoQoS, delay: u32, encoded: usize) {
    qos.advance_ms(1000);
    sync_bitrate(qos);
    qos.user_network_delay(1, delay);
    sync_bitrate(qos);
    qos.update_display_data("test", encoded);
    sync_bitrate(qos);
}

#[test]
fn stable_high_rtt_restores_bitrate() {
    for rtt in [180, 300] {
        let mut qos = abr_session_from_scratch();
        let target = qos.latest_quality().ratio();
        for _ in 0..120 {
            second(&mut qos, rtt, 30);
        }
        assert_eq!(qos.fps(), FPS, "rtt {rtt}");
        assert!(
            qos.ratio() >= target * 0.99,
            "rtt {rtt}: ratio {}",
            qos.ratio()
        );
    }
}

#[test]
fn congestion_bitrate_reduction_resets_dynamic_screen_window() {
    // A static screen encodes about one frame per second.  While the congestion path
    // adjusts the ratio at every cooldown, the periodic branch never runs, so the
    // encode counter must not keep accumulating across the whole episode.
    let mut qos = abr_session();
    for _ in 0..10 {
        qos.advance_ms(4000);
        qos.user_network_delay(1, 10);
        qos.user_network_delay(1, 400);
        qos.user_network_delay(1, 400);
        qos.update_display_data("test", 1);
    }
    for _ in 0..12 {
        qos.advance_ms(1000);
        qos.user_network_delay(1, 10);
    }
    let ratio = qos.ratio();
    qos.advance_ms(4000);
    qos.update_display_data("test", 1);
    assert!(
        qos.ratio() <= ratio,
        "a static screen must not look dynamic after congestion"
    );
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
fn stable_high_rtt_does_not_dip_at_start() {
    for rtt in [180, 300] {
        let mut qos = super::smoke::session(30, Quality::Balanced);
        for _ in 0..20 {
            qos.advance_ms(1000);
            qos.user_network_delay(1, rtt);
            assert!(qos.fps() >= INIT_FPS, "rtt {rtt}: {}", qos.fps());
        }
        assert_eq!(qos.fps(), FPS, "rtt {rtt}");
    }
}

#[test]
fn confirmed_severe_congestion_halves_bitrate() {
    let mut qos = abr_session();
    let ratio = qos.ratio();
    qos.user_network_delay(1, 800);
    qos.user_network_delay(1, 800); // two bad replies: an ordinary step
    let after_first = qos.ratio();
    assert!(
        after_first < ratio && after_first > ratio * 0.75,
        "{after_first}"
    );
    qos.user_network_delay(1, 800); // three: confirmed
    qos.advance_ms(4000);
    qos.update_display_data("test", 30);
    assert!(qos.ratio() <= after_first * 0.55, "{}", qos.ratio());
}

#[test]
fn fps_holds_its_floor_while_bitrate_can_still_drop() {
    // With a bitrate-targeted encoder fewer frames do not mean fewer bytes, so the
    // bitrate comes down first and the frame rate keeps its floor meanwhile.
    let mut qos = abr_session();
    let mut reached_floor = false;
    // Keep the queue growing, rather than presenting a stable new path delay.
    let mut delay = 400;
    for _ in 0..60 {
        qos.advance_ms(3000);
        second(&mut qos, delay, 30);
        delay += 10;
        if qos.ratio() > 0.17 {
            assert!(
                qos.fps() >= 10,
                "fps {} at ratio {}",
                qos.fps(),
                qos.ratio()
            );
        } else {
            reached_floor = true;
            break;
        }
    }
    assert!(
        reached_floor,
        "bitrate must reach its floor: {}",
        qos.ratio()
    );
    for _ in 0..24 {
        qos.user_network_delay(1, delay);
        delay += 10;
    }
    assert!(
        qos.fps() < 10,
        "an exhausted bitrate frees the frame rate: {}",
        qos.fps()
    );
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
    for _ in 0..3 {
        qos.user_network_delay(1, 1200);
    }
    assert_eq!(qos.fps(), 5);
    for delay in [10, 350].repeat(30) {
        qos.user_network_delay(1, delay);
    }
    assert_eq!(qos.fps(), FPS);
}

#[test]
fn custom_limit_of_one_viewer_does_not_lower_another_viewers_target() {
    let mut qos = stable_qos();
    qos.users.insert(2, UserData::default());
    for _ in 0..30 {
        qos.user_network_delay(2, 10);
        qos.user_network_delay(1, 10);
    }
    qos.user_custom_fps(2, 12);
    qos.user_network_delay(1, 10);
    assert_eq!(qos.fps(), 12, "the stream follows the lowest limit");
    assert_eq!(
        qos.users[&1].delay.fps,
        Some(FPS),
        "viewer 1's own target is not a function of viewer 2's limit"
    );
    qos.on_connection_close(2);
    assert_eq!(
        qos.fps(),
        FPS,
        "the stream is back the moment the limit is gone"
    );
}

#[test]
fn new_viewers_first_reply_does_not_bypass_bitrate_cooldown() {
    let mut qos = abr_session();
    for _ in 0..3 {
        qos.user_network_delay(1, 800);
    }
    // Viewer 1 is confirmed and its evidence was spent on a cut a moment ago.
    let ratio = qos.ratio();
    assert!(ratio < Quality::Balanced.ratio());
    qos.users.insert(2, UserData::default());
    qos.user_network_delay(2, 10);
    assert_eq!(
        qos.ratio(),
        ratio,
        "viewer 2's first reply must not spend viewer 1's evidence again inside the cooldown"
    );
}

#[test]
fn new_viewer_does_not_inherit_another_viewers_congested_fps() {
    let mut qos = stable_qos();
    for _ in 0..2 {
        qos.user_network_delay(1, 1200);
    }
    assert_eq!(qos.fps(), 8);
    qos.users.insert(2, UserData::default());
    qos.user_network_delay(2, 10);
    assert!(
        qos.users[&2].delay.fps >= Some(INIT_FPS),
        "a new viewer starts from INIT_FPS, not from the congested stream: {:?}",
        qos.users[&2].delay.fps
    );
}

#[test]
fn unconfirmed_severe_viewer_does_not_amplify_another_viewers_confirmed_mild_congestion() {
    let mut qos = abr_session();
    qos.users.insert(2, UserData::default());
    for _ in 0..30 {
        qos.user_network_delay(2, 10);
        qos.user_network_delay(1, 10);
    }
    // The first reply of a new viewer adjusts the ratio and restarts the cooldown.
    qos.advance_ms(4000);
    let ratio = qos.ratio();
    // Viewer 2: mild congestion, confirmed over three replies.  Viewer 1: one
    // severe spike, never confirmed.  Each viewer on its own calls for at most a
    // five percent step; together they must not turn into a halving.
    qos.user_network_delay(2, 200);
    qos.user_network_delay(1, 1200);
    qos.user_network_delay(2, 200);
    let after_two = qos.ratio();
    assert!(after_two < ratio, "viewer 2's second bad reply cuts");
    assert!(
        after_two >= ratio * 0.94,
        "viewer 2's own mild excess is a five percent step, not {after_two}"
    );
    qos.user_network_delay(2, 200);
    qos.advance_ms(4000);
    qos.update_display_data("test", 30);
    assert!(
        qos.ratio() >= after_two * 0.94,
        "viewer 1's severity must not be paired with viewer 2's confirmation: {}",
        qos.ratio()
    );
}

/// What `on_connection_open` inserts, without touching the config store.
fn newcomer(qos: &VideoQoS) -> UserData {
    UserData {
        joined_at: Some(qos.now()),
        ..Default::default()
    }
}

#[test]
fn closing_a_just_opened_viewer_does_not_throttle_existing_viewers() {
    let mut qos = stable_qos();
    assert_eq!(qos.fps(), FPS);
    qos.users.insert(2, newcomer(&qos));
    assert_eq!(
        qos.fps(),
        FPS,
        "nothing changes until the stream is re-aggregated"
    );
    qos.on_connection_close(2);
    assert_eq!(
        qos.fps(),
        FPS,
        "the guard leaves with the viewer that brought it"
    );
}

#[test]
fn a_new_viewer_caps_the_stream_at_init_fps_for_a_second() {
    let mut qos = stable_qos();
    qos.users.insert(2, newcomer(&qos));
    qos.user_network_delay(1, 10);
    assert_eq!(qos.fps(), INIT_FPS);
    qos.advance_ms(1000);
    qos.user_network_delay(2, 10);
    qos.user_network_delay(1, 10);
    assert!(
        qos.fps() > INIT_FPS,
        "after a second the stream follows the viewers' own targets: {}",
        qos.fps()
    );
}

#[test]
fn closing_the_latest_newcomer_keeps_an_earlier_newcomers_guard() {
    let mut qos = stable_qos();
    qos.users.insert(2, newcomer(&qos));
    qos.user_network_delay(2, 10);
    assert_eq!(qos.fps(), INIT_FPS);
    qos.advance_ms(100);
    qos.users.insert(3, newcomer(&qos));
    qos.advance_ms(100);
    qos.on_connection_close(3);
    assert_eq!(
        qos.fps(),
        INIT_FPS,
        "viewer 2 is still inside its own start-up window"
    );
}
