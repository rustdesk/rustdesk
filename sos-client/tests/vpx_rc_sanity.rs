//! 验证码率控制是否**真正在工作**。
//!
//! 绝对精度对合成内容没有意义（高频纹理本来就压不到低码率），
//! 但有一条必须成立的不变量：**目标码率翻倍，产出也应当大致翻倍**。
//! 如果这条不成立，说明码率控制根本没生效，参数再调也没用。
//!
//! 运行：cargo test --release --test vpx_rc_sanity -- --nocapture

#![cfg(all(windows, sos_vpx))]

#[path = "../src/sos_vpx.rs"]
mod sos_vpx;

use sos_vpx::{SosCodec, SosVpxConfig, SosVpxEncoder};
use std::time::{Duration, Instant};

/// 运动型内容：斜向纹理 + 每帧整体位移
fn make_motion(w: usize, h: usize, seed: usize) -> Vec<u8> {
    let (cw, ch) = (w / 2, h / 2);
    let mut buf = vec![0u8; w * h + 2 * cw * ch];
    let shift = seed * 7;
    for y in 0..h {
        for x in 0..w {
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

fn encode_kbps(target: u32, frames: u32, seed_shift: usize) -> (f64, usize) {
    let (w, h) = (1920usize, 1080usize);
    let cfg = SosVpxConfig {
        width: w,
        height: h,
        codec: SosCodec::Vp9,
        threads: 5,
        cpu_used: 8,
        bitrate_kbps: target,
        keyframe_interval: None,
    };
    let mut enc = SosVpxEncoder::new(cfg).expect("encoder");
    let mut bytes = 0usize;
    let mut dropped = 0usize;
    let origin = Instant::now();
    let spf = Duration::from_secs_f64(1.0 / 30.0);
    for i in 0..frames as usize {
        let deadline = origin + spf * i as u32;
        let now = Instant::now();
        if deadline > now {
            std::thread::sleep(deadline - now);
        }
        let yuv = make_motion(w, h, i + seed_shift);
        let pts = origin.elapsed().as_millis() as i64;
        if let Ok(out) = enc.encode(&yuv, pts) {
            dropped += enc.take_dropped_frames();
            for (d, _k) in out {
                bytes += d.len();
            }
        }
    }
    let secs = origin.elapsed().as_secs_f64();
    let kbps = bytes as f64 * 8.0 / 1000.0 / secs;
    (kbps, dropped)
}

/// 核心不变量：目标翻倍 → 产出翻倍（允许一定容差）
#[test]
fn target_bitrate_scales_output() {
    let low = 800u32;
    let high = 3200u32;
    let frames = 45u32;
    let (a, drop_a) = encode_kbps(low, frames, 0);
    let (b, drop_b) = encode_kbps(high, frames, 100);

    println!("目标 {low}kbps  →  实际 {a:.0}kbps, 丢帧 {drop_a}/{frames}");
    println!("目标 {high}kbps →  实际 {b:.0}kbps, 丢帧 {drop_b}/{frames}");
    let ratio = b / a.max(1.0);
    println!("产出比 {ratio:.2}（目标比 4.00）");

    assert!(
        ratio > 1.6,
        "目标码率翻 4 倍，产出只涨了 {ratio:.2} 倍 —— 码率控制没有生效"
    );
    // 内容接近不可压缩，低码率下必须靠丢帧才能守住预算；
    // 高码率下丢帧应当更少。这是「允许丢帧、尽量不丢」的核心观察点。
    assert!(
        drop_a >= drop_b,
        "低码率({drop_a} 帧) 丢帧反而少于高码率({drop_b} 帧)，帧率下采样逻辑异常"
    );
}

/// 真实桌面内容在默认码率下**不应丢帧** —— 这是「尽量不丢」的验收条件。
#[test]
fn desktop_content_does_not_drop_frames() {
    let (w, h) = (1920usize, 1080usize);
    let cfg = SosVpxConfig {
        width: w,
        height: h,
        codec: SosCodec::Vp9,
        threads: 5,
        cpu_used: 8,
        bitrate_kbps: 1675,
        keyframe_interval: None,
    };
    let mut enc = SosVpxEncoder::new(cfg).expect("encoder");
    let mut dropped = 0usize;
    let frames = 60u32;
    let origin = Instant::now();
    let spf = Duration::from_secs_f64(1.0 / 30.0);

    for i in 0..frames as usize {
        let deadline = origin + spf * i as u32;
        let now = Instant::now();
        if deadline > now {
            std::thread::sleep(deadline - now);
        }
        let yuv = make_desktop(w, h, i);
        let pts = origin.elapsed().as_millis() as i64;
        if let Ok(out) = enc.encode(&yuv, pts) {
            dropped += enc.take_dropped_frames();
            std::hint::black_box(out.len());
        }
    }
    println!("桌面内容 1675kbps: 丢帧 {dropped}/{frames}");
    assert_eq!(dropped, 0, "普通桌面内容不应触发丢帧（丢了 {dropped} 帧）");
}

/// 接近真实桌面的画面：平背景 + 文字状细节 + 每帧小幅位移
fn make_desktop(w: usize, h: usize, seed: usize) -> Vec<u8> {
    let (cw, ch) = (w / 2, h / 2);
    let mut buf = vec![0u8; w * h + 2 * cw * ch];
    for y in 0..h {
        let base = 200u32.saturating_sub((y * 30 / h.max(1)) as u32);
        for x in 0..w {
            buf[y * w + x] = base as u8;
        }
    }
    let shift = seed % 40;
    for block in 0..12usize {
        let bx = (block * 137 + shift) % w.saturating_sub(120).max(1);
        let by = (block * 71) % h.saturating_sub(60).max(1);
        for row in 0..3usize {
            let y = (by + row * 4).min(h - 1);
            for x in bx..(bx + 100).min(w) {
                if (x + row) % 3 == 0 {
                    buf[y * w + x] = 30;
                }
            }
        }
        let y = by.min(h - 1);
        for x in bx..(bx + 110).min(w) {
            buf[y * w + x] = 90;
        }
    }
    let uv = w * h;
    for i in 0..(cw * ch) {
        buf[uv + i] = 128;
        buf[uv + cw * ch + i] = 128;
    }
    buf
}

/// 静态内容应当极省带宽：说明码率控制能往下走
#[test]
fn static_content_is_cheap() {
    let (w, h) = (1920usize, 1080usize);
    let cfg = SosVpxConfig {
        width: w,
        height: h,
        codec: SosCodec::Vp9,
        threads: 5,
        cpu_used: 8,
        bitrate_kbps: 1675,
        keyframe_interval: None,
    };
    let mut enc = SosVpxEncoder::new(cfg).expect("encoder");
    let mut bytes = 0usize;
    let origin = Instant::now();
    let spf = Duration::from_secs_f64(1.0 / 30.0);
    let frames = 45u32;
    // 完全不变的画面
    for i in 0..frames as usize {
        let deadline = origin + spf * i as u32;
        let now = Instant::now();
        if deadline > now {
            std::thread::sleep(deadline - now);
        }
        let mut yuv = vec![0u8; w * h + 2 * (w / 2) * (h / 2)];
        for (idx, b) in yuv.iter_mut().enumerate() {
            *b = if idx < w * h { 120 } else { 128 };
        }
        let pts = origin.elapsed().as_millis() as i64;
        if let Ok(out) = enc.encode(&yuv, pts) {
            for (d, _k) in out {
                bytes += d.len();
            }
        }
    }
    let secs = origin.elapsed().as_secs_f64();
    let kbps = bytes as f64 * 8.0 / 1000.0 / secs;
    println!("静态画面: {kbps:.0}kbps（目标 1675）");
    assert!(
        kbps < 200.0,
        "静态画面用了 {kbps:.0}kbps，码率控制无法向下走"
    );
}
