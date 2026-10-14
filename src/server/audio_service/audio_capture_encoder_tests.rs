use super::super::{new_pcm_handoff, CAPTURE_PCM_QUEUE_PACKETS};
use super::*;
use magnum_opus::{Application::LowDelay, Decoder, Encoder};

const SAMPLE_RATE: u32 = 48_000;
const PACKETS_PER_SECOND: usize = 100;
const PACKET_COUNT: usize = 120;
const PAUSE_PACKET: usize = 50;
const DROPPED_PACKETS: usize = 6;
const SIGNAL_FREQUENCY: f64 = 97.0;
const SIGNAL_AMPLITUDE: f64 = 0.5;
const MAX_ENCODE_BYTES_PER_SAMPLE: usize = 6;

fn signal_packet(index: usize, channels: usize) -> Vec<f32> {
    let frames = SAMPLE_RATE as usize / PACKETS_PER_SECOND;
    (0..frames)
        .flat_map(|frame| {
            let phase = std::f64::consts::TAU * SIGNAL_FREQUENCY * (index * frames + frame) as f64
                / f64::from(SAMPLE_RATE);
            (0..channels)
                .map(move |channel| (SIGNAL_AMPLITUDE * (phase + channel as f64).sin()) as f32)
        })
        .collect()
}

fn encoded_audio(dropped: usize, channels: Channels) -> Vec<f32> {
    let samples = SAMPLE_RATE as usize / PACKETS_PER_SECOND * channels as usize;
    let (mut sender, receiver) = new_pcm_handoff(CAPTURE_PCM_QUEUE_PACKETS, samples).unwrap();
    let mut state = CaptureEncoderState::new(SAMPLE_RATE, channels);
    let mut encoder = Encoder::new(SAMPLE_RATE, channels, LowDelay).unwrap();
    let mut decoder = Decoder::new(SAMPLE_RATE, channels).unwrap();
    let mut decoded = vec![0.0; samples];
    let mut output = Vec::new();
    let resume_packet = PAUSE_PACKET + CAPTURE_PCM_QUEUE_PACKETS + dropped - 1;
    for index in 0..PACKET_COUNT {
        sender.submit(&signal_packet(index, channels as usize));
        if (PAUSE_PACKET..resume_packet).contains(&index) {
            continue;
        }
        if index == resume_packet {
            let stats = receiver.take_stats();
            assert_eq!(stats.max_queued_packets, CAPTURE_PCM_QUEUE_PACKETS);
            assert_eq!(stats.loss.dropped, dropped);
            assert_eq!(stats.loss.contention_dropped, 0);
            assert_eq!(stats.loss.oversized, 0);
            assert_eq!(stats.loss.recycle_failures, 0);
        }
        while let Some(packet) = state.next_packet(&receiver) {
            let encoded = encoder
                .encode_vec_float(&packet, samples * MAX_ENCODE_BYTES_PER_SAMPLE)
                .unwrap();
            let frames = decoder.decode_float(&encoded, &mut decoded, false).unwrap();
            assert_eq!(frames * channels as usize, samples);
            output.extend_from_slice(&decoded);
            receiver.recycle(packet);
        }
    }
    assert_eq!(output.len(), (PACKET_COUNT - dropped) * samples);
    output
}

fn maximum_join_step(pcm: &[f32], channels: usize) -> f32 {
    let samples = SAMPLE_RATE as usize / PACKETS_PER_SECOND * channels;
    pcm[(PAUSE_PACKET - 1) * samples..(PAUSE_PACKET + 2) * samples]
        .windows(channels + 1)
        .map(|window| (window[channels] - window[0]).abs())
        .fold(0.0, f32::max)
}

#[test]
fn capture_overflow_is_smoothed_before_encoding() {
    const MAX_STEP_RATIO: f32 = 2.0;
    for channels in [Channels::Mono, Channels::Stereo] {
        let clean = encoded_audio(0, channels);
        let overflow = encoded_audio(DROPPED_PACKETS, channels);
        let clean_step = maximum_join_step(&clean, channels as usize);
        let overflow_step = maximum_join_step(&overflow, channels as usize);
        assert!(clean_step > 0.0);
        assert!(
            overflow_step < clean_step * MAX_STEP_RATIO,
            "capture discard introduced a sharp join: clean={clean_step}, overflow={overflow_step}"
        );
    }
}
