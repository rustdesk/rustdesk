use super::{send_f32, GenericService};
use crossbeam_queue::ArrayQueue;
use hbb_common::{
    anyhow::{bail, Context, Result},
    log,
};
use magnum_opus::{Application::LowDelay, Channels, Encoder};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, OnceLock,
    },
    thread::{JoinHandle, Thread},
};

#[path = "audio_capture_encoder.rs"]
mod encoder;

const CAPTURE_PCM_QUEUE_PACKETS: usize = 10;
const CAPTURE_ENCODER_THREAD_NAME: &str = "audio-encoder";

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct CapturePcmLoss {
    pub(super) dropped: usize,
    pub(super) oversized: usize,
    pub(super) recycle_failures: usize,
}

impl CapturePcmLoss {
    pub(super) fn is_empty(&self) -> bool {
        self.dropped == 0 && self.oversized == 0 && self.recycle_failures == 0
    }

    pub(super) fn add(&mut self, other: Self) {
        self.dropped += other.dropped;
        self.oversized += other.oversized;
        self.recycle_failures += other.recycle_failures;
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct CapturePcmStats {
    pub(super) loss: CapturePcmLoss,
    pub(super) max_queued_packets: usize,
}

impl CapturePcmStats {
    pub(super) fn is_empty(&self) -> bool {
        self.loss.is_empty() && self.max_queued_packets == 0
    }

    pub(super) fn add(&mut self, other: Self) {
        self.loss.add(other.loss);
        self.max_queued_packets = self.max_queued_packets.max(other.max_queued_packets);
    }
}

struct CapturePcmHandoff {
    available: ArrayQueue<Vec<f32>>,
    ready: ArrayQueue<(usize, Vec<f32>)>,
    wake_thread: OnceLock<Thread>,
    dropped: AtomicUsize,
    oversized: AtomicUsize,
    recycle_failures: AtomicUsize,
    max_queued_packets: AtomicUsize,
    max_samples: usize,
}

pub(super) struct CapturePcmSender {
    handoff: Arc<CapturePcmHandoff>,
    spare: Option<Vec<f32>>,
    sequence: usize,
}

pub(super) struct CapturePcmReceiver {
    handoff: Arc<CapturePcmHandoff>,
}

pub(super) struct CaptureEncoderConfig {
    pub(super) sample_rate: u32,
    pub(super) encode_channel: Channels,
    pub(super) max_packet_samples: usize,
}

pub(super) struct CaptureEncoderWorker {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Drop for CaptureEncoderWorker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(handle) = self.handle.take() {
            handle.thread().unpark();
            if let Err(error) = handle.join() {
                log::error!("Failed to join audio encoder thread: {error:?}");
            }
        }
    }
}

struct CaptureEncoderContext {
    receiver: CapturePcmReceiver,
    encoder: Encoder,
    service: GenericService,
    stop: Arc<AtomicBool>,
}

pub(super) fn new_pcm_handoff(
    capacity: usize,
    max_samples: usize,
) -> Result<(CapturePcmSender, CapturePcmReceiver)> {
    if capacity == 0 || max_samples == 0 {
        bail!("Audio capture PCM handoff requires nonzero capacity and packet size");
    }
    let handoff = Arc::new(CapturePcmHandoff {
        available: ArrayQueue::new(capacity),
        ready: ArrayQueue::new(capacity),
        wake_thread: OnceLock::new(),
        dropped: AtomicUsize::new(0),
        oversized: AtomicUsize::new(0),
        recycle_failures: AtomicUsize::new(0),
        max_queued_packets: AtomicUsize::new(0),
        max_samples,
    });
    for _ in 0..capacity {
        if handoff
            .available
            .push(Vec::with_capacity(max_samples))
            .is_err()
        {
            bail!("Failed to initialize audio capture PCM buffer pool");
        }
    }
    Ok((
        CapturePcmSender {
            handoff: handoff.clone(),
            spare: None,
            sequence: 0,
        },
        CapturePcmReceiver { handoff },
    ))
}

impl CapturePcmSender {
    pub(super) fn set_wake_thread(&self, thread: Thread) -> Result<()> {
        if self.handoff.wake_thread.set(thread).is_err() {
            bail!("Audio capture PCM wake thread is already configured");
        }
        Ok(())
    }

    pub(super) fn submit(&mut self, input: &[f32]) {
        let sequence = self.sequence;
        self.sequence = self.sequence.wrapping_add(1);
        if input.len() > self.handoff.max_samples {
            self.handoff.oversized.fetch_add(1, Ordering::Relaxed);
            self.wake();
            return;
        }
        let Some(mut buffer) = self.take_buffer() else {
            self.handoff.dropped.fetch_add(1, Ordering::Relaxed);
            self.wake();
            return;
        };
        buffer.clear();
        buffer.extend_from_slice(input);
        if let Err((_, buffer)) = self.handoff.ready.push((sequence, buffer)) {
            self.handoff
                .recycle_failures
                .fetch_add(1, Ordering::Relaxed);
            self.spare = Some(buffer);
        } else {
            self.handoff
                .max_queued_packets
                .fetch_max(self.handoff.ready.len(), Ordering::Relaxed);
        }
        self.wake();
    }

    fn take_buffer(&mut self) -> Option<Vec<f32>> {
        self.spare
            .take()
            .or_else(|| self.handoff.available.pop())
            .or_else(|| {
                let buffer = self.handoff.ready.pop().map(|(_, buffer)| buffer);
                if buffer.is_some() {
                    self.handoff.dropped.fetch_add(1, Ordering::Relaxed);
                }
                buffer
            })
    }

    fn wake(&self) {
        if let Some(thread) = self.handoff.wake_thread.get() {
            thread.unpark();
        }
    }
}

impl CapturePcmReceiver {
    #[cfg(test)]
    pub(super) fn pop(&self) -> Option<Vec<f32>> {
        self.pop_packet().map(|(_, buffer)| buffer)
    }

    fn pop_packet(&self) -> Option<(usize, Vec<f32>)> {
        self.handoff.ready.pop()
    }

    pub(super) fn recycle(&self, mut buffer: Vec<f32>) {
        buffer.clear();
        if self.handoff.available.push(buffer).is_err() {
            self.handoff
                .recycle_failures
                .fetch_add(1, Ordering::Relaxed);
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.handoff.ready.is_empty()
    }

    pub(super) fn take_loss(&self) -> CapturePcmLoss {
        CapturePcmLoss {
            dropped: self.handoff.dropped.swap(0, Ordering::Relaxed),
            oversized: self.handoff.oversized.swap(0, Ordering::Relaxed),
            recycle_failures: self.handoff.recycle_failures.swap(0, Ordering::Relaxed),
        }
    }

    pub(super) fn take_stats(&self) -> CapturePcmStats {
        CapturePcmStats {
            loss: self.take_loss(),
            max_queued_packets: self.handoff.max_queued_packets.swap(0, Ordering::Relaxed),
        }
    }
}

pub(super) fn start_capture_encoder(
    config: CaptureEncoderConfig,
    service: GenericService,
) -> Result<(CapturePcmSender, CaptureEncoderWorker)> {
    let (sender, receiver) = new_pcm_handoff(CAPTURE_PCM_QUEUE_PACKETS, config.max_packet_samples)?;
    let encoder = Encoder::new(config.sample_rate, config.encode_channel, LowDelay)?;
    let stop = Arc::new(AtomicBool::new(false));
    let context = CaptureEncoderContext {
        receiver,
        encoder,
        service,
        stop: stop.clone(),
    };
    let handle = std::thread::Builder::new()
        .name(CAPTURE_ENCODER_THREAD_NAME.to_owned())
        .spawn(move || encoder::run_capture_encoder(context, config))
        .with_context(|| "Failed to start audio encoder thread")?;
    let wake_thread = handle.thread().clone();
    let worker = CaptureEncoderWorker {
        stop,
        handle: Some(handle),
    };
    sender.set_wake_thread(wake_thread)?;
    Ok((sender, worker))
}
