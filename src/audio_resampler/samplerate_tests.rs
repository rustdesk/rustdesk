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
const DOWNSAMPLE_RATE: u32 = 24_000;
const REJECTED_TONE_HZ: f64 = 18_000.0;
const MAX_ALIAS_RMS: f64 = 0.01;
const MIN_PASSBAND_RMS: f64 = 0.3;

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
fn moving_capture_resampler_preserves_pending_audio() {
    let input = stereo_tone(INPUT_PACKET_FRAMES * PACKET_COUNT);
    let packet_samples = INPUT_PACKET_FRAMES * CHANNELS as usize;
    let mut expected_resampler =
        FixedFrameAudioResampler::new(stereo_config(), OUTPUT_PACKET_FRAMES).unwrap();
    let expected: Vec<_> = input
        .chunks(packet_samples)
        .flat_map(|packet| expected_resampler.process(packet).unwrap())
        .collect();
    let mut moved_resampler =
        FixedFrameAudioResampler::new(stereo_config(), OUTPUT_PACKET_FRAMES).unwrap();
    let mut output = moved_resampler.process(&input[..packet_samples]).unwrap();
    let remaining = std::thread::spawn(move || {
        input[packet_samples..]
            .chunks(packet_samples)
            .flat_map(|packet| moved_resampler.process(packet).unwrap())
            .collect::<Vec<_>>()
    })
    .join()
    .unwrap();
    output.extend(remaining);

    assert!(output.len() >= MIN_CONTINUITY_PACKETS);
    assert!(output
        .iter()
        .all(|packet| packet.len() == OUTPUT_PACKET_FRAMES * CHANNELS as usize));
    assert!(maximum_boundary_residual(&output) <= MAX_BOUNDARY_RESIDUAL);
    assert_eq!(output, expected);
}

fn downsampled_rms(input: &[f32]) -> f64 {
    let config = AudioResamplerConfig {
        output_rate: DOWNSAMPLE_RATE,
        ..stereo_config()
    };
    let output_frames = DOWNSAMPLE_RATE as usize / PACKETS_PER_SECOND;
    let mut resampler = FixedFrameAudioResampler::new(config, output_frames).unwrap();
    let output: Vec<f32> = input
        .chunks(INPUT_PACKET_FRAMES * CHANNELS as usize)
        .flat_map(|packet| resampler.process(packet).unwrap().into_iter().flatten())
        .collect();

    assert!(output.len() >= output_frames * CHANNELS as usize * MIN_CONTINUITY_PACKETS);
    let mean_square = output
        .iter()
        .map(|sample| f64::from(*sample).powi(2))
        .sum::<f64>()
        / output.len() as f64;
    mean_square.sqrt()
}

#[test]
fn capture_downsampling_filters_out_of_band_audio() {
    let input: Vec<_> = (0..INPUT_PACKET_FRAMES * PACKET_COUNT)
        .flat_map(|frame| {
            let phase =
                std::f64::consts::TAU * REJECTED_TONE_HZ * frame as f64 / f64::from(INPUT_RATE);
            let sample = (f64::from(TONE_AMPLITUDE) * phase.sin()) as f32;
            [sample, sample]
        })
        .collect();
    let rms = downsampled_rms(&input);
    assert!(
        rms < MAX_ALIAS_RMS,
        "out-of-band output RMS {rms} exceeded {MAX_ALIAS_RMS}"
    );

    let input = stereo_tone(INPUT_PACKET_FRAMES * PACKET_COUNT);
    let rms = downsampled_rms(&input);
    assert!(
        rms > MIN_PASSBAND_RMS,
        "in-band output RMS {rms} fell below {MIN_PASSBAND_RMS}"
    );
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
