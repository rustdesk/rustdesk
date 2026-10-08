use super::*;
use crate::client::{
    audio_playback::AudioPlaybackStatus, audio_playback_recovery::STARTUP_CONFIRMATION_TIMEOUT,
};
use cpal::StreamError;
use crossbeam_queue::SegQueue;
use hbb_common::tokio::time::Instant;
use std::time::Duration;

const AFTER_COOLDOWN: Duration = Duration::from_secs(2);

type PendingOutput = (
    Arc<AtomicBool>,
    Arc<AudioPlaybackStatus>,
    Arc<SegQueue<StreamError>>,
);

fn install_output(handler: &mut AudioHandler, dropped: Arc<AtomicBool>) {
    handler.sample_rate = (INPUT_RATE, OUTPUT_RATE);
    handler.device_channel = CHANNELS;
    handler.audio_stream = Some(Box::new(TrackedAudioStream(dropped)));
    handler.playback_status.ready.store(true, Ordering::Release);
}

fn recovery_handler() -> (AudioHandler, Arc<AtomicBool>) {
    let dropped = Arc::new(AtomicBool::new(false));
    let mut handler = AudioHandler::default();
    handler.handle_format_with_start(format(INPUT_RATE, CHANNELS), |candidate, _| {
        install_output(candidate, dropped.clone());
        Ok(())
    });
    (handler, dropped)
}

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
fn unconfirmed_start_retries_without_callback_or_error() {
    let dropped = Arc::new(AtomicBool::new(false));
    let mut handler = AudioHandler::default();
    handler.handle_format_with_start(format(INPUT_RATE, CHANNELS), |candidate, _| {
        install_output(candidate, dropped.clone());
        candidate
            .playback_status
            .ready
            .store(false, Ordering::Release);
        Ok(())
    });
    handler.recover_playback_with(Instant::now(), |_, _| {
        panic!("Startup confirmation deadline has not elapsed")
    });
    assert!(!dropped.load(Ordering::SeqCst));
    let expired = Instant::now() + STARTUP_CONFIRMATION_TIMEOUT;
    let mut attempts = 0;
    handler.recover_playback_with(expired, |candidate, requested| {
        attempts += 1;
        assert!(dropped.load(Ordering::SeqCst));
        assert_eq!(requested, format(INPUT_RATE, CHANNELS));
        install_output(candidate, Arc::new(AtomicBool::new(false)));
        Ok(())
    });
    assert_eq!(attempts, 1);
    handler.recover_playback_with(expired + STARTUP_CONFIRMATION_TIMEOUT, |_, _| {
        panic!("Confirmed output must not be reopened")
    });
}

#[test]
fn already_terminal_candidate_cannot_replace_compatible_output() {
    let (mut handler, dropped) = recovery_handler();
    let candidate_dropped = Arc::new(AtomicBool::new(false));
    let old_buffer = handler.audio_buffer.0.clone();
    handler.handle_format_with_start(format(INPUT_RATE, CHANNELS), |candidate, _| {
        install_output(candidate, candidate_dropped.clone());
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
fn rollback_keeps_the_restarted_decoders_accumulated_history() {
    let (mut handler, old_dropped) = recovery_handler();
    let frame = audio_frame();
    handler.handle_frame(frame.clone());
    let (candidate_dropped, _, errors) = begin_pending(&mut handler);
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
    assert!(candidate_dropped.load(Ordering::SeqCst));
    assert_eq!(
        &handler.audio_decoder.as_ref().unwrap().1[..samples],
        &expected[..samples]
    );
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
