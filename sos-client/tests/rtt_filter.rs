//! RTT 下界滤波（min-filter）的行为验证。
//!
//! 背景：`VideoQoS::user_network_delay` 的延迟历史窗口只有 3 个样本，
//! `fps = 150 * min_fps / avg_delay`。实测日志中 `qos_fps` 反复被打到 1-6：
//!
//! ```text
//! encode_out=29.2fps qos_fps=30 rtt=178ms    ← 正常
//! encode_out=14.0fps qos_fps=1  rtt=725ms    ← 崩到 1fps
//! ```
//!
//! 尝试过的方案：
//! - 上限阈值 600ms：490ms 照样通过，无效
//! - 中值滤波窗口 5：尖峰常常连续 3 个以上（控制端解码大帧时主线程忙），
//!   中位数会跟着跑，实测仍有 725ms 漏过
//!
//! 最终方案：**窗口最小值**。排队延迟恒为正，真实 RTT 是下界，
//! 只有窗口内所有样本都变高才判定网络真的恶化。
//!
//! 运行：cargo test --release --test rtt_filter -- --nocapture

use std::collections::VecDeque;

const RTT_WINDOW: usize = 5;

/// 与 `sos_connection.rs` 中的实现保持一致
struct RttFilter {
    window: VecDeque<u32>,
}

impl RttFilter {
    fn new() -> Self {
        Self {
            window: VecDeque::new(),
        }
    }

    /// 返回窗口最小值（RTT 下界估计）
    fn push(&mut self, raw: u32) -> u32 {
        if self.window.len() >= RTT_WINDOW {
            self.window.pop_front();
        }
        self.window.push_back(raw);
        self.window.iter().copied().min().unwrap_or(raw)
    }
}

#[test]
fn isolated_spike_is_ignored() {
    let mut f = RttFilter::new();
    for _ in 0..RTT_WINDOW {
        assert_eq!(f.push(36), 36);
    }

    let out = f.push(490);
    println!("36x5 之后来一个 490 → 输出 {out}");
    assert_eq!(out, 36, "孤立尖峰不应影响下界估计");

    let out = f.push(880);
    println!("再来一个 880 → 输出 {out}");
    assert_eq!(out, 36);
}

/// 这是中值滤波失败的场景：连续 3-4 个高样本
#[test]
fn consecutive_spikes_are_ignored() {
    let mut f = RttFilter::new();
    for _ in 0..RTT_WINDOW {
        f.push(36);
    }

    // 实测中控制端忙 3 秒以上很常见
    let mut outs = Vec::new();
    for i in 0..4 {
        let out = f.push(725);
        outs.push(out);
        println!("连续第 {} 个 725ms 样本 → 输出 {out}", i + 1);
    }
    println!("（中值滤波在这里会输出 725，导致 qos_fps 崩到 1）");
    assert_eq!(outs[3], 36, "连续 4 个尖峰也不能影响下界估计");
}

#[test]
fn sustained_degradation_is_detected() {
    let mut f = RttFilter::new();
    for _ in 0..RTT_WINDOW {
        f.push(36);
    }

    // 网络真的变差了：全部样本都是 300ms，走满一个窗口后必须反映出来
    let mut outs = Vec::new();
    for i in 0..8 {
        let out = f.push(300);
        outs.push(out);
        println!("第 {} 个 300ms 样本 → 输出 {out}", i + 1);
    }
    assert_eq!(*outs.last().unwrap(), 300, "持续恶化必须被反映出来");
}

#[test]
fn recovers_immediately() {
    let mut f = RttFilter::new();
    for _ in 0..RTT_WINDOW {
        f.push(300);
    }
    // 第一个低样本到达就立即恢复（min 的天然优势）
    let out = f.push(36);
    println!("从 300ms 恢复到 36ms → 输出 {out}");
    assert_eq!(out, 36, "min-filter 恢复应该是立刻的");
}

#[test]
fn realistic_sequence() {
    // 复刻实测序列：稳定 36ms，中间穿插控制端忙导致的尖峰
    let samples = [
        36, 35, 37, 36, 35, 490, 36, 725, 725, 35, 36, 38, 225, 36, 35, 36, 178, 35, 36, 36,
    ];
    let mut f = RttFilter::new();
    let mut worst = 0u32;
    for (i, s) in samples.iter().enumerate() {
        let out = f.push(*s);
        // 前 5 个样本是窗口填充期，跳过
        if i >= RTT_WINDOW {
            worst = worst.max(out);
        }
        println!("采样 {s:>3}ms → 输出 {out:>3}ms");
    }
    println!("窗口填充后的最大输出: {worst}ms");
    assert!(
        worst <= 40,
        "真实序列里绝不应把延迟报成 {worst}ms（那会让 QoS 压帧率）"
    );
}
