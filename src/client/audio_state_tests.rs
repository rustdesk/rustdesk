use super::{create_audio_resampler, AudioDecoder, AudioFormat, AudioFrame, AudioHandler, Stereo};
use cpal::traits::StreamTrait;
use hbb_common::anyhow::anyhow;
use magnum_opus::{Application::LowDelay, Encoder};
use ringbuf::{ring_buffer::RbBase, Rb};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

const INPUT_RATE: u32 = 24_000;
const OUTPUT_RATE: u32 = 48_000;
const CHANNELS: u16 = 2;
const PACKETS_PER_SECOND: usize = 100;
const MAX_PACKET_BYTES: usize = 4_096;
const SAMPLE_VALUE: f32 = 0.25;
const MONO_CHANNELS: u16 = 1;

struct TrackedAudioStream(Arc<AtomicBool>);

impl StreamTrait for TrackedAudioStream {
    fn play(&self) -> Result<(), cpal::PlayStreamError> {
        Ok(())
    }

    fn pause(&self) -> Result<(), cpal::PauseStreamError> {
        Ok(())
    }
}

impl Drop for TrackedAudioStream {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

fn decoder(sample_rate: u32) -> (AudioDecoder, Vec<f32>) {
    (
        AudioDecoder::new(sample_rate, Stereo).unwrap(),
        vec![0.0; sample_rate as usize * CHANNELS as usize],
    )
}

fn active_handler(input_rate: u32) -> (AudioHandler, Arc<AtomicBool>) {
    let dropped = Arc::new(AtomicBool::new(false));
    let handler = AudioHandler {
        audio_decoder: Some(decoder(input_rate)),
        audio_resampler: create_audio_resampler(input_rate, OUTPUT_RATE, CHANNELS).unwrap(),
        sample_rate: (input_rate, OUTPUT_RATE),
        audio_stream: Some(Box::new(TrackedAudioStream(dropped.clone()))),
        channels: CHANNELS,
        device_channel: CHANNELS,
        ..Default::default()
    };
    handler.playback_status.ready.store(true, Ordering::Release);
    (handler, dropped)
}

fn audio_frame() -> AudioFrame {
    let samples = OUTPUT_RATE as usize / PACKETS_PER_SECOND * CHANNELS as usize;
    let mut encoder = Encoder::new(OUTPUT_RATE, Stereo, LowDelay).unwrap();
    AudioFrame {
        data: encoder
            .encode_vec_float(&vec![SAMPLE_VALUE; samples], MAX_PACKET_BYTES)
            .unwrap()
            .into(),
        ..Default::default()
    }
}

#[test]
fn failed_format_change_discards_old_playback_state() {
    let (mut handler, dropped) = active_handler(INPUT_RATE);
    handler
        .audio_buffer
        .0
        .lock()
        .unwrap()
        .push_slice(&[SAMPLE_VALUE; CHANNELS as usize]);
    handler.audio_decoder = Some(decoder(OUTPUT_RATE));
    handler.sample_rate = (OUTPUT_RATE, OUTPUT_RATE);

    handler.handle_audio_start_result(
        Err(anyhow!("Injected output stream startup failure")),
        false,
    );

    assert!(dropped.load(Ordering::SeqCst));
    assert!(handler.audio_stream.is_none());
    assert!(handler.audio_resampler.is_none());
    assert!(handler.audio_decoder.is_none());
    assert!(!handler.playback_status.ready.load(Ordering::Acquire));
    handler.handle_frame(audio_frame());
    assert_eq!(handler.audio_buffer.0.lock().unwrap().occupied_len(), 0);
}

#[test]
fn successful_start_or_compatible_failure_preserves_audio_packet_duration() {
    for result in [
        Ok(()),
        Err(anyhow!("Injected compatible stream replacement failure")),
    ] {
        let (mut handler, dropped) = active_handler(OUTPUT_RATE);

        handler.handle_audio_start_result(result, true);
        handler.handle_frame(audio_frame());

        assert!(!dropped.load(Ordering::SeqCst));
        assert_eq!(
            handler.audio_buffer.0.lock().unwrap().occupied_len(),
            OUTPUT_RATE as usize / PACKETS_PER_SECOND * CHANNELS as usize
        );
    }
}

#[derive(Clone, Copy, Debug)]
enum StartupFailure {
    DeviceLookup,
    ConfigLookup,
    StreamBuild,
    StreamPlay,
}

fn fail_start(handler: &mut AudioHandler, failure: StartupFailure) -> hbb_common::ResultType<()> {
    if matches!(
        failure,
        StartupFailure::StreamBuild | StartupFailure::StreamPlay
    ) {
        handler.sample_rate = (INPUT_RATE, INPUT_RATE);
        handler.device_channel = MONO_CHANNELS;
        handler
            .audio_buffer
            .resize(INPUT_RATE as _, MONO_CHANNELS as _);
        handler
            .playback_status
            .ready
            .store(false, Ordering::Release);
    }
    Err(anyhow!("Injected playback failure: {failure:?}"))
}

fn format(sample_rate: u32, channels: u16) -> AudioFormat {
    AudioFormat {
        sample_rate,
        channels: u32::from(channels),
        ..Default::default()
    }
}

#[test]
fn identical_format_failure_preserves_playback_and_resampler_history() {
    for failure in [
        StartupFailure::DeviceLookup,
        StartupFailure::ConfigLookup,
        StartupFailure::StreamBuild,
        StartupFailure::StreamPlay,
    ] {
        let (mut handler, dropped) = active_handler(INPUT_RATE);
        let (mut uninterrupted, _) = active_handler(INPUT_RATE);
        handler.handle_frame(audio_frame());
        uninterrupted.handle_frame(audio_frame());
        let buffer = handler.audio_buffer.0.clone();
        let status = handler.playback_status.clone();
        let generation = handler.audio_buffer.3.clone();

        handler.handle_format_with_start(format(INPUT_RATE, CHANNELS), |candidate, _| {
            fail_start(candidate, failure)
        });
        handler.handle_frame(audio_frame());
        uninterrupted.handle_frame(audio_frame());

        assert!(!dropped.load(Ordering::SeqCst), "{failure:?}");
        assert!(Arc::ptr_eq(&buffer, &handler.audio_buffer.0));
        assert!(Arc::ptr_eq(&status, &handler.playback_status));
        assert!(Arc::ptr_eq(&generation, &handler.audio_buffer.3));
        assert_eq!(handler.sample_rate, (INPUT_RATE, OUTPUT_RATE));
        assert_eq!(handler.device_channel, CHANNELS);
        assert!(handler.playback_status.ready.load(Ordering::Acquire));
        let expected: Vec<_> = uninterrupted
            .audio_buffer
            .0
            .lock()
            .unwrap()
            .pop_iter()
            .collect();
        let actual: Vec<_> = handler.audio_buffer.0.lock().unwrap().pop_iter().collect();
        assert!(!actual.is_empty());
        assert_eq!(actual, expected, "{failure:?}");
    }
}

#[test]
fn incompatible_format_failure_invalidates_playback() {
    for changed_format in [
        format(OUTPUT_RATE, CHANNELS),
        format(INPUT_RATE, MONO_CHANNELS),
    ] {
        let (mut handler, dropped) = active_handler(INPUT_RATE);
        handler.handle_format_with_start(changed_format, |candidate, _| {
            fail_start(candidate, StartupFailure::DeviceLookup)
        });
        assert!(dropped.load(Ordering::SeqCst));
        assert!(handler.audio_stream.is_none());
        assert!(handler.audio_resampler.is_none());
        assert!(handler.audio_decoder.is_none());
        handler.handle_frame(audio_frame());
        assert_eq!(handler.audio_buffer.0.lock().unwrap().occupied_len(), 0);
    }
}

#[test]
fn successful_format_replacement_commits_independent_playback_state() {
    let (mut handler, dropped) = active_handler(INPUT_RATE);
    let old_buffer = handler.audio_buffer.0.clone();
    let old_status = handler.playback_status.clone();
    let replacement_dropped = Arc::new(AtomicBool::new(false));

    handler.handle_format_with_start(format(OUTPUT_RATE, CHANNELS), |candidate, incoming| {
        assert!(candidate.audio_stream.is_none());
        assert!(!Arc::ptr_eq(&old_buffer, &candidate.audio_buffer.0));
        assert!(!Arc::ptr_eq(&old_status, &candidate.playback_status));
        candidate.sample_rate = (incoming.sample_rate, OUTPUT_RATE);
        candidate.device_channel = CHANNELS;
        candidate.audio_stream = Some(Box::new(TrackedAudioStream(replacement_dropped.clone())));
        candidate
            .playback_status
            .ready
            .store(true, Ordering::Release);
        Ok(())
    });
    handler.handle_frame(audio_frame());

    assert!(dropped.load(Ordering::SeqCst));
    assert!(!replacement_dropped.load(Ordering::SeqCst));
    assert!(handler.audio_resampler.is_none());
    assert_eq!(handler.sample_rate, (OUTPUT_RATE, OUTPUT_RATE));
    assert_eq!(
        handler.audio_buffer.0.lock().unwrap().occupied_len(),
        OUTPUT_RATE as usize / PACKETS_PER_SECOND * CHANNELS as usize
    );
}
