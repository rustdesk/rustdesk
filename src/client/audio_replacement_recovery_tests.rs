use super::{super::audio_playback_recovery_tests::backend_error, *};
use cpal::StreamError;
use hbb_common::{tokio::time::Instant, ResultType};
use std::time::Duration;

const AFTER_COOLDOWN: Duration = Duration::from_secs(2);
const BEFORE_DEADLINE: Duration = Duration::from_millis(1);

#[path = "audio_pending_replacement_tests.rs"]
mod pending_tests;

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

#[test]
fn compatible_failure_keeps_recovery_and_discards_candidate_errors() {
    let (mut handler, dropped) = recovery_handler();
    let retained_errors = handler.playback_recovery.errors.clone();
    let retained_status = handler.playback_status.clone();
    retained_errors.push(backend_error("SetThreadPriority failed: access denied"));
    let mut failed_callback = None;
    handler.handle_format_with_start(format(INPUT_RATE, CHANNELS), |candidate, _| {
        let mut error = candidate.playback_recovery.new_error_callback();
        error(StreamError::DeviceNotAvailable);
        failed_callback = Some(error);
        fail_start(candidate, true)
    });
    failed_callback.as_mut().unwrap()(StreamError::DeviceNotAvailable);
    assert!(!dropped.load(Ordering::SeqCst));
    assert!(Arc::ptr_eq(
        &retained_errors,
        &handler.playback_recovery.errors
    ));
    assert!(Arc::ptr_eq(&retained_status, &handler.playback_status));
    assert!(handler.playback_recovery.retry_at.is_none());
    let now = Instant::now() + AFTER_COOLDOWN;
    handler.recover_playback_with(now, |_, _| panic!("Healthy retained output was reopened"));

    retained_errors.push(StreamError::DeviceNotAvailable);
    let mut attempts = 0;
    handler.recover_playback_with(now, |_, requested| {
        attempts += 1;
        assert!(dropped.load(Ordering::SeqCst));
        assert_eq!(requested, format(INPUT_RATE, CHANNELS));
        Ok(())
    });
    assert_eq!(attempts, 1);
}

#[test]
fn failed_replacement_does_not_retain_an_output_with_a_terminal_error() {
    for during_start in [false, true] {
        let (mut handler, dropped) = recovery_handler();
        let retained_errors = handler.playback_recovery.errors.clone();
        if !during_start {
            retained_errors.push(StreamError::DeviceNotAvailable);
        }
        handler.handle_format_with_start(format(INPUT_RATE, CHANNELS), |candidate, _| {
            if during_start {
                std::thread::spawn(move || retained_errors.push(StreamError::DeviceNotAvailable))
                    .join()
                    .unwrap();
            }
            fail_start(candidate, true)
        });
        assert!(
            dropped.load(Ordering::SeqCst),
            "during_start: {during_start}"
        );
        assert!(handler.audio_stream.is_none());
        let due = handler.playback_recovery.retry_at.unwrap();
        handler.recover_playback_with(due - BEFORE_DEADLINE, |_, _| panic!("Retry was not paced"));
        let mut attempts = 0;
        handler.recover_playback_with(due, |_, requested| {
            attempts += 1;
            assert_eq!(requested, format(INPUT_RATE, CHANNELS));
            Ok(())
        });
        assert_eq!(attempts, 1);
    }
}

#[test]
fn successful_replacement_keeps_only_the_new_outputs_errors() {
    let (mut handler, dropped) = recovery_handler();
    let old_errors = handler.playback_recovery.errors.clone();
    let replacement_dropped = Arc::new(AtomicBool::new(false));
    let mut replacement_callback = None;
    handler.handle_format_with_start(format(INPUT_RATE, CHANNELS), |candidate, _| {
        assert!(!dropped.load(Ordering::SeqCst));
        install_output(candidate, replacement_dropped.clone());
        replacement_callback = Some(candidate.playback_recovery.new_error_callback());
        Ok(())
    });
    assert!(dropped.load(Ordering::SeqCst));
    old_errors.push(StreamError::DeviceNotAvailable);
    let now = Instant::now() + AFTER_COOLDOWN;
    handler.recover_playback_with(now, |_, _| {
        panic!("Retired output invalidated the replacement")
    });
    assert!(!replacement_dropped.load(Ordering::SeqCst));

    replacement_callback.as_mut().unwrap()(StreamError::DeviceNotAvailable);
    let mut attempts = 0;
    handler.recover_playback_with(now, |_, requested| {
        attempts += 1;
        assert!(replacement_dropped.load(Ordering::SeqCst));
        assert_eq!(requested, format(INPUT_RATE, CHANNELS));
        Ok(())
    });
    assert_eq!(attempts, 1);
}

fn failed_output(handler: &mut AudioHandler, _: AudioFormat) -> ResultType<()> {
    fail_start(handler, true)
}

#[test]
fn initial_and_incompatible_failures_retry_the_latest_announced_format() {
    let (mut active, dropped) = recovery_handler();
    active.handle_format_with_start(format(OUTPUT_RATE, CHANNELS), failed_output);
    assert!(dropped.load(Ordering::SeqCst));
    assert!(active.audio_stream.is_none());
    assert!(active.playback_recovery.retry_at.is_some());
    for mut handler in [AudioHandler::default(), active] {
        let latest = format(INPUT_RATE, MONO_CHANNELS);
        handler.handle_format_with_start(latest.clone(), failed_output);
        assert!(handler.audio_stream.is_none());
        assert!(handler.audio_decoder.is_none());
        let due = handler.playback_recovery.retry_at.unwrap();
        let mut attempts = 0;
        handler.recover_playback_with(due, |_, requested| {
            attempts += 1;
            assert_eq!(requested, latest);
            Ok(())
        });
        assert_eq!(attempts, 1);
    }
}
