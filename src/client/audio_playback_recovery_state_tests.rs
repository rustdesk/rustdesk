use super::{
    super::{
        audio_playback::{AudioPlaybackConfig, AudioPlaybackWriter},
        audio_playback_recovery_tests::{backend_error, format, report_error},
        create_audio_resampler,
    },
    AudioDecoder, AudioFormat, AudioHandler, Instant, Ordering, Stereo,
};
use hbb_common::anyhow::anyhow;
use ringbuf::ring_buffer::RbBase;
use std::sync::Arc;

const INPUT_RATE: u32 = 48_000;
const OUTPUT_RATE: u32 = 44_100;
const CHANNELS: u16 = 2;
const BLOCK_FRAMES: usize = 480;
const BLOCK_SAMPLES: usize = BLOCK_FRAMES * CHANNELS as usize;
const SAMPLE_VALUE: f32 = 0.25;
const OLD_GENERATION: usize = 7;

fn configured_handler() -> AudioHandler {
    let mut handler = AudioHandler::default();
    handler.prepare_playback(&format());
    handler.audio_decoder = Some((
        AudioDecoder::new(INPUT_RATE, Stereo).unwrap(),
        vec![0.0; INPUT_RATE as usize * CHANNELS as usize],
    ));
    handler.audio_resampler = create_audio_resampler(INPUT_RATE, OUTPUT_RATE, CHANNELS).unwrap();
    handler.sample_rate = (INPUT_RATE, OUTPUT_RATE);
    handler.channels = CHANNELS;
    handler.device_channel = CHANNELS;
    handler.playback_status.ready.store(true, Ordering::Release);
    handler
        .audio_buffer
        .append_pcm2(&[SAMPLE_VALUE; BLOCK_SAMPLES]);
    handler
}

fn assert_cleared(handler: &AudioHandler) {
    assert!(handler.audio_stream.is_none());
    assert!(handler.audio_resampler.is_none());
    assert!(handler.audio_decoder.is_none());
    assert_eq!(handler.sample_rate, (0, 0));
    assert_eq!(handler.channels, 0);
    assert_eq!(handler.device_channel, 0);
    assert!(!handler.playback_status.ready.load(Ordering::Acquire));
    assert_eq!(handler.audio_buffer.0.lock().unwrap().occupied_len(), 0);
}

#[test]
fn failed_start_clears_decoded_state_but_keeps_the_retry_format() {
    let mut handler = configured_handler();
    handler.finish_playback_start(Err(anyhow!("Injected startup failure")));
    assert_cleared(&handler);

    let due = handler.playback_recovery.retry_at.unwrap();
    let cooldown = handler.playback_recovery.restart_not_before.unwrap();
    handler.clear_playback_stream();
    assert_cleared(&handler);
    assert_eq!(handler.playback_recovery.format, Some(format()));
    assert_eq!(handler.playback_recovery.retry_at, Some(due));
    assert_eq!(handler.playback_recovery.restart_not_before, Some(cooldown));
    handler.recover_playback_with(due, |_, requested| {
        assert_eq!(requested, format());
        Ok(())
    });
    assert!(handler.playback_recovery.retry_at.is_none());
}

#[test]
fn format_change_discards_decoded_state_before_attempting_a_new_stream() {
    const NEW_RATE: u32 = 24_000;
    const NEW_CHANNELS: u32 = 1;
    let mut handler = configured_handler();
    let changed = AudioFormat {
        sample_rate: NEW_RATE,
        channels: NEW_CHANNELS,
        ..Default::default()
    };
    handler.prepare_playback(&changed);
    assert_cleared(&handler);
    handler.finish_playback_start(Err(anyhow!("Changed device is unavailable")));

    let due = handler.playback_recovery.retry_at.unwrap();
    handler.recover_playback_with(due, |_, requested| {
        assert_eq!(requested, changed);
        Ok(())
    });
}

#[test]
fn failed_rebuild_discards_partial_output_configuration() {
    let mut handler = configured_handler();
    handler.finish_playback_start(Err(anyhow!("Injected initial failure")));
    let due = handler.playback_recovery.retry_at.unwrap();
    handler.recover_playback_with(due, |state, _| {
        state.audio_resampler = create_audio_resampler(INPUT_RATE, OUTPUT_RATE, CHANNELS)?;
        state.sample_rate = (INPUT_RATE, OUTPUT_RATE);
        state.device_channel = CHANNELS;
        Err(anyhow!("Injected failure after querying the output device"))
    });
    assert_cleared(&handler);
    assert!(handler.playback_recovery.retry_at.is_some());
}

#[test]
fn priority_warning_preserves_resampler_history() {
    let mut handler = configured_handler();
    let mut reference = create_audio_resampler(INPUT_RATE, OUTPUT_RATE, CHANNELS)
        .unwrap()
        .unwrap();
    let first = [SAMPLE_VALUE; BLOCK_SAMPLES];
    let second = [0.0; BLOCK_SAMPLES];
    let resampler = handler.audio_resampler.as_mut().unwrap();
    assert_eq!(
        resampler.process(&first).unwrap(),
        reference.process(&first).unwrap()
    );
    report_error(
        &handler,
        backend_error("SetThreadPriority failed: access denied"),
    );
    handler.recover_playback_with(Instant::now(), |_, _| panic!("Warning reopened playback"));
    assert_eq!(
        handler
            .audio_resampler
            .as_mut()
            .unwrap()
            .process(&second)
            .unwrap(),
        reference.process(&second).unwrap()
    );
    assert!(handler.audio_decoder.is_some());
    assert_eq!(handler.channels, CHANNELS);
}

fn writer(handler: &AudioHandler) -> AudioPlaybackWriter {
    AudioPlaybackWriter::new(
        AudioPlaybackConfig {
            sample_rate: INPUT_RATE,
            channels: CHANNELS as usize,
        },
        handler.audio_buffer.0.clone(),
        handler.audio_buffer.3.clone(),
    )
    .unwrap()
}

#[test]
fn replacement_writer_has_no_old_pcm_or_transition_state() {
    let mut handler = configured_handler();
    let mut old_writer = writer(&handler);
    handler.playback_status = old_writer.status.clone();
    let mut output = [0.0_f32; BLOCK_SAMPLES];
    old_writer.write_output(&mut output);
    assert!(output.iter().any(|sample| *sample > 0.0));
    handler
        .audio_buffer
        .3
        .store(OLD_GENERATION, Ordering::Relaxed);

    handler.prepare_playback(&format());
    old_writer.write_output(&mut output);
    assert!(!handler.playback_status.ready.load(Ordering::Acquire));
    let mut replacement = writer(&handler);
    assert!(!Arc::ptr_eq(&old_writer.status, &replacement.status));
    assert_eq!(handler.audio_buffer.3.load(Ordering::Relaxed), 0);
    replacement.write_output(&mut output);
    assert!(output.iter().all(|sample| *sample == 0.0));
    assert!(replacement.status.ready.load(Ordering::Acquire));
}
