//! 码率控制精度 vs `cpu-used` 的实测。
//!
//! 生产日志显示：目标 1675kbps，实际 `sent_kbps` 稳定在 2520-2720（162%），
//! 由此在中继链路上养出 bufferbloat（RTT 34ms → 211ms），QoS 再把帧率压到 2-8。
//!
//! 同时日志显示 `enc=5.4ms`（1080p / 5 线程 / cpu_used=8），
//! 而 30fps 的预算是 33ms —— **CPU 有大量余量**。
//! VP9 realtime 模式下 `cpu-used` 越大速度越快、码率控制越粗。
//! 若降低 cpu-used 能显著改善码率精度，就是用闲置 CPU 换带宽与延迟。
//!
//! 运行：cargo test --release --test vpx_cpuused -- --nocapture

#![cfg(all(windows, sos_vpx))]

#[path = "../src/sos_vpx.rs"]
mod sos_vpx;

use sos_vpx::{SosCodec, SosVpxConfig, SosVpxEncoder};
use std::time::{Duration, Instant};

/// 高复杂度但非纯噪声：渐变 + 细密纹理 + 每帧位移。
/// 介于真实桌面和高斯噪声之间，用来压力测试码率控制。
fn make_motion(w: usize, h: usize, seed: usize) -> Vec<u8> {
    let (cw, ch) = (w / 2, h / 2);
    let mut buf = vec![0u8; w * h + 2 * cw * ch];
    let shift = seed * 7;
    for y in 0..h {
        for x in 0..w {
            // 斜向条纹 + 每帧整体位移，制造大量真实运动矢量
            let v = ((x + shift) * 3 + y * 5) % 251;
            buf[y * w + x] = v as u8;
        }
    }
    let uv = w * h;
    for y in 0..ch {
        for x in 0..cw {
            let v = ((x + shift) * 2 + y * 3) % 251;
            buf[uv + y * cw + x] = v as u8;
            buf[uv + cw * ch + y * cw + x] = (250 - v) as u8;
        }
    }
    buf
}

struct Result {
    fps: f64,
    kbps: f64,
    enc_ms: f64,
    dev_pct: f64,
}

fn measure(
    codec: SosCodec,
    cpu_used: u32,
    threads: usize,
    target: u32,
    frames: u32,
) -> Option<Result> {
    let (w, h) = (1920usize, 1080usize);
    let cfg = SosVpxConfig {
        width: w,
        height: h,
        codec,
        threads,
        cpu_used,
        bitrate_kbps: target,
        keyframe_interval: None,
    };
    let mut enc = SosVpxEncoder::new(cfg).ok()?;
    let mut bytes = 0usize;
    let mut enc_ns = 0u128;
    let origin = Instant::now();
    let spf = Duration::from_secs_f64(1.0 / 30.0);

    for i in 0..frames as usize {
        let deadline = origin + spf * i as u32;
        let now = Instant::now();
        if deadline > now {
            std::thread::sleep(deadline - now);
        }
        let yuv = make_motion(w, h, i);
        let pts = origin.elapsed().as_millis() as i64;
        let t = Instant::now();
        if let Ok(out) = enc.encode(&yuv, pts) {
            enc_ns += t.elapsed().as_nanos();
            for (d, _k) in out {
                bytes += d.len();
            }
        }
    }
    let secs = origin.elapsed().as_secs_f64();
    let kbps = bytes as f64 * 8.0 / 1000.0 / secs;
    Some(Result {
        fps: frames as f64 / secs,
        kbps,
        enc_ms: enc_ns as f64 / 1e6 / frames as f64,
        dev_pct: (kbps - target as f64) / target as f64 * 100.0,
    })
}

#[test]
fn cpu_used_vs_rate_control_accuracy() {
    let target = 1675u32;
    println!("目标码率 {target}kbps, 1920x1080@30fps, 运动型内容");
    println!(
        "{:<10} {:>8} {:>8} {:>8} {:>10} {:>12} {:>10}",
        "codec", "cpu-used", "threads", "fps", "enc_ms", "实际kbps", "偏离"
    );

    for codec in [SosCodec::Vp9] {
        for cpu_used in [8u32, 6, 5, 4] {
            if let Some(r) = measure(codec, cpu_used, 5, target, 60) {
                println!(
                    "{:<10} {:>8} {:>8} {:>8.1} {:>10.2} {:>12.0} {:>9.0}%",
                    format!("{codec:?}"),
                    cpu_used,
                    5,
                    r.fps,
                    r.enc_ms,
                    r.kbps,
                    r.dev_pct
                );
            }
        }
    }
    println!();
    println!("说明：enc_ms 是单帧编码耗时。30fps 预算 33ms，");
    println!("只要 enc_ms 远低于 33ms 就还有余量换取更好的码率精度。");
}
