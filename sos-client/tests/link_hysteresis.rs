//! 链路档位的**迟滞**：单个探测样本不能翻转码率预算。
//!
//! 档位直接决定两件事：
//!
//! 1. 码率是否乘 `GOOD_LINK_BITRATE_GAIN`（2 倍）；
//! 2. `rc_dropframe_thresh` 是 0（不丢帧）还是 20（守码率）。
//!
//! 用单样本判据时，一次迟到的 TestDelay 回显就能在编码器重建的瞬间把
//! 5208kbps 砍到 2604kbps **并**打开丢帧。生产日志里的证据：
//!
//! ```text
//! 211.291  peer requested refresh of display 0   ← 控制端主线程正忙
//! 211.464  重建编码器 → bitrate=2604kbps         ← 1736 × 1.5，Constrained
//! 213.036  ratio 1.50 ... -> 5208kbps            ← 两秒后又回到 Generous
//! 214.174  3/133 frames dropped (2%)             ← 这一秒在丢帧
//! ```
//!
//! 反向抖动（一个漂亮样本就把码率翻倍）同样有害，会在真实拥塞的链路上
//! 养出 bufferbloat。所以两个方向都要连续确认。
//!
//! 运行：cargo test --release --test link_hysteresis -- --nocapture

#![cfg(all(windows, sos_vpx))]

#[path = "../src/sos_vpx.rs"]
mod sos_vpx;

use sos_vpx::{LinkTier, LINK_BAD_SAMPLES_TO_DOWNGRADE, LINK_GOOD_SAMPLES_TO_UPGRADE};

/// 生产日志里最典型的「好链路」样本：有线内网直连
const GOOD: (Option<u32>, Option<u32>) = (Some(1), Some(0));
/// 单个迟到回显：RTT 下界仍是 1ms，但这一趟等了 45ms
const LATE_ECHO: (Option<u32>, Option<u32>) = (Some(1), Some(45));

fn feed(tier: &mut LinkTier, n: u32, sample: (Option<u32>, Option<u32>)) -> u32 {
    let mut changes = 0;
    for _ in 0..n {
        if tier.sample(sample.0, sample.1) {
            changes += 1;
        }
    }
    changes
}

/// 初始必须保守：还没有样本时不能超发
#[test]
fn starts_constrained() {
    assert!(!LinkTier::new().generous());
    assert!(!LinkTier::default().generous());
}

/// 进档需要连续 [`LINK_GOOD_SAMPLES_TO_UPGRADE`] 个好样本
#[test]
fn upgrade_needs_consecutive_good_samples() {
    let mut tier = LinkTier::new();
    assert_eq!(
        feed(&mut tier, LINK_GOOD_SAMPLES_TO_UPGRADE - 1, GOOD),
        0,
        "差一个样本就不能进档"
    );
    assert!(!tier.generous());
    assert_eq!(feed(&mut tier, 1, GOOD), 1, "第 N 个样本触发进档");
    assert!(tier.generous());
}

/// **核心回归**：单个迟到回显不能把档位拉回 Constrained
#[test]
fn single_late_echo_does_not_downgrade() {
    let mut tier = LinkTier::new();
    feed(&mut tier, LINK_GOOD_SAMPLES_TO_UPGRADE, GOOD);
    assert!(tier.generous());

    assert_eq!(feed(&mut tier, 1, LATE_ECHO), 0, "一个坏样本不足以翻档");
    assert!(
        tier.generous(),
        "编码器重建/请求刷新时的一个迟到回显不该把码率砍半"
    );

    assert_eq!(
        feed(&mut tier, LINK_BAD_SAMPLES_TO_DOWNGRADE - 1, LATE_ECHO),
        1,
        "第 2 个连续坏样本才出档"
    );
    assert!(!tier.generous());
}

/// 出档后必须重新攒满好样本，不能继承旧进度
#[test]
fn downgrade_resets_upgrade_progress() {
    let mut tier = LinkTier::new();
    feed(&mut tier, LINK_GOOD_SAMPLES_TO_UPGRADE, GOOD);
    // 攒一半进度
    feed(&mut tier, LINK_GOOD_SAMPLES_TO_UPGRADE - 1, GOOD);
    assert!(tier.generous());

    feed(&mut tier, LINK_BAD_SAMPLES_TO_DOWNGRADE, LATE_ECHO);
    assert!(!tier.generous());

    // 只剩 1 个样本就该重新进档 —— 若旧进度没清零，这里会提前进档
    assert_eq!(
        feed(&mut tier, LINK_GOOD_SAMPLES_TO_UPGRADE - 1, GOOD),
        0,
        "出档瞬间进度必须是 0"
    );
    assert_eq!(feed(&mut tier, 1, GOOD), 1);
    assert!(tier.generous());
}

/// 坏样本中间的孤立好样本同样会中断「连续坏」
#[test]
fn good_sample_interrupts_bad_streak() {
    let mut tier = LinkTier::new();
    feed(&mut tier, LINK_GOOD_SAMPLES_TO_UPGRADE, GOOD);

    feed(&mut tier, 1, LATE_ECHO);
    feed(&mut tier, 1, GOOD);
    assert_eq!(feed(&mut tier, 1, LATE_ECHO), 0, "计数已被好样本清零");
    assert!(tier.generous(), "2 个坏样本必须**连续**才出档");
}

/// 样本未知（还没测到 / 回显超时）按坏样本处理，但不会让保守档位来回抖
#[test]
fn unknown_samples_are_conservative() {
    let mut tier = LinkTier::new();
    assert!(!tier.sample(None, None), "保守档位下未知样本不是状态变化");
    assert_eq!(feed(&mut tier, 1, (None, None)), 0);
    assert!(!tier.generous());

    // 已有的进档进度会被未知样本打断
    feed(&mut tier, LINK_GOOD_SAMPLES_TO_UPGRADE - 1, GOOD);
    feed(&mut tier, 1, (Some(1), None));
    assert_eq!(
        feed(&mut tier, LINK_GOOD_SAMPLES_TO_UPGRADE - 1, GOOD),
        0,
        "进度清零后要重新攒满"
    );
    assert!(!tier.generous());
}

/// 低 RTT 但有排队（低延迟 Wi-Fi）不能进档
#[test]
fn queued_link_never_upgrades() {
    let mut tier = LinkTier::new();
    assert_eq!(
        feed(
            &mut tier,
            LINK_GOOD_SAMPLES_TO_UPGRADE * 3,
            (Some(8), Some(120))
        ),
        0
    );
    assert!(!tier.generous());
}

/// 重置回保守档位
#[test]
fn reset_returns_to_conservative() {
    let mut tier = LinkTier::new();
    feed(&mut tier, LINK_GOOD_SAMPLES_TO_UPGRADE, GOOD);
    assert!(tier.generous());
    tier.reset();
    assert!(!tier.generous());
    assert_eq!(feed(&mut tier, LINK_GOOD_SAMPLES_TO_UPGRADE - 1, GOOD), 0);
}
