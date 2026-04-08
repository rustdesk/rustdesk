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
/// by video_service.rs handle_one_frame(). This ensures any change to the
/// encode path (flush behavior, frame creation, etc.) is reflected here.
///
/// Includes single-frame, sequence (static + movement), and quality variations.

const W: usize = 1920;
const H: usize = 1080;

// ---------------------------------------------------------------------------
// Single-frame encode (VP8, VP9, AV1)
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
        let (yuv, _) = make_i420(W, H, 0);

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::from_parameter(label), &(), |b, _| {
            let mut pts = 0i64;
            b.iter(|| {
                let input = EncodeInput::YUV(&yuv);
                // encode_to_message may return Err("no valid frame") when the codec drops a frame — this is normal
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
        let (yuv, _) = make_i420(W, H, 0);

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::from_parameter("av1_1080p"), &(), |b, _| {
            let mut pts = 0i64;
            b.iter(|| {
                let input = EncodeInput::YUV(&yuv);
                // encode_to_message may return Err("no valid frame") when the codec drops a frame — this is normal
drop(encoder.encode_to_message(input, pts));
                pts += 1;
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// 4K encode
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
        let (yuv, _) = make_i420(w4k, h4k, 0);

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::from_parameter(label), &(), |b, _| {
            let mut pts = 0i64;
            b.iter(|| {
                let input = EncodeInput::YUV(black_box(&yuv));
                // encode_to_message may return Err("no valid frame") when the codec drops a frame — this is normal
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

    // Pre-generate 100 frames with progressive shift
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
// Quality ratio impact (VP9 1080p)
// ---------------------------------------------------------------------------

fn bench_vp9_encode_quality(c: &mut Criterion) {
    let mut group = c.benchmark_group("vp9_encode_quality");
    group.measurement_time(Duration::from_secs(10));

    let qualities: &[(&str, f32)] = &[
        ("q0.5_speed", 0.5),
        ("q1.0_balanced", 1.0),
        ("q2.0_best", 2.0),
    ];
    let (yuv, _) = make_i420(W, H, 0);

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
                let input = EncodeInput::YUV(black_box(&yuv));
                // encode_to_message may return Err("no valid frame") when the codec drops a frame — this is normal
drop(encoder.encode_to_message(input, pts));
                pts += 1;
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_vpx_encode_single,
    bench_encode_4k,
    bench_vp9_encode_sequence_static,
    bench_vp9_encode_sequence_movement,
    bench_vp9_encode_quality,
);
criterion_main!(benches);
