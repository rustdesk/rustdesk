mod common;

use common::{
    i420_layout, i444_layout, make_bgra, make_bgra_strided, nv12_layout, Pattern, RESOLUTIONS,
};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

/// A. Color space conversion benchmarks (server-side hot path).
///
/// Measures BGRA → I420/NV12/I444 via libyuv FFI at multiple resolutions
/// and with different input patterns (solid, gradient, random).

// ---------------------------------------------------------------------------
// BGRA → I420
// ---------------------------------------------------------------------------

fn bench_bgra_to_i420(c: &mut Criterion) {
    let mut group = c.benchmark_group("bgra_to_i420");

    for &(w, h, label) in RESOLUTIONS {
        let (src, src_stride) = make_bgra(w, h, &Pattern::Gradient);
        let layout = i420_layout(w, h);

        group.throughput(Throughput::Bytes((w * h * 4) as u64));
        group.bench_with_input(BenchmarkId::from_parameter(label), &(), |b, _| {
            let mut dst = vec![0u8; layout.total];
            b.iter(|| unsafe {
                let dst_y = dst.as_mut_ptr();
                let dst_u = dst_y.add(layout.y_size);
                let dst_v = dst_u.add(layout.uv_size);
                scrap::ARGBToI420(
                    black_box(src.as_ptr()),
                    src_stride as _,
                    dst_y,
                    layout.stride_y as _,
                    dst_u,
                    layout.stride_uv as _,
                    dst_v,
                    layout.stride_uv as _,
                    w as _,
                    h as _,
                );
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// BGRA → NV12
// ---------------------------------------------------------------------------

fn bench_bgra_to_nv12(c: &mut Criterion) {
    let mut group = c.benchmark_group("bgra_to_nv12");

    for &(w, h, label) in RESOLUTIONS {
        let (src, src_stride) = make_bgra(w, h, &Pattern::Gradient);
        let layout = nv12_layout(w, h);

        group.throughput(Throughput::Bytes((w * h * 4) as u64));
        group.bench_with_input(BenchmarkId::from_parameter(label), &(), |b, _| {
            let mut dst = vec![0u8; layout.total];
            b.iter(|| unsafe {
                let dst_y = dst.as_mut_ptr();
                let dst_uv = dst_y.add(layout.y_size);
                scrap::ARGBToNV12(
                    black_box(src.as_ptr()),
                    src_stride as _,
                    dst_y,
                    layout.stride_y as _,
                    dst_uv,
                    layout.stride_uv as _,
                    w as _,
                    h as _,
                );
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// BGRA → I444
// ---------------------------------------------------------------------------

fn bench_bgra_to_i444(c: &mut Criterion) {
    let mut group = c.benchmark_group("bgra_to_i444");

    for &(w, h, label) in RESOLUTIONS {
        let (src, src_stride) = make_bgra(w, h, &Pattern::Gradient);
        let layout = i444_layout(w, h);

        group.throughput(Throughput::Bytes((w * h * 4) as u64));
        group.bench_with_input(BenchmarkId::from_parameter(label), &(), |b, _| {
            let mut dst = vec![0u8; layout.total];
            b.iter(|| unsafe {
                let dst_y = dst.as_mut_ptr();
                let dst_u = dst_y.add(layout.plane_size);
                let dst_v = dst_u.add(layout.plane_size);
                scrap::ARGBToI444(
                    black_box(src.as_ptr()),
                    src_stride as _,
                    dst_y,
                    layout.stride as _,
                    dst_u,
                    layout.stride as _,
                    dst_v,
                    layout.stride as _,
                    w as _,
                    h as _,
                );
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Input pattern impact (1080p BGRA → I420, solid vs gradient vs random)
// ---------------------------------------------------------------------------

fn bench_bgra_to_i420_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("bgra_to_i420_patterns");
    let (w, h) = (1920, 1080);
    let layout = i420_layout(w, h);

    let patterns: &[(&str, Pattern)] = &[
        ("solid", Pattern::Solid(128)),
        ("gradient", Pattern::Gradient),
        ("random", Pattern::Random(0xDEAD_BEEF)),
    ];

    group.throughput(Throughput::Bytes((w * h * 4) as u64));
    for (name, pat) in patterns {
        let (src, src_stride) = make_bgra(w, h, pat);
        group.bench_with_input(BenchmarkId::from_parameter(*name), &(), |b, _| {
            let mut dst = vec![0u8; layout.total];
            b.iter(|| unsafe {
                let dst_y = dst.as_mut_ptr();
                let dst_u = dst_y.add(layout.y_size);
                let dst_v = dst_u.add(layout.uv_size);
                scrap::ARGBToI420(
                    black_box(src.as_ptr()),
                    src_stride as _,
                    dst_y,
                    layout.stride_y as _,
                    dst_u,
                    layout.stride_uv as _,
                    dst_v,
                    layout.stride_uv as _,
                    w as _,
                    h as _,
                );
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Stride alignment impact (1080p BGRA → I420, aligned vs +64 padding)
// ---------------------------------------------------------------------------

fn bench_bgra_to_i420_stride(c: &mut Criterion) {
    let mut group = c.benchmark_group("bgra_to_i420_stride");
    let (w, h) = (1920, 1080);
    let layout = i420_layout(w, h);

    let strides: &[(&str, usize)] = &[
        ("aligned", w * 4),
        ("padded_64", w * 4 + 64),
        ("padded_128", w * 4 + 128),
    ];

    group.throughput(Throughput::Bytes((w * h * 4) as u64));
    for (name, stride) in strides {
        let src = make_bgra_strided(w, h, *stride, &Pattern::Gradient);
        group.bench_with_input(BenchmarkId::from_parameter(*name), &(), |b, _| {
            let mut dst = vec![0u8; layout.total];
            b.iter(|| unsafe {
                let dst_y = dst.as_mut_ptr();
                let dst_u = dst_y.add(layout.y_size);
                let dst_v = dst_u.add(layout.uv_size);
                scrap::ARGBToI420(
                    black_box(src.as_ptr()),
                    *stride as _,
                    dst_y,
                    layout.stride_y as _,
                    dst_u,
                    layout.stride_uv as _,
                    dst_v,
                    layout.stride_uv as _,
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
    bench_bgra_to_i420,
    bench_bgra_to_nv12,
    bench_bgra_to_i444,
    bench_bgra_to_i420_patterns,
    bench_bgra_to_i420_stride,
);
criterion_main!(benches);
