use super::{super::super::audio_playback::AudioPlaybackStatus, *};
use crossbeam_queue::SegQueue;

#[path = "audio_native_startup_tests.rs"]
mod native_tests;

type PendingOutput = (
    Arc<AtomicBool>,
    Arc<AudioPlaybackStatus>,
    Arc<SegQueue<StreamError>>,
);

fn begin_pending(handler: &mut AudioHandler) -> PendingOutput {
    let dropped = Arc::new(AtomicBool::new(false));
    let mut state = None;
    handler.handle_format_with_start(format(INPUT_RATE, CHANNELS), |candidate, _| {
        install_output(candidate, dropped.clone());
        candidate
            .playback_status
            .ready
            .store(false, Ordering::Release);
        state = Some((
            candidate.playback_status.clone(),
            candidate.playback_recovery.errors.clone(),
        ));
        Ok(())
    });
    let (status, errors) = state.unwrap();
    (dropped, status, errors)
}

#[test]
fn already_terminal_candidate_cannot_replace_compatible_output() {
    for ready in [false, true] {
        let (mut handler, dropped) = recovery_handler();
        let candidate_dropped = Arc::new(AtomicBool::new(false));
        let old_buffer = handler.audio_buffer.0.clone();
        handler.handle_format_with_start(format(INPUT_RATE, CHANNELS), |candidate, _| {
            install_output(candidate, candidate_dropped.clone());
            candidate
                .playback_status
                .ready
                .store(ready, Ordering::Release);
            candidate
                .playback_recovery
                .errors
                .push(StreamError::DeviceNotAvailable);
            Ok(())
        });
        assert!(!dropped.load(Ordering::SeqCst));
        assert!(candidate_dropped.load(Ordering::SeqCst));
        assert!(Arc::ptr_eq(&old_buffer, &handler.audio_buffer.0));
        handler.recover_playback_with(Instant::now() + AFTER_COOLDOWN, |_, _| {
            panic!("Compatible active output must not be reopened")
        });
    }
}

#[test]
fn asynchronous_candidate_failure_preserves_the_active_output() {
    let (mut handler, old_dropped) = recovery_handler();
    let old_status = handler.playback_status.clone();
    let (candidate_dropped, _, errors) = begin_pending(&mut handler);
    assert!(!old_dropped.load(Ordering::SeqCst));
    errors.push(StreamError::DeviceNotAvailable);
    handler.recover_playback_with(Instant::now() + AFTER_COOLDOWN, |_, _| {
        panic!("Candidate failure must not reopen the healthy active output")
    });
    assert!(!old_dropped.load(Ordering::SeqCst));
    assert!(candidate_dropped.load(Ordering::SeqCst));
    assert!(Arc::ptr_eq(&old_status, &handler.playback_status));
}

#[test]
fn pending_output_keeps_playing_and_transfers_decoder_history_on_commit() {
    let (mut handler, old_dropped) = recovery_handler();
    let old_buffer = handler.audio_buffer.0.clone();
    let (_, status, _) = begin_pending(&mut handler);
    let frame = audio_frame();
    let (mut reference, mut expected) = decoder(INPUT_RATE);
    reference
        .decode_float(&frame.data, &mut expected, false)
        .unwrap();
    handler.handle_frame(frame.clone());
    assert!(Arc::ptr_eq(&old_buffer, &handler.audio_buffer.0));
    assert!(!drain_audio(&handler).is_empty());
    status.ready.store(true, Ordering::Release);
    handler.recover_playback_with(Instant::now(), |_, _| panic!("Candidate already exists"));
    assert!(old_dropped.load(Ordering::SeqCst));
    assert!(Arc::ptr_eq(&status, &handler.playback_status));
    let samples = reference
        .decode_float(&frame.data, &mut expected, false)
        .unwrap()
        * CHANNELS as usize;
    handler.handle_frame(frame);
    assert_eq!(
        &handler.audio_decoder.as_ref().unwrap().1[..samples],
        &expected[..samples]
    );
}

#[test]
fn failed_active_output_keeps_the_pending_candidate() {
    let (mut handler, old_dropped) = recovery_handler();
    let old_errors = handler.playback_recovery.errors.clone();
    let (candidate_dropped, status, _) = begin_pending(&mut handler);
    old_errors.push(StreamError::DeviceNotAvailable);
    handler.recover_playback_with(Instant::now() + AFTER_COOLDOWN, |_, _| {
        panic!("A pending healthy candidate must not be discarded")
    });
    assert!(old_dropped.load(Ordering::SeqCst));
    assert!(!candidate_dropped.load(Ordering::SeqCst));
    assert!(Arc::ptr_eq(&status, &handler.playback_status));
    assert!(!handler.playback_status.ready.load(Ordering::Acquire));
}

#[test]
fn both_outputs_failing_retains_format_and_paces_recovery() {
    let (mut handler, old_dropped) = recovery_handler();
    let old_errors = handler.playback_recovery.errors.clone();
    let (candidate_dropped, _, errors) = begin_pending(&mut handler);
    old_errors.push(StreamError::DeviceNotAvailable);
    errors.push(StreamError::DeviceNotAvailable);
    handler.recover_playback_with(Instant::now(), |_, _| panic!("Retry must be paced"));
    assert!(old_dropped.load(Ordering::SeqCst));
    assert!(candidate_dropped.load(Ordering::SeqCst));
    assert!(handler.audio_stream.is_none());
    let due = handler.playback_recovery.retry_at.unwrap();
    let mut attempts = 0;
    handler.recover_playback_with(due, |_, requested| {
        attempts += 1;
        assert_eq!(requested, format(INPUT_RATE, CHANNELS));
        Ok(())
    });
    assert_eq!(attempts, 1);
}

#[test]
fn later_compatible_format_retires_only_the_pending_attempt() {
    let (mut handler, old_dropped) = recovery_handler();
    let (first_dropped, _, first_errors) = begin_pending(&mut handler);
    let (second_dropped, second_status, _) = begin_pending(&mut handler);
    assert!(first_dropped.load(Ordering::SeqCst));
    assert!(!old_dropped.load(Ordering::SeqCst));
    first_errors.push(StreamError::DeviceNotAvailable);
    second_status.ready.store(true, Ordering::Release);
    handler.recover_playback_with(Instant::now(), |_, _| {
        panic!("Retired attempt affected current output")
    });
    assert!(old_dropped.load(Ordering::SeqCst));
    assert!(!second_dropped.load(Ordering::SeqCst));
    assert!(Arc::ptr_eq(&second_status, &handler.playback_status));
}

#[test]
fn incompatible_format_discards_both_old_outputs_and_retries_new_format() {
    let (mut handler, old_dropped) = recovery_handler();
    let (candidate_dropped, _, _) = begin_pending(&mut handler);
    let latest = format(OUTPUT_RATE, MONO_CHANNELS);
    handler.handle_format_with_start(latest.clone(), failed_output);
    assert!(old_dropped.load(Ordering::SeqCst));
    assert!(candidate_dropped.load(Ordering::SeqCst));
    assert!(handler.audio_stream.is_none());
    let due = handler.playback_recovery.retry_at.unwrap();
    handler.recover_playback_with(due, |_, requested| {
        assert_eq!(requested, latest);
        Ok(())
    });
}

#[test]
fn owner_shutdown_drops_active_and_pending_streams() {
    let (mut handler, old_dropped) = recovery_handler();
    let (candidate_dropped, _, _) = begin_pending(&mut handler);
    assert!(!old_dropped.load(Ordering::SeqCst));
    assert!(!candidate_dropped.load(Ordering::SeqCst));
    drop(handler);
    assert!(old_dropped.load(Ordering::SeqCst));
    assert!(candidate_dropped.load(Ordering::SeqCst));
}

#[test]
fn candidate_warning_does_not_confirm_or_fail_startup() {
    let (mut handler, old_dropped) = recovery_handler();
    let (candidate_dropped, status, errors) = begin_pending(&mut handler);
    errors.push(backend_error("SetThreadPriority failed: access denied"));
    handler.recover_playback_with(Instant::now(), |_, _| panic!("Warning triggered recovery"));
    assert!(!old_dropped.load(Ordering::SeqCst));
    assert!(!candidate_dropped.load(Ordering::SeqCst));
    status.ready.store(true, Ordering::Release);
    handler.recover_playback_with(Instant::now(), |_, _| panic!("Candidate already started"));
    assert!(old_dropped.load(Ordering::SeqCst));
    assert!(!candidate_dropped.load(Ordering::SeqCst));
}

#[test]
fn rejected_format_does_not_cancel_pending_startup() {
    let (mut handler, old_dropped) = recovery_handler();
    let (candidate_dropped, status, _) = begin_pending(&mut handler);
    handler.handle_format_with_start(format(INPUT_RATE, 0), |_, _| {
        panic!("Invalid format reached startup")
    });
    status.ready.store(true, Ordering::Release);
    handler.recover_playback_with(Instant::now(), |_, _| panic!("Pending output was lost"));
    assert!(old_dropped.load(Ordering::SeqCst));
    assert!(!candidate_dropped.load(Ordering::SeqCst));
}

#[test]
fn rollback_keeps_the_restarted_decoders_accumulated_history() {
    let (mut handler, old_dropped) = recovery_handler();
    let frame = audio_frame();
    handler.handle_frame(frame.clone());
    let (_, _, errors) = begin_pending(&mut handler);
    let (mut reference, mut expected) = decoder(INPUT_RATE);
    handler.handle_frame(frame.clone());
    reference
        .decode_float(&frame.data, &mut expected, false)
        .unwrap();
    errors.push(StreamError::DeviceNotAvailable);
    handler.recover_playback_with(Instant::now(), |_, _| panic!("Old output still works"));
    let samples = reference
        .decode_float(&frame.data, &mut expected, false)
        .unwrap()
        * CHANNELS as usize;
    handler.handle_frame(frame);
    assert!(!old_dropped.load(Ordering::SeqCst));
    assert_eq!(
        &handler.audio_decoder.as_ref().unwrap().1[..samples],
        &expected[..samples]
    );
}

#[test]
fn superseding_format_keeps_ready_candidate_when_active_output_failed() {
    let (mut handler, old_dropped) = recovery_handler();
    let old_errors = handler.playback_recovery.errors.clone();
    let (candidate_dropped, candidate_status, _) = begin_pending(&mut handler);
    candidate_status.ready.store(true, Ordering::Release);
    old_errors.push(StreamError::DeviceNotAvailable);
    handler.handle_format_with_start(format(INPUT_RATE, CHANNELS), failed_output);
    assert!(!candidate_dropped.load(Ordering::SeqCst));
    assert!(old_dropped.load(Ordering::SeqCst));
    assert!(Arc::ptr_eq(&candidate_status, &handler.playback_status));
    assert!(handler.audio_decoder.is_some());
    assert!(handler.playback_recovery.retry_at.is_none());
}

#[test]
fn superseding_format_preserves_failure_when_both_outputs_failed() {
    let (mut handler, old_dropped) = recovery_handler();
    let old_errors = handler.playback_recovery.errors.clone();
    let (candidate_dropped, status, errors) = begin_pending(&mut handler);
    status.ready.store(true, Ordering::Release);
    old_errors.push(StreamError::DeviceNotAvailable);
    errors.push(StreamError::DeviceNotAvailable);
    handler.handle_format_with_start(format(INPUT_RATE, CHANNELS), failed_output);
    assert!(candidate_dropped.load(Ordering::SeqCst));
    assert!(old_dropped.load(Ordering::SeqCst));
    assert!(handler.audio_stream.is_none());
    assert!(handler.playback_recovery.retry_at.is_some());
}
