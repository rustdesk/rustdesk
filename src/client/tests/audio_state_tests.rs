use super::{create_audio_resampler, AudioDecoder, AudioFormat, AudioFrame, AudioHandler, Stereo};
use cpal::traits::StreamTrait;
use hbb_common::{anyhow::anyhow, ResultType};
use magnum_opus::{Application::LowDelay, Encoder};
use ringbuf::Rb;
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

#[cfg(target_os = "windows")]
#[path = "audio_playback_recovery_tests.rs"]
mod recovery_tests;

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

fn failed_output(candidate: &mut AudioHandler, _: AudioFormat) -> ResultType<()> {
    candidate.sample_rate = (INPUT_RATE, INPUT_RATE);
    candidate.device_channel = MONO_CHANNELS;
    candidate
        .audio_buffer
        .resize(INPUT_RATE as _, MONO_CHANNELS as _);
    candidate
        .playback_status
        .ready
        .store(false, Ordering::Release);
    Err(anyhow!("Injected playback failure"))
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
    let (mut handler, dropped) = active_handler(INPUT_RATE);
    let (mut reference, _) = active_handler(INPUT_RATE);
    let (mut retained_decoder, _) = active_handler(INPUT_RATE);
    retained_decoder.handle_frame(audio_frame());
    retained_decoder.handle_frame(audio_frame());
    handler.handle_frame(audio_frame());
    reference.handle_frame(audio_frame());
    let buffer = handler.audio_buffer.0.clone();
    let status = handler.playback_status.clone();

    handler.handle_format_with_start(format(INPUT_RATE, CHANNELS), failed_output);
    reference.audio_decoder = Some(decoder(INPUT_RATE));
    handler.handle_frame(audio_frame());
    reference.handle_frame(audio_frame());

    assert!(!dropped.load(Ordering::SeqCst));
    assert!(Arc::ptr_eq(&buffer, &handler.audio_buffer.0));
    assert!(Arc::ptr_eq(&status, &handler.playback_status));
    assert_eq!(handler.sample_rate, (INPUT_RATE, OUTPUT_RATE));
    assert_eq!(handler.device_channel, CHANNELS);
    assert!(handler.playback_status.ready.load(Ordering::Acquire));
    let expected = drain_audio(&reference);
    let actual = drain_audio(&handler);
    assert!(!actual.is_empty());
    assert_ne!(drain_audio(&retained_decoder), expected);
    assert_eq!(actual, expected);
}

fn drain_audio(handler: &AudioHandler) -> Vec<f32> {
    handler.audio_buffer.0.lock().unwrap().pop_iter().collect()
}
