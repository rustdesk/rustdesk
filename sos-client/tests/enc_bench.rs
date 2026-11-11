//! 临时性能基准：测量不同编码线程数 / 分辨率 / 画面复杂度下的软件编码吞吐
//!
//! 运行：cargo test --release --test enc_bench -- --nocapture

use hbb_common::protobuf::Message as _;
use scrap::codec::{Encoder, EncoderCfg};
use scrap::{EncodeInput, VpxEncoderConfig, VpxVideoCodecId};
use std::time::Instant;

fn make_i420(w: u32, h: u32, frame_idx: u32, noise: bool) -> Vec<u8> {
    let (w, h) = (w as usize, h as usize);
    let mut buf = vec![0u8; w * h * 3 / 2];
    let (y, rest) = buf.split_at_mut(w * h);
    let (u, v) = rest.split_at_mut(w * h / 4);
    let mut seed = frame_idx.wrapping_mul(2654435761).wrapping_add(1);
    if noise {
        // 高细节内容（文字/纹理桌面），逐帧变化，编码器无法偷懒
        for p in y.iter_mut().chain(u.iter_mut()).chain(v.iter_mut()) {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            *p = (seed >> 24) as u8;
        }
    } else {
        // 平滑滚动内容（动画/视频），运动估计容易
        let shift = (frame_idx as usize * 7) % w.max(1);
        for row in 0..h {
            for col in 0..w {
                let x = (col + shift) % w.max(1);
                y[row * w + col] = ((x * 255 / w.max(1)) ^ (row * 3)) as u8;
            }
        }
        for row in 0..h / 2 {
            for col in 0..w / 2 {
                u[row * (w / 2) + col] = ((col + shift) % 256) as u8;
                v[row * (w / 2) + col] = ((row * 5) % 256) as u8;
            }
        }
    }
    buf
}

fn bench(name: &str, w: u32, h: u32, codec: VpxVideoCodecId, frames: u32, noise: bool) {
    let cfg = EncoderCfg::VPX(VpxEncoderConfig {
        width: w,
        height: h,
        quality: 1.0,
        codec,
        keyframe_interval: None,
    });
    let mut enc = match Encoder::new(cfg, false) {
        Ok(e) => e,
        Err(e) => {
            println!("[{name}] create failed: {e}");
            return;
        }
    };
    let mut total_bytes = 0usize;
    let start = Instant::now();
    for i in 0..frames {
        let yuv = make_i420(w, h, i, noise);
        if let Ok(vf) = enc.encode_to_message(EncodeInput::YUV(&yuv), (i * 33) as i64) {
            total_bytes += vf.compute_size() as usize;
        }
    }
    let total = start.elapsed().as_secs_f64();
    println!(
        "[{name}] {w}x{h} fps={:.1} frame_ms={:.1} avg_kb={:.0}",
        frames as f64 / total,
        total * 1000.0 / frames as f64,
        total_bytes as f64 / frames as f64 / 1024.0
    );
}

#[test]
fn bench_software_encoders() {
    let frames = 40;
    println!(
        "=== codec threads (scrap heuristic): {} ===",
        scrap::codec::codec_thread_num(64)
    );
    for (w, h, tag) in [(1920u32, 1080u32, "1080p"), (1280, 720, "720p")] {
        for (codec, ctag) in [
            (VpxVideoCodecId::VP9, "vp9"),
            (VpxVideoCodecId::VP8, "vp8"),
        ] {
            bench(&format!("{ctag}-{tag}-text"), w, h, codec, frames, true);
            bench(&format!("{ctag}-{tag}-scroll"), w, h, codec, frames, false);
        }
    }
}
