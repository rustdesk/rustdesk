use super::{send_f32, GenericService};
use hbb_common::{
    anyhow::{bail, Context, Result},
    log,
};
use magnum_opus::{Application::LowDelay, Channels, Encoder};
use std::{
    cell::RefCell,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, OnceLock,
    },
    thread::{JoinHandle, Thread},
};

mod buffer_pool;
#[path = "audio_capture_encoder.rs"]
mod encoder;

const CAPTURE_PCM_QUEUE_PACKETS: usize = 10;
const CAPTURE_ENCODER_THREAD_NAME: &str = "audio-encoder";

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct CapturePcmLoss {
    pub(super) dropped: usize,
    // Retain the report field; atomic ownership transfer no longer rejects contention.
    pub(super) contention_dropped: usize,
    pub(super) oversized: usize,
    pub(super) recycle_failures: usize,
}

impl CapturePcmLoss {
    pub(super) fn is_empty(&self) -> bool {
        self.dropped == 0 && self.oversized == 0 && self.recycle_failures == 0
    }

    pub(super) fn add(&mut self, other: Self) {
        self.dropped += other.dropped;
        self.contention_dropped += other.contention_dropped;
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
    buffers: buffer_pool::BufferPool,
    wake_thread: OnceLock<Thread>,
    other_dropped: AtomicUsize,
    oversized: AtomicUsize,
    recycle_failures: AtomicUsize,
    max_queued_packets: AtomicUsize,
    max_samples: usize,
}

pub(super) struct CapturePcmSender {
    handoff: Arc<CapturePcmHandoff>,
    sequence: usize,
}

pub(super) struct CapturePcmReceiver {
    handoff: Arc<CapturePcmHandoff>,
    in_use: RefCell<Vec<usize>>,
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
    let handoff = Arc::new(CapturePcmHandoff {
        buffers: buffer_pool::BufferPool::new(capacity, max_samples)?,
        wake_thread: OnceLock::new(),
        other_dropped: AtomicUsize::new(0),
        oversized: AtomicUsize::new(0),
        recycle_failures: AtomicUsize::new(0),
        max_queued_packets: AtomicUsize::new(0),
        max_samples,
    });
    Ok((
        CapturePcmSender {
            handoff: handoff.clone(),
            sequence: 0,
        },
        CapturePcmReceiver {
            handoff,
            in_use: RefCell::new(Vec::with_capacity(capacity)),
        },
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
        if let Some(index) = self.take_buffer() {
            self.handoff.buffers.write(index, sequence, input);
            let queued = self.handoff.buffers.publish(index);
            self.handoff
                .max_queued_packets
                .fetch_max(queued, Ordering::Relaxed);
        } else {
            self.handoff.other_dropped.fetch_add(1, Ordering::Relaxed);
        }
        self.wake();
    }

    fn take_buffer(&self) -> Option<usize> {
        let (index, was_ready) = self.handoff.buffers.claim(true)?;
        if was_ready {
            self.handoff.other_dropped.fetch_add(1, Ordering::Relaxed);
        }
        Some(index)
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
        let (index, _) = self.handoff.buffers.claim(false)?;
        self.in_use.borrow_mut().push(index);
        Some(self.handoff.buffers.take_packet(index))
    }

    pub(super) fn recycle(&self, mut buffer: Vec<f32>) {
        buffer.clear();
        let index = self.in_use.borrow_mut().pop();
        if let Some(index) = index {
            self.handoff.buffers.store_recycled(index, buffer);
            self.handoff.buffers.publish_available(index);
        } else {
            self.handoff
                .recycle_failures
                .fetch_add(1, Ordering::Relaxed);
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.handoff.buffers.is_empty()
    }

    pub(super) fn take_loss(&self) -> CapturePcmLoss {
        CapturePcmLoss {
            dropped: self.handoff.other_dropped.swap(0, Ordering::Relaxed),
            oversized: self.handoff.oversized.swap(0, Ordering::Relaxed),
            recycle_failures: self.handoff.recycle_failures.swap(0, Ordering::Relaxed),
            ..Default::default()
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

#[cfg(test)]
#[path = "audio_capture_queue_tests.rs"]
mod tests;
