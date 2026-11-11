//! 诊断「码率未超目标却大量丢帧」。
//!
//! 生产日志现象：
//! ```text
//! sent_kbps=1610 qos_fps=30 spf=33ms skip=38 edrop=68
//! ```
//! 5 秒内输出 55 帧、丢 68 帧，而码率 1610 还**低于**目标 1675。
//! 说明丢帧不是码率驱动，而是别的因素。
//!
//! 怀疑：`rc_dropframe_thresh` 基于**缓冲充盈度百分比**判断，
//! 而 `rc_buf_sz` 只有 200ms。桌面静止时 DXGI 会跳过大量帧，
//! 实际编码间隔远大于 33ms，甚至超过整个缓冲窗口（200ms），
//! 导致 libvpx 的缓冲记账失真、误判为「欠载」而丢帧。
//!
//! 本测试对比不同 (buf_sz, dropframe_thresh) 组合在
//! **真实不均匀节奏**下的丢帧率。
//!
//! 运行：cargo test --release --test vpx_drop_diag -- --nocapture --test-threads=1

#![cfg(all(windows, sos_vpx))]

#[path = "../src/sos_vpx.rs"]
mod sos_vpx;

use sos_vpx::{SosCodec, SosVpxConfig, SosVpxEncoder};
use std::time::{Duration, Instant};

/// 桌面向内容：平背景 + 文字状细节
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

struct Outcome {
    outputs: usize,
    drops: usize,
    kbps: f64,
}

/// 复刻生产节奏：每 3 帧只有 1 帧真的送进编码器（模拟 DXGI 跳过），
/// 于是实际编码间隔在 33ms 与 100ms 之间交替。
fn run(buf_ms: u32, thresh: u32, irregular: bool, seconds: f64) -> Outcome {
    let (w, h) = (1920usize, 1080usize);
    sos_vpx::set_rc_override(Some((buf_ms, thresh)));
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
    // 构造已完成，恢复默认，避免影响同线程后续用例
    sos_vpx::set_rc_override(None);
    let (mut outputs, mut drops, mut bytes) = (0usize, 0usize, 0usize);

    let origin = Instant::now();
    let spf = Duration::from_secs_f64(1.0 / 30.0);
    let mut tick: u32 = 0;
    let mut fed: usize = 0;
    while origin.elapsed().as_secs_f64() < seconds {
        // 节奏：不均匀模式下每 3 个 tick 只喂 1 帧
        let should_feed = !irregular || tick % 3 == 0;
        if should_feed {
            let yuv = make_desktop(w, h, fed);
            let pts = origin.elapsed().as_millis() as i64;
            if let Ok(out) = enc.encode(&yuv, pts) {
                let d = enc.take_dropped_frames();
                drops += d;
                if out.is_empty() {
                    // 已在 take_dropped_frames 中计入
                } else {
                    outputs += 1;
                    for (b, _k) in out {
                        bytes += b.len();
                    }
                }
            }
            fed += 1;
        }
        tick += 1;
        let deadline = origin + spf * tick;
        let now = Instant::now();
        if deadline > now {
            std::thread::sleep(deadline - now);
        }
    }
    let secs = origin.elapsed().as_secs_f64();
    Outcome {
        outputs,
        drops,
        kbps: bytes as f64 * 8.0 / 1000.0 / secs,
    }
}

#[test]
fn drop_rate_under_irregular_cadence() {
    println!("目标 1675kbps，桌面向内容，5 秒");
    println!(
        "{:<10} {:>8} {:>10} {:>8} {:>8} {:>10} {:>8}",
        "节奏", "buf_ms", "threshold", "输出", "丢帧", "丢帧率", "kbps"
    );

    for (label, irregular) in [("均匀33ms", false), ("不均匀", true)] {
        for buf in [200u32, 600, 1000] {
            for thresh in [5u32, 20] {
                let r = run(buf, thresh, irregular, 5.0);
                let total = r.outputs + r.drops;
                let rate = if total > 0 {
                    r.drops as f64 * 100.0 / total as f64
                } else {
                    0.0
                };
                println!(
                    "{:<10} {:>8} {:>10} {:>8} {:>8} {:>9.0}% {:>8.0}",
                    label, buf, thresh, r.outputs, r.drops, rate, r.kbps
                );
            }
        }
    }
    println!();
    println!("验收标准：不均匀节奏下丢帧率应接近 0，且 kbps 不超过目标太多。");
}

/// 运动内容（近不可压缩）：斜向纹理 + 每帧位移
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

fn measure(bitrate: u32, thresh: u32, motion: bool, frames: usize) -> (f64, usize, usize) {
    measure_paced(bitrate, thresh, motion, frames, 30.0)
}

/// `feed_fps` 是**喂帧速率**。libvpx 的 CBR 信用是按编码调用次数发放的
/// （每次调用发放 `target_bitrate / framerate` 比特），
/// 所以喂得比标称帧率快，缓冲会被人为抽干 —— 这正是丢帧正反馈的来源。
fn measure_paced(
    bitrate: u32,
    thresh: u32,
    motion: bool,
    frames: usize,
    feed_fps: f64,
) -> (f64, usize, usize) {
    let (w, h) = (1920usize, 1080usize);
    sos_vpx::set_rc_override(Some((200, thresh)));
    let cfg = SosVpxConfig {
        width: w,
        height: h,
        codec: SosCodec::Vp9,
        threads: 5,
        cpu_used: 8,
        bitrate_kbps: bitrate,
        keyframe_interval: None,
    };
    let mut enc = SosVpxEncoder::new(cfg).expect("encoder");
    sos_vpx::set_rc_override(None);

    let (mut bytes, mut drops, mut outs) = (0usize, 0usize, 0usize);
    let origin = Instant::now();
    let spf = Duration::from_secs_f64(1.0 / feed_fps);
    for i in 0..frames {
        let deadline = origin + spf * i as u32;
        let now = Instant::now();
        if deadline > now {
            std::thread::sleep(deadline - now);
        }
        let yuv = if motion {
            make_motion(w, h, i)
        } else {
            make_desktop(w, h, i)
        };
        let pts = origin.elapsed().as_millis() as i64;
        if let Ok(out) = enc.encode(&yuv, pts) {
            drops += enc.take_dropped_frames();
            if out.is_empty() {
                continue;
            }
            outs += 1;
            for (d, _k) in out {
                bytes += d.len();
            }
        }
    }
    let secs = origin.elapsed().as_secs_f64();
    (bytes as f64 * 8.0 / 1000.0 / secs, drops, outs)
}

/// 隔离实验：`rc_dropframe_thresh` 到底是不是码率控制生效的**必要条件**？
///
/// 生产里 `rc_max_quantizer = 63` 与 `rc_dropframe_thresh = 20` 是同时加上的，
/// 从没单独验证过。如果只靠 quantizer 上限就能守住码率，
/// 那就应当把丢帧**彻底关掉**（用户要求「允许丢帧、尽量不丢」）。
#[test]
fn dropframe_thresh_is_not_required_for_rate_control() {
    println!("运动内容 60 帧（30fps 真实节奏），buf=200ms");
    println!(
        "{:>8} {:>8} {:>10} {:>8} {:>8} {:>8}",
        "目标", "thresh", "实际kbps", "偏差", "输出", "丢帧"
    );
    for target in [800u32, 3200] {
        for thresh in [0u32, 20] {
            let (kbps, drops, outs) = measure(target, thresh, true, 60);
            let dev = kbps / target as f64;
            println!(
                "{:>8} {:>8} {:>10.0} {:>8.2} {:>8} {:>8}",
                target, thresh, kbps, dev, outs, drops
            );
        }
    }
    println!();
    println!("桌面内容 60 帧：");
    println!(
        "{:>8} {:>8} {:>10} {:>8} {:>8} {:>8}",
        "目标", "thresh", "实际kbps", "偏差", "输出", "丢帧"
    );
    for target in [800u32, 3200] {
        for thresh in [0u32, 20] {
            let (kbps, drops, outs) = measure(target, thresh, false, 60);
            let dev = kbps / target as f64;
            println!(
                "{:>8} {:>8} {:>10.0} {:>8.2} {:>8} {:>8}",
                target, thresh, kbps, dev, outs, drops
            );
        }
    }
}

/// 喂帧过快是否会**自己把自己饿死**？
///
/// 生产里编码器丢帧后走 `continue`，跳过了帧间距补足，
/// 于是立刻又去采集下一帧 —— 编码调用频率可能高于标称 30fps。
/// 若下面的表格显示「喂得越快，丢得越多」，就证明存在丢帧正反馈。
#[test]
fn overfeeding_causes_self_inflicted_drops() {
    println!("桌面内容 60 帧，目标 1675kbps，thresh=20（生产配置），buf=200ms");
    println!(
        "{:>10} {:>10} {:>8} {:>8} {:>8} {:>10}",
        "喂帧fps", "实际kbps", "相对目标", "输出", "丢帧", "丢帧率"
    );
    for feed in [15.0f64, 30.0, 60.0, 120.0] {
        let (kbps, drops, outs) = measure_paced(1675, 20, false, 60, feed);
        let total = outs + drops;
        let rate = if total > 0 {
            drops as f64 * 100.0 / total as f64
        } else {
            0.0
        };
        println!(
            "{:>10.0} {:>10.0} {:>8.2} {:>8} {:>8} {:>9.0}%",
            feed,
            kbps,
            kbps / 1675.0,
            outs,
            drops,
            rate
        );
    }
    println!();
    println!("若丢帧率随喂帧速率上升，说明丢帧是管道自己造成的，不是内容造成的。");
}

/// 运行中反复 `set_bitrate()` 会不会打乱编码器的码率控制状态？
///
/// 生产里 QoS 的 `ratio` 每秒都会重新计算，只要变化超过 0.05 就调用
/// `set_bitrate()` —— 而它背后是 `vpx_codec_enc_config_set()`，
/// 每次调用都会在 libvpx 内部重走一遍配置流程。
#[test]
fn frequent_set_bitrate_does_not_cause_drops() {
    println!("桌面内容 90 帧，目标在 1400/1675/1950 之间轮转，buf=200ms thresh=20");
    println!(
        "{:>12} {:>10} {:>8} {:>8} {:>10}",
        "重配间隔", "实际kbps", "输出", "丢帧", "丢帧率"
    );
    for every in [0usize, 30, 10, 3] {
        let (w, h) = (1920usize, 1080usize);
        sos_vpx::set_rc_override(None);
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
        let (mut bytes, mut drops, mut outs) = (0usize, 0usize, 0usize);
        let origin = Instant::now();
        let spf = Duration::from_secs_f64(1.0 / 30.0);
        let targets = [1400u32, 1675, 1950];
        for i in 0..90usize {
            let deadline = origin + spf * i as u32;
            let now = Instant::now();
            if deadline > now {
                std::thread::sleep(deadline - now);
            }
            if every > 0 && i % every == 0 {
                enc.set_bitrate(targets[(i / every) % targets.len()])
                    .expect("set_bitrate");
            }
            let yuv = make_desktop(w, h, i);
            let pts = origin.elapsed().as_millis() as i64;
            if let Ok(out) = enc.encode(&yuv, pts) {
                drops += enc.take_dropped_frames();
                if out.is_empty() {
                    continue;
                }
                outs += 1;
                for (d, _k) in out {
                    bytes += d.len();
                }
            }
        }
        let secs = origin.elapsed().as_secs_f64();
        let kbps = bytes as f64 * 8.0 / 1000.0 / secs;
        let total = outs + drops;
        let rate = if total > 0 {
            drops as f64 * 100.0 / total as f64
        } else {
            0.0
        };
        let label = if every == 0 {
            "从不".to_string()
        } else {
            format!("每 {} 帧", every)
        };
        println!(
            "{:>12} {:>10.0} {:>8} {:>8} {:>9.0}%",
            label, kbps, outs, drops, rate
        );
    }
}
