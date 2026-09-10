use super::{audio_playback_recovery::*, *};
use cpal::{BackendSpecificError, PauseStreamError, PlayStreamError, StreamError};
use std::sync::atomic::{AtomicBool, Ordering};

const SAMPLE_RATE: u32 = 48_000;
const OUTPUT_RATE: u32 = 44_100;
const CHANNELS: u32 = 2;

struct TestStream(Arc<AtomicBool>);

impl StreamTrait for TestStream {
    fn play(&self) -> Result<(), PlayStreamError> {
        Ok(())
    }

    fn pause(&self) -> Result<(), PauseStreamError> {
        Ok(())
    }
}

impl Drop for TestStream {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

fn format() -> AudioFormat {
    AudioFormat {
        sample_rate: SAMPLE_RATE,
        channels: CHANNELS,
        ..Default::default()
    }
}

fn active_handler() -> (AudioHandler, Arc<AtomicBool>) {
    let dropped = Arc::new(AtomicBool::new(false));
    let mut handler = AudioHandler::default();
    handler.prepare_playback(&format());
    handler.audio_stream = Some(Box::new(TestStream(dropped.clone())));
    handler.audio_resampler =
        create_audio_resampler(SAMPLE_RATE, OUTPUT_RATE, CHANNELS as _).unwrap();
    handler.playback_status.ready.store(true, Ordering::Release);
    handler.audio_buffer.append_pcm2(&[0.25, 0.25]);
    (handler, dropped)
}

fn backend_error(description: &str) -> StreamError {
    StreamError::BackendSpecific {
        err: BackendSpecificError {
            description: description.to_owned(),
        },
    }
}

#[test]
fn terminal_error_is_delivered_to_the_owner() {
    let errors = PlaybackErrors::default();
    errors.report(StreamError::DeviceNotAvailable);
    assert!(matches!(
        errors.pop(),
        Some(StreamError::DeviceNotAvailable)
    ));
    assert!(errors.pop().is_none());
}

#[test]
fn runtime_failure_drops_the_dead_stream_and_clears_stale_state() {
    let (mut handler, dropped) = active_handler();
    let old_status = handler.playback_status.clone();
    handler
        .playback_recovery
        .errors
        .report(StreamError::DeviceNotAvailable);
    let mut attempts = 0;
    handler.recover_playback_with(Instant::now(), |state, requested| {
        attempts += 1;
        assert!(dropped.load(Ordering::Relaxed));
        assert!(state.audio_stream.is_none());
        assert!(!state.playback_status.ready.load(Ordering::Acquire));
        assert!(!Arc::ptr_eq(&old_status, &state.playback_status));
        assert!(state.audio_resampler.is_none());
        assert_eq!(state.audio_buffer.0.lock().unwrap().occupied_len(), 0);
        assert_eq!(requested, format());
        Ok(())
    });
    assert_eq!(attempts, 1);
}

#[test]
fn backend_failure_requests_recovery_but_priority_warning_does_not() {
    let (mut handler, dropped) = active_handler();
    handler
        .playback_recovery
        .errors
        .report(backend_error("SetThreadPriority failed: access denied"));
    handler.recover_playback_with(Instant::now(), |_, _| {
        panic!("Nonfatal warning restarted playback")
    });
    assert!(!dropped.load(Ordering::Relaxed));
    assert!(handler.playback_status.ready.load(Ordering::Acquire));
    handler
        .playback_recovery
        .errors
        .report(backend_error("IAudioClient::GetCurrentPadding failed"));
    let mut restarted = false;
    handler.recover_playback_with(Instant::now(), |_, _| {
        restarted = true;
        Ok(())
    });
    assert!(restarted);
}

#[test]
fn warning_does_not_hide_a_subsequent_terminal_error() {
    let (mut handler, dropped) = active_handler();
    let errors = handler.playback_recovery.errors.clone();
    errors.report(backend_error("SetThreadPriority failed: access denied"));
    errors.report(StreamError::DeviceNotAvailable);
    handler.recover_playback_with(Instant::now(), |_, _| Ok(()));
    assert!(dropped.load(Ordering::Relaxed));
}

#[test]
fn failed_rebuild_retries_without_another_error_and_without_busy_looping() {
    let (mut handler, _) = active_handler();
    handler
        .playback_recovery
        .errors
        .report(StreamError::DeviceNotAvailable);
    handler.recover_playback_with(Instant::now(), |_, _| bail!("No default output device"));
    let retry_at = handler
        .playback_recovery
        .retry_at
        .expect("Retry was not scheduled");
    handler.recover_playback_with(retry_at - Duration::from_millis(1), |_, _| {
        panic!("Retried before the deadline")
    });
    let mut attempts = 0;
    handler.recover_playback_with(retry_at, |_, _| {
        attempts += 1;
        Ok(())
    });
    assert_eq!(attempts, 1);
    assert!(handler.playback_recovery.retry_at.is_none());
}

#[test]
fn prolonged_device_absence_remains_an_explicit_pending_failure() {
    let mut handler = AudioHandler::default();
    handler.prepare_playback(&format());
    handler.finish_playback_start(Err(anyhow!("No output device")));
    for _ in 0..5 {
        let due = handler.playback_recovery.retry_at.unwrap();
        handler.recover_playback_with(due, |_, _| bail!("Device still unavailable"));
        assert!(handler.audio_stream.is_none());
        assert!(handler.playback_recovery.retry_at.is_some());
        assert!(!handler.playback_status.ready.load(Ordering::Acquire));
    }
}

#[test]
fn late_error_and_readiness_from_old_stream_do_not_affect_replacement() {
    let (mut handler, _) = active_handler();
    let old_errors = handler.playback_recovery.errors.clone();
    let old_status = handler.playback_status.clone();
    handler.prepare_playback(&format());
    old_errors.report(StreamError::DeviceNotAvailable);
    old_status.ready.store(true, Ordering::Release);
    handler.recover_playback_with(Instant::now(), |_, _| {
        panic!("Stale callback requested recovery")
    });
    assert!(!handler.playback_status.ready.load(Ordering::Acquire));
}

#[test]
fn latest_remote_format_replaces_a_pending_retry() {
    let (mut handler, _) = active_handler();
    handler.finish_playback_start(Err(anyhow!("Device unavailable")));
    let changed = AudioFormat {
        sample_rate: 24_000,
        channels: 1,
        ..Default::default()
    };
    handler.prepare_playback(&changed);
    assert!(handler.playback_recovery.retry_at.is_none());
    handler.finish_playback_start(Err(anyhow!("Device still unavailable")));
    let due = handler.playback_recovery.retry_at.unwrap();
    handler.recover_playback_with(due, |_, requested| {
        assert_eq!(requested, changed);
        Ok(())
    });
}

#[test]
fn new_stream_error_is_not_cleared_by_successful_start_result() {
    let mut handler = AudioHandler::default();
    handler.prepare_playback(&format());
    handler
        .playback_recovery
        .errors
        .report(StreamError::DeviceNotAvailable);
    handler.finish_playback_start(Ok(()));
    let mut attempts = 0;
    handler.recover_playback_with(Instant::now() + Duration::from_secs(1), |_, _| {
        attempts += 1;
        Ok(())
    });
    assert_eq!(attempts, 1);
}

#[test]
fn no_format_does_not_open_an_output_device() {
    let mut handler = AudioHandler::default();
    handler
        .playback_recovery
        .errors
        .report(StreamError::DeviceNotAvailable);
    handler.recover_playback_with(Instant::now(), |_, _| panic!("Invented a remote format"));
}

#[test]
fn recovery_runs_on_owner_not_error_callback_thread() {
    let (mut handler, dropped) = active_handler();
    let errors = handler.playback_recovery.errors.clone();
    std::thread::spawn(move || errors.report(StreamError::DeviceNotAvailable))
        .join()
        .unwrap();
    assert!(!dropped.load(Ordering::Relaxed));
    let owner = std::thread::current().id();
    handler.recover_playback_with(Instant::now(), |_, _| {
        assert_eq!(std::thread::current().id(), owner);
        Ok(())
    });
    assert!(dropped.load(Ordering::Relaxed));
}

#[test]
fn silence_still_polls_recovery_and_disconnect_exits() {
    let (sender, receiver) = mpsc::channel();
    let mut sender = Some(sender);
    let mut polls = 0;
    let result = receive_with_recovery(&receiver, Duration::from_millis(1), || {
        polls += 1;
        if polls == 2 {
            drop(sender.take());
        }
    });
    assert!(result.is_err());
    assert_eq!(polls, 2);
}

#[test]
fn pending_media_is_not_delayed_by_recovery_poll_interval() {
    let (sender, receiver) = mpsc::channel();
    sender.send(MediaData::AudioFrame(Box::default())).unwrap();
    let mut polls = 0;
    let result = receive_with_recovery(&receiver, Duration::from_secs(60), || polls += 1);
    assert!(matches!(result, Ok(MediaData::AudioFrame(_))));
    assert_eq!(polls, 1);
}

#[test]
fn disconnected_receiver_does_not_recreate_a_stream() {
    let (sender, receiver) = mpsc::channel();
    drop(sender);
    let result = receive_with_recovery(&receiver, Duration::from_secs(60), || {
        panic!("Recovery ran after disconnection")
    });
    assert!(result.is_err());
}

#[test]
fn pending_format_is_handled_before_retrying_the_old_format() {
    let (sender, receiver) = mpsc::channel();
    sender.send(MediaData::AudioFormat(format())).unwrap();
    let result = receive_with_recovery(&receiver, Duration::from_secs(60), || {
        panic!("Recovery ran before the queued format was processed")
    });
    assert!(matches!(result, Ok(MediaData::AudioFormat(_))));
}

#[test]
fn successful_open_followed_by_immediate_runtime_failure_is_paced() {
    let (mut handler, _) = active_handler();
    handler.finish_playback_start(Ok(()));
    handler
        .playback_recovery
        .errors
        .report(StreamError::DeviceNotAvailable);
    let now = Instant::now();
    handler.recover_playback_with(now, |_, _| {
        panic!("Runtime failure caused a tight reopen loop")
    });
    assert!(handler.playback_recovery.retry_at.unwrap() > now);
}
