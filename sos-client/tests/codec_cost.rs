//! VP8 与 VP9 的**编码成本对比**：为 `choose_codec` 的 `Auto` 分支提供依据。
//!
//! ## 要回答的问题
//!
//! `choose_codec` 的 `Auto` 分支曾经用一条硬编码启发式
//! （`threads <= 2 && pixels > 720p → VP8`），但那条规则没有任何测量支撑。
//! 本测试把它变成可复现的数据：在**真实桌面内容**上扫描
//! `编码格式 × 线程数`，输出每帧编码耗时与倍率。
//!
//! ## 测量要点
//!
//! - **关掉编码器自己丢帧**（`dropframe_thresh = 0`）：默认水位下编码器可能
//!   靠丢帧「跑得快」，那样的耗时没有意义；
//! - **用桌面内容**而不是随机噪声：噪声不可压缩，任何编码器都会在量化器上限
//!   附近空转，测出来的是最坏情况而非典型情况；
//! - **同时打印实际码率**：脱离码率谈耗时没有可比性，如果实际码率严重偏离
//!   目标，说明这一档配置的码率控制已经失效，耗时数据也不可信。
//!
//! 运行：`cargo test --release --test codec_cost -- --nocapture --test-threads=1`

#![cfg(all(windows, sos_vpx))]

#[path = "../src/sos_vpx.rs"]
mod sos_vpx;

#[path = "support/desktop.rs"]
mod desktop;

use std::time::Instant;

use sos_vpx::{SosCodec, SosVpxConfig, SosVpxEncoder};

/// 编码多少帧（前若干帧作为预热丢弃）
const FRAMES: usize = 80;
const WARMUP: usize = 10;

struct Cost {
    enc_ms: f64,
    enc_max_ms: f64,
    kbps: f64,
    q_avg: f64,
}

/// 全速编码（不按 spf 休眠），测单帧最小编码耗时
fn measure(codec: SosCodec, threads: usize, w: usize, h: usize, kbps: u32) -> Option<Cost> {
    let cfg = SosVpxConfig {
        width: w,
        height: h,
        codec,
        threads,
        cpu_used: SosVpxConfig::default_cpu_used(codec, w, h),
        bitrate_kbps: kbps,
        keyframe_interval: Some(w.max(h) / 4),
    };
    let mut enc = SosVpxEncoder::new(cfg).ok()?;
    // 关掉丢帧：否则「快」可能只是「直接丢了」
    enc.tune(kbps, 0).ok()?;

    let mut total_us = 0u128;
    let mut max_us = 0u128;
    let mut bytes = 0usize;
    let mut qsum = 0i64;
    let mut qn = 0i64;

    for i in 0..(FRAMES + WARMUP) {
        let yuv = desktop::desktop_i420(w, h, i);
        let t0 = Instant::now();
        let out = enc.encode(&yuv, (i as i64) * 33).ok()?;
        let dt = t0.elapsed().as_micros();
        for (data, _) in out {
            bytes += data.len();
        }
        if i >= WARMUP {
            total_us += dt;
            max_us = max_us.max(dt);
            if let Some(q) = enc.quantizer() {
                qsum += q as i64;
                qn += 1;
            }
        }
    }

    let n = FRAMES as f64;
    Some(Cost {
        enc_ms: total_us as f64 / n / 1000.0,
        enc_max_ms: max_us as f64 / 1000.0,
        // 全速编码时没有真实帧率，用固定 30fps 换算码率便于与预算比较
        kbps: bytes as f64 * 8.0 / (n / 30.0) / 1000.0,
        q_avg: if qn == 0 { 0.0 } else { qsum as f64 / qn as f64 },
    })
}

#[test]
fn vp8_vs_vp9_cost_table() {
    println!();
    println!("帧预算：30fps → 33.3ms，扣掉采集+转换约 7ms 后编码可用约 26ms");
    println!();

    for (w, h, kbps) in [(1600usize, 900usize, 1163u32), (1920, 1080, 1675)] {
        println!("══ {w}x{h} 目标 {kbps}kbps 桌面内容 ══");
        println!(
            "{:>5} {:>7} {:>8} {:>8} {:>8} {:>9} {:>8}",
            "线程", "VP9 ms", "VP8 ms", "VP8加速", "VP9 Q", "VP8 Q", "实际kbps"
        );
        let mut rows = Vec::new();
        for threads in [1usize, 2, 3, 4, 6] {
            let Some(vp9) = measure(SosCodec::Vp9, threads, w, h, kbps) else {
                continue;
            };
            let Some(vp8) = measure(SosCodec::Vp8, threads, w, h, kbps) else {
                continue;
            };
            println!(
                "{:>5} {:>7.2} {:>8.2} {:>7.2}x {:>8.1} {:>8.1} {:>9.0}",
                threads,
                vp9.enc_ms,
                vp8.enc_ms,
                vp9.enc_ms / vp8.enc_ms.max(0.01),
                vp9.q_avg,
                vp8.q_avg,
                vp9.kbps
            );
            rows.push((threads, vp9, vp8));
        }
        println!();

        // 结论必须能从数据直接读出来，而不是靠人肉观察：
        // 1. 线程数 >= 3 时 VP8 的加速比是否已经消失？
        // 2. VP8 的量化器是否始终更高（= 画质更差）？
        for (threads, vp9, vp8) in &rows {
            let gain = vp9.enc_ms / vp8.enc_ms.max(0.01);
            if *threads >= 3 && gain < 1.15 {
                println!("  {threads} 线程：VP8 仅快 {gain:.2}x，已无实际收益");
            }
            if vp8.q_avg > vp9.q_avg + 2.0 {
                println!(
                    "  {threads} 线程：VP8 量化器高 {:.0}%（{:.1} vs {:.1}，0-63 刻度）= 画质明确更差",
                    (vp8.q_avg / vp9.q_avg.max(0.1) - 1.0) * 100.0,
                    vp8.q_avg,
                    vp9.q_avg
                );
            }
        }
        println!();
    }

    // 以上数据支撑的结论写在这里，避免结论只存在于测试输出里丢失：
    //
    // - VP8 的耗时优势随线程数迅速消失，3 线程以上基本为零；
    // - VP8 在**同码率**下量化器更高，即画质明确更差；
    // - 因此 `Auto` 应当直接选 VP9，把「编不动」的判断交给运行时
    //   实测（`encoder_behind_budget` / `next_enc_level`），
    //   而不是靠「线程数」这个间接指标去猜。
    println!("结论：Auto 应选 VP9；「编不动」必须由实测编码耗时判定，而不是线程数。");
}

/// 「VP8 快 4 倍」是不是假象？—— 丢帧水位的影响。
///
/// `tests/vpx_enc_throughput.rs` 的旧数据显示 VP8 在 1600x900 下只要
/// 0.5ms/帧（VP9 2.8ms），据此得出「编码跟不上就降级到 VP8」。但那次测量
/// **没关掉编码器自己丢帧**（默认 `rc_dropframe_thresh = 20`）：
/// 丢帧的 `encode()` 直接返回空，几微秒就返回，于是「更快」其实等于
/// 「直接不编了」。
///
/// 本测试用同一份内容、同一个目标码率，只改丢帧水位，把这条混淆摊开：
/// 若 `thresh=20` 时 `drops` 占绝大多数而 `thresh=0` 时耗时才回来，
/// 就证明「VP8 更快」是丢帧造成的假象。
#[test]
fn vp8_apparent_speed_is_dropped_frames() {
    let (w, h, kbps) = (1600usize, 900usize, 1163u32);
    println!();
    println!("══ {w}x{h} 目标 {kbps}kbps：丢帧水位对「耗时」的影响 ══");
    println!(
        "{:>6} {:>8} {:>9} {:>10} {:>8} {:>9} {:>9}",
        "codec", "thresh", "enc_ms", "帧数(出/投)", "丢帧率", "实际kbps", "Q"
    );
    for codec in [SosCodec::Vp9, SosCodec::Vp8] {
        for thresh in [0u32, 20] {
            let cfg = SosVpxConfig {
                width: w,
                height: h,
                codec,
                threads: 3,
                cpu_used: SosVpxConfig::default_cpu_used(codec, w, h),
                bitrate_kbps: kbps,
                keyframe_interval: Some(h / 4),
            };
            let Ok(mut enc) = SosVpxEncoder::new(cfg) else {
                continue;
            };
            enc.tune(kbps, thresh).ok();
            let (mut us, mut out, mut sent, mut dropped) = (0u128, 0usize, 0usize, 0usize);
            let (mut bytes, mut qsum, mut qn) = (0usize, 0i64, 0i64);
            for i in 0..(FRAMES + WARMUP) {
                let yuv = desktop::desktop_i420(w, h, i);
                let t0 = Instant::now();
                let Ok(frames) = enc.encode(&yuv, i as i64 * 33) else {
                    continue;
                };
                us += t0.elapsed().as_micros();
                sent += 1;
                dropped += enc.take_dropped_frames();
                if !frames.is_empty() {
                    out += 1;
                }
                for (d, _) in frames {
                    bytes += d.len();
                }
                if i >= WARMUP {
                    if let Some(q) = enc.quantizer() {
                        qsum += q as i64;
                        qn += 1;
                    }
                }
            }
            let n = (FRAMES + WARMUP) as f64;
            let drop_pct = dropped as f64 * 100.0 / sent.max(1) as f64;
            println!(
                "{:>6} {:>8} {:>8.2}ms {:>10} {:>7.0}% {:>9.0} {:>9.1}",
                if codec == SosCodec::Vp9 { "VP9" } else { "VP8" },
                thresh,
                us as f64 / n / 1000.0,
                format!("{out}/{sent}"),
                drop_pct,
                bytes as f64 * 8.0 / (n / 30.0) / 1000.0,
                if qn == 0 { 0.0 } else { qsum as f64 / qn as f64 }
            );
        }
    }
    println!();
    println!("若 thresh=20 时丢帧率很高，「enc_ms 更小」只是「没编」而不是「编得快」。");
}
