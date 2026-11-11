//! 编码吞吐实测：找出「编码耗时 ≤ 帧预算」的配置。
//!
//! 真实机器日志（4 核 / 1600x900 / DXGI 可用）显示瓶颈已经完全转移到编码器：
//!
//! ```text
//! [VIDEO:PERF] loop=19.6fps it=51.1ms cap=5.0ms conv=2.0ms enc=43.5ms spf=33ms skip=0
//! [VIDEO:PERF] loop=19.9fps it=50.3ms cap=5.2ms conv=1.8ms enc=42.4ms spf=33ms skip=1
//! ```
//!
//! 采集只要 5ms，编码却要 43ms —— 超过 30fps 的 33ms 预算，
//! 所以循环实际只跑到 19.6fps，且**与码率、丢帧策略完全无关**。
//!
//! 本测试在 1600x900 上扫描 `threads × cpu_used`，找到：
//! 1. 达到 30fps 所需（`enc_ms ≤ 26ms`，留 7ms 给采集+转换）的最小代价配置；
//! 2. VP8 与 VP9 的吞吐差距，用于判断「编码跟不上时是否该降级到 VP8」。
//!
//! 运行：cargo test --release --test vpx_enc_throughput -- --nocapture --test-threads=1

#![cfg(all(windows, sos_vpx))]

#[path = "../src/sos_vpx.rs"]
mod sos_vpx;

use sos_vpx::{SosCodec, SosVpxConfig, SosVpxEncoder};
use std::time::Instant;

/// 桌面典型内容：大面积平坦区 + 局部文字状细节 + 每帧小幅位移。
/// 比纯噪声轻、比纯静态重，接近「滚动网页 / 拖动窗口」的真实负载。
fn make_desktop(w: usize, h: usize, seed: usize) -> Vec<u8> {
    let (cw, ch) = (w / 2, h / 2);
    let mut buf = vec![0u8; w * h + 2 * cw * ch];
    // 平坦渐变背景
    for y in 0..h {
        let base = 232u32.saturating_sub((y * 40 / h.max(1)) as u32);
        for x in 0..w {
            buf[y * w + x] = base as u8;
        }
    }
    // 文字状细节块，随 seed 滚动
    let shift = seed * 4;
    for block in 0..40usize {
        let bx = (block * 71) % w.saturating_sub(140).max(1);
        let by = (block * 37 + shift) % h.saturating_sub(40).max(1);
        for row in 0..14usize {
            let y = (by + row * 2).min(h - 1);
            for x in bx..(bx + 120).min(w) {
                if (x + row * 3) % 6 < 3 {
                    buf[y * w + x] = 45;
                }
            }
        }
    }
    let uv = w * h;
    for i in 0..(cw * ch) {
        buf[uv + i] = 128;
        buf[uv + cw * ch + i] = 128;
    }
    buf
}

struct Measured {
    enc_ms: f64,
    enc_max_ms: f64,
    kbps: f64,
    drops: usize,
}

/// 纯吞吐测量：不刻意按 spf 休眠，让编码器全速跑，
/// 这样测到的是「单帧最小编码耗时」，不受调度抖动干扰。
fn measure(codec: SosCodec, cpu_used: u32, threads: usize, w: usize, h: usize) -> Measured {
    let target = SosVpxConfig::default_bitrate_kbps(w, h);
    let cfg = SosVpxConfig {
        width: w,
        height: h,
        codec,
        threads,
        cpu_used,
        bitrate_kbps: target,
        keyframe_interval: None,
    };
    let mut enc = match SosVpxEncoder::new(cfg) {
        Ok(e) => e,
        Err(e) => {
            println!("    ({} {}x{} cpu={} th={} 初始化失败: {})", codec_name(codec), w, h, cpu_used, threads, e);
            return Measured { enc_ms: f64::NAN, enc_max_ms: f64::NAN, kbps: 0.0, drops: 0 };
        }
    };
    let frames = 60usize;
    let mut bytes = 0usize;
    let mut ns = 0u128;
    let mut max_ns = 0u128;
    let mut drops = 0usize;
    let origin = Instant::now();
    for i in 0..frames {
        let yuv = make_desktop(w, h, i);
        let pts = origin.elapsed().as_millis() as i64;
        let t = Instant::now();
        if let Ok(out) = enc.encode(&yuv, pts) {
            let d = t.elapsed();
            ns += d.as_nanos();
            max_ns = max_ns.max(d.as_nanos());
            drops += enc.take_dropped_frames();
            for (b, _) in out {
                bytes += b.len();
            }
        }
    }
    let secs = origin.elapsed().as_secs_f64().max(0.001);
    Measured {
        enc_ms: ns as f64 / 1e6 / frames as f64,
        enc_max_ms: max_ns as f64 / 1e6,
        kbps: bytes as f64 * 8.0 / 1000.0 / secs,
        drops,
    }
}

fn codec_name(c: SosCodec) -> &'static str {
    match c {
        SosCodec::Vp8 => "VP8",
        SosCodec::Vp9 => "VP9",
    }
}

/// 本机核数，用于把结论外推到目标机
#[test]
fn t0_host_info() {
    let cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(0);
    println!("本机逻辑核数: {}", cpus);
    println!("recommended_threads() = {}", SosVpxConfig::recommended_threads());
    println!();
    println!("目标机（日志）: 4 核, 1600x900, enc=43.5ms @ VP9 threads=2 cpu_used=7");
    println!("30fps 预算: spf=33.3ms，采集 5.0 + 转换 2.0 = 7ms → 编码必须 ≤ 26ms");
}

/// **核心实验**：1600x900 下 VP9 的 threads × cpu_used 网格
#[test]
fn t1_vp9_grid_1600x900() {
    let (w, h) = (1600usize, 900usize);
    println!("VP9 {}x{} (目标 {}kbps)  —— 编码 ≤26ms 才能跑到 30fps", w, h, SosVpxConfig::default_bitrate_kbps(w, h));
    println!(
        "{:>8} {:>8} {:>10} {:>10} {:>10} {:>8} {:>9}",
        "threads", "cpu-used", "enc_avg", "enc_max", "kbps", "drops", "能否30fps"
    );
    for threads in [2usize, 3, 4] {
        for cpu_used in [7u32, 8, 9] {
            let m = measure(SosCodec::Vp9, cpu_used, threads, w, h);
            println!(
                "{:>8} {:>8} {:>9.1}ms {:>9.1}ms {:>10.0} {:>8} {:>9}",
                threads,
                cpu_used,
                m.enc_ms,
                m.enc_max_ms,
                m.kbps,
                m.drops,
                if m.enc_ms <= 26.0 { "是" } else { "否" }
            );
        }
    }
    println!("\n注：cpu-used 越大越快。VP9 realtime 模式上限为 9。");
}

/// **对照实验**：VP8 在同分辨率下的吞吐
///
/// 若 VP8 明显更快，那么「编码跟不上时自动降级到 VP8」就比
/// 硬扛 VP9 更有价值 —— 画面更流畅，而 1600x900 下 VP8 的画质损失
/// 远小于 20fps 卡顿带来的损失。
#[test]
fn t2_vp8_vs_vp9_1600x900() {
    let (w, h) = (1600usize, 900usize);
    println!("VP8 vs VP9 @ {}x{}", w, h);
    println!(
        "{:>6} {:>8} {:>8} {:>10} {:>10} {:>8}",
        "codec", "threads", "cpu-used", "enc_avg", "enc_max", "kbps"
    );
    for (codec, cpu_used) in [
        (SosCodec::Vp8, 12u32),
        (SosCodec::Vp8, 14),
        (SosCodec::Vp9, 7),
        (SosCodec::Vp9, 9),
    ] {
        let m = measure(codec, cpu_used, 3, w, h);
        println!(
            "{:>6} {:>8} {:>8} {:>9.1}ms {:>9.1}ms {:>8.0}",
            codec_name(codec),
            3,
            cpu_used,
            m.enc_ms,
            m.enc_max_ms,
            m.kbps
        );
    }
}

/// 1080p 的同一张网格，确认分辨率对耗时的影响是否线性
#[test]
fn t3_vp9_grid_1920x1080() {
    let (w, h) = (1920usize, 1080usize);
    println!("VP9 {}x{} (目标 {}kbps)", w, h, SosVpxConfig::default_bitrate_kbps(w, h));
    println!(
        "{:>8} {:>8} {:>10} {:>10} {:>10} {:>8}",
        "threads", "cpu-used", "enc_avg", "enc_max", "kbps", "drops"
    );
    for threads in [2usize, 3, 4] {
        for cpu_used in [8u32, 9] {
            let m = measure(SosCodec::Vp9, cpu_used, threads, w, h);
            println!(
                "{:>8} {:>8} {:>9.1}ms {:>9.1}ms {:>10.0} {:>8}",
                threads, cpu_used, m.enc_ms, m.enc_max_ms, m.kbps, m.drops
            );
        }
    }
}
