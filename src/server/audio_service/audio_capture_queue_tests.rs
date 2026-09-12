use super::*;
use crate::audio_resampler::allocation_tests::assert_no_allocations;
use std::{sync::mpsc, time::Duration};

const PACKET_SAMPLES: usize = 4;
const TEST_TIMEOUT: Duration = Duration::from_secs(2);
const CONCURRENT_PACKETS: usize = 10_000;

#[derive(Clone, Copy)]
enum PausePoint {
    BeforeRecycle,
    Recycle,
    Consume,
}

struct WorkerPause {
    entered: mpsc::Sender<()>,
    resume: mpsc::Receiver<()>,
}

impl WorkerPause {
    fn wait(&self) {
        self.entered.send(()).unwrap();
        self.resume.recv().unwrap();
    }
}

fn paused_worker(
    receiver: CapturePcmReceiver,
    point: PausePoint,
    pause: WorkerPause,
) -> CapturePcmReceiver {
    let (index, _) = receiver.handoff.buffers.claim(false).unwrap();
    let mut slot = receiver.handoff.buffers.slots[index].lock().unwrap();
    let mut packet = std::mem::take(&mut slot.1);
    if matches!(point, PausePoint::Consume) {
        pause.wait();
    }
    packet.clear();
    slot.1 = packet;
    if matches!(point, PausePoint::BeforeRecycle) {
        pause.wait();
    }
    drop(slot);
    receiver.handoff.buffers.publish_available(index);
    if matches!(point, PausePoint::Recycle) {
        pause.wait();
    }
    receiver
}

fn assert_callback_progress(point: PausePoint, initial_packets: usize) {
    let (mut sender, receiver) =
        new_pcm_handoff(CAPTURE_PCM_QUEUE_PACKETS, PACKET_SAMPLES).unwrap();
    for sequence in 0..initial_packets {
        sender.submit(&[sequence as f32; PACKET_SAMPLES]);
    }
    let (entered_tx, entered_rx) = mpsc::channel();
    let (resume_tx, resume_rx) = mpsc::channel();
    let pause = WorkerPause {
        entered: entered_tx,
        resume: resume_rx,
    };
    let worker = std::thread::spawn(move || paused_worker(receiver, point, pause));
    sender.set_wake_thread(worker.thread().clone()).unwrap();
    entered_rx.recv_timeout(TEST_TIMEOUT).unwrap();
    let (completed_tx, completed_rx) = mpsc::channel();
    let callback = std::thread::spawn(move || {
        let packet = [initial_packets as f32; PACKET_SAMPLES];
        assert_no_allocations(|| sender.submit(&packet));
        completed_tx.send(()).unwrap();
        sender
    });
    let completed = completed_rx.recv_timeout(TEST_TIMEOUT);
    // Release and join both threads before asserting progress, including on failure.
    resume_tx.send(()).unwrap();
    let receiver = worker.join().unwrap();
    let mut sender = callback.join().unwrap();
    assert!(completed.is_ok(), "callback waited for the encoder worker");
    let dropped = usize::from(
        initial_packets == CAPTURE_PCM_QUEUE_PACKETS && !matches!(point, PausePoint::Recycle),
    );
    assert_eq!(
        receiver.take_loss(),
        CapturePcmLoss {
            dropped,
            ..Default::default()
        }
    );
    assert_retained_packets(&mut sender, &receiver, (dropped + 1)..=initial_packets);
}

fn assert_retained_packets(
    sender: &mut CapturePcmSender,
    receiver: &CapturePcmReceiver,
    retained: std::ops::RangeInclusive<usize>,
) {
    let next_sequence = retained.end() + 1;
    for expected in retained.chain(std::iter::once(next_sequence)) {
        if expected == next_sequence {
            sender.submit(&[next_sequence as f32; PACKET_SAMPLES]);
        }
        let (sequence, packet) = receiver.pop_packet().unwrap();
        assert_eq!(sequence, expected);
        assert_eq!(packet, [expected as f32; PACKET_SAMPLES]);
        receiver.recycle(packet);
    }
    assert!(receiver.is_empty());
    assert!(receiver.take_loss().is_empty());
    assert_pool_restored(receiver, CAPTURE_PCM_QUEUE_PACKETS);
}

fn assert_pool_restored(receiver: &CapturePcmReceiver, capacity: usize) {
    let buffers = &receiver.handoff.buffers;
    assert_eq!(buffers.available_len(), capacity);
    assert!(buffers.is_empty());
    assert_eq!(buffers.slots.len(), capacity);
    assert!(buffers
        .slots
        .iter()
        .all(|slot| slot.lock().unwrap().1.capacity() >= PACKET_SAMPLES));
}

#[test]
fn callback_finishes_while_worker_recycles_a_buffer() {
    for queued in [1, CAPTURE_PCM_QUEUE_PACKETS - 1, CAPTURE_PCM_QUEUE_PACKETS] {
        assert_callback_progress(PausePoint::BeforeRecycle, queued);
        assert_callback_progress(PausePoint::Recycle, queued);
    }
}

#[test]
fn callback_finishes_while_worker_releases_a_ready_packet() {
    for queued in [1, CAPTURE_PCM_QUEUE_PACKETS - 1, CAPTURE_PCM_QUEUE_PACKETS] {
        assert_callback_progress(PausePoint::Consume, queued);
    }
}

fn consume_concurrently(
    receiver: CapturePcmReceiver,
    finished: Arc<AtomicBool>,
    first_sequence: usize,
) -> (CapturePcmReceiver, usize) {
    let mut received = 0;
    let mut next_ordinal = 0;
    loop {
        if let Some((sequence, packet)) = receiver.pop_packet() {
            let ordinal = sequence.wrapping_sub(first_sequence);
            assert!(ordinal >= next_ordinal && ordinal < CONCURRENT_PACKETS);
            assert_eq!(packet, [ordinal as f32; PACKET_SAMPLES]);
            next_ordinal = ordinal + 1;
            received += 1;
            receiver.recycle(packet);
        } else if finished.load(Ordering::Acquire) && receiver.is_empty() {
            return (receiver, received);
        } else {
            std::thread::yield_now();
        }
    }
}

#[test]
fn concurrent_handoff_preserves_order_buffers_and_loss_counts_across_sequence_wrap() {
    const FIRST_SEQUENCE: usize = usize::MAX - CONCURRENT_PACKETS / 2;
    for capacity in [1, 2, CAPTURE_PCM_QUEUE_PACKETS, buffer_pool::MAX_BUFFERS] {
        let (mut sender, receiver) = new_pcm_handoff(capacity, PACKET_SAMPLES).unwrap();
        sender.sequence = FIRST_SEQUENCE;
        let finished = Arc::new(AtomicBool::new(false));
        let worker_finished = finished.clone();
        let worker = std::thread::spawn(move || {
            consume_concurrently(receiver, worker_finished, FIRST_SEQUENCE)
        });
        assert_no_allocations(|| {
            for ordinal in 0..CONCURRENT_PACKETS {
                sender.submit(&[ordinal as f32; PACKET_SAMPLES]);
            }
        });
        finished.store(true, Ordering::Release);
        let (receiver, received) = worker.join().unwrap();
        let stats = receiver.take_stats();
        assert_eq!(received + stats.loss.dropped, CONCURRENT_PACKETS);
        assert_eq!(stats.loss.contention_dropped, 0);
        assert_eq!(stats.loss.oversized, 0);
        assert_eq!(stats.loss.recycle_failures, 0);
        assert!(stats.max_queued_packets <= capacity);
        assert_pool_restored(&receiver, capacity);
    }
}
