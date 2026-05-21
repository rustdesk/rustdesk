use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Barrier, Mutex, RwLock};
use std::thread;

/// H. Mutex contention benchmarks (simulated patterns).
///
/// Reproduces the locking patterns from video_service.rs hot loop.
/// Multi-threaded benchmarks use persistent threads + Barrier to avoid
/// measuring thread::spawn/join overhead (~40-200µs per iteration).

const ENTRIES: usize = 5;
const OPS_PER_THREAD: usize = 1000;
const NUM_THREADS: usize = 4;

fn make_map() -> HashMap<i32, u64> {
    (0..ENTRIES as i32).map(|i| (i, i as u64 * 100)).collect()
}

// ---------------------------------------------------------------------------
// Mutex: single-thread lock/read/unlock
// ---------------------------------------------------------------------------

fn bench_mutex_single_thread(c: &mut Criterion) {
    let mut group = c.benchmark_group("mutex_1thread");
    let m = Mutex::new(make_map());

    group.throughput(Throughput::Elements(1));
    group.bench_function("lock_read_unlock", |b| {
        b.iter(|| {
            let guard = m.lock().unwrap();
            let val = guard.get(&0).copied();
            drop(guard);
            black_box(val)
        });
    });
    group.finish();
}

// ---------------------------------------------------------------------------
// Mutex vs RwLock: 4 reader threads concurrent (persistent threads)
// ---------------------------------------------------------------------------

fn bench_mutex_vs_rwlock_readers(c: &mut Criterion) {
    let mut group = c.benchmark_group("lock_4readers");
    group.measurement_time(std::time::Duration::from_secs(10));

    let total_ops = NUM_THREADS * OPS_PER_THREAD;
    group.throughput(Throughput::Elements(total_ops as u64));

    // Mutex
    {
        let m = Arc::new(Mutex::new(make_map()));
        let barrier = Arc::new(Barrier::new(NUM_THREADS + 1));
        let stop = Arc::new(AtomicBool::new(false));

        let handles: Vec<_> = (0..NUM_THREADS)
            .map(|_| {
                let m = m.clone();
                let barrier = barrier.clone();
                let stop = stop.clone();
                thread::spawn(move || loop {
                    barrier.wait();
                    if stop.load(Ordering::Relaxed) {
                        return;
                    }
                    for _ in 0..OPS_PER_THREAD {
                        let guard = m.lock().unwrap();
                        black_box(guard.get(&0).copied());
                    }
                    barrier.wait();
                })
            })
            .collect();

        group.bench_function(BenchmarkId::from_parameter("mutex"), |b| {
            b.iter(|| {
                barrier.wait(); // start workers
                barrier.wait(); // wait for completion
            });
        });

        stop.store(true, Ordering::Relaxed);
        barrier.wait();
        for h in handles {
            h.join().unwrap();
        }
    }

    // RwLock
    {
        let m = Arc::new(RwLock::new(make_map()));
        let barrier = Arc::new(Barrier::new(NUM_THREADS + 1));
        let stop = Arc::new(AtomicBool::new(false));

        let handles: Vec<_> = (0..NUM_THREADS)
            .map(|_| {
                let m = m.clone();
                let barrier = barrier.clone();
                let stop = stop.clone();
                thread::spawn(move || loop {
                    barrier.wait();
                    if stop.load(Ordering::Relaxed) {
                        return;
                    }
                    for _ in 0..OPS_PER_THREAD {
                        let guard = m.read().unwrap();
                        black_box(guard.get(&0).copied());
                    }
                    barrier.wait();
                })
            })
            .collect();

        group.bench_function(BenchmarkId::from_parameter("rwlock"), |b| {
            b.iter(|| {
                barrier.wait();
                barrier.wait();
            });
        });

        stop.store(true, Ordering::Relaxed);
        barrier.wait();
        for h in handles {
            h.join().unwrap();
        }
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Mutex: 4 writer threads concurrent (persistent threads)
// ---------------------------------------------------------------------------

fn bench_mutex_writers(c: &mut Criterion) {
    let mut group = c.benchmark_group("mutex_4writers");
    group.measurement_time(std::time::Duration::from_secs(10));

    let total_ops = NUM_THREADS * OPS_PER_THREAD;
    group.throughput(Throughput::Elements(total_ops as u64));

    let m = Arc::new(Mutex::new(make_map()));
    let barrier = Arc::new(Barrier::new(NUM_THREADS + 1));
    let stop = Arc::new(AtomicBool::new(false));

    let handles: Vec<_> = (0..NUM_THREADS)
        .map(|t| {
            let m = m.clone();
            let barrier = barrier.clone();
            let stop = stop.clone();
            thread::spawn(move || {
                let mut i = 0u64;
                loop {
                    barrier.wait();
                    if stop.load(Ordering::Relaxed) {
                        return;
                    }
                    for _ in 0..OPS_PER_THREAD {
                        let mut guard = m.lock().unwrap();
                        guard.insert(0, t as u64 * 1000 + i);
                        i += 1;
                    }
                    barrier.wait();
                }
            })
        })
        .collect();

    group.bench_function("lock_write_unlock", |b| {
        b.iter(|| {
            barrier.wait();
            barrier.wait();
        });
    });

    stop.store(true, Ordering::Relaxed);
    barrier.wait();
    for h in handles {
        h.join().unwrap();
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Atomic vs Mutex for single u32 (like VIDEO_QOS.fps)
// ---------------------------------------------------------------------------

fn bench_atomic_vs_mutex_u32(c: &mut Criterion) {
    let mut group = c.benchmark_group("atomic_vs_mutex_u32");

    // AtomicU32
    {
        let v = AtomicU32::new(30);
        group.throughput(Throughput::Elements(1));
        group.bench_function(BenchmarkId::from_parameter("atomic_read"), |b| {
            b.iter(|| black_box(v.load(Ordering::Relaxed)));
        });

        group.bench_function(BenchmarkId::from_parameter("atomic_write"), |b| {
            let mut val = 0u32;
            b.iter(|| {
                v.store(black_box(val), Ordering::Relaxed);
                val = val.wrapping_add(1);
            });
        });
    }

    // Mutex<u32>
    {
        let v = Mutex::new(30u32);
        group.bench_function(BenchmarkId::from_parameter("mutex_read"), |b| {
            b.iter(|| {
                let guard = v.lock().unwrap();
                black_box(*guard)
            });
        });

        group.bench_function(BenchmarkId::from_parameter("mutex_write"), |b| {
            let mut val = 0u32;
            b.iter(|| {
                let mut guard = v.lock().unwrap();
                *guard = black_box(val);
                val = val.wrapping_add(1);
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_mutex_single_thread,
    bench_mutex_vs_rwlock_readers,
    bench_mutex_writers,
    bench_atomic_vs_mutex_u32,
);
criterion_main!(benches);
