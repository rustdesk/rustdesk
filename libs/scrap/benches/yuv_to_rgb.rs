mod common;

use common::{make_i420, make_i444, make_nv12, RESOLUTIONS};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

/// D. YUV → RGB conversion benchmarks (client-side decode output path).
///
/// Measures I420/NV12/I444 → ARGB/ABGR via libyuv FFI.
/// Corresponds to GoogleImage::to() in the client decoder.

// ---------------------------------------------------------------------------
// I420 → ARGB
// ---------------------------------------------------------------------------

fn bench_i420_to_argb(c: &mut Criterion) {
    let mut group = c.benchmark_group("i420_to_argb");

    for &(w, h, label) in RESOLUTIONS {
        let (frame, layout) = make_i420(w, h, 0);
        let dst_stride = w * 4;

        group.throughput(Throughput::Bytes((w * h * 4) as u64));
        group.bench_with_input(BenchmarkId::from_parameter(label), &(), |b, _| {
            let mut dst = vec![0u8; dst_stride * h];
            b.iter(|| unsafe {
                let y = frame.as_ptr();
                let u = y.add(layout.y_size);
                let v = u.add(layout.uv_size);
                scrap::I420ToARGB(
                    black_box(y),
                    layout.stride_y as _,
                    u,
                    layout.stride_uv as _,
                    v,
                    layout.stride_uv as _,
                    dst.as_mut_ptr(),
                    dst_stride as _,
                    w as _,
                    h as _,
                );
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// I420 → ABGR
// ---------------------------------------------------------------------------

fn bench_i420_to_abgr(c: &mut Criterion) {
    let mut group = c.benchmark_group("i420_to_abgr");

    for &(w, h, label) in RESOLUTIONS {
        let (frame, layout) = make_i420(w, h, 0);
        let dst_stride = w * 4;

        group.throughput(Throughput::Bytes((w * h * 4) as u64));
        group.bench_with_input(BenchmarkId::from_parameter(label), &(), |b, _| {
            let mut dst = vec![0u8; dst_stride * h];
            b.iter(|| unsafe {
                let y = frame.as_ptr();
                let u = y.add(layout.y_size);
                let v = u.add(layout.uv_size);
                scrap::I420ToABGR(
                    black_box(y),
                    layout.stride_y as _,
                    u,
                    layout.stride_uv as _,
                    v,
                    layout.stride_uv as _,
                    dst.as_mut_ptr(),
                    dst_stride as _,
                    w as _,
                    h as _,
                );
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// I444 → ARGB
// ---------------------------------------------------------------------------

fn bench_i444_to_argb(c: &mut Criterion) {
    let mut group = c.benchmark_group("i444_to_argb");

    for &(w, h, label) in RESOLUTIONS {
        let (frame, layout) = make_i444(w, h);
        let dst_stride = w * 4;

        group.throughput(Throughput::Bytes((w * h * 4) as u64));
        group.bench_with_input(BenchmarkId::from_parameter(label), &(), |b, _| {
            let mut dst = vec![0u8; dst_stride * h];
            b.iter(|| unsafe {
                let y = frame.as_ptr();
                let u = y.add(layout.plane_size);
                let v = u.add(layout.plane_size);
                scrap::I444ToARGB(
                    black_box(y),
                    layout.stride as _,
                    u,
                    layout.stride as _,
                    v,
                    layout.stride as _,
                    dst.as_mut_ptr(),
                    dst_stride as _,
                    w as _,
                    h as _,
                );
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// NV12 → ARGB
// ---------------------------------------------------------------------------

fn bench_nv12_to_argb(c: &mut Criterion) {
    let mut group = c.benchmark_group("nv12_to_argb");

    for &(w, h, label) in RESOLUTIONS {
        let (frame, layout) = make_nv12(w, h);
        let dst_stride = w * 4;

        group.throughput(Throughput::Bytes((w * h * 4) as u64));
        group.bench_with_input(BenchmarkId::from_parameter(label), &(), |b, _| {
            let mut dst = vec![0u8; dst_stride * h];
            b.iter(|| unsafe {
                let y = frame.as_ptr();
                let uv = y.add(layout.y_size);
                scrap::NV12ToARGB(
                    black_box(y),
                    layout.stride_y as _,
                    uv,
                    layout.stride_uv as _,
                    dst.as_mut_ptr(),
                    dst_stride as _,
                    w as _,
                    h as _,
                );
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// NV12 → ABGR
// ---------------------------------------------------------------------------

fn bench_nv12_to_abgr(c: &mut Criterion) {
    let mut group = c.benchmark_group("nv12_to_abgr");

    for &(w, h, label) in RESOLUTIONS {
        let (frame, layout) = make_nv12(w, h);
        let dst_stride = w * 4;

        group.throughput(Throughput::Bytes((w * h * 4) as u64));
        group.bench_with_input(BenchmarkId::from_parameter(label), &(), |b, _| {
            let mut dst = vec![0u8; dst_stride * h];
            b.iter(|| unsafe {
                let y = frame.as_ptr();
                let uv = y.add(layout.y_size);
                scrap::NV12ToABGR(
                    black_box(y),
                    layout.stride_y as _,
                    uv,
                    layout.stride_uv as _,
                    dst.as_mut_ptr(),
                    dst_stride as _,
                    w as _,
                    h as _,
                );
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_i420_to_argb,
    bench_i420_to_abgr,
    bench_i444_to_argb,
    bench_nv12_to_argb,
    bench_nv12_to_abgr,
);
criterion_main!(benches);
