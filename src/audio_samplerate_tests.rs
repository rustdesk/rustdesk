use super::{AudioResampler, AudioResamplerConfig, AudioResamplerError, FixedFrameAudioResampler};

const INPUT_RATE: u32 = 44_100;
const OUTPUT_RATE: u32 = 48_000;
const CHANNELS: u16 = 2;
const INPUT_PACKET_FRAMES: usize = INPUT_RATE as usize / PACKETS_PER_SECOND;
const OUTPUT_PACKET_FRAMES: usize = OUTPUT_RATE as usize / PACKETS_PER_SECOND;
const PACKET_COUNT: usize = 20;
const PACKETS_PER_SECOND: usize = 100;
const MIN_CONTINUITY_PACKETS: usize = 2;
const TONE_FREQUENCY_HZ: f32 = 997.0;
const TONE_AMPLITUDE: f32 = 0.5;
const MAX_BOUNDARY_RESIDUAL: f32 = 0.02;
const INCOMPLETE_SAMPLE_COUNT: usize = 1;

fn stereo_tone(frames: usize) -> Vec<f32> {
    (0..frames)
        .flat_map(|frame| {
            let phase =
                std::f32::consts::TAU * TONE_FREQUENCY_HZ * frame as f32 / INPUT_RATE as f32;
            let sample = TONE_AMPLITUDE * phase.sin();
            [sample, sample]
        })
        .collect()
}

fn maximum_boundary_residual(packets: &[Vec<f32>]) -> f32 {
    packets.windows(2).fold(0.0, |maximum, pair| {
        let previous = &pair[0];
        let current = &pair[1];
        let last = previous.len() - CHANNELS as usize;
        let penultimate = last - CHANNELS as usize;
        (0..CHANNELS as usize).fold(maximum, |maximum, channel| {
            let predicted = previous[last + channel]
                + (previous[last + channel] - previous[penultimate + channel]);
            maximum.max((current[channel] - predicted).abs())
        })
    })
}

fn stereo_config() -> AudioResamplerConfig {
    AudioResamplerConfig {
        input_rate: INPUT_RATE,
        output_rate: OUTPUT_RATE,
        channels: CHANNELS,
    }
}

#[test]
fn receiver_selects_samplerate_backend() {
    fn assert_samplerate_backend(_: &samplerate::Samplerate) {}

    let resampler = AudioResampler::new(stereo_config()).unwrap();

    assert_samplerate_backend(&resampler.backend);
}

#[test]
fn capture_resampler_emits_complete_continuous_frames() {
    let input = stereo_tone(INPUT_PACKET_FRAMES * PACKET_COUNT);
    let mut resampler =
        FixedFrameAudioResampler::new(stereo_config(), OUTPUT_PACKET_FRAMES).unwrap();
    let packets: Vec<_> = input
        .chunks(INPUT_PACKET_FRAMES * CHANNELS as usize)
        .flat_map(|packet| resampler.process(packet).unwrap())
        .collect();

    assert!(packets.len() >= MIN_CONTINUITY_PACKETS);
    assert!(packets
        .iter()
        .all(|packet| packet.len() == OUTPUT_PACKET_FRAMES * CHANNELS as usize));
    assert!(maximum_boundary_residual(&packets) <= MAX_BOUNDARY_RESIDUAL);
}

#[test]
fn capture_resampler_can_move_into_the_audio_callback() {
    fn assert_send<T: Send>() {}

    assert_send::<FixedFrameAudioResampler>();
}

#[test]
fn samplerate_backend_preserves_streaming_continuity() {
    let input = stereo_tone(INPUT_PACKET_FRAMES * PACKET_COUNT);
    let mut resampler = AudioResampler::new(stereo_config()).unwrap();
    let packets: Vec<_> = input
        .chunks(INPUT_PACKET_FRAMES * CHANNELS as usize)
        .map(|packet| resampler.process(packet).unwrap())
        .filter(|packet| !packet.is_empty())
        .collect();

    assert!(packets.len() >= MIN_CONTINUITY_PACKETS);
    assert!(packets
        .iter()
        .all(|packet| packet.len() % CHANNELS as usize == 0));
    assert!(maximum_boundary_residual(&packets) <= MAX_BOUNDARY_RESIDUAL);
}

#[test]
fn samplerate_backend_reports_incomplete_frame_context() {
    let mut resampler = AudioResampler::new(stereo_config()).unwrap();

    assert_eq!(
        resampler.process(&[0.0]).unwrap_err(),
        AudioResamplerError::IncompleteFrame {
            samples: INCOMPLETE_SAMPLE_COUNT,
            channels: CHANNELS as usize,
        }
    );
}
