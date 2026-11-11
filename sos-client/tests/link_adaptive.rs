//! 链路自适应：丢帧水位 + **码率预算**。
//!
//! 两条互相独立的策略都由同一个「链路是否充裕」判据驱动：
//!
//! 1. **丢帧水位**：充裕 → 0（不丢帧）；否则 → 20（守码率）。
//!    实测 1080p 高复杂度内容 @1675kbps：
//!
//!    | rc_dropframe_thresh | 出帧 | 丢帧率 | 实际码率 |
//!    |---|---|---|---|
//!    | 20（守码率） | 10/60 | 83% | 1853 kbps |
//!    | 0（守帧数） | 60/60 | 0% | 3294 kbps |
//!
//! 2. **码率预算**：充裕 → 不按帧率缩放，另给 2 倍放宽；否则 → 按 `fps/30` 缩放。
//!
//! 第 2 条的依据是量化器实测（`tests/vpx_budget.rs`）：默认工作点
//! （1600x900 / Balanced / 1163kbps）把量化器推到 47.7/63（刻度 76%），
//! 文字边缘基本被抹平；放宽到 2 倍后落在 30 附近，是能看清又不浪费的平衡点。
//!
//! 运行：cargo test --release --test link_adaptive -- --nocapture

#![cfg(all(windows, sos_vpx))]

#[path = "../src/sos_vpx.rs"]
mod sos_vpx;

use sos_vpx::{LinkBudget, SosVpxConfig};

/// **判据**：必须同时满足「RTT 低」与「无排队」
#[test]
fn generous_link_requires_low_rtt_and_no_queue() {
    use sos_vpx::link_is_generous;

    // 有线内网直连（生产日志实测 rtt=1ms、raw=1-13ms → queue 0-12ms）
    assert!(link_is_generous(Some(1), Some(0)));
    assert!(link_is_generous(Some(1), Some(12)));
    assert!(link_is_generous(Some(34), Some(0)), "34ms 中继但仍无排队");
    assert!(link_is_generous(Some(49), Some(39)), "阈值内");

    // 中继链路：RTT 本身已高
    assert!(!link_is_generous(Some(50), Some(0)));
    assert!(!link_is_generous(Some(211), Some(0)));

    // **关键**：低延迟 Wi-Fi —— RTT 好看但出口在排队
    assert!(
        !link_is_generous(Some(5), Some(40)),
        "低 RTT + 明显排队 = 不能放宽，否则在无线路由器缓冲里爆掉"
    );
    assert!(!link_is_generous(Some(8), Some(120)));
    assert!(
        !link_is_generous(Some(20), Some(200)),
        "RTT 20ms 但排队 200ms，这是典型的 Wi-Fi bufferbloat"
    );

    // 样本缺失时必须保守
    assert!(!link_is_generous(None, Some(0)), "没有 RTT 样本");
    assert!(!link_is_generous(Some(1), None), "没有排队样本");
    assert!(!link_is_generous(None, None));
}

/// **丢帧水位**：由同一判据驱动
#[test]
fn dropframe_thresh_follows_budget() {
    let decide = |rtt, queue| {
        if sos_vpx::link_is_generous(rtt, queue) {
            0
        } else {
            sos_vpx::RC_DROPFRAME_THRESH
        }
    };
    assert_eq!(decide(Some(1), Some(0)), 0);
    assert_eq!(decide(Some(1), Some(60)), sos_vpx::RC_DROPFRAME_THRESH);
    assert_eq!(decide(Some(200), Some(0)), sos_vpx::RC_DROPFRAME_THRESH);
    assert_eq!(decide(None, None), sos_vpx::RC_DROPFRAME_THRESH);
}

/// **核心**：充裕链路不按帧率缩放，拥塞链路上必须缩放
#[test]
fn generous_budget_ignores_fps() {
    let (w, h) = (1600usize, 900usize);
    let ratio = 0.67;
    let base = SosVpxConfig::default_bitrate_kbps(w, h) as f32;
    let expect_generous = (base * ratio * SosVpxConfig::GOOD_LINK_BITRATE_GAIN) as u32;

    println!(
        "1600x900 ratio 0.67 base={}kbps gain={}",
        base,
        SosVpxConfig::GOOD_LINK_BITRATE_GAIN
    );
    println!(
        "{:>6} {:>14} {:>16} {:>10}",
        "fps", "generous", "constrained", "差值"
    );
    for fps in [8u32, 15, 20, 27, 30] {
        let g = SosVpxConfig::bitrate_kbps_for_link(w, h, ratio, fps, LinkBudget::Generous);
        let c = SosVpxConfig::bitrate_kbps_for_link(w, h, ratio, fps, LinkBudget::Constrained);
        println!(
            "{:>6} {:>13}kbps {:>15}kbps {:>9.0}%",
            fps,
            g,
            c,
            (g as f32 / c as f32 - 1.0) * 100.0
        );
        assert_eq!(g, expect_generous, "充裕链路码率必须与帧率无关");
        // 帧率越低，两者差距越大 —— 这正是「双重惩罚」被消除的地方
        assert!(g >= c, "充裕链路给出的码率不应低于拥塞链路");
    }

    // 帧率降到 8fps 时，旧公式只剩 0.5 倍，新公式仍是全量
    let old = SosVpxConfig::bitrate_kbps_for_link(w, h, ratio, 8, LinkBudget::Constrained);
    assert!(
        (old as f32) < expect_generous as f32 * 0.55,
        "拥塞链路 8fps 时确实应该只剩一半左右（MIN_FPS_FACTOR=0.5）"
    );
}

/// 拥塞链路的公式必须与原有实现完全一致（不能有回归）
#[test]
fn constrained_budget_is_unchanged() {
    for (w, h) in [(1920usize, 1080usize), (1600, 900), (1280, 720)] {
        for ratio in [0.5f32, 0.67, 1.0, 1.5] {
            for fps in [8u32, 15, 20, 30, 60] {
                let new =
                    SosVpxConfig::bitrate_kbps_for_link(w, h, ratio, fps, LinkBudget::Constrained);
                let old = SosVpxConfig::bitrate_kbps_for(w, h, ratio, fps);
                assert_eq!(
                    new, old,
                    "{}x{} ratio={} fps={} 必须与原公式一致",
                    w, h, ratio, fps
                );
            }
        }
    }
    println!("拥塞链路路径与旧公式逐点一致（无回归）");
}

/// **效果验证**：充裕链路把量化器从「糊」拉到「能看清」
///
/// 用真实测量验证 2.0 倍的经验值是否成立，而不是只做算术。
#[test]
fn generous_gain_actually_improves_quantizer() {
    use sos_vpx::{SosCodec, SosVpxEncoder};
    use std::time::Instant;

    fn desktop_text(w: usize, h: usize) -> Vec<u8> {
        let (cw, ch) = (w / 2, h / 2);
        let mut buf = vec![0u8; w * h + 2 * cw * ch];
        for y in 0..h {
            let base = 246u32.saturating_sub((y * 12 / h.max(1)) as u32);
            for x in 0..w {
                buf[y * w + x] = base as u8;
            }
        }
        let title_h = (h / 18).max(8);
        for y in 0..title_h {
            for x in 0..w {
                buf[y * w + x] = 62;
            }
        }
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
                        if (pat >> ((row + col) % 8)) & 1 == 1 {
                            buf[(y + row) * w + x0 + col] = 40;
                        }
                    }
                }
            }
            y += line_h;
            line += 1;
        }
        let uv = w * h;
        for i in 0..(cw * ch) {
            buf[uv + i] = 128;
            buf[uv + cw * ch + i] = 128;
        }
        buf
    }

    fn avg_quantizer(target_kbps: u32) -> f64 {
        let (w, h) = (1600usize, 900usize);
        let cfg = SosVpxConfig {
            width: w,
            height: h,
            codec: SosCodec::Vp9,
            threads: 3,
            cpu_used: 7,
            bitrate_kbps: target_kbps,
            keyframe_interval: None,
        };
        let mut enc = match SosVpxEncoder::new(cfg) {
            Ok(e) => e,
            Err(_) => return f64::NAN,
        };
        let origin = Instant::now();
        let mut qs = Vec::new();
        for i in 0..45usize {
            let deadline = (i as f64) / 30.0;
            let now = Instant::now().duration_since(origin).as_secs_f64();
            if deadline > now {
                std::thread::sleep(std::time::Duration::from_secs_f64(deadline - now));
            }
            let yuv = desktop_text(w, h);
            let pts = origin.elapsed().as_millis() as i64;
            let q = if i >= 10 { enc.quantizer() } else { None };
            if enc.encode(&yuv, pts).is_ok() {
                if let Some(q) = q {
                    qs.push(q as f64);
                }
            }
        }
        if qs.is_empty() {
            return f64::NAN;
        }
        qs.iter().sum::<f64>() / qs.len() as f64
    }

    let (w, h) = (1600usize, 900usize);
    let ratio = 0.67;
    let constrained = SosVpxConfig::bitrate_kbps_for_link(w, h, ratio, 20, LinkBudget::Constrained);
    let generous = SosVpxConfig::bitrate_kbps_for_link(w, h, ratio, 20, LinkBudget::Generous);
    let q_constrained = avg_quantizer(constrained);
    let q_generous = avg_quantizer(generous);

    println!("{:>12} {:>10} {:>14}", "预算档位", "码率", "量化器均值");
    println!(
        "{:>12} {:>9}kbps {:>13.1}",
        "constrained", constrained, q_constrained
    );
    println!(
        "{:>12} {:>9}kbps {:>13.1}",
        "generous", generous, q_generous
    );
    println!("\n量化器刻度 0-63（rc_max_quantizer=63）");

    assert!(
        q_generous < q_constrained,
        "充裕链路必须真的把量化器降下来（{:.1} → {:.1}）",
        q_constrained,
        q_generous
    );
    let improvement = (q_constrained - q_generous) / q_constrained * 100.0;
    println!("改善 {:.0}%", improvement);
    assert!(
        improvement > 20.0,
        "改善幅度应显著（实测约 35-40%），实际仅 {:.0}%",
        improvement
    );
}
