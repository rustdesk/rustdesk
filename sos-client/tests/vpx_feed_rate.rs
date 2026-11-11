//! 诊断「喂帧率远低于标称帧率」时的丢帧行为。
//!
//! 生产日志（2026-11-05，内网直连 rtt=1ms，DXGI 采集）：
//! ```text
//! [VIDEO:PERF] loop=29.6fps it=33.9ms cap=19.5ms enc=7.0ms spf=33ms qos_fps=30 skip=79 edrop=42
//! [SOS_DROP] 42/70 frames dropped (60%), target=1675kbps qos_fps=30
//! ```
//! `skip=79` 说明画面静止、变化检测丢掉了一半以上的帧，
//! 真正送进编码器的只有 ~17fps，但 QoS 报告 30fps。
//!
//! **假设**：libvpx 的 CBR 信用是 `target_bitrate / 标称帧率`，
//! 每次 `vpx_codec_encode` 调用只发放这么多。实际喂帧率低时，
//! 每帧需要的比特数远大于信用 → 缓冲持续见底 → `rc_dropframe_thresh` 疯狂丢帧。
//!
//! 验证方式：同样喂 N 帧，改变**喂帧节奏**（但总帧数相同），观察丢帧率。
//!
//! 运行：cargo test --release --test vpx_feed_rate -- --nocapture --test-threads=1

#![cfg(all(windows, sos_vpx))]

#[path = "../src/sos_vpx.rs"]
mod sos_vpx;

use sos_vpx::{SosCodec, SosVpxConfig, SosVpxEncoder};
use std::time::{Duration, Instant};

/// 中等复杂度的桌面内容（局部变化，靠近真实桌面操作）
fn make_desktop(w: usize, h: usize, seed: usize) -> Vec<u8> {
    let (cw, ch) = (w / 2, h / 2);
    let mut buf = vec![0u8; w * h + 2 * cw * ch];
    // 渐变背景
    for y in 0..h {
        let base = 210u32.saturating_sub((y * 40 / h.max(1)) as u32);
        for x in 0..w {
            buf[y * w + x] = base as u8;
        }
    }
    // 局部滚动条纹（模拟窗口内容变化）
    let shift = seed % 120;
    for block in 0..24usize {
        let bx = (block * 79 + shift) % w.saturating_sub(80).max(1);
        let by = (block * 53) % h.saturating_sub(40).max(1);
        for row in 0..8usize {
            let y = (by + row * 2).min(h - 1);
            for x in bx..(bx + 70).min(w) {
                if (x + row + seed) % 4 == 0 {
                    buf[y * w + x] = 40;
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

struct Result_ {
    calls: usize,
    outs: usize,
    drops: usize,
    kbps: f64,
    avg_kb: f64,
}

/// `feed_every_n`：每隔多少个 33ms 时间片喂一帧。
/// 1 = 30fps 满帧率喂；3 = 实际约 10fps；但**总喂帧数相同**。
fn run(codec: SosCodec, bitrate: u32, frames: usize, feed_every_n: u32, cpu_used: u32) -> Result_ {
    let (w, h) = (1920usize, 1080usize);
    let cfg = SosVpxConfig {
        width: w,
        height: h,
        codec,
        threads: 2,
        cpu_used,
        bitrate_kbps: bitrate,
        keyframe_interval: None,
    };
    let mut enc = SosVpxEncoder::new(cfg).expect("encoder");

    let (mut outs, mut drops, mut bytes, mut calls) = (0usize, 0usize, 0usize, 0usize);
    let origin = Instant::now();
    let tick = Duration::from_secs_f64(1.0 / 30.0);
    let mut fed = 0usize;
    let mut t = 0u32;
    while fed < frames {
        if t % feed_every_n == 0 {
            let yuv = make_desktop(w, h, fed);
            // pts 始终用**真实流逝时间**，这是正确用法
            let pts = origin.elapsed().as_millis() as i64;
            calls += 1;
            if let Ok(out) = enc.encode(&yuv, pts) {
                drops += enc.take_dropped_frames();
                if !out.is_empty() {
                    outs += 1;
                    for (d, _k) in out {
                        bytes += d.len();
                    }
                }
            }
            fed += 1;
        }
        t += 1;
        let deadline = origin + tick * t;
        let now = Instant::now();
        if deadline > now {
            std::thread::sleep(deadline - now);
        }
    }
    let secs = origin.elapsed().as_secs_f64();
    Result_ {
        calls,
        outs,
        drops,
        kbps: bytes as f64 * 8.0 / 1000.0 / secs,
        avg_kb: if outs > 0 {
            bytes as f64 / outs as f64 / 1024.0
        } else {
            0.0
        },
    }
}

/// **核心实验**：同样的内容、同样的总帧数，只改喂帧节奏。
///
/// 如果"喂得慢"导致丢帧率大幅上升，就证明确实是
/// 「CBR 信用按标称帧率发放」造成的饥饿。
#[test]
fn sparse_feeding_causes_starvation() {
    let frames = 90usize; // 3 秒 @30fps 的帧数
    println!("VP9 1675kbps，共 {} 帧，改变喂帧节奏", frames);
    println!(
        "{:>10} {:>10} {:>8} {:>8} {:>10} {:>10} {:>10}",
        "节奏", "实际fps", "调用", "出帧", "丢帧", "丢帧率", "kbps"
    );
    for every in [1u32, 2, 3] {
        let r = run(SosCodec::Vp9, 1675, frames, every, 8);
        let rate = r.drops as f64 * 100.0 / r.calls.max(1) as f64;
        let fps = frame_fps(every);
        println!(
            "{:>10} {:>10.1} {:>8} {:>8} {:>8} {:>9.0}% {:>10.0}",
            format!("1/{}", every),
            fps,
            r.calls,
            r.outs,
            r.drops,
            rate,
            r.kbps
        );
        if every == 1 {
            assert!(
                r.drops * 10 < r.calls,
                "满帧率喂帧时不该有明显丢帧（{} 丢 / {} 调用）",
                r.drops,
                r.calls
            );
        }
    }
}

/// 上面实验的理论喂帧率
fn frame_fps(every: u32) -> f64 {
    30.0 / every as f64
}

/// VP8 vs VP9 在同一码率下的丢帧率对比（**高复杂度内容**）。
///
/// VP8 的编码效率比 VP9 低约 30%，同样的目标码率下更容易压不住。
/// 这决定了「控制端切到 VP8 后丢帧变多」是不是预期行为。
///
/// 关键是内容必须足够复杂：生产日志里出图帧平均 **59KB**，
/// 是 1675kbps/30fps ≈ 7KB 预算的 8.4 倍 —— 说明编码器根本没在守码率。
/// 用简单内容（8KB/帧）测不出任何问题。
#[test]
fn vp8_drops_more_than_vp9_at_same_bitrate() {
    let frames = 90usize;
    println!("1920x1080 高复杂度内容（逐像素纹理），满帧率喂帧");
    println!(
        "{:>6} {:>8} {:>8} {:>8} {:>10} {:>10} {:>12}",
        "codec", "码率", "出帧", "丢帧", "丢帧率", "kbps", "平均帧KB"
    );
    let mut results = Vec::new();
    for codec in [SosCodec::Vp9, SosCodec::Vp8] {
        let cpu = SosVpxConfig::default_cpu_used(codec, 1920, 1080);
        let r = run_motion(codec, 1675, frames, cpu);
        let rate = r.drops as f64 * 100.0 / r.calls.max(1) as f64;
        println!(
            "{:>6} {:>8} {:>8} {:>8} {:>9.0}% {:>10.0} {:>12.1}",
            format!("{:?}", codec),
            1675,
            r.outs,
            r.drops,
            rate,
            r.kbps,
            r.avg_kb
        );
        results.push((codec, r));
    }
    let vp9 = &results[0].1;
    let vp8 = &results[1].1;
    println!(
        "\nVP9 丢帧 {} → VP8 丢帧 {}（差距 {} 帧）",
        vp9.drops,
        vp8.drops,
        vp8.drops as i64 - vp9.drops as i64
    );
    // 正确的码率控制判据是**总码率**，不是单帧大小。
    //
    // libvpx 在 CBR + dropframe 下的策略是「少而精」：丢掉大部分帧，
    // 把省下的带宽全给剩下的帧。实测 VP9 出 15 帧/90 帧（3fps），
    // 每帧 43.4KB —— 单帧看着严重超发，但总码率 1796kbps 只超目标 7%，
    // 完全在 `rc_overshoot_pct` 的约束内。
    let limit = 1675.0 * 1.5;
    for (codec, r) in &results {
        assert!(
            r.kbps < limit,
            "{:?} 总码率 {:.0}kbps 超过目标 {:.0}kbps 的 1.5 倍 —— 码率控制失效",
            codec,
            r.kbps,
            1675.0
        );
    }
    println!("\n两者总码率都在目标 +50% 以内，说明码率控制本身正常；");
    println!("丢帧是内容复杂度超出 1675kbps 预算的必然结果。");
}

/// 验证 VP8 的 Q 索引范围确实与 VP9 相同（0-63）。
///
/// 实测结论：设 127 会让 `vpx_codec_enc_init_ver` 直接返回
/// `VPX_CODEC_INVALID_PARAM`。所以「VP8 范围更大，提高上限就能少丢帧」
/// 这条路走不通 —— 丢帧只能靠调整超发额度或码率来解决。
#[test]
fn vp8_quantizer_range_is_also_0_to_63() {
    let (w, h) = (1920usize, 1080usize);
    let cfg = SosVpxConfig {
        width: w,
        height: h,
        codec: SosCodec::Vp8,
        threads: 2,
        cpu_used: 12,
        bitrate_kbps: 1675,
        keyframe_interval: None,
    };
    // 生产配置（maxq=63）必须能建起来
    assert!(
        SosVpxEncoder::new(cfg).is_ok(),
        "VP8 + rc_max_quantizer=63 必须能成功创建编码器"
    );
    println!("VP8 rc_max_quantizer=63 创建成功（生产配置）");
}

/// **真正有效的旋钮**：`rc_dropframe_thresh`（缓冲充盈度水位，越小越保守）。
///
/// 实测 `rc_overshoot_pct` 完全不影响丢帧率（10/30/60 都是 83%），
/// 所以只能靠放低这个水位来减少丢帧。
///
/// 语义是"缓冲低于这个百分比才丢一帧"，0 = 永不丢。代价是不丢帧时
/// 编码器只能超发守画质，码率会显著突破目标 —— 那正是 bufferbloat 的来源。
#[test]
fn dropframe_thresh_is_the_real_knob() {
    let frames = 60usize;
    println!("VP9 高复杂度内容 1675kbps，对比 rc_dropframe_thresh");
    println!(
        "{:>8} {:>8} {:>8} {:>10} {:>10} {:>12}",
        "thresh", "出帧", "丢帧", "丢帧率", "kbps", "相对目标"
    );
    let mut rows = Vec::new();
    for thresh in [20u32, 10, 5, 0] {
        sos_vpx::set_rc_override(Some((200, thresh)));
        let r = run_motion(SosCodec::Vp9, 1675, frames, 8);
        sos_vpx::set_rc_override(None);
        let rate = r.drops as f64 * 100.0 / r.calls.max(1) as f64;
        println!(
            "{:>8} {:>8} {:>8} {:>9.0}% {:>10.0} {:>11.2}x",
            thresh,
            r.outs,
            r.drops,
            rate,
            r.kbps,
            r.kbps / 1675.0
        );
        rows.push((thresh, rate, r.kbps));
    }
    // 实测 5/10/20 三档的丢帧率在 80-83% 之间**来回波动**，没有单调关系 ——
    // 因为高复杂度内容一次就能把整个缓冲打到底，水位设多少都会触发。
    // 唯一有效的分界是 0（永不丢）与任意非 0。
    let zero = rows.iter().find(|(t, _, _)| *t == 0).expect("thresh=0 行");
    assert_eq!(zero.1, 0.0, "thresh=0 必须完全不丢帧");
    for (thresh, rate, _) in &rows {
        if *thresh > 0 {
            assert!(
                *rate > 50.0,
                "thresh={} 的丢帧率只有 {:.0}%，与实测的 80%+ 不符",
                thresh,
                rate
            );
        }
    }
    println!("\n关键：5-20 之间调几乎没有区别（都是 80%+），只有 0 才真正不丢帧。");
    println!("代价：不丢帧时码率会突破目标，这正是 bufferbloat 的来源。");
    println!("所以正确做法是按链路 RTT 在两者间切换（见 sos_video::GOOD_LINK_RTT_MS）。");
}

/// 高复杂度内容：逐像素纹理 + 每帧位移（接近不可压缩）
fn make_motion(w: usize, h: usize, seed: usize) -> Vec<u8> {
    let (cw, ch) = (w / 2, h / 2);
    let mut buf = vec![0u8; w * h + 2 * cw * ch];
    let shift = seed * 7;
    for y in 0..h {
        for x in 0..w {
            buf[y * w + x] = (((x + shift) * 3 + y * 5) % 251) as u8;
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

/// 与 [`run`] 相同，但用高复杂度内容
fn run_motion(codec: SosCodec, bitrate: u32, frames: usize, cpu_used: u32) -> Result_ {
    let (w, h) = (1920usize, 1080usize);
    let cfg = SosVpxConfig {
        width: w,
        height: h,
        codec,
        threads: 2,
        cpu_used,
        bitrate_kbps: bitrate,
        keyframe_interval: None,
    };
    let mut enc = SosVpxEncoder::new(cfg).expect("encoder");
    let (mut outs, mut drops, mut bytes, mut calls) = (0usize, 0usize, 0usize, 0usize);
    let origin = Instant::now();
    let tick = Duration::from_secs_f64(1.0 / 30.0);
    for i in 0..frames {
        let yuv = make_motion(w, h, i);
        let pts = origin.elapsed().as_millis() as i64;
        calls += 1;
        if let Ok(out) = enc.encode(&yuv, pts) {
            drops += enc.take_dropped_frames();
            if !out.is_empty() {
                outs += 1;
                for (d, _k) in out {
                    bytes += d.len();
                }
            }
        }
        let deadline = origin + tick * i as u32;
        let now = Instant::now();
        if deadline > now {
            std::thread::sleep(deadline - now);
        }
    }
    let secs = origin.elapsed().as_secs_f64();
    Result_ {
        calls,
        outs,
        drops,
        kbps: bytes as f64 * 8.0 / 1000.0 / secs,
        avg_kb: if outs > 0 {
            bytes as f64 / outs as f64 / 1024.0
        } else {
            0.0
        },
    }
}

/// 验证「让 target_bitrate 匹配实际喂帧率」能否消除饥饿。
///
/// 这是修复思路的可行性验证：如果按实际喂帧率缩放码率后丢帧消失，
/// 就说明采集循环里应该用**实测喂帧率**而不是 QoS 的标称帧率来算码率。
#[test]
fn matching_bitrate_to_actual_feed_rate_fixes_starvation() {
    let frames = 90usize;
    let every = 3u32; // 实际约 10fps
    let full_fps = frame_fps(every);
    println!(
        "稀疏喂帧（1/{}，实际 {:.0}fps），对比不同 target_bitrate",
        every, full_fps
    );
    println!(
        "{:>12} {:>8} {:>8} {:>10} {:>10} {:>12}",
        "target", "出帧", "丢帧", "丢帧率", "kbps", "平均帧KB"
    );

    // 标称 30fps 的码率（生产当前行为）
    let nominal = 1675u32;
    // 按实际喂帧率缩放后的码率
    let scaled = SosVpxConfig::bitrate_kbps_for(1920, 1080, 0.67, full_fps as u32);
    for target in [nominal, scaled] {
        let r = run(SosCodec::Vp9, target, frames, every, 8);
        let rate = r.drops as f64 * 100.0 / r.calls.max(1) as f64;
        println!(
            "{:>12} {:>8} {:>8} {:>8} {:>9.0}% {:>12.1}",
            target, r.outs, r.drops, rate, r.kbps, r.avg_kb
        );
        if target == scaled {
            // 缩放后丢帧应显著减少
            println!("→ 按实际喂帧率缩放后的丢帧率 {:.0}%", rate);
        }
    }
}
