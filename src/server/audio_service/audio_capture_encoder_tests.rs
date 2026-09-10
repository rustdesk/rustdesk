use super::super::{new_pcm_handoff, CAPTURE_PCM_QUEUE_PACKETS};
use super::*;
use crate::audio_resampler::allocation_tests::assert_no_allocations;
use magnum_opus::{Application::LowDelay, Decoder, Encoder};

const SAMPLE_RATE: u32 = 48_000;
const PACKETS_PER_SECOND: usize = 100;
const PACKET_COUNT: usize = 120;
const PAUSE_PACKET: usize = 50;
const DROPPED_PACKETS: usize = 6;
const SIGNAL_FREQUENCY: f64 = 97.0;
const SIGNAL_AMPLITUDE: f64 = 0.5;
const ACTIVE_LEVEL: f32 = 0.8;
const SAMPLE_TOLERANCE: f32 = 0.000001;
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

#[test]
fn rejected_packets_mark_the_gap_after_already_queued_audio() {
    const CAPACITY: usize = 3;
    let samples = SAMPLE_RATE as usize / PACKETS_PER_SECOND;
    let (mut sender, receiver) = new_pcm_handoff(CAPACITY, samples).unwrap();
    let mut state = CaptureEncoderState::new(SAMPLE_RATE, Channels::Mono);
    let active = vec![ACTIVE_LEVEL; samples];
    let opposite = vec![-ACTIVE_LEVEL; samples];
    sender.submit(&active);
    let packet = state.next_packet(&receiver).unwrap();
    assert_eq!(packet, active);
    receiver.recycle(packet);
    sender.submit(&active);
    sender.submit(&active);
    sender.submit(&vec![ACTIVE_LEVEL; samples + 1]);
    sender.submit(&opposite);
    assert_eq!(receiver.take_loss().oversized, 1);
    assert_no_allocations(|| {
        for _ in 0..CAPACITY - 1 {
            let packet = state.next_packet(&receiver).unwrap();
            assert_eq!(packet, active);
            receiver.recycle(packet);
        }
        let packet = state.next_packet(&receiver).unwrap();
        let expected = ACTIVE_LEVEL * (1.0 - 2.0 / state.fade_frames as f32);
        assert!((packet[0] - expected).abs() < SAMPLE_TOLERANCE);
        assert_eq!(&packet[samples / 2..], &opposite[samples / 2..]);
        receiver.recycle(packet);
    });
}

#[test]
fn gaps_and_sequence_wrap_preserve_channel_history() {
    const RATE_8_KHZ: u32 = 8_000;
    for (rate, channels) in [
        (RATE_8_KHZ, Channels::Mono),
        (RATE_8_KHZ, Channels::Stereo),
        (SAMPLE_RATE, Channels::Mono),
        (SAMPLE_RATE, Channels::Stereo),
    ] {
        let channels_count = channels as usize;
        let mut state = CaptureEncoderState::new(rate, channels);
        let samples = rate as usize / PACKETS_PER_SECOND * channels_count;
        let active: Vec<_> = [ACTIVE_LEVEL, -ACTIVE_LEVEL][..channels_count]
            .iter()
            .copied()
            .cycle()
            .take(samples)
            .collect();
        let mut first = active.clone();
        state.smooth_packet(0, &mut first);
        assert_eq!(first, active);
        let mut opposite: Vec<_> = active.iter().map(|sample| -sample).collect();
        let mut resumed = active.clone();
        assert_no_allocations(|| {
            state.smooth_packet(2, &mut opposite);
            state.smooth_packet(4, &mut resumed);
        });
        for channel in 0..channels_count {
            let expected = active[channel] * (1.0 - 2.0 / state.fade_frames as f32);
            assert!((opposite[channel] - expected).abs() < SAMPLE_TOLERANCE);
            let previous = opposite[samples - channels_count + channel];
            let expected = previous + (active[channel] - previous) / state.fade_frames as f32;
            assert!((resumed[channel] - expected).abs() < SAMPLE_TOLERANCE);
        }
        assert_eq!(&resumed[samples / 2..], &active[samples / 2..]);
        state.expected_sequence = usize::MAX;
        let mut last = active.clone();
        let mut wrapped: Vec<_> = active.iter().map(|sample| -sample).collect();
        let expected = wrapped.clone();
        assert_no_allocations(|| {
            state.smooth_packet(usize::MAX, &mut last);
            state.smooth_packet(0, &mut wrapped);
        });
        assert_eq!(last, active);
        assert_eq!(wrapped, expected);
    }
}

#[test]
fn capture_loss_is_reported_while_packets_remain_queued() {
    const CAPACITY: usize = 2;
    let samples = SAMPLE_RATE as usize / PACKETS_PER_SECOND;
    let (mut sender, receiver) = new_pcm_handoff(CAPACITY, samples).unwrap();
    let mut state = CaptureEncoderState::new(SAMPLE_RATE, Channels::Mono);
    let before_report = Instant::now() - CAPTURE_STATS_LOG_INTERVAL;
    state.reporter.last_report = before_report;
    let packet = vec![ACTIVE_LEVEL; samples];
    for _ in 0..=CAPACITY {
        sender.submit(&packet);
    }
    let packet = state.next_packet(&receiver).unwrap();
    assert!(!receiver.is_empty());
    assert!(state.reporter.last_report > before_report);
    assert!(state.reporter.pending.is_empty());
    assert!(receiver.take_loss().is_empty());
    receiver.recycle(packet);
}
