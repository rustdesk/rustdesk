mod common;

use common::pre_encode_vpx;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use hbb_common::{
    bytes::Bytes,
    message_proto::{EncodedVideoFrame, EncodedVideoFrames, Message, VideoFrame},
    protobuf::Message as ProtoMessage,
};

/// E. Protobuf serialization/deserialization benchmarks.
///
/// Measures Message wrapping VideoFrame → write_to_bytes → parse_from_bytes.
/// Tests both typical (30 KB VP9) and large (200 KB, simulating 4K best quality) payloads.

fn make_video_frame(payload_size: usize) -> Message {
    let mut evf = EncodedVideoFrame::new();
    evf.data = Bytes::from(vec![0xABu8; payload_size]);
    evf.key = true;
    evf.pts = 1234;

    let mut evfs = EncodedVideoFrames::new();
    evfs.frames.push(evf);

    let mut vf = VideoFrame::new();
    vf.set_vp9s(evfs);
    vf.display = 0;

    let mut msg = Message::new();
    msg.set_video_frame(vf);
    msg
}

fn make_video_frame_from_real_encode() -> Message {
    let frames = pre_encode_vpx(scrap::VpxVideoCodecId::VP9, 1920, 1080, 1.0, 1);
    let mut evf = EncodedVideoFrame::new();
    evf.data = Bytes::from(frames[0].data.clone());
    evf.key = frames[0].key;
    evf.pts = frames[0].pts;

    let mut evfs = EncodedVideoFrames::new();
    evfs.frames.push(evf);

    let mut vf = VideoFrame::new();
    vf.set_vp9s(evfs);
    vf.display = 0;

    let mut msg = Message::new();
    msg.set_video_frame(vf);
    msg
}

// ---------------------------------------------------------------------------
// Serialize VideoFrame
// ---------------------------------------------------------------------------

fn bench_serialize(c: &mut Criterion) {
    let mut group = c.benchmark_group("protobuf_serialize");

    let cases: &[(&str, usize)] = &[
        ("30KB_typical", 30_000),
        ("100KB_hq", 100_000),
        ("200KB_4k_best", 200_000),
    ];

    for (label, size) in cases {
        let msg = make_video_frame(*size);

        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(*label), &(), |b, _| {
            b.iter(|| {
                black_box(msg.write_to_bytes().unwrap())
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Deserialize VideoFrame
// ---------------------------------------------------------------------------

fn bench_deserialize(c: &mut Criterion) {
    let mut group = c.benchmark_group("protobuf_deserialize");

    let cases: &[(&str, usize)] = &[
        ("30KB_typical", 30_000),
        ("100KB_hq", 100_000),
        ("200KB_4k_best", 200_000),
    ];

    for (label, size) in cases {
        let msg = make_video_frame(*size);
        let bytes = msg.write_to_bytes().unwrap();

        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(*label), &(), |b, _| {
            b.iter(|| {
                black_box(Message::parse_from_bytes(black_box(&bytes)).unwrap())
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Roundtrip: serialize + deserialize
// ---------------------------------------------------------------------------

fn bench_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("protobuf_roundtrip");

    let msg = make_video_frame_from_real_encode();
    let serialized_size = msg.compute_size() as u64;

    group.throughput(Throughput::Bytes(serialized_size));
    group.bench_function(BenchmarkId::from_parameter("real_vp9_1080p"), |b| {
        b.iter(|| {
            let bytes = msg.write_to_bytes().unwrap();
            black_box(Message::parse_from_bytes(&bytes).unwrap())
        });
    });
    group.finish();
}

criterion_group!(benches, bench_serialize, bench_deserialize, bench_roundtrip);
criterion_main!(benches);
