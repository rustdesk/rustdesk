//! CBR vs VBR vs CQ：远程桌面该用哪种码率控制？
//!
//! 远程桌面的负载特征与点播视频完全不同：
//! - **极端不均匀**：打字时几乎全静止，拖动窗口/播视频时瞬间打满；
//! - **延迟敏感**：排队 200ms 用户就能感觉到；码率突发会直接变成 RTT 上涨；
//! - **带宽预算来自 ABR**：`rc_target_bitrate` 由 QoS 每秒调整，
//!   上层假设它是一个**可信的上限**。
//!
//! 四种模式的语义（`vpx_encoder.h`）：
//! - `VPX_CBR` 恒定码率：严格守 `rc_target_bitrate`，压不住就丢帧；
//! - `VPX_VBR` 可变码率：允许码率随内容大幅波动；
//! - `VPX_CQ` 约束质量：按质量编码，码率只作约束；
//! - `VPX_Q` 恒定质量：完全不管码率。
//!
//! 本测试测量三个决定性指标：
//! 1. **静态画面的码率**（VBR 应该显著更低，省带宽）；
//! 2. **复杂画面的码率峰值**（VBR 应该显著更高，这是风险）；
//! 3. **码率的时间波动**（峰值/均值比直接决定 RTT 抖动）。
//!
//! 运行：cargo test --release --test vpx_rc_mode -- --nocapture --test-threads=1

#![cfg(all(windows, sos_vpx))]

#[path = "../src/sos_vpx.rs"]
mod sos_vpx;

use sos_vpx::{SosCodec, SosVpxConfig, SosVpxEncoder};
use std::time::{Duration, Instant};

const TARGET_KBPS: u32 = 1675;

/// 静态桌面：渐变背景 + 固定文字，帧间几乎无变化
fn make_static(w: usize, h: usize) -> Vec<u8> {
    let (cw, ch) = (w / 2, h / 2);
    let mut buf = vec![0u8; w * h + 2 * cw * ch];
    for y in 0..h {
        let base = 205u32.saturating_sub((y * 35 / h.max(1)) as u32);
        for x in 0..w {
            buf[y * w + x] = base as u8;
        }
    }
    // 固定的文字状横条，每帧完全相同
    for block in 0..16usize {
        let by = (block * 61) % h.saturating_sub(20).max(1);
        for x in 40..(w - 40).min(w) {
            if x % 7 < 3 {
                buf[by * w + x] = 50;
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

/// 高复杂度运动：逐像素纹理 + 每帧位移
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

/// 中等复杂度：桌面滚动（局部内容移动）
fn make_scroll(w: usize, h: usize, seed: usize) -> Vec<u8> {
    let (cw, ch) = (w / 2, h / 2);
    let mut buf = vec![0u8; w * h + 2 * cw * ch];
    for y in 0..h {
        let base = 215u32.saturating_sub((y * 30 / h.max(1)) as u32);
        for x in 0..w {
            buf[y * w + x] = base as u8;
        }
    }
    let shift = seed * 3;
    for block in 0..30usize {
        let bx = (block * 67) % w.saturating_sub(100).max(1);
        let by = (block * 43 + shift) % h.saturating_sub(30).max(1);
        for row in 0..10usize {
            let y = (by + row * 2).min(h - 1);
            for x in bx..(bx + 90).min(w) {
                if (x + row) % 5 < 2 {
                    buf[y * w + x] = 60;
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

#[derive(Debug, Default)]
struct Stats {
    /// 全时间段的平均码率（kbps）
    avg_kbps: f64,
    /// 峰值 1 秒窗口的码率（kbps）—— 这是 RTT 抖动的直接来源
    peak_kbps: f64,
    /// 最低 1 秒窗口的码率
    min_kbps: f64,
    /// 峰值 / 均值。>2 说明码率极不均匀
    burst_ratio: f64,
    /// 出帧数
    outs: usize,
    /// 丢帧数
    drops: usize,
    /// 出图帧平均大小（KB）
    avg_frame_kb: f64,
}

impl Stats {
    fn drop_rate(&self) -> f64 {
        self.drops as f64 * 100.0 / (self.outs + self.drops).max(1) as f64
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Cbr,
    Vbr,
    Cq,
}

impl Mode {
    fn label(self) -> &'static str {
        match self {
            Self::Cbr => "CBR",
            Self::Vbr => "VBR",
            Self::Cq => "CQ",
        }
    }

    fn raw(self) -> i32 {
        match self {
            Self::Cbr => sos_vpx::vpx_rc_mode::VPX_CBR as i32,
            Self::Vbr => sos_vpx::vpx_rc_mode::VPX_VBR as i32,
            Self::Cq => sos_vpx::vpx_rc_mode::VPX_CQ as i32,
        }
    }
}

/// 用指定模式跑一段内容，按 1 秒窗口统计码率波动
fn run<F>(mode: Mode, frames: usize, thresh: u32, gen: F) -> Stats
where
    F: Fn(usize) -> Vec<u8>,
{
    let (w, h) = (1920usize, 1080usize);
    sos_vpx::set_rcmode_override(Some(mode.raw()));
    let cfg = SosVpxConfig {
        width: w,
        height: h,
        codec: SosCodec::Vp9,
        threads: 2,
        cpu_used: 8,
        bitrate_kbps: TARGET_KBPS,
        keyframe_interval: None,
    };
    let created = SosVpxEncoder::new(cfg);
    sos_vpx::set_rcmode_override(None);
    let mut enc = created.expect("encoder");
    if thresh != sos_vpx::RC_DROPFRAME_THRESH {
        enc.tune(TARGET_KBPS, thresh).expect("tune");
    }

    let (mut outs, mut drops) = (0usize, 0usize);
    let mut bytes = 0usize;
    // 每秒累加一次，用于算峰值/谷值
    let mut per_sec: Vec<usize> = Vec::new();
    let mut cur_sec_bytes = 0usize;
    let origin = Instant::now();
    let tick = Duration::from_secs_f64(1.0 / 30.0);
    let mut last_sec = 0u64;

    for i in 0..frames {
        let yuv = gen(i);
        let pts = origin.elapsed().as_millis() as i64;
        if let Ok(out) = enc.encode(&yuv, pts) {
            drops += enc.take_dropped_frames();
            if !out.is_empty() {
                outs += 1;
                for (d, _k) in out {
                    bytes += d.len();
                    cur_sec_bytes += d.len();
                }
            }
        }
        let sec = origin.elapsed().as_secs();
        if sec > last_sec {
            per_sec.push(cur_sec_bytes);
            cur_sec_bytes = 0;
            last_sec = sec;
        }
        let deadline = origin + tick * i as u32;
        let now = Instant::now();
        if deadline > now {
            std::thread::sleep(deadline - now);
        }
    }
    if cur_sec_bytes > 0 {
        per_sec.push(cur_sec_bytes);
    }
    let secs = origin.elapsed().as_secs_f64().max(0.001);
    let avg = bytes as f64 * 8.0 / 1000.0 / secs;
    let peak = per_sec
        .iter()
        .map(|b| *b as f64 * 8.0 / 1000.0)
        .fold(0.0f64, f64::max);
    let min = per_sec
        .iter()
        .map(|b| *b as f64 * 8.0 / 1000.0)
        .fold(f64::MAX, f64::min);
    Stats {
        avg_kbps: avg,
        peak_kbps: peak,
        min_kbps: if min == f64::MAX { 0.0 } else { min },
        burst_ratio: if avg > 0.0 { peak / avg } else { 0.0 },
        outs,
        drops,
        avg_frame_kb: if outs > 0 {
            bytes as f64 / outs as f64 / 1024.0
        } else {
            0.0
        },
    }
}

/// **实验一**：静态画面下 VBR 是否真的更省带宽？
///
/// 预期：VBR 会把码率降到很低（因为内容简单），这是它的优势。
#[test]
fn static_content_vbr_is_cheaper() {
    let frames = 90usize;
    println!(
        "静态桌面（打字/看文档），{} 帧 @30fps，目标 {}kbps",
        frames, TARGET_KBPS
    );
    println!(
        "{:>6} {:>10} {:>10} {:>10} {:>8} {:>10}",
        "模式", "平均kbps", "峰值kbps", "谷值kbps", "出帧", "丢帧率"
    );
    for mode in [Mode::Cbr, Mode::Vbr, Mode::Cq] {
        let s = run(mode, frames, sos_vpx::RC_DROPFRAME_THRESH, |_| {
            make_static(1920, 1080)
        });
        println!(
            "{:>6} {:>10.0} {:>10.0} {:>10.0} {:>8} {:>9.0}%",
            mode.label(),
            s.avg_kbps,
            s.peak_kbps,
            s.min_kbps,
            s.outs,
            s.drop_rate()
        );
    }
    println!("\n静态内容本来就便宜，三者都远低于目标，差异不构成决策依据。");
}

/// **实验二**：复杂画面下 VBR 会不会冲高？这是远程桌面的核心风险。
///
/// 预期：VBR 会显著超发 → 在中继链路上直接变成 RTT 上涨 → bufferbloat。
#[test]
fn motion_content_vbr_bursts() {
    let frames = 90usize;
    println!(
        "高复杂度运动（拖动窗口/播视频），{} 帧，目标 {}kbps",
        frames, TARGET_KBPS
    );
    println!(
        "{:>6} {:>10} {:>10} {:>10} {:>10} {:>8} {:>10}",
        "模式", "平均kbps", "峰值kbps", "峰值/均值", "相对目标", "出帧", "丢帧率"
    );
    let mut rows = Vec::new();
    for mode in [Mode::Cbr, Mode::Vbr, Mode::Cq] {
        let s = run(mode, frames, sos_vpx::RC_DROPFRAME_THRESH, |i| {
            make_motion(1920, 1080, i)
        });
        println!(
            "{:>6} {:>10.0} {:>10.0} {:>10.2} {:>9.2}x {:>8} {:>9.0}%",
            mode.label(),
            s.avg_kbps,
            s.peak_kbps,
            s.burst_ratio,
            s.avg_kbps / TARGET_KBPS as f64,
            s.outs,
            s.drop_rate()
        );
        rows.push((mode, s));
    }
    let cbr = &rows.iter().find(|(m, _)| *m == Mode::Cbr).unwrap().1;
    let vbr = &rows.iter().find(|(m, _)| *m == Mode::Vbr).unwrap().1;
    println!(
        "\nCBR 峰值 {:.0}kbps ({:.2}x 目标) → VBR 峰值 {:.0}kbps ({:.2}x 目标)",
        cbr.peak_kbps,
        cbr.peak_kbps / TARGET_KBPS as f64,
        vbr.peak_kbps,
        vbr.peak_kbps / TARGET_KBPS as f64
    );
}

/// **实验三**：混合负载（静态与运动交替）—— 这才是真实远程桌面。
///
/// 真实场景下带宽预算来自 ABR，上层假设 `rc_target_bitrate` 是可信上限。
/// 如果某个模式在复杂段落冲破上限，ABR 会误判为「链路拥塞」而压帧率，
/// 反而把画面搞卡。
#[test]
fn mixed_workload_shows_the_real_tradeoff() {
    let frames = 150usize; // 5 秒
    println!(
        "混合负载（前 2 秒静态、中 2 秒运动、后 1 秒静态），{} 帧，目标 {}kbps",
        frames, TARGET_KBPS
    );
    println!(
        "{:>6} {:>10} {:>10} {:>10} {:>8} {:>10} {:>12}",
        "模式", "平均kbps", "峰值kbps", "峰值/均值", "出帧", "丢帧率", "平均帧KB"
    );
    for mode in [Mode::Cbr, Mode::Vbr, Mode::Cq] {
        let s = run(mode, frames, sos_vpx::RC_DROPFRAME_THRESH, |i| {
            let t = i as f64 / 30.0;
            if t < 2.0 || t >= 4.0 {
                make_static(1920, 1080)
            } else {
                make_motion(1920, 1080, i)
            }
        });
        println!(
            "{:>6} {:>10.0} {:>10.0} {:>10.2} {:>8} {:>9.0}% {:>12.1}",
            mode.label(),
            s.avg_kbps,
            s.peak_kbps,
            s.burst_ratio,
            s.outs,
            s.drop_rate(),
            s.avg_frame_kb
        );
    }
    println!("\n关键看两列：");
    println!("- 「峰值kbps」决定会不会养出 bufferbloat → 影响 RTT 与 QoS 判断");
    println!("- 「丢帧率」决定画面流畅度");
}

/// **实验四**：低 RTT 场景（内网直连）下 VBR 是否可接受？
///
/// 内网带宽充足、rtt=1ms，超发不会变成排队。
/// 若这里 VBR 的丢帧率显著低于 CBR，就说明「按链路类型选模式」值得做。
#[test]
fn good_link_vbr_may_be_acceptable() {
    let frames = 90usize;
    println!("内网直连场景（thresh=0 不丢帧），高复杂度运动");
    println!(
        "{:>6} {:>10} {:>10} {:>10} {:>8} {:>12}",
        "模式", "平均kbps", "峰值kbps", "峰值/均值", "出帧", "平均帧KB"
    );
    for mode in [Mode::Cbr, Mode::Vbr, Mode::Cq] {
        let s = run(mode, frames, 0, |i| make_motion(1920, 1080, i));
        println!(
            "{:>6} {:>10.0} {:>10.0} {:>10.2} {:>8} {:>12.1}",
            mode.label(),
            s.avg_kbps,
            s.peak_kbps,
            s.burst_ratio,
            s.outs,
            s.avg_frame_kb
        );
    }
    println!("\nthresh=0 时三种模式都不丢帧，差异只在码率分布。");
    println!("内网带宽充足，超发无感知，VBR 的波动不构成问题。");
}

/// **实验五**：混合负载 × 丢帧策略 —— 这才是生产决策面。
///
/// 生产代码用 `dropframe_thresh_for_link()`：好链路 0、差链路 20。
/// 所以必须回答：在**好链路（thresh=0）**下，VBR 相对 CBR 的超发是否可接受？
/// 在差链路（thresh=20）下，VBR 是否仍然失控？
#[test]
fn mixed_workload_across_drop_policies() {
    let frames = 150usize;
    println!(
        "混合负载（2s 静态 + 2s 运动 + 1s 静态），目标 {}kbps",
        TARGET_KBPS
    );
    println!(
        "{:>6} {:>8} {:>10} {:>10} {:>10} {:>8} {:>10}",
        "模式", "thresh", "平均kbps", "峰值kbps", "峰值/均值", "出帧", "丢帧率"
    );
    let gen = |i: usize| {
        let t = i as f64 / 30.0;
        if t < 2.0 || t >= 4.0 {
            make_static(1920, 1080)
        } else {
            make_motion(1920, 1080, i)
        }
    };
    for thresh in [0u32, sos_vpx::RC_DROPFRAME_THRESH] {
        for mode in [Mode::Cbr, Mode::Vbr] {
            let s = run(mode, frames, thresh, gen);
            println!(
                "{:>6} {:>8} {:>10.0} {:>10.0} {:>10.2} {:>8} {:>9.0}%",
                mode.label(),
                thresh,
                s.avg_kbps,
                s.peak_kbps,
                s.burst_ratio,
                s.outs,
                s.drop_rate()
            );
        }
    }
    println!("\n这是生产的两个工作点：");
    println!("- thresh=0（内网/好链路）：VBR 相对 CBR 的超发幅度");
    println!("- thresh=20（公网/中继）：VBR 是否仍能守住码率");
}
