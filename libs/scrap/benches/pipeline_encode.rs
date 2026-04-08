mod common;

use common::{i420_layout, make_bgra, Pattern};
use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};
use hbb_common::{
    message_proto::Message,
    protobuf::Message as ProtoMessage,
};
use scrap::{
    codec::{EncoderApi, EncoderCfg},
    EncodeInput, VpxEncoder, VpxEncoderConfig, VpxVideoCodecId,
};
use std::time::Duration;

/// J. Full encode pipeline benchmarks.
///
/// BGRA capture → YUV conversion → encode_to_message → protobuf serialize.
/// Uses the real encode_to_message() API (see vpxcodec.rs EncoderApi impl)
/// which is the same path as video_service.rs handle_one_frame().

// ---------------------------------------------------------------------------
// Single frame pipeline: VP9 1080p
// ---------------------------------------------------------------------------

fn bench_pipeline_encode_1080p(c: &mut Criterion) {
    let mut group = c.benchmark_group("pipeline_encode");
    let (w, h) = (1920, 1080);

    let cfg = EncoderCfg::VPX(VpxEncoderConfig {
        width: w as _,
        height: h as _,
        quality: 1.0,
        codec: VpxVideoCodecId::VP9,
        keyframe_interval: None,
    });
    let mut encoder = VpxEncoder::new(cfg, false).unwrap();
    let (bgra, bgra_stride) = make_bgra(w, h, &Pattern::Gradient);
    let layout = i420_layout(w, h);

    group.throughput(Throughput::Elements(1));
    group.bench_function(BenchmarkId::from_parameter("vp9_1080p"), |b| {
        let mut yuv = vec![0u8; layout.total];
        let mut pts = 0i64;

        b.iter(|| {
            // Step 1: BGRA → I420 (same as convert_to_yuv)
            unsafe {
                let dst_y = yuv.as_mut_ptr();
                let dst_u = dst_y.add(layout.y_size);
                let dst_v = dst_u.add(layout.uv_size);
                scrap::ARGBToI420(
                    bgra.as_ptr(),
                    bgra_stride as _,
                    dst_y,
                    layout.stride_y as _,
                    dst_u,
                    layout.stride_uv as _,
                    dst_v,
                    layout.stride_uv as _,
                    w as _,
                    h as _,
                );
            }

            // Step 2+3: encode_to_message (real API from EncoderApi trait)
            let input = EncodeInput::YUV(&yuv);
            let vf = encoder.encode_to_message(input, pts);

            // Step 4: Wrap in Message + serialize (real send path)
            if let Ok(vf) = vf {
                let mut msg = Message::new();
                msg.set_video_frame(vf);
                msg.write_to_bytes().unwrap()
            } else {
                Vec::new()
            };
            pts += 1;
        });
    });
    group.finish();
}

// ---------------------------------------------------------------------------
// Single frame pipeline: VP9 4K
// ---------------------------------------------------------------------------

fn bench_pipeline_encode_4k(c: &mut Criterion) {
    let mut group = c.benchmark_group("pipeline_encode");
    group.measurement_time(Duration::from_secs(15));
    let (w, h) = (3840, 2160);

    let cfg = EncoderCfg::VPX(VpxEncoderConfig {
        width: w as _,
        height: h as _,
        quality: 1.0,
        codec: VpxVideoCodecId::VP9,
        keyframe_interval: None,
    });
    let mut encoder = VpxEncoder::new(cfg, false).unwrap();
    let layout = i420_layout(w, h);

    let (bgra, bgra_stride) = make_bgra(w, h, &Pattern::Gradient);

    group.throughput(Throughput::Elements(1));
    group.bench_function(BenchmarkId::from_parameter("vp9_4k"), |b| {
        let mut yuv = vec![0u8; layout.total];
        let mut pts = 0i64;

        b.iter(|| {
            unsafe {
                let dst_y = yuv.as_mut_ptr();
                let dst_u = dst_y.add(layout.y_size);
                let dst_v = dst_u.add(layout.uv_size);
                scrap::ARGBToI420(
                    bgra.as_ptr(),
                    bgra_stride as _,
                    dst_y,
                    layout.stride_y as _,
                    dst_u,
                    layout.stride_uv as _,
                    dst_v,
                    layout.stride_uv as _,
                    w as _,
                    h as _,
                );
            }

            let input = EncodeInput::YUV(&yuv);
            let vf = encoder.encode_to_message(input, pts);
            if let Ok(vf) = vf {
                let mut msg = Message::new();
                msg.set_video_frame(vf);
                msg.write_to_bytes().unwrap()
            } else {
                Vec::new()
            };
            pts += 1;
        });
    });
    group.finish();
}

// ---------------------------------------------------------------------------
// 100-frame sequence pipeline: VP9 1080p with movement
// ---------------------------------------------------------------------------

fn bench_pipeline_encode_sequence(c: &mut Criterion) {
    let mut group = c.benchmark_group("pipeline_encode_sequence");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(30));
    let (w, h) = (1920, 1080);

    let cfg = EncoderCfg::VPX(VpxEncoderConfig {
        width: w as _,
        height: h as _,
        quality: 1.0,
        codec: VpxVideoCodecId::VP9,
        keyframe_interval: None,
    });
    let mut encoder = VpxEncoder::new(cfg, false).unwrap();
    let layout = i420_layout(w, h);

    // Pre-generate 100 BGRA frames with movement
    let bgra_frames: Vec<(Vec<u8>, usize)> = (0..100)
        .map(|i| make_bgra(w, h, &Pattern::Random(i as u64 * 12345)))
        .collect();

    group.throughput(Throughput::Elements(100));
    group.bench_function(
        BenchmarkId::from_parameter("vp9_1080p_100frames"),
        |b| {
            let mut yuv = vec![0u8; layout.total];

            b.iter(|| {
                let mut total_output_bytes = 0usize;
                for (pts, (bgra, bgra_stride)) in bgra_frames.iter().enumerate() {
                    unsafe {
                        let dst_y = yuv.as_mut_ptr();
                        let dst_u = dst_y.add(layout.y_size);
                        let dst_v = dst_u.add(layout.uv_size);
                        scrap::ARGBToI420(
                            bgra.as_ptr(),
                            *bgra_stride as _,
                            dst_y,
                            layout.stride_y as _,
                            dst_u,
                            layout.stride_uv as _,
                            dst_v,
                            layout.stride_uv as _,
                            w as _,
                            h as _,
                        );
                    }

                    let input = EncodeInput::YUV(&yuv);
                    if let Ok(vf) = encoder.encode_to_message(input, pts as i64) {
                        total_output_bytes += vf.compute_size() as usize;
                    }
                }
                black_box(total_output_bytes)
            });
        },
    );
    group.finish();
}

criterion_group!(
    benches,
    bench_pipeline_encode_1080p,
    bench_pipeline_encode_4k,
    bench_pipeline_encode_sequence,
);
criterion_main!(benches);
