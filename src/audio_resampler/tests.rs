use super::{AudioResampler, AudioResamplerConfig, FixedFrameAudioResampler};

const INPUT_RATE: u32 = 24_000;
const OUTPUT_RATE: u32 = 48_000;
const CHANNELS: u16 = 2;
const CHUNK_FRAMES: usize = 240;
const CHUNK_COUNT: usize = 4;
const TONE_FREQUENCY_HZ: f32 = 997.0;
const TONE_AMPLITUDE: f32 = 0.5;
const MAX_BOUNDARY_RESIDUAL: f32 = 0.02;
const LOOK_AHEAD_OUTPUT_FRAMES: usize = 1;
const UNEVEN_CHUNK_FRAMES: usize = 73;
const MONO_CHANNELS: u16 = 1;
const UNIT_RATE: u32 = 1;
const DOUBLE_RATE: u32 = 2;
const FIRST_DOWNSAMPLE_PACKET: [f32; 3] = [0.0, 1.0, 2.0];
const SECOND_DOWNSAMPLE_PACKET: [f32; 4] = [3.0, 4.0, 5.0, 6.0];
const EXPECTED_DOWNSAMPLED_OUTPUT: [f32; 4] = [0.0, 2.0, 4.0, 6.0];
const PACKETS_PER_SECOND: usize = 100;
const OUTPUT_PACKET_FRAMES: usize = OUTPUT_RATE as usize / PACKETS_PER_SECOND;
const RATE_44_1_KHZ: u32 = 44_100;
const FLOAT_TOLERANCE: f32 = 0.000_001;
const MIN_CONTINUITY_PACKETS: usize = 2;

fn stereo_tone_at_rate(frames: usize, sample_rate: u32) -> Vec<f32> {
    (0..frames)
        .flat_map(|frame| {
            let phase =
                std::f32::consts::TAU * TONE_FREQUENCY_HZ * frame as f32 / sample_rate as f32;
            let sample = TONE_AMPLITUDE * phase.sin();
            [sample, sample]
        })
        .collect()
}

fn stereo_tone(frames: usize) -> Vec<f32> {
    stereo_tone_at_rate(frames, INPUT_RATE)
}

fn maximum_tone_prediction_residual(sample_rate: u32) -> f32 {
    let half_step = std::f32::consts::PI * TONE_FREQUENCY_HZ / sample_rate as f32;
    4.0 * TONE_AMPLITUDE * half_step.sin().powi(2)
}

fn maximum_boundary_residual(chunks: &[Vec<f32>]) -> f32 {
    chunks.windows(2).fold(0.0, |maximum, pair| {
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
fn preserves_decoded_packet_continuity_and_output_ratio() {
    let input = stereo_tone(CHUNK_FRAMES * CHUNK_COUNT);
    let mut whole_resampler = AudioResampler::new(stereo_config()).unwrap();
    let whole_output = whole_resampler.process(&input).unwrap();
    let expected_frames = CHUNK_FRAMES * CHUNK_COUNT * OUTPUT_RATE as usize / INPUT_RATE as usize
        - LOOK_AHEAD_OUTPUT_FRAMES;

    for chunk_frames in [CHUNK_FRAMES, UNEVEN_CHUNK_FRAMES] {
        let mut resampler = AudioResampler::new(stereo_config()).unwrap();
        let output: Vec<_> = input
            .chunks(chunk_frames * CHANNELS as usize)
            .map(|chunk| resampler.process(chunk).unwrap())
            .collect();
        let residual = maximum_boundary_residual(&output);
        assert!(
            residual <= MAX_BOUNDARY_RESIDUAL,
            "packet boundary residual {residual} exceeded {MAX_BOUNDARY_RESIDUAL}, chunk_frames={chunk_frames}"
        );
        let output_frames = output.iter().map(Vec::len).sum::<usize>() / CHANNELS as usize;
        assert_eq!(output_frames, expected_frames);
        assert_eq!(output.concat(), whole_output, "chunk_frames={chunk_frames}");
    }
}

#[test]
fn rejects_incomplete_interleaved_frames() {
    let mut resampler = AudioResampler::new(stereo_config()).unwrap();

    assert!(resampler.process(&[TONE_AMPLITUDE]).is_err());
}

#[test]
fn interpolates_mono_samples() {
    let config = AudioResamplerConfig {
        input_rate: UNIT_RATE,
        output_rate: DOUBLE_RATE,
        channels: MONO_CHANNELS,
    };
    let mut resampler = AudioResampler::new(config).unwrap();

    assert_eq!(
        resampler.process(&[0.0, 1.0, 2.0]).unwrap(),
        [0.0, 0.5, 1.0, 1.5, 2.0]
    );
}

#[test]
fn rejects_zero_rate_configuration() {
    let config = AudioResamplerConfig {
        input_rate: 0,
        output_rate: OUTPUT_RATE,
        channels: CHANNELS,
    };

    assert!(AudioResampler::new(config).is_err());
}

#[test]
fn downsamples_across_packet_boundaries() {
    let config = AudioResamplerConfig {
        input_rate: DOUBLE_RATE,
        output_rate: UNIT_RATE,
        channels: MONO_CHANNELS,
    };
    let mut resampler = AudioResampler::new(config).unwrap();
    let mut output = resampler.process(&FIRST_DOWNSAMPLE_PACKET).unwrap();
    output.extend(resampler.process(&SECOND_DOWNSAMPLE_PACKET).unwrap());

    assert_eq!(output, EXPECTED_DOWNSAMPLED_OUTPUT);
}

#[test]
fn sender_resampler_emits_only_complete_continuous_frames() {
    let input = stereo_tone(CHUNK_FRAMES * CHUNK_COUNT);
    let mut resampler =
        FixedFrameAudioResampler::new(stereo_config(), OUTPUT_PACKET_FRAMES).unwrap();
    let output: Vec<_> = input
        .chunks(CHUNK_FRAMES * CHANNELS as usize)
        .flat_map(|chunk| resampler.process(chunk).unwrap())
        .collect();

    assert!(output.len() >= MIN_CONTINUITY_PACKETS);
    assert!(output
        .iter()
        .all(|packet| packet.len() == OUTPUT_PACKET_FRAMES * CHANNELS as usize));
    assert!(maximum_boundary_residual(&output) <= MAX_BOUNDARY_RESIDUAL);
}

#[test]
fn sender_downsampling_preserves_packet_continuity() {
    let input_packet_frames = RATE_44_1_KHZ as usize / PACKETS_PER_SECOND;
    let input = stereo_tone_at_rate(input_packet_frames * CHUNK_COUNT, RATE_44_1_KHZ);
    let config = AudioResamplerConfig {
        input_rate: RATE_44_1_KHZ,
        output_rate: INPUT_RATE,
        channels: CHANNELS,
    };
    let mut resampler =
        FixedFrameAudioResampler::new(config, INPUT_RATE as usize / PACKETS_PER_SECOND).unwrap();
    let packets: Vec<_> = input
        .chunks(input_packet_frames * CHANNELS as usize)
        .flat_map(|packet| resampler.process(packet).unwrap())
        .collect();
    let residual = maximum_boundary_residual(&packets);

    assert_eq!(packets.len(), CHUNK_COUNT);
    assert!(
        residual <= maximum_tone_prediction_residual(INPUT_RATE) + FLOAT_TOLERANCE,
        "sender packet boundary residual {residual} exceeded the tone curvature"
    );
}
