use super::{
    send_f32, CaptureEncoderConfig, CaptureEncoderContext, CapturePcmLoss, CapturePcmReceiver,
};
use hbb_common::log;
use magnum_opus::Channels;
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

const CAPTURE_DECLICK_MS: usize = 5;
const MILLISECONDS_PER_SECOND: usize = 1_000;
const MAX_ENCODE_CHANNELS: usize = Channels::Stereo as usize;
const CAPTURE_LOSS_LOG_INTERVAL: Duration = Duration::from_secs(5);

struct CaptureEncoderState {
    channels: usize,
    expected_sequence: usize,
    fade_frames: usize,
    last_frame: [f32; MAX_ENCODE_CHANNELS],
    reporter: CaptureLossReporter,
}

impl CaptureEncoderState {
    fn new(sample_rate: u32, channels: Channels) -> Self {
        // The encoder has already validated the supported Opus rate and channel count.
        let fade_frames = sample_rate as usize * CAPTURE_DECLICK_MS / MILLISECONDS_PER_SECOND;
        Self {
            channels: channels as usize,
            expected_sequence: 0,
            fade_frames,
            last_frame: [0.0; MAX_ENCODE_CHANNELS],
            reporter: CaptureLossReporter::new(),
        }
    }

    fn next_packet(&mut self, receiver: &CapturePcmReceiver) -> Option<Vec<f32>> {
        self.reporter.record(receiver.take_loss());
        self.reporter.report(false);
        let (sequence, mut packet) = receiver.pop_packet()?;
        self.smooth_packet(sequence, &mut packet);
        Some(packet)
    }

    fn smooth_packet(&mut self, sequence: usize, packet: &mut [f32]) {
        if sequence != self.expected_sequence {
            // Capture packets contain 10 ms of PCM, so the transition fits in this packet.
            for (index, frame) in packet
                .chunks_exact_mut(self.channels)
                .take(self.fade_frames)
                .enumerate()
            {
                let weight = (index + 1) as f32 / self.fade_frames as f32;
                for (channel, sample) in frame.iter_mut().enumerate() {
                    *sample = self.last_frame[channel] * (1.0 - weight) + *sample * weight;
                }
            }
        }
        self.expected_sequence = sequence.wrapping_add(1);
        if let Some(frame) = packet.chunks_exact(self.channels).next_back() {
            self.last_frame[..self.channels].copy_from_slice(frame);
        }
    }
}

struct CaptureLossReporter {
    pending: CapturePcmLoss,
    last_report: Instant,
}

impl CaptureLossReporter {
    fn new() -> Self {
        Self {
            pending: Default::default(),
            last_report: Instant::now(),
        }
    }

    fn record(&mut self, loss: CapturePcmLoss) {
        self.pending.add(loss);
    }

    fn report(&mut self, force: bool) {
        if self.pending.is_empty() {
            return;
        }
        if !force && self.last_report.elapsed() < CAPTURE_LOSS_LOG_INTERVAL {
            return;
        }
        let loss = std::mem::take(&mut self.pending);
        log::warn!(
            "Audio capture PCM handoff loss: dropped={}, oversized={}, recycle_failures={}",
            loss.dropped,
            loss.oversized,
            loss.recycle_failures
        );
        self.last_report = Instant::now();
    }
}

pub(super) fn run_capture_encoder(
    mut context: CaptureEncoderContext,
    config: CaptureEncoderConfig,
) {
    let mut state = CaptureEncoderState::new(config.sample_rate, config.encode_channel);
    loop {
        while let Some(packet) = state.next_packet(&context.receiver) {
            send_f32(&packet, &mut context.encoder, &context.service);
            context.receiver.recycle(packet);
        }
        if context.stop.load(Ordering::Acquire) && context.receiver.is_empty() {
            state.reporter.record(context.receiver.take_loss());
            state.reporter.report(true);
            return;
        }
        std::thread::park_timeout(CAPTURE_LOSS_LOG_INTERVAL);
    }
}

#[cfg(test)]
#[path = "audio_capture_encoder_tests.rs"]
mod tests;
