mod common;

use common::{pre_encode_av1, pre_encode_vpx};
use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};
use hbb_common::{
    bytes::Bytes,
    message_proto::{video_frame, Chroma, EncodedVideoFrame, EncodedVideoFrames},
};
use scrap::{
    codec::Decoder, CodecFormat, ImageFormat, ImageRgb, ImageTexture, VpxDecoder,
    VpxDecoderConfig, VpxVideoCodecId,
};
use std::time::Duration;

/// C. Video decode benchmarks.
///
/// Calls the real `Decoder::handle_video_frame()` — the exact function used
/// by the client-side VideoHandler. This includes codec dispatch, the
/// "keep only last frame" pattern, and YUV→RGB conversion.
/// See libs/scrap/src/common/codec.rs:631.

const W: usize = 1920;
const H: usize = 1080;

/// Build a `video_frame::Union` from pre-encoded data, ready for handle_video_frame.
fn make_union_vp9(frames: &[common::EncodedFrame]) -> Vec<video_frame::Union> {
    frames
        .iter()
        .map(|f| {
            let mut evf = EncodedVideoFrame::new();
            evf.data = Bytes::from(f.data.clone());
            evf.key = f.key;
            evf.pts = f.pts;
            let mut evfs = EncodedVideoFrames::new();
            evfs.frames.push(evf);
            video_frame::Union::Vp9s(evfs)
        })
        .collect()
}

fn make_union_vp8(frames: &[common::EncodedFrame]) -> Vec<video_frame::Union> {
    frames
        .iter()
        .map(|f| {
            let mut evf = EncodedVideoFrame::new();
            evf.data = Bytes::from(f.data.clone());
            evf.key = f.key;
            evf.pts = f.pts;
            let mut evfs = EncodedVideoFrames::new();
            evfs.frames.push(evf);
            video_frame::Union::Vp8s(evfs)
        })
        .collect()
}

fn make_union_av1(frames: &[common::EncodedFrame]) -> Vec<video_frame::Union> {
    frames
        .iter()
        .map(|f| {
            let mut evf = EncodedVideoFrame::new();
            evf.data = Bytes::from(f.data.clone());
            evf.key = f.key;
            evf.pts = f.pts;
            let mut evfs = EncodedVideoFrames::new();
            evfs.frames.push(evf);
            video_frame::Union::Av1s(evfs)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Single-frame decode (VP8, VP9, AV1) — via Decoder::handle_video_frame
// ---------------------------------------------------------------------------

fn bench_decode_single(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode_single");

    // VP8
    {
        let encoded = pre_encode_vpx(VpxVideoCodecId::VP8, W, H, 1.0, 30);
        let unions = make_union_vp8(&encoded);
        let mut decoder = Decoder::new(CodecFormat::VP8, None);
        let mut rgb = ImageRgb::new(ImageFormat::ARGB, 1);
        let mut texture = ImageTexture::default();
        let mut pixelbuffer = true;
        let mut chroma: Option<Chroma> = None;

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::from_parameter("vp8_1080p"), &(), |b, _| {
            let mut idx = 0;
            b.iter(|| {
                let union = &unions[idx % unions.len()];
                decoder.handle_video_frame(
                    black_box(union),
                    &mut rgb,
                    &mut texture,
                    &mut pixelbuffer,
                    &mut chroma,
                ).expect("decode failed");
                idx += 1;
            });
        });
    }

    // VP9
    {
        let encoded = pre_encode_vpx(VpxVideoCodecId::VP9, W, H, 1.0, 30);
        let unions = make_union_vp9(&encoded);
        let mut decoder = Decoder::new(CodecFormat::VP9, None);
        let mut rgb = ImageRgb::new(ImageFormat::ARGB, 1);
        let mut texture = ImageTexture::default();
        let mut pixelbuffer = true;
        let mut chroma: Option<Chroma> = None;

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::from_parameter("vp9_1080p"), &(), |b, _| {
            let mut idx = 0;
            b.iter(|| {
                let union = &unions[idx % unions.len()];
                decoder.handle_video_frame(
                    black_box(union),
                    &mut rgb,
                    &mut texture,
                    &mut pixelbuffer,
                    &mut chroma,
                ).expect("decode failed");
                idx += 1;
            });
        });
    }

    // AV1
    {
        let encoded = pre_encode_av1(W, H, 1.0, 30);
        let unions = make_union_av1(&encoded);
        let mut decoder = Decoder::new(CodecFormat::AV1, None);
        let mut rgb = ImageRgb::new(ImageFormat::ARGB, 1);
        let mut texture = ImageTexture::default();
        let mut pixelbuffer = true;
        let mut chroma: Option<Chroma> = None;

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::from_parameter("av1_1080p"), &(), |b, _| {
            let mut idx = 0;
            b.iter(|| {
                let union = &unions[idx % unions.len()];
                decoder.handle_video_frame(
                    black_box(union),
                    &mut rgb,
                    &mut texture,
                    &mut pixelbuffer,
                    &mut chroma,
                ).expect("decode failed");
                idx += 1;
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Decode with align=1 vs align=64 (macOS texture rendering uses 64)
// ---------------------------------------------------------------------------

fn bench_vp9_decode_alignment(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode_alignment");

    let encoded = pre_encode_vpx(VpxVideoCodecId::VP9, W, H, 1.0, 30);
    let unions = make_union_vp9(&encoded);

    for (label, align) in [("align_1", 1usize), ("align_64", 64)] {
        let mut decoder = Decoder::new(CodecFormat::VP9, None);
        let mut rgb = ImageRgb::new(ImageFormat::ARGB, align);
        let mut texture = ImageTexture::default();
        let mut pixelbuffer = true;
        let mut chroma: Option<Chroma> = None;

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::from_parameter(label), &(), |b, _| {
            let mut idx = 0;
            b.iter(|| {
                let union = &unions[idx % unions.len()];
                decoder
                    .handle_video_frame(
                        black_box(union),
                        &mut rgb,
                        &mut texture,
                        &mut pixelbuffer,
                        &mut chroma,
                    )
                    .expect("decode failed");
                idx += 1;
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Sequence decode: 100 frames (VP9)
// ---------------------------------------------------------------------------

fn bench_vp9_decode_sequence(c: &mut Criterion) {
    let mut group = c.benchmark_group("vp9_decode_sequence");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(20));

    let encoded = pre_encode_vpx(VpxVideoCodecId::VP9, W, H, 1.0, 100);
    let unions = make_union_vp9(&encoded);
    let mut decoder = Decoder::new(CodecFormat::VP9, None);
    let mut rgb = ImageRgb::new(ImageFormat::ARGB, 1);
    let mut texture = ImageTexture::default();
    let mut pixelbuffer = true;
    let mut chroma: Option<Chroma> = None;

    group.throughput(Throughput::Elements(100));
    group.bench_function(BenchmarkId::from_parameter("100frames_1080p"), |b| {
        b.iter(|| {
            for union in &unions {
                decoder.handle_video_frame(
                    black_box(union),
                    &mut rgb,
                    &mut texture,
                    &mut pixelbuffer,
                    &mut chroma,
                ).expect("decode failed");
            }
        });
    });
    group.finish();
}

// ---------------------------------------------------------------------------
// 4K decode (VP9)
// ---------------------------------------------------------------------------

fn bench_vp9_decode_4k(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode_4k");
    group.measurement_time(Duration::from_secs(15));

    let (w4k, h4k) = (3840, 2160);
    let encoded = pre_encode_vpx(VpxVideoCodecId::VP9, w4k, h4k, 1.0, 10);
    let unions = make_union_vp9(&encoded);
    let mut decoder = Decoder::new(CodecFormat::VP9, None);
    let mut rgb = ImageRgb::new(ImageFormat::ARGB, 1);
    let mut texture = ImageTexture::default();
    let mut pixelbuffer = true;
    let mut chroma: Option<Chroma> = None;

    group.throughput(Throughput::Elements(1));
    group.bench_function(BenchmarkId::from_parameter("vp9"), |b| {
        let mut idx = 0;
        b.iter(|| {
            let union = &unions[idx % unions.len()];
            decoder.handle_video_frame(
                black_box(union),
                &mut rgb,
                &mut texture,
                &mut pixelbuffer,
                &mut chroma,
            );
            idx += 1;
        });
    });
    group.finish();
}

// ---------------------------------------------------------------------------
// Cold-start: decoder creation cost (reconnection / codec switch)
// ---------------------------------------------------------------------------

fn bench_decoder_cold_start(c: &mut Criterion) {
    let mut group = c.benchmark_group("decoder_cold_start");

    for (label, format) in [
        ("vp8_1080p", CodecFormat::VP8),
        ("vp9_1080p", CodecFormat::VP9),
        ("av1_1080p", CodecFormat::AV1),
    ] {
        group.throughput(Throughput::Elements(1));
        group.bench_function(BenchmarkId::from_parameter(label), |b| {
            b.iter(|| {
                black_box(Decoder::new(format, None));
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Decode-only: VP9 without YUV→RGB conversion (isolates codec cost)
// ---------------------------------------------------------------------------

fn bench_vp9_decode_raw(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode_raw");

    let encoded = pre_encode_vpx(VpxVideoCodecId::VP9, W, H, 1.0, 30);
    let mut decoder =
        VpxDecoder::new(VpxDecoderConfig { codec: VpxVideoCodecId::VP9 }).unwrap();

    group.throughput(Throughput::Elements(1));
    group.bench_function(BenchmarkId::from_parameter("vp9_1080p"), |b| {
        let mut idx = 0;
        b.iter(|| {
            let frame = &encoded[idx % encoded.len()];
            for img in decoder.decode(black_box(&frame.data)).unwrap() {
                black_box(&img);
            }
            for img in decoder.flush().unwrap() {
                black_box(&img);
            }
            idx += 1;
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_decode_single,
    bench_vp9_decode_alignment,
    bench_decoder_cold_start,
    bench_vp9_decode_raw,
    bench_vp9_decode_sequence,
    bench_vp9_decode_4k,
);
criterion_main!(benches);
