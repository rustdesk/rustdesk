mod common;

use common::pre_encode_vpx;
use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};
use hbb_common::{
    bytes::Bytes,
    message_proto::{video_frame, Chroma, EncodedVideoFrame, EncodedVideoFrames, Message, VideoFrame},
    protobuf::Message as ProtoMessage,
};
use scrap::{
    codec::Decoder, CodecFormat, ImageFormat, ImageRgb, ImageTexture, VpxVideoCodecId,
};
use std::time::Duration;

/// K. Full decode pipeline benchmarks.
///
/// Protobuf deserialize → Decoder::handle_video_frame().
/// Uses the real Decoder::handle_video_frame() which includes codec dispatch,
/// the "keep only last frame" pattern, and YUV→RGB conversion.
/// This is the exact client-side path (see codec.rs:631).

fn make_serialized_messages(
    codec: VpxVideoCodecId,
    w: usize,
    h: usize,
    n: usize,
) -> Vec<Vec<u8>> {
    let frames = pre_encode_vpx(codec, w, h, 1.0, n);
    frames
        .iter()
        .map(|f| {
            let mut evf = EncodedVideoFrame::new();
            evf.data = Bytes::from(f.data.clone());
            evf.key = f.key;
            evf.pts = f.pts;

            let mut evfs = EncodedVideoFrames::new();
            evfs.frames.push(evf);

            let mut vf = VideoFrame::new();
            match codec {
                VpxVideoCodecId::VP8 => vf.set_vp8s(evfs),
                VpxVideoCodecId::VP9 => vf.set_vp9s(evfs),
            }

            let mut msg = Message::new();
            msg.set_video_frame(vf);
            msg.write_to_bytes().unwrap()
        })
        .collect()
}

/// Extract the video_frame::Union from a serialized Message.
fn extract_union(msg_bytes: &[u8]) -> Option<video_frame::Union> {
    let msg = Message::parse_from_bytes(msg_bytes).ok()?;
    let vf = msg.video_frame();
    vf.union.clone()
}

// ---------------------------------------------------------------------------
// Single frame pipeline: VP9 1080p
// ---------------------------------------------------------------------------

fn bench_pipeline_decode_1080p(c: &mut Criterion) {
    let mut group = c.benchmark_group("pipeline_decode");
    let (w, h) = (1920, 1080);

    let messages = make_serialized_messages(VpxVideoCodecId::VP9, w, h, 30);
    let mut decoder = Decoder::new(CodecFormat::VP9, None);
    let mut rgb = ImageRgb::new(ImageFormat::ARGB, 1);
    let mut texture = ImageTexture::default();
    let mut pixelbuffer = true;
    let mut chroma: Option<Chroma> = None;

    group.throughput(Throughput::Elements(1));
    group.bench_function(BenchmarkId::from_parameter("vp9_1080p"), |b| {
        let mut idx = 0;
        b.iter(|| {
            let msg_bytes = &messages[idx % messages.len()];

            // Step 1: Protobuf deserialize
            // Step 2+3: Decode + YUV→RGB via real Decoder::handle_video_frame
            if let Some(union) = extract_union(black_box(msg_bytes)) {
                decoder.handle_video_frame(
                    &union,
                    &mut rgb,
                    &mut texture,
                    &mut pixelbuffer,
                    &mut chroma,
                ).expect("decode failed");
            }

            idx += 1;
            black_box(rgb.raw.len())
        });
    });
    group.finish();
}

// ---------------------------------------------------------------------------
// Single frame pipeline: VP9 4K
// ---------------------------------------------------------------------------

fn bench_pipeline_decode_4k(c: &mut Criterion) {
    let mut group = c.benchmark_group("pipeline_decode");
    group.measurement_time(Duration::from_secs(15));
    let (w, h) = (3840, 2160);

    let messages = make_serialized_messages(VpxVideoCodecId::VP9, w, h, 10);
    let mut decoder = Decoder::new(CodecFormat::VP9, None);
    let mut rgb = ImageRgb::new(ImageFormat::ARGB, 1);
    let mut texture = ImageTexture::default();
    let mut pixelbuffer = true;
    let mut chroma: Option<Chroma> = None;

    group.throughput(Throughput::Elements(1));
    group.bench_function(BenchmarkId::from_parameter("vp9_4k"), |b| {
        let mut idx = 0;
        b.iter(|| {
            let msg_bytes = &messages[idx % messages.len()];
            if let Some(union) = extract_union(black_box(msg_bytes)) {
                decoder.handle_video_frame(
                    &union,
                    &mut rgb,
                    &mut texture,
                    &mut pixelbuffer,
                    &mut chroma,
                ).expect("decode failed");
            }
            idx += 1;
            black_box(rgb.raw.len())
        });
    });
    group.finish();
}

// ---------------------------------------------------------------------------
// 100-frame sequence pipeline: VP9 1080p
// ---------------------------------------------------------------------------

fn bench_pipeline_decode_sequence(c: &mut Criterion) {
    let mut group = c.benchmark_group("pipeline_decode_sequence");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(20));
    let (w, h) = (1920, 1080);

    let messages = make_serialized_messages(VpxVideoCodecId::VP9, w, h, 100);
    let mut decoder = Decoder::new(CodecFormat::VP9, None);
    let mut rgb = ImageRgb::new(ImageFormat::ARGB, 1);
    let mut texture = ImageTexture::default();
    let mut pixelbuffer = true;
    let mut chroma: Option<Chroma> = None;

    group.throughput(Throughput::Elements(100));
    group.bench_function(
        BenchmarkId::from_parameter("vp9_1080p_100frames"),
        |b| {
            b.iter(|| {
                for msg_bytes in &messages {
                    if let Some(union) = extract_union(msg_bytes) {
                        decoder.handle_video_frame(
                            &union,
                            &mut rgb,
                            &mut texture,
                            &mut pixelbuffer,
                            &mut chroma,
                        );
                    }
                }
                black_box(rgb.raw.len())
            });
        },
    );
    group.finish();
}

criterion_group!(
    benches,
    bench_pipeline_decode_1080p,
    bench_pipeline_decode_4k,
    bench_pipeline_decode_sequence,
);
criterion_main!(benches);
