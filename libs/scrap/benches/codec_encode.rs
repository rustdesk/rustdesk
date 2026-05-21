mod common;

use common::make_i420;
use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};
use scrap::{
    aom::{AomEncoder, AomEncoderConfig},
    codec::{EncoderApi, EncoderCfg},
    EncodeInput, VpxEncoder, VpxEncoderConfig, VpxVideoCodecId,
};
use std::time::Duration;

/// B. Video encode benchmarks.
///
/// Calls the real `EncoderApi::encode_to_message()` — the same function used
/// by video_service.rs handle_one_frame().
///
/// Single-frame benchmarks alternate between multiple distinct frames to avoid
/// the encoder's rate controller dropping frames on identical input
/// (rc_dropframe_thresh=25 causes Err("no valid frame") on static content).

const W: usize = 1920;
const H: usize = 1080;
const NUM_FRAMES: usize = 8;

/// Pre-generate a pool of distinct YUV frames for realistic encode benchmarks.
fn make_frame_pool(w: usize, h: usize, n: usize) -> Vec<Vec<u8>> {
    (0..n).map(|i| make_i420(w, h, i * 37).0).collect()
}

// ---------------------------------------------------------------------------
// Single-frame encode with varied input (VP8, VP9, AV1)
// ---------------------------------------------------------------------------

fn bench_vpx_encode_single(c: &mut Criterion) {
    let mut group = c.benchmark_group("encode_single");

    for codec in [VpxVideoCodecId::VP8, VpxVideoCodecId::VP9] {
        let label = match codec {
            VpxVideoCodecId::VP8 => "vp8_1080p",
            VpxVideoCodecId::VP9 => "vp9_1080p",
        };
        let cfg = EncoderCfg::VPX(VpxEncoderConfig {
            width: W as _,
            height: H as _,
            quality: 1.0,
            codec,
            keyframe_interval: None,
        });
        let mut encoder = VpxEncoder::new(cfg, false).unwrap();
        let pool = make_frame_pool(W, H, NUM_FRAMES);

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::from_parameter(label), &(), |b, _| {
            let mut pts = 0i64;
            b.iter(|| {
                let yuv = &pool[pts as usize % pool.len()];
                let input = EncodeInput::YUV(black_box(yuv));
                drop(encoder.encode_to_message(input, pts));
                pts += 1;
            });
        });
    }

    // AV1
    {
        let cfg = EncoderCfg::AOM(AomEncoderConfig {
            width: W as _,
            height: H as _,
            quality: 1.0,
            keyframe_interval: None,
        });
        let mut encoder = AomEncoder::new(cfg, false).unwrap();
        let pool = make_frame_pool(W, H, NUM_FRAMES);

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::from_parameter("av1_1080p"), &(), |b, _| {
            let mut pts = 0i64;
            b.iter(|| {
                let yuv = &pool[pts as usize % pool.len()];
                let input = EncodeInput::YUV(black_box(yuv));
                drop(encoder.encode_to_message(input, pts));
                pts += 1;
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Keyframe-only encode (worst case — every frame is a keyframe)
// ---------------------------------------------------------------------------

fn bench_vpx_encode_keyframe(c: &mut Criterion) {
    let mut group = c.benchmark_group("encode_keyframe");

    for codec in [VpxVideoCodecId::VP8, VpxVideoCodecId::VP9] {
        let label = match codec {
            VpxVideoCodecId::VP8 => "vp8_1080p",
            VpxVideoCodecId::VP9 => "vp9_1080p",
        };
        let cfg = EncoderCfg::VPX(VpxEncoderConfig {
            width: W as _,
            height: H as _,
            quality: 1.0,
            codec,
            keyframe_interval: Some(1), // force keyframe every frame
        });
        let mut encoder = VpxEncoder::new(cfg, false).unwrap();
        let pool = make_frame_pool(W, H, NUM_FRAMES);

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::from_parameter(label), &(), |b, _| {
            let mut pts = 0i64;
            b.iter(|| {
                let yuv = &pool[pts as usize % pool.len()];
                let input = EncodeInput::YUV(black_box(yuv));
                drop(encoder.encode_to_message(input, pts));
                pts += 1;
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// 4K encode with varied input
// ---------------------------------------------------------------------------

fn bench_encode_4k(c: &mut Criterion) {
    let mut group = c.benchmark_group("encode_4k");
    group.measurement_time(Duration::from_secs(15));
    let (w4k, h4k) = (3840, 2160);

    for codec in [VpxVideoCodecId::VP8, VpxVideoCodecId::VP9] {
        let label = match codec {
            VpxVideoCodecId::VP8 => "vp8",
            VpxVideoCodecId::VP9 => "vp9",
        };
        let cfg = EncoderCfg::VPX(VpxEncoderConfig {
            width: w4k as _,
            height: h4k as _,
            quality: 1.0,
            codec,
            keyframe_interval: None,
        });
        let mut encoder = VpxEncoder::new(cfg, false).unwrap();
        let pool = make_frame_pool(w4k, h4k, 4); // fewer frames for 4K (memory)

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::from_parameter(label), &(), |b, _| {
            let mut pts = 0i64;
            b.iter(|| {
                let yuv = &pool[pts as usize % pool.len()];
                let input = EncodeInput::YUV(black_box(yuv));
                drop(encoder.encode_to_message(input, pts));
                pts += 1;
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Sequence encode: 100 static frames (simulates idle screen)
// ---------------------------------------------------------------------------

fn bench_vp9_encode_sequence_static(c: &mut Criterion) {
    let mut group = c.benchmark_group("vp9_encode_seq_static");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(30));

    let cfg = EncoderCfg::VPX(VpxEncoderConfig {
        width: W as _,
        height: H as _,
        quality: 1.0,
        codec: VpxVideoCodecId::VP9,
        keyframe_interval: None,
    });
    let mut encoder = VpxEncoder::new(cfg, false).unwrap();
    let (yuv, _) = make_i420(W, H, 0);

    group.throughput(Throughput::Elements(100));
    group.bench_function(BenchmarkId::from_parameter("100frames_1080p"), |b| {
        b.iter(|| {
            for i in 0..100 {
                let input = EncodeInput::YUV(black_box(&yuv));
                drop(encoder.encode_to_message(input, i));
            }
        });
    });
    group.finish();
}

// ---------------------------------------------------------------------------
// Sequence encode: 100 varied frames (simulates scroll / movement)
// ---------------------------------------------------------------------------

fn bench_vp9_encode_sequence_movement(c: &mut Criterion) {
    let mut group = c.benchmark_group("vp9_encode_seq_movement");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(30));

    let cfg = EncoderCfg::VPX(VpxEncoderConfig {
        width: W as _,
        height: H as _,
        quality: 1.0,
        codec: VpxVideoCodecId::VP9,
        keyframe_interval: None,
    });
    let mut encoder = VpxEncoder::new(cfg, false).unwrap();
    let frames: Vec<Vec<u8>> = (0..100).map(|i| make_i420(W, H, i * 5).0).collect();

    group.throughput(Throughput::Elements(100));
    group.bench_function(BenchmarkId::from_parameter("100frames_1080p"), |b| {
        b.iter(|| {
            for (i, yuv) in frames.iter().enumerate() {
                let input = EncodeInput::YUV(black_box(yuv));
                drop(encoder.encode_to_message(input, i as i64));
            }
        });
    });
    group.finish();
}

// ---------------------------------------------------------------------------
// Quality ratio impact (VP9 1080p, varied input)
// ---------------------------------------------------------------------------

fn bench_vp9_encode_quality(c: &mut Criterion) {
    let mut group = c.benchmark_group("vp9_encode_quality");
    group.measurement_time(Duration::from_secs(10));

    let qualities: &[(&str, f32)] = &[
        ("q0.5_speed", 0.5),
        ("q1.0_balanced", 1.0),
        ("q2.0_best", 2.0),
    ];
    let pool = make_frame_pool(W, H, NUM_FRAMES);

    for (label, quality) in qualities {
        let cfg = EncoderCfg::VPX(VpxEncoderConfig {
            width: W as _,
            height: H as _,
            quality: *quality,
            codec: VpxVideoCodecId::VP9,
            keyframe_interval: None,
        });
        let mut encoder = VpxEncoder::new(cfg, false).unwrap();

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::from_parameter(*label), &(), |b, _| {
            let mut pts = 0i64;
            b.iter(|| {
                let yuv = &pool[pts as usize % pool.len()];
                let input = EncodeInput::YUV(black_box(yuv));
                drop(encoder.encode_to_message(input, pts));
                pts += 1;
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Cold-start: encoder creation cost (reconnection / codec switch)
// ---------------------------------------------------------------------------

fn bench_encoder_cold_start(c: &mut Criterion) {
    let mut group = c.benchmark_group("encoder_cold_start");

    for codec in [VpxVideoCodecId::VP8, VpxVideoCodecId::VP9] {
        let label = match codec {
            VpxVideoCodecId::VP8 => "vp8_1080p",
            VpxVideoCodecId::VP9 => "vp9_1080p",
        };
        group.throughput(Throughput::Elements(1));
        group.bench_function(BenchmarkId::from_parameter(label), |b| {
            b.iter(|| {
                let cfg = EncoderCfg::VPX(VpxEncoderConfig {
                    width: W as _,
                    height: H as _,
                    quality: 1.0,
                    codec,
                    keyframe_interval: None,
                });
                black_box(VpxEncoder::new(cfg, false).unwrap());
            });
        });
    }

    {
        group.bench_function(BenchmarkId::from_parameter("av1_1080p"), |b| {
            b.iter(|| {
                let cfg = EncoderCfg::AOM(AomEncoderConfig {
                    width: W as _,
                    height: H as _,
                    quality: 1.0,
                    keyframe_interval: None,
                });
                black_box(AomEncoder::new(cfg, false).unwrap());
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_vpx_encode_single,
    bench_vpx_encode_keyframe,
    bench_encode_4k,
    bench_encoder_cold_start,
    bench_vp9_encode_sequence_static,
    bench_vp9_encode_sequence_movement,
    bench_vp9_encode_quality,
);
criterion_main!(benches);
