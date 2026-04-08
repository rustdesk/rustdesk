use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;

/// H. Mutex contention benchmarks (simulated patterns).
///
/// Reproduces the locking patterns from video_service.rs hot loop:
/// - HashMap behind Mutex (current pattern for subscribers/connections)
/// - Comparison with RwLock and Atomic alternatives
///
/// No RustDesk-specific types needed — pure synchronization primitives.

const ENTRIES: usize = 5;

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
// Mutex vs RwLock: 4 reader threads concurrent
// ---------------------------------------------------------------------------

fn bench_mutex_vs_rwlock_readers(c: &mut Criterion) {
    let mut group = c.benchmark_group("lock_4readers");
    group.measurement_time(std::time::Duration::from_secs(10));

    // Mutex
    {
        let m = Arc::new(Mutex::new(make_map()));
        group.throughput(Throughput::Elements(1));
        group.bench_function(BenchmarkId::from_parameter("mutex"), |b| {
            b.iter(|| {
                let mut handles = Vec::new();
                for _ in 0..4 {
                    let m = m.clone();
                    handles.push(thread::spawn(move || {
                        for _ in 0..1000 {
                            let guard = m.lock().unwrap();
                            black_box(guard.get(&0).copied());
                        }
                    }));
                }
                for h in handles {
                    h.join().unwrap();
                }
            });
        });
    }

    // RwLock
    {
        let m = Arc::new(RwLock::new(make_map()));
        group.throughput(Throughput::Elements(1));
        group.bench_function(BenchmarkId::from_parameter("rwlock"), |b| {
            b.iter(|| {
                let mut handles = Vec::new();
                for _ in 0..4 {
                    let m = m.clone();
                    handles.push(thread::spawn(move || {
                        for _ in 0..1000 {
                            let guard = m.read().unwrap();
                            black_box(guard.get(&0).copied());
                        }
                    }));
                }
                for h in handles {
                    h.join().unwrap();
                }
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Mutex: 4 writer threads concurrent
// ---------------------------------------------------------------------------

fn bench_mutex_writers(c: &mut Criterion) {
    let mut group = c.benchmark_group("mutex_4writers");
    group.measurement_time(std::time::Duration::from_secs(10));

    let m = Arc::new(Mutex::new(make_map()));
    group.throughput(Throughput::Elements(1));
    group.bench_function("lock_write_unlock", |b| {
        b.iter(|| {
            let mut handles = Vec::new();
            for t in 0..4u64 {
                let m = m.clone();
                handles.push(thread::spawn(move || {
                    for i in 0..1000 {
                        let mut guard = m.lock().unwrap();
                        guard.insert(0, t * 1000 + i);
                    }
                }));
            }
            for h in handles {
                h.join().unwrap();
            }
        });
    });
    group.finish();
}

// ---------------------------------------------------------------------------
// Atomic vs Mutex for single u32 (like VIDEO_QOS.fps)
// ---------------------------------------------------------------------------

fn bench_atomic_vs_mutex_u32(c: &mut Criterion) {
    let mut group = c.benchmark_group("atomic_vs_mutex_u32");

    // AtomicU32
    {
        let v = Arc::new(AtomicU32::new(30));
        group.throughput(Throughput::Elements(1));
        group.bench_function(BenchmarkId::from_parameter("atomic_read"), |b| {
            b.iter(|| {
                black_box(v.load(Ordering::Relaxed))
            });
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
        let v = Arc::new(Mutex::new(30u32));
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
