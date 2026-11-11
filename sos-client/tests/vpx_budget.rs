//! 码率是否「太死」的实测：用**量化器**作为画质的直接证据。
//!
//! 争议点：「实际码率 ≈ 目标码率」到底代表码率够用，还是代表画质被牺牲？
//! 前者说明 CBR 工作良好，后者说明码率上限压死了画质。
//!
//! 两者光看码率数字**无法区分**。唯一可靠的直接证据是**量化器**：
//!
//! - 码率充裕 → 编码器停在低量化器（~4-20），细节保留；
//! - 码率不足 → 量化器一路推到 `rc_max_quantizer`(63)，画面变成色块和糊字。
//!
//! 本测试对**真实桌面内容**（大块平坦区 + 细密文字）扫描码率，
//! 输出每档的量化器分布。若量化器在目标码率处就已经贴着 63，
//! 那码率上限就是画质瓶颈，这就是「太死」的量化定义。
//!
//! 运行：cargo test --release --test vpx_budget -- --nocapture --test-threads=1

#![cfg(all(windows, sos_vpx))]

#[path = "../src/sos_vpx.rs"]
mod sos_vpx;

use sos_vpx::{SosCodec, SosVpxConfig, SosVpxEncoder};
use std::time::Instant;

/// 真实桌面截图的样子：大面积浅色背景 + 成行的小字 + 一条标题栏。
/// 文字是最考验码率的内容 —— 平坦区几乎不耗码率，
/// 但每个字的边缘都需要比特，码率不够就直接糊成一团。
///
/// `dense = false` 用**笔画式**字形（横竖撇，接近真实字体渲染）；
/// `dense = true` 用准随机点阵（高熵细节，相当于最坏情况）。
/// 两者一起测量才能给出「需求码率」的合理区间：
/// 只用随机点阵会把需求估高，只用笔画则可能低估。
fn make_desktop_text_impl(w: usize, h: usize, dense: bool) -> Vec<u8> {
    let (cw, ch) = (w / 2, h / 2);
    let mut buf = vec![0u8; w * h + 2 * cw * ch];
    // 浅色背景，带极轻微渐变（模拟窗口阴影）
    for y in 0..h {
        let base = 246u32.saturating_sub((y * 12 / h.max(1)) as u32);
        for x in 0..w {
            buf[y * w + x] = base as u8;
        }
    }
    // 标题栏
    let title_h = (h / 18).max(8);
    for y in 0..title_h {
        for x in 0..w {
            buf[y * w + x] = 62;
        }
    }
    // 小字：每个字符 5x7 点阵，行距 14px，字符间距 7px
    let line_h = 14usize;
    let mut y = title_h + 8;
    let mut line = 0usize;
    while y + 7 < h - 4 {
        let text_len = 40 + (line * 17) % 60;
        for ch_i in 0..text_len {
            let x0 = 12 + ch_i * 7;
            if x0 + 5 >= w {
                break;
            }
            let pat = (line * 31 + ch_i * 17) % 251;
            for row in 0..7usize {
                for col in 0..5usize {
                    let on = if dense {
                        (pat >> ((row + col) % 8)) & 1 == 1
                    } else {
                        // 笔画式：左竖、上横、中横、右竖（像 A / H / E 这类字母）
                        col == 0 || col == 4 || row == 0 || row == 3 || (row == 6 && col < 3)
                    };
                    if on {
                        buf[(y + row) * w + x0 + col] = 40;
                    }
                }
            }
        }
        y += line_h;
        line += 1;
    }
    // 色度平面保持中性（桌面内容几乎没有色度细节）
    let uv = w * h;
    for i in 0..(cw * ch) {
        buf[uv + i] = 128;
        buf[uv + cw * ch + i] = 128;
    }
    buf
}

fn make_desktop_text(w: usize, h: usize, _seed: usize) -> Vec<u8> {
    make_desktop_text_impl(w, h, true)
}

/// 笔画式字形（更接近真实字体）
fn make_desktop_strokes(w: usize, h: usize) -> Vec<u8> {
    make_desktop_text_impl(w, h, false)
}

#[derive(Debug, Default)]
struct Stat {
    kbps: f64,
    q_avg: f64,
    q_min: i32,
    q_max: i32,
    /// 量化器贴着上限（>=60）的帧占比 —— 画质被码率压死的直接证据
    q_saturated_pct: f64,
    frame_kb: f64,
    drops: usize,
}

/// 在固定 30fps 节奏下用指定码率编码指定内容
fn measure_with<F>(codec: SosCodec, cpu_used: u32, target_kbps: u32, frames: usize, gen: F) -> Stat
where
    F: Fn(usize, usize) -> Vec<u8>,
{
    let (w, h) = (1600usize, 900usize);
    let cfg = SosVpxConfig {
        width: w,
        height: h,
        codec,
        threads: 3,
        cpu_used,
        bitrate_kbps: target_kbps,
        keyframe_interval: None,
    };
    let mut enc = match SosVpxEncoder::new(cfg) {
        Ok(e) => e,
        Err(e) => {
            println!("  初始化失败: {}", e);
            return Stat::default();
        }
    };
    let mut bytes = 0usize;
    let mut drops = 0usize;
    let mut qs: Vec<i32> = Vec::new();
    let mut out_frames = 0usize;
    let origin = Instant::now();
    let spf = 1.0 / 30.0;
    for i in 0..frames {
        let deadline = spf * i as f64;
        let now = Instant::now().duration_since(origin).as_secs_f64();
        if deadline > now {
            std::thread::sleep(std::time::Duration::from_secs_f64(deadline - now));
        }
        let yuv = gen(w, h);
        let pts = origin.elapsed().as_millis() as i64;
        if let Ok(out) = enc.encode(&yuv, pts) {
            drops += enc.take_dropped_frames();
            // 第 10 帧之后才开始统计，跳过码控收敛期
            let q = if i >= 10 { enc.quantizer() } else { None };
            if let Some(q) = q {
                qs.push(q);
            }
            if !out.is_empty() {
                out_frames += 1;
                for (b, _) in out {
                    bytes += b.len();
                }
            }
        }
    }
    let secs = origin.elapsed().as_secs_f64().max(0.001);
    if qs.is_empty() {
        return Stat::default();
    }
    let q_avg = qs.iter().map(|q| *q as f64).sum::<f64>() / qs.len() as f64;
    // rc_max_quantizer = 63；贴 60+ 视为画质已被码率压死
    let sat = qs.iter().filter(|q| **q >= 60).count() as f64 * 100.0 / qs.len() as f64;
    Stat {
        kbps: bytes as f64 * 8.0 / 1000.0 / secs,
        q_avg,
        q_min: *qs.iter().min().unwrap_or(&0),
        q_max: *qs.iter().max().unwrap_or(&0),
        q_saturated_pct: sat,
        frame_kb: if out_frames > 0 {
            bytes as f64 / out_frames as f64 / 1024.0
        } else {
            0.0
        },
        drops,
    }
}

/// 在固定 30fps 节奏下用指定码率编码桌面文字内容
fn measure(codec: SosCodec, cpu_used: u32, target_kbps: u32, frames: usize) -> Stat {
    measure_with(codec, cpu_used, target_kbps, frames, |w, h| {
        make_desktop_text(w, h, 0)
    })
}

/// **核心实验**：桌面文字内容在不同码率下的量化器
///
/// 若在「实际使用的码率」处量化器就贴着 63，说明码率是画质瓶颈。
#[test]
fn t1_quantizer_vs_bitrate_desktop_text() {
    println!("VP9 1600x900 桌面文字内容 @30fps —— 量化器越高越糊");
    println!("rc_max_quantizer = 63（贴 63 = 画质被码率压死）");
    println!();
    println!(
        "{:>10} {:>10} {:>8} {:>8} {:>8} {:>10} {:>9} {:>7}",
        "目标kbps", "实际kbps", "Q均值", "Q最小", "Q最大", "饱和帧%", "帧KB", "丢帧"
    );
    // 覆盖从「极低」到「宽裕」的完整范围
    for target in [600u32, 800, 1000, 1163, 1500, 2000, 2604, 4000, 6000] {
        let s = measure(SosCodec::Vp9, 7, target, 45);
        println!(
            "{:>10} {:>10.0} {:>8.1} {:>8} {:>8} {:>9.0}% {:>9.1} {:>7}",
            target, s.kbps, s.q_avg, s.q_min, s.q_max, s.q_saturated_pct, s.frame_kb, s.drops
        );
    }
    println!();
    println!("标注的两个生产工作点：");
    println!("  1163kbps = ratio 0.67 (Balanced) @ 1600x900, fps 30");
    println!("  2604kbps = ratio 1.50 (Best)     @ 1600x900, fps 30");
}

/// **对照**：同一内容下 VP8 的量化器曲线
///
/// VP8 的压缩效率低约 20-30%，若它在相同码率下量化器更高，
/// 说明「降级到 VP8」会额外损失画质 —— 这会影响降级策略的取舍。
#[test]
fn t2_vp8_quantizer_same_content() {
    println!("VP8 1600x900 桌面文字内容 @30fps（对照 VP9）");
    println!(
        "{:>10} {:>10} {:>8} {:>8} {:>8} {:>9} {:>9}",
        "目标kbps", "实际kbps", "Q均值", "Q最小", "Q最大", "饱和帧%", "帧KB"
    );
    for target in [1163u32, 2604, 4000] {
        let s = measure(SosCodec::Vp8, 12, target, 45);
        println!(
            "{:>10} {:>10.0} {:>8.1} {:>8} {:>8} {:>8.0}% {:>9.1}",
            target, s.kbps, s.q_avg, s.q_min, s.q_max, s.q_saturated_pct, s.frame_kb
        );
    }
}

/// **结论验证**：找到量化器降到「细节良好」区间所需的码率
///
/// 若这个码率明显高于 2604kbps（Best 档），就证明当前的码率公式
/// 对桌面文字内容确实太保守。
#[test]
fn t3_bitrate_needed_for_crisp_text() {
    println!("找到「文字清晰」所需码率：量化器均值 <= 20 视为清晰");
    println!(
        "{:>10} {:>8} {:>9} {:>12}",
        "目标kbps", "Q均值", "饱和帧%", "是否清晰"
    );
    let mut found: Option<u32> = None;
    for target in [2000u32, 2604, 3200, 4000, 5000, 6000, 8000] {
        let s = measure(SosCodec::Vp9, 7, target, 45);
        let ok = s.q_avg <= 20.0;
        if ok && found.is_none() {
            found = Some(target);
        }
        println!(
            "{:>10} {:>8.1} {:>9.0}% {:>12}",
            target,
            s.q_avg,
            s.q_saturated_pct,
            if ok { "是" } else { "否" }
        );
    }
    match found {
        Some(t) => println!("\n结论：需要约 {}kbps 才能让文字保持清晰", t),
        None => println!("\n结论：即使 8000kbps 也没能降到 Q<=20"),
    }
}

/// **真实字形对照**：随机点阵是最坏情况，笔画式字形更接近真实字体
///
/// 两者给出的「需求码率」区间才是可以拿来定参数的依据。
#[test]
fn t4_dense_vs_strokes() {
    println!("1600x900 两种文字内容在同一码率下的量化器对比");
    println!("（dense = 准随机点阵，最坏情况；strokes = 笔画字形，接近真实字体）");
    println!(
        "{:>10} {:>14} {:>10} {:>14} {:>10}",
        "目标kbps", "dense Q均值", "dense饱和", "strokes Q均值", "strokes饱和"
    );
    for target in [1163u32, 1736, 2604, 3500, 5000] {
        let d = measure_with(SosCodec::Vp9, 7, target, 45, |w, h| {
            make_desktop_text(w, h, 0)
        });
        let s = measure_with(SosCodec::Vp9, 7, target, 45, |w, h| {
            make_desktop_strokes(w, h)
        });
        println!(
            "{:>10} {:>14.1} {:>9.0}% {:>14.1} {:>9.0}%",
            target, d.q_avg, d.q_saturated_pct, s.q_avg, s.q_saturated_pct
        );
    }
    println!("\n若两者在「Balanced 工作点」(1163) 都饱和，说明码率确实太紧；");
    println!("若 strokes 明显好于 dense，则以 strokes 为定参依据更公允。");
}
