use super::{create_audio_resampler, AudioDecoder, AudioFrame, AudioHandler, Stereo};
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
