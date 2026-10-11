use super::*;
use crate::audio_resampler::allocation_tests::assert_no_allocations;
use std::{sync::mpsc, time::Duration};

const PACKET_SAMPLES: usize = 4;
const TEST_TIMEOUT: Duration = Duration::from_secs(2);
const CONCURRENT_PACKETS: usize = 10_000;

#[derive(Clone, Copy)]
enum PausePoint {
    Recycle,
    Consume,
}

struct WorkerPause {
    entered: mpsc::Sender<()>,
    resume: mpsc::Receiver<()>,
}

fn paused_worker(
    receiver: CapturePcmReceiver,
    point: PausePoint,
    pause: WorkerPause,
) -> CapturePcmReceiver {
    let recycled = match point {
        PausePoint::Recycle => Some(receiver.pop_packet().unwrap().1),
        PausePoint::Consume => None,
    };
    let consumed = {
        let mut buffers = receiver.handoff.buffers.lock().unwrap();
        let consumed = match recycled {
            Some(mut packet) => {
                packet.clear();
                buffers.available.push(packet);
                None
            }
            None => Some(buffers.ready.pop_front().unwrap().1),
        };
        pause.entered.send(()).unwrap();
        pause.resume.recv().unwrap();
        consumed
    };
    if let Some(packet) = consumed {
        receiver.recycle(packet);
    }
    receiver
}

fn assert_callback_progress(point: PausePoint) {
    let (mut sender, receiver) =
        new_pcm_handoff(CAPTURE_PCM_QUEUE_PACKETS, PACKET_SAMPLES).unwrap();
    for sequence in 0..CAPTURE_PCM_QUEUE_PACKETS {
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
        let packet = [CAPTURE_PCM_QUEUE_PACKETS as f32; PACKET_SAMPLES];
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
    assert_eq!(
        receiver.take_loss(),
        CapturePcmLoss {
            dropped: 1,
            contention_dropped: 1,
            ..Default::default()
        }
    );
    assert_packets_after_contention(&mut sender, &receiver);
}

fn assert_packets_after_contention(sender: &mut CapturePcmSender, receiver: &CapturePcmReceiver) {
    let next_sequence = CAPTURE_PCM_QUEUE_PACKETS + 1;
    sender.submit(&[next_sequence as f32; PACKET_SAMPLES]);
    for expected in (1..CAPTURE_PCM_QUEUE_PACKETS).chain(std::iter::once(next_sequence)) {
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
    let buffers = receiver.handoff.buffers.lock().unwrap();
    assert_eq!(buffers.available.len(), capacity);
    assert!(buffers
        .available
        .iter()
        .all(|buffer| buffer.capacity() >= PACKET_SAMPLES));
}

#[test]
fn callback_finishes_while_worker_recycles_a_buffer() {
    assert_callback_progress(PausePoint::Recycle);
}

#[test]
fn callback_finishes_while_worker_releases_a_ready_packet() {
    assert_callback_progress(PausePoint::Consume);
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
    for capacity in [1, 2, CAPTURE_PCM_QUEUE_PACKETS] {
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
        assert_eq!(stats.loss.oversized, 0);
        assert_eq!(stats.loss.recycle_failures, 0);
        assert!(stats.max_queued_packets <= capacity);
        assert_pool_restored(&receiver, capacity);
    }
}
