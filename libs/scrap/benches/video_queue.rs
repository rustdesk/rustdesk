use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use crossbeam_queue::ArrayQueue;
use hbb_common::{
    bytes::Bytes,
    message_proto::{EncodedVideoFrame, EncodedVideoFrames, VideoFrame},
};
use std::sync::Arc;
use std::thread;

/// I. ArrayQueue (video queue client-side) benchmarks.
///
/// Simulates the client video queue from io_loop.rs:2318.
/// ArrayQueue<VideoFrame> with capacity 120 — the ring buffer between
/// network reception and the decoder thread.

const QUEUE_CAP: usize = 120;
const PAYLOAD_SIZE: usize = 30_000; // typical VP9 frame

fn make_video_frame(pts: i64) -> VideoFrame {
    let mut evf = EncodedVideoFrame::new();
    evf.data = Bytes::from(vec![0u8; PAYLOAD_SIZE]);
    evf.key = pts == 0;
    evf.pts = pts;

    let mut evfs = EncodedVideoFrames::new();
    evfs.frames.push(evf);

    let mut vf = VideoFrame::new();
    vf.set_vp9s(evfs);
    vf.display = 0;
    vf
}

// ---------------------------------------------------------------------------
// Push 120 VideoFrames
// ---------------------------------------------------------------------------

fn bench_push(c: &mut Criterion) {
    let mut group = c.benchmark_group("video_queue_push");
    let frames: Vec<VideoFrame> = (0..QUEUE_CAP as i64).map(make_video_frame).collect();

    group.throughput(Throughput::Elements(QUEUE_CAP as u64));
    group.bench_function(BenchmarkId::from_parameter("120_frames"), |b| {
        b.iter(|| {
            let q = ArrayQueue::new(QUEUE_CAP);
            for f in &frames {
                let _ = q.push(black_box(f.clone()));
            }
        });
    });
    group.finish();
}

// ---------------------------------------------------------------------------
// Pop 120 VideoFrames
// ---------------------------------------------------------------------------

fn bench_pop(c: &mut Criterion) {
    let mut group = c.benchmark_group("video_queue_pop");

    group.throughput(Throughput::Elements(QUEUE_CAP as u64));
    group.bench_function(BenchmarkId::from_parameter("120_frames"), |b| {
        b.iter_with_setup(
            || {
                let q = ArrayQueue::new(QUEUE_CAP);
                for i in 0..QUEUE_CAP as i64 {
                    let _ = q.push(make_video_frame(i));
                }
                q
            },
            |q| {
                while let Some(f) = q.pop() {
                    black_box(f);
                }
            },
        );
    });
    group.finish();
}

// ---------------------------------------------------------------------------
// force_push when full (drop oldest + push)
// ---------------------------------------------------------------------------

fn bench_force_push(c: &mut Criterion) {
    let mut group = c.benchmark_group("video_queue_force_push");

    group.throughput(Throughput::Elements(1));
    group.bench_function(BenchmarkId::from_parameter("full_queue"), |b| {
        let q = ArrayQueue::new(QUEUE_CAP);
        for i in 0..QUEUE_CAP as i64 {
            let _ = q.push(make_video_frame(i));
        }
        b.iter(|| {
            // Real code: io_loop.rs:1310 uses video_queue.force_push(vf)
            black_box(q.force_push(black_box(make_video_frame(999))))
        });
    });
    group.finish();
}

// ---------------------------------------------------------------------------
// Producer-consumer: 1 producer thread, 1 consumer thread, 1000 frames
// ---------------------------------------------------------------------------

fn bench_producer_consumer(c: &mut Criterion) {
    let mut group = c.benchmark_group("video_queue_producer_consumer");
    group.measurement_time(std::time::Duration::from_secs(10));

    let n_frames = 1000;
    group.throughput(Throughput::Elements(n_frames));
    group.bench_function(BenchmarkId::from_parameter("1000_frames"), |b| {
        b.iter(|| {
            let q = Arc::new(ArrayQueue::new(QUEUE_CAP));

            let q_prod = q.clone();
            let producer = thread::spawn(move || {
                for i in 0..n_frames as i64 {
                    // Real code uses force_push (drops oldest if full)
                    q_prod.force_push(make_video_frame(i));
                }
            });

            let q_cons = q.clone();
            let consumer = thread::spawn(move || {
                let mut consumed = 0u64;
                while consumed < n_frames {
                    if let Some(f) = q_cons.pop() {
                        black_box(f);
                        consumed += 1;
                    } else {
                        thread::yield_now();
                    }
                }
            });

            producer.join().unwrap();
            consumer.join().unwrap();
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_push,
    bench_pop,
    bench_force_push,
    bench_producer_consumer,
);
criterion_main!(benches);
