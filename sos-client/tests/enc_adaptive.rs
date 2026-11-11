//! 编码成本自适应：判据与降级阶梯的单测。
//!
//! 这套逻辑用来在**编不动的机器**上自动降级编码配置。它的正确性有两个方面：
//!
//! 1. **不能误判**：高性能机器上永远不应触发降级，否则会白白牺牲画质；
//! 2. **不能漏判**：真的编不动时必须降级，否则画面永远卡在 20fps。
//!
//! 判据必须同时看采集耗时。真实日志里就出现过误判：
//!
//! ```text
//! [VIDEO:PERF] cap=50.3ms conv=2.4ms enc=31.8ms spf=33ms skip=1  ← 瓶颈是 GDI 采集
//! [VIDEO:PERF] cap=4.6ms  conv=1.8ms enc=34.3ms spf=33ms skip=0  ← 瓶颈是编码
//! ```
//!
//! 旧判据只看 `enc > 0.9 × spf`，把第一种也判成「编码超预算」并降了一级 ——
//! 但采集要 50ms 时，编码再快也跑不到 30fps。
//!
//! 运行：cargo test --release --test enc_adaptive -- --nocapture

#![cfg(all(windows, sos_vpx))]

#[path = "../src/sos_vpx.rs"]
mod sos_vpx;

use sos_vpx::{
    encoder_behind_budget, next_enc_level, overload_is_severe, vp8_trial_worth_keeping,
    ENCODER_BUDGET_RATIO, VP8_TRIAL_KEEP_RATIO,
};

/// 30fps 的帧预算
const BUDGET_33: f64 = 33.3;

/// **判据**：真实日志里的样本必须给出正确结论
///
/// 重点是那个**误判案例**：`cap=50.3ms enc=31.8ms`。
/// 旧判据只看 `enc > 0.9 × spf`，会把它判成"编码超预算"并降级 ——
/// 但那台机器上瓶颈明明是 GDI 采集（50ms），降编码档位对帧率毫无帮助。
#[test]
fn real_log_samples_are_classified_correctly() {
    println!(
        "budget_ratio = {}  （需要 enc > cap 且 enc+conv > {:.0}% x spf）",
        ENCODER_BUDGET_RATIO,
        ENCODER_BUDGET_RATIO * 100.0
    );
    println!(
        "{:>9} {:>9} {:>9} {:>8} {:>10} {:>8}",
        "enc_ms", "cap_ms", "conv_ms", "budget", "判定", "说明"
    );
    // (enc, cap, conv, budget, 是否应降级, 说明)
    let cases = [
        // ---- 真实误判案例：GDI 采集 50ms，编码 31.8ms ----
        (31.8, 50.3, 2.4, 33.3, false, "GDI 50ms 才是瓶颈"),
        (26.3, 49.6, 2.4, 37.0, false, "同上，另一窗口"),
        // ---- 真正的编码瓶颈：DXGI 采集 5ms ----
        (34.3, 4.6, 1.8, 33.3, true, "DXGI 5ms，编码 34.3ms"),
        (32.2, 5.8, 1.6, 33.3, true, "32.2+1.6=33.8 > 29.97"),
        (30.8, 5.1, 1.7, 33.3, true, "30.8+1.7=32.5 > 29.97"),
        // ---- 健康机器 ----
        (19.4, 32.8, 1.2, 33.3, false, "编码充足"),
        (9.7, 8.2, 1.1, 33.3, false, "静止画面"),
        (13.7, 4.5, 1.4, 33.3, false, "VP8 30fps 达标"),
    ];
    let mut failures = Vec::new();
    for (enc, cap, conv, budget, expect, note) in cases {
        let got = encoder_behind_budget(enc, cap, conv, budget);
        println!(
            "{:>9.1} {:>9.1} {:>9.1} {:>8.1} {:>10} {}",
            enc,
            cap,
            conv,
            budget,
            if got { "降级" } else { "不降" },
            note
        );
        if got != expect {
            failures.push(format!(
                "enc={enc} cap={cap} conv={conv} budget={budget} 期望 {expect} 实际 {got} ({note})"
            ));
        }
    }
    assert!(failures.is_empty(), "判定错误:\n{}", failures.join("\n"));
}

/// **关键回归**：采集比编码慢时，无论编码多慢都不得降级
#[test]
fn slow_capturer_never_blames_encoder() {
    // GDI BitBlt 实测 35-88ms。只要采集比编码慢，降编码档位就不可能提升帧率。
    for cap in [45.0f64, 50.0, 88.0] {
        for enc in [10.0f64, 30.0, 40.0] {
            assert!(
                enc <= cap,
                "这个用例只覆盖采集主导的组合（enc {enc} <= cap {cap}）"
            );
            assert!(
                !encoder_behind_budget(enc, cap, 2.0, 33.3),
                "采集 {cap}ms 比编码 {enc}ms 慢时不应降编码档位"
            );
        }
    }
    // 编码比采集慢得多时才是真的编码瓶颈
    assert!(
        encoder_behind_budget(80.0, 45.0, 2.0, 33.3),
        "编码 80ms 才是大头"
    );
    // 采集变快（换到 DXGI）后，同样的编码耗时就该降级了
    assert!(encoder_behind_budget(40.0, 5.0, 2.0, 33.3));
    println!("采集主导时不降级；采集变快后同样的编码耗时才降级");
}

/// **边界**：恰好卡在阈值上不应降级，稍微超过才降
#[test]
fn threshold_boundary_is_exclusive() {
    let cap = 5.0;
    let conv = 1.0;
    let exact = BUDGET_33 * ENCODER_BUDGET_RATIO - conv;
    assert!(
        !encoder_behind_budget(exact, cap, conv, BUDGET_33),
        "恰好等于阈值不应降级（用 > 而非 >=，避免临界值反复横跳）"
    );
    assert!(
        !encoder_behind_budget(exact - 0.1, cap, conv, BUDGET_33),
        "低于阈值不应降级"
    );
    assert!(
        encoder_behind_budget(exact + 0.1, cap, conv, BUDGET_33),
        "高于阈值应降级"
    );
}

/// **防御**：无效输入一律判为「不降级」，宁可画质差也不乱降
#[test]
fn invalid_inputs_never_trigger() {
    assert!(
        !encoder_behind_budget(100.0, 1.0, 1.0, 0.0),
        "预算为 0 = 无法判断"
    );
    assert!(
        !encoder_behind_budget(100.0, 1.0, 1.0, -1.0),
        "负预算无意义"
    );
    assert!(
        !encoder_behind_budget(f64::NAN, 1.0, 1.0, BUDGET_33),
        "NaN 不可比"
    );
    assert!(
        !encoder_behind_budget(f64::INFINITY, 1.0, 1.0, BUDGET_33),
        "无穷大视为无效"
    );
    assert!(
        !encoder_behind_budget(100.0, 1.0, 1.0, f64::NAN),
        "无效预算"
    );
}

/// **阶梯**：0 → 1 → 2，且对端解不了 VP8 时停在第 1 级
#[test]
fn downgrade_ladder() {
    // 正常路径
    assert_eq!(next_enc_level(0, true), Some(1));
    assert_eq!(next_enc_level(1, true), Some(2));
    assert_eq!(next_enc_level(2, true), None, "已到底");

    // 对端只支持 VP9：换成 VP8 会导致黑屏，必须停在第 1 级
    assert_eq!(
        next_enc_level(0, false),
        Some(1),
        "提速 cpu-used 不改变格式"
    );
    assert_eq!(next_enc_level(1, false), None);
    assert_eq!(next_enc_level(2, false), None);

    // 越界值不应 panic，也不应继续降
    assert_eq!(next_enc_level(9, true), None);
    assert_eq!(next_enc_level(255, false), None);

    println!("阶梯：0 → 1 (cpu-used) → 2 (VP8 试验)");
    println!("对端仅支持 VP9 时停在 1，避免推出无法解码的格式");
}

/// **严重超预算**：一个窗口就够，不必等满 3 个
///
/// 乐观的自救速度是有代价的（误判 → 白白降画质），所以只有超得很狠时
/// 才允许「一窗口定论」。这里的用例把分界定清楚。
#[test]
fn severe_overload_shortens_the_wait() {
    println!(
        "severe 判据：enc + conv > {:.1} × budget",
        sos_vpx::ENCODER_SEVERE_RATIO
    );
    // 真实日志里那台 4 核机器最惨的窗口：enc=43.5 + conv=2.0 > 33.3×1.5=50.0？
    // 43.5+2.0 = 45.5 < 50.0 → 不算 severe，仍走 3 窗口策略。
    // 这是刻意的：45.5ms 是「超了 37%」，还没到「差一倍」的程度。
    assert!(!overload_is_severe(43.5, 2.0, BUDGET_33));
    // 真正编不动的机器：1080p 单线程软编常见 80-120ms
    assert!(overload_is_severe(80.0, 2.0, BUDGET_33), "82ms > 50ms");
    assert!(overload_is_severe(120.0, 3.0, BUDGET_33));
    // 边界：恰好等于 1.5 倍时不触发（用 > 而非 >=）
    let exact = BUDGET_33 * sos_vpx::ENCODER_SEVERE_RATIO - 2.0;
    assert!(!overload_is_severe(exact, 2.0, BUDGET_33));
    assert!(overload_is_severe(exact + 0.1, 2.0, BUDGET_33));
    // 无效输入不触发
    assert!(!overload_is_severe(100.0, 2.0, 0.0));
    assert!(!overload_is_severe(f64::NAN, 2.0, BUDGET_33));
    assert!(!overload_is_severe(100.0, f64::NAN, BUDGET_33));
    assert!(!overload_is_severe(100.0, 2.0, f64::NAN));
}

/// **VP8 试验去留**：只有实测明显更快才保留
///
/// 实测（`tests/codec_cost.rs`，同码率桌面内容）里 VP8 在 2 线程及以上
/// 比 VP9 **慢 27%~127%**。如果程序换了 VP8 却从不回退，用户就会永久停在
/// 「更慢 + 更糊」的配置上 —— 这个判据就是那道闸门。
#[test]
fn vp8_trial_decision() {
    println!(
        "保留 VP8 需要至少快 {:.0}%",
        (1.0 - VP8_TRIAL_KEEP_RATIO) * 100.0
    );

    // 明显更快 → 保留（实测某些内容下 VP8 真的能快数倍）
    let baseline = 45.0;
    assert!(vp8_trial_worth_keeping(baseline, 10.0), "快 78%，保留");
    assert!(vp8_trial_worth_keeping(baseline, 30.0), "快 33%，保留");
    // 刚好达到门槛
    assert!(vp8_trial_worth_keeping(baseline, baseline * 0.84));
    // 没快多少 → 回退（画质换来的流畅度用户看不出来）
    assert!(
        !vp8_trial_worth_keeping(baseline, baseline * VP8_TRIAL_KEEP_RATIO),
        "恰好等于门槛不算改善（用严格小于）"
    );
    assert!(
        !vp8_trial_worth_keeping(baseline, 44.0),
        "只快 2%，不值得降画质"
    );
    // 更慢 → 必须回退（这正是实测里 2 线程以上的情形：45ms → 63ms+）
    assert!(!vp8_trial_worth_keeping(45.0, 63.0), "慢了 40%，必须回退");

    // 基线数据无效 → 保守保留，不拿垃圾数据把工作正常的配置换掉
    assert!(vp8_trial_worth_keeping(0.0, 5.0));
    assert!(vp8_trial_worth_keeping(-1.0, 5.0));
    assert!(vp8_trial_worth_keeping(f64::NAN, 5.0));
    assert!(vp8_trial_worth_keeping(45.0, f64::NAN));
    assert!(vp8_trial_worth_keeping(f64::INFINITY, 5.0));
}

/// **不误伤**：连续 3 个窗口的累计判定逻辑
///
/// 单个窗口超预算不降级（拖窗/播视频的正常波动），
/// 连续 3 次才降 —— 15 秒。这里模拟两种真实的交替场景。
#[test]
fn consecutive_window_policy() {
    const NEEDED: u32 = 3;
    let mut run = |series: &[bool], label: &str| -> u32 {
        let mut overload = 0u32;
        let mut downs = 0u32;
        for &over in series {
            if over {
                overload += 1;
                if overload >= NEEDED {
                    overload = 0;
                    downs += 1;
                }
            } else {
                overload = 0;
            }
        }
        println!("{:<28} 窗口={:?} → 降级次数={}", label, series, downs);
        downs
    };

    // 持续编不动：应该降级
    let d = run(&[true, true, true, true, true, true], "持续超预算");
    assert!(d >= 1, "持续超预算必须降级");

    // 偶发超预算（拖动窗口）：不应降级
    let d = run(
        &[true, false, true, false, true, false, true, false],
        "偶发超预算",
    );
    assert_eq!(d, 0, "不连续的超预算不应降级");

    // 连续两次就恢复：不应降级
    let d = run(&[true, true, false, true, true, false], "最多连续两次");
    assert_eq!(d, 0, "不足 3 个连续窗口不应降级");
}
