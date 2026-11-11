//! VP9 **屏幕内容调优**（`VP9E_CONTENT_SCREEN`）的 A/B 实测。
//!
//! ## 要回答的问题
//!
//! libvpx 提供 `VP9E_SET_TUNE_CONTENT = VP9E_CONTENT_SCREEN`，对桌面内容
//! 启用 palette 与 intra block copy。上游 `libs/scrap` 与我们的 `sos_vpx`
//! **都没有设置它**（上游只设了 cpu-used / row-mt / tile-columns）。
//!
//! 「有个选项」不等于「应该打开」：screen content 模式在 realtime 档位下
//! 也可能改变模式决策，未必更快。这里用同一份桌面内容、同一个目标码率
//! 对比：编码耗时、实际码率、量化器。
//!
//! ## 测量要点
//!
//! - 关掉编码器丢帧（`tune(kbps, 0)`）：否则「快」可能只是「直接丢了」；
//! - 用桌面内容而不是噪声（`support/desktop.rs`）：screen content 模式
//!   只对这类内容有意义；
//! - 同时看量化器：码率省下来但量化器升高 = 画质变差，不算收益。
//!
//! 运行：`cargo test --release --test vpx_screen_content -- --nocapture`

#![cfg(all(windows, sos_vpx))]

#[path = "../src/sos_vpx.rs"]
mod sos_vpx;

#[path = "support/desktop.rs"]
mod desktop;

use std::time::Instant;

use sos_vpx::{SosCodec, SosVpxConfig, SosVpxEncoder};

const FRAMES: usize = 80;
const WARMUP: usize = 10;

/// `VP9E_CONTENT_SCREEN`
const CONTENT_SCREEN: i32 = 1;

struct Cost {
    enc_ms: f64,
    kbps: f64,
    q_avg: f64,
    frames: usize,
}

fn measure(
    threads: usize,
    w: usize,
    h: usize,
    kbps: u32,
    tune_content: Option<i32>,
) -> Option<Cost> {
    sos_vpx::set_tune_content_override(tune_content);
    let cfg = SosVpxConfig {
        width: w,
        height: h,
        codec: SosCodec::Vp9,
        threads,
        cpu_used: SosVpxConfig::default_cpu_used(SosCodec::Vp9, w, h),
        bitrate_kbps: kbps,
        keyframe_interval: None,
    };
    let mut enc = SosVpxEncoder::new(cfg).ok()?;
    // 关掉丢帧：否则「快」可能只是「直接丢了」
    enc.tune(kbps, 0).ok()?;

    let mut total_us = 0u128;
    let mut bytes = 0usize;
    let mut qsum = 0i64;
    let mut qn = 0i64;
    let mut frames = 0usize;

    for i in 0..(FRAMES + WARMUP) {
        let yuv = desktop::desktop_i420(w, h, i);
        let t0 = Instant::now();
        let out = enc.encode(&yuv, (i as i64) * 33).ok()?;
        let dt = t0.elapsed().as_micros();
        let n = out.len();
        for (data, _) in out {
            bytes += data.len();
        }
        if i >= WARMUP {
            total_us += dt;
            frames += n;
            if let Some(q) = enc.quantizer() {
                qsum += q as i64;
                qn += 1;
            }
        }
    }
    sos_vpx::set_tune_content_override(None);

    let n = FRAMES as f64;
    Some(Cost {
        enc_ms: total_us as f64 / n / 1000.0,
        // 全速编码时没有真实帧率，按 30fps 换算便于与预算比较
        kbps: bytes as f64 * 8.0 / (n / 30.0) / 1000.0,
        q_avg: if qn == 0 {
            0.0
        } else {
            qsum as f64 / qn as f64
        },
        frames,
    })
}

#[test]
fn vp9_screen_content_tuning() {
    println!();
    println!(
        "VP9 cpu_used={}（屏幕内容调优 A/B；3 线程对应现场 4 核机，{} 线程为本机自动值）",
        SosVpxConfig::default_cpu_used(SosCodec::Vp9, 1600, 900),
        SosVpxConfig::recommended_threads()
    );

    for threads in [3usize, SosVpxConfig::recommended_threads()] {
        println!();
        println!("── threads={threads} ──");
        println!(
            "{:>9} {:>6} {:>9} {:>9} {:>8} {:>9} {:>7}",
            "分辨率", "目标", "默认 ms", "screen ms", "提速", "默认 kbps", "screen"
        );

        for (w, h, kbps) in [
            (1600usize, 900usize, 1163u32),
            (1600, 900, 2604),
            (1920, 1080, 1675),
        ] {
            let Some(base) = measure(threads, w, h, kbps, None) else {
                println!("{w}x{h} 默认配置初始化失败，跳过");
                continue;
            };
            let Some(sc) = measure(threads, w, h, kbps, Some(CONTENT_SCREEN)) else {
                println!("{w}x{h} screen 配置初始化失败，跳过");
                continue;
            };
            println!(
                "{:>9} {:>6} {:>9.2} {:>9.2} {:>7.2}x {:>9.0} {:>7.0}",
                format!("{w}x{h}"),
                kbps,
                base.enc_ms,
                sc.enc_ms,
                base.enc_ms / sc.enc_ms.max(0.01),
                base.kbps,
                sc.kbps
            );
            println!(
                "{:>9} {:>6} {:>9.1} {:>9.1} {:>8} {:>9} {:>7}",
                "",
                "量化器",
                base.q_avg,
                sc.q_avg,
                format!("出帧 {}/{}", base.frames, sc.frames),
                "",
                ""
            );
        }
    }
    println!();
}
