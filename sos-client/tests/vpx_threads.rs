//! 对比基准：`libs/scrap` 的编码器（线程数被 available_memory 限制）
//! vs SOS 自建编码器（线程数完全可控）。
//!
//! 运行：cargo test --release --test vpx_threads -- --nocapture

#![cfg(sos_vpx)]

#[path = "../src/sos_vpx.rs"]
mod sos_vpx;

use sos_vpx::{SosCodec, SosVpxConfig, SosVpxEncoder};
use std::time::Instant;

fn make_i420(w: u32, h: u32, frame_idx: u32) -> Vec<u8> {
    let (w, h) = (w as usize, h as usize);
    let mut buf = vec![0u8; w * h * 3 / 2];
    let (y, rest) = buf.split_at_mut(w * h);
    let (u, v) = rest.split_at_mut(w * h / 4);
    // 高细节内容（文字/纹理桌面），逐帧变化
    let mut seed = frame_idx.wrapping_mul(2654435761).wrapping_add(1);
    for p in y.iter_mut().chain(u.iter_mut()).chain(v.iter_mut()) {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        *p = (seed >> 24) as u8;
    }
    buf
}

fn bench(codec: SosCodec, w: u32, h: u32, threads: usize, frames: u32) {
    let cfg = SosVpxConfig {
        width: w as usize,
        height: h as usize,
        codec,
        threads,
        cpu_used: SosVpxConfig::default_cpu_used(codec, w as usize, h as usize),
        bitrate_kbps: SosVpxConfig::default_bitrate_kbps(w as usize, h as usize),
        keyframe_interval: None,
    };
    let mut enc = match SosVpxEncoder::new(cfg) {
        Ok(e) => e,
        Err(e) => {
            println!("[sos-vpx {codec:?} {w}x{h} threads={threads}] create failed: {e}");
            return;
        }
    };
    let mut total_bytes = 0usize;
    let start = Instant::now();
    for i in 0..frames {
        let yuv = make_i420(w, h, i);
        match enc.encode(&yuv, (i * 33) as i64) {
            Ok(frames_out) => {
                for (data, _key) in frames_out {
                    total_bytes += data.len();
                }
            }
            Err(e) => {
                println!("[sos-vpx {codec:?} {w}x{h} threads={threads}] encode #{i} failed: {e}");
                return;
            }
        }
    }
    let total = start.elapsed().as_secs_f64();
    println!(
        "[sos-vpx {:?} {w}x{h} threads={threads}] fps={:.1} frame_ms={:.1} avg_kb={:.0}",
        codec,
        frames as f64 / total,
        total * 1000.0 / frames as f64,
        total_bytes as f64 / frames as f64 / 1024.0
    );
}

#[test]
fn sos_vpx_thread_scaling() {
    let frames = 40;
    for w_h in [(1920u32, 1080u32), (1280, 720)] {
        let (w, h) = w_h;
        for threads in [1usize, 2, 4, 8] {
            bench(SosCodec::Vp9, w, h, threads, frames);
            bench(SosCodec::Vp8, w, h, threads, frames);
        }
    }
}

/// 用与 `libs/scrap` 完全相同的参数（cpu_used=7、1 线程、日志中的 1388kbps）
/// 运行自建编码器。结果应与 scrap 的 ~8.4fps 接近，以此证明实现等价，
/// 从而说明后续 `cpu_used` / 线程数带来的提升是真实的，而不是实现差异。
#[test]
fn sos_vpx_matches_scrap_baseline() {
    let cfg = SosVpxConfig {
        width: 1920,
        height: 1080,
        codec: SosCodec::Vp9,
        threads: 1,
        cpu_used: 7,
        bitrate_kbps: 1388,
        keyframe_interval: None,
    };
    let mut enc = SosVpxEncoder::new(cfg).expect("create encoder");
    let frames = 40u32;
    let mut total_bytes = 0usize;
    let start = Instant::now();
    for i in 0..frames {
        let yuv = make_i420(1920, 1080, i);
        for (data, _key) in enc.encode(&yuv, (i * 33) as i64).expect("encode") {
            total_bytes += data.len();
        }
    }
    let total = start.elapsed().as_secs_f64();
    println!(
        "[baseline-scrap-params] 1920x1080 cpu_used=7 threads=1 bitrate=1388kbps fps={:.1} frame_ms={:.1} avg_kb={:.0}",
        frames as f64 / total,
        total * 1000.0 / frames as f64,
        total_bytes as f64 / frames as f64 / 1024.0
    );
}

/// 同上参数，但**每帧额外调用 flush**——复刻 `libs/scrap` 的
/// `encode_to_message()`（encode + flush 每帧各一次）。
/// 若帧率塌到个位数，即可确认「每帧 flush」是 1080p 低帧率的主因。
#[test]
fn sos_vpx_with_per_frame_flush() {
    let cfg = SosVpxConfig {
        width: 1920,
        height: 1080,
        codec: SosCodec::Vp9,
        threads: 1,
        cpu_used: 7,
        bitrate_kbps: 1388,
        keyframe_interval: None,
    };
    let mut enc = SosVpxEncoder::new(cfg).expect("create encoder");
    let frames = 40u32;
    let mut total_bytes = 0usize;
    let start = Instant::now();
    for i in 0..frames {
        let yuv = make_i420(1920, 1080, i);
        for (data, _key) in enc.encode(&yuv, (i * 33) as i64).expect("encode") {
            total_bytes += data.len();
        }
        for (data, _key) in enc.flush().expect("flush") {
            total_bytes += data.len();
        }
    }
    let total = start.elapsed().as_secs_f64();
    println!(
        "[scrap-style-per-frame-flush] 1920x1080 cpu_used=7 threads=1 fps={:.1} frame_ms={:.1} avg_kb={:.0}",
        frames as f64 / total,
        total * 1000.0 / frames as f64,
        total_bytes as f64 / frames as f64 / 1024.0
    );
}
