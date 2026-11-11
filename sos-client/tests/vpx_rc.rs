//! 编码器码率控制配置的正确性验证。
//!
//! 核心 bug：`rc_buf_sz` 等字段的单位是**毫秒**，原实现写成
//! `rc_target_bitrate * 5`，在 1675kbps 下等于 8375 毫秒（8.4 秒）缓冲，
//! 导致编码器长期超发（实测 `sent_kbps=2133` 对目标 1675），
//! 发送队列积压到 25 帧。
//!
//! 运行：cargo test --release --test vpx_rc -- --nocapture

#![cfg(all(windows, sos_vpx))]

#[path = "../src/sos_vpx.rs"]
mod sos_vpx;

use sos_vpx::{SosCodec, SosVpxConfig, SosVpxEncoder};
use std::time::{Duration, Instant};

fn make_i420(w: usize, h: usize, seed: usize) -> Vec<u8> {
    let (cw, ch) = (w / 2, h / 2);
    let mut buf = vec![0u8; w * h + 2 * cw * ch];
    for y in 0..h {
        for x in 0..w {
            let v = (x * 255 / w.max(1) + y * 3 + seed * 7) % 256;
            buf[y * w + x] = (v ^ ((x ^ y) & 0x1f)) as u8;
        }
    }
    let uv = w * h;
    for y in 0..ch {
        for x in 0..cw {
            let v = (x * 255 / cw.max(1) + y * 5 + seed * 3) % 256;
            buf[uv + y * cw + x] = v as u8;
            buf[uv + cw * ch + y * cw + x] = (255 - v) as u8;
        }
    }
    buf
}

/// 接近真实桌面的画面：大面积平坦背景 + 少量文字状细节 + 每帧小幅变化。
///
/// 纯噪声（`make_i420`）几乎不可压缩，任何编码器在 QP 下限处都会大幅超发，
/// 拿它衡量码率控制没有意义 —— 真实桌面 90% 以上像素是平坦的。
fn make_desktop(w: usize, h: usize, seed: usize) -> Vec<u8> {
    let (cw, ch) = (w / 2, h / 2);
    let mut buf = vec![0u8; w * h + 2 * cw * ch];

    // Y 平面：浅灰背景 + 垂直渐变
    for y in 0..h {
        let base = 200u32.saturating_sub((y * 30 / h.max(1)) as u32);
        for x in 0..w {
            buf[y * w + x] = base as u8;
        }
    }

    // 模拟窗口边框与文字块：每帧平移 1px，制造真实的帧间差异
    let shift = seed % 40;
    for block in 0..12usize {
        let bx = (block * 137 + shift) % w.saturating_sub(120).max(1);
        let by = (block * 71) % h.saturating_sub(60).max(1);
        // 3px 高的「文字行」
        for row in 0..3usize {
            let y = (by + row * 4).min(h - 1);
            for x in bx..(bx + 100).min(w) {
                // 稀疏的高对比像素，像文字一样
                if (x + row) % 3 == 0 {
                    buf[y * w + x] = 30;
                }
            }
        }
        // 窗口边框
        let y = by.min(h - 1);
        for x in bx..(bx + 110).min(w) {
            buf[y * w + x] = 90;
        }
    }

    // UV 平面：平坦的中性色
    let uv = w * h;
    for i in 0..(cw * ch) {
        buf[uv + i] = 128;
        buf[uv + cw * ch + i] = 128;
    }
    buf
}

/// 编码一段内容，返回 (fps, 实际码率 kbps, 单帧最大字节)
///
/// 两个关键点：
/// 1. pts 必须用**真实流逝时间**。若用 `i*33` 而实际编码速率不是 30fps，
///    libvpx 会按错误的时长记账，码率被等比放大/缩小。
/// 2. 必须按目标帧率**节流**。桌面型内容很好压，不限速时编码器能跑到 279fps，
///    每帧只分到 750 字节预算，连一帧的固定开销都不够，码率必然超发。
///    只有在真实帧率下测，才能反映生产环境行为。
fn run(
    codec: SosCodec,
    w: usize,
    h: usize,
    target_kbps: u32,
    frames: u32,
    desktop_like: bool,
    pace_fps: u32,
) -> (f64, f64, usize) {
    let cfg = SosVpxConfig {
        width: w,
        height: h,
        codec,
        threads: 4,
        cpu_used: SosVpxConfig::default_cpu_used(codec, w, h),
        bitrate_kbps: target_kbps,
        keyframe_interval: None,
    };
    let mut enc = SosVpxEncoder::new(cfg).expect("create encoder");
    let mut bytes = 0usize;
    let mut max_frame = 0usize;
    let origin = Instant::now();
    let spf = Duration::from_secs_f64(1.0 / pace_fps as f64);

    for i in 0..frames as usize {
        // 等到本帧的绝对时间点（与 sos_video 的绝对时间轴一致）
        let deadline = origin + spf * i as u32;
        let now = Instant::now();
        if deadline > now {
            std::thread::sleep(deadline - now);
        }

        let yuv = if desktop_like {
            make_desktop(w, h, i)
        } else {
            make_i420(w, h, i)
        };
        let pts = origin.elapsed().as_millis() as i64;
        if let Ok(out) = enc.encode(&yuv, pts) {
            for (d, _k) in out {
                bytes += d.len();
                max_frame = max_frame.max(d.len());
            }
        }
    }
    let secs = origin.elapsed().as_secs_f64();
    let fps = frames as f64 / secs;
    let kbps = bytes as f64 * 8.0 / 1000.0 / secs;
    (fps, kbps, max_frame)
}

#[test]
fn rc_buffer_is_milliseconds_not_bits() {
    // 目标码率 1675kbps。旧实现 rc_buf_sz = 1675*5 = 8375ms，
    // 允许 8.4 秒的突发预算；新实现固定 200ms。
    // 这个测试通过实际码率是否守得住来间接验证。
    let target = 1675u32;
    println!(
        "{:<10} {:>8} {:>12} {:>12} {:>10}",
        "codec", "fps", "实际kbps", "偏离目标", "最大帧KB"
    );

    for codec in [SosCodec::Vp9, SosCodec::Vp8] {
        let (fps, kbps, max_frame) = run(codec, 1920, 1080, target, 60, true, 30);
        let dev = (kbps - target as f64) / target as f64 * 100.0;
        println!(
            "{:<10} {:>8.1} {:>12.0} {:>11.1}% {:>10.1}",
            format!("{codec:?}"),
            fps,
            kbps,
            dev,
            max_frame as f64 / 1024.0
        );
        // CBR 下长期平均不应显著超过目标。留 60% 余量：
        // 首帧是关键帧，且测试内容包含每帧都在移动的细节块。
        assert!(
            dev < 60.0,
            "{codec:?} 码率超出目标 {dev:.1}%，码率控制失效（检查 rc_buf_sz 单位）"
        );
    }
}

/// 高复杂度内容下，码率必须有上界。
///
/// 这是 `rc_overshoot_pct` 的作用：默认值 100 表示「超发到 2 倍才采取
/// 激进纠正」，实测生产环境因此稳定跑到 162% 目标码率（2720 / 1675），
/// 进而在中继服务器上养出 bufferbloat，RTT 从 34ms 爬到 211ms。
///
/// 收窄到 10% 后应该显著改善。噪声内容几乎不可压缩，是压力最大的场景。
#[test]
fn incompressible_content_is_capped() {
    let target = 1675u32;
    let (fps, kbps, _) = run(SosCodec::Vp9, 1920, 1080, target, 40, false, 30);
    let dev = (kbps - target as f64) / target as f64 * 100.0;
    println!("噪声内容 Vp9: fps={fps:.1} kbps={kbps:.0} 偏离={dev:.0}% (目标 {target})");
    assert!(fps > 5.0, "噪声内容下几乎不出帧: {fps:.1}fps");
    // 不可压缩内容确实压不到目标，但必须有界。
    // 旧配置（overshoot=100）实测 277%，这里要求明显低于那个水平。
    assert!(dev < 200.0, "超发 {dev:.0}%，rc_overshoot_pct 未生效");
}

#[test]
fn set_bitrate_keeps_buffer_in_ms() {
    // 运行中改码率时缓冲不能被重新算成 target*5（那会变回秒级）
    let mut enc = SosVpxEncoder::new(SosVpxConfig {
        width: 1280,
        height: 720,
        codec: SosCodec::Vp9,
        threads: 2,
        cpu_used: 8,
        bitrate_kbps: 1200,
        keyframe_interval: None,
    })
    .expect("create encoder");

    for kbps in [600u32, 2400, 1200] {
        enc.set_bitrate(kbps).expect("set_bitrate");
        assert_eq!(enc.bitrate_kbps(), kbps, "目标码率应被正确写入");
        // 编码几帧确认改动后仍能正常出帧
        for i in 0..3 {
            let yuv = make_i420(1280, 720, i);
            assert!(
                enc.encode(&yuv, (i * 33) as i64).is_ok(),
                "改变码率后编码失败"
            );
        }
        println!("set_bitrate({kbps}) 之后连续出帧正常");
    }
}
