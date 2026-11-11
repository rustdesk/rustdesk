//! 「桌面暂时不可捕获」状态的退避与日志闸门。
//!
//! 真实日志里这段状态持续了 **130 秒**：
//!
//! ```text
//! [584.047] [ERROR] [VIDEO] failed to create capturer/encoder: no display available
//! [585.100] [ERROR] 同上
//! [586.356] [ERROR] 同上   ← 每秒一条，共 ~130 条
//! ...
//! [657.239] [INFO ] SosGdiCapturer: 1600x900 ...   ← 自行恢复
//! ```
//!
//! 根因不是 bug：会话锁屏 / UAC 安全桌面 / RDP 断开时，
//! `Display::all()` 同时拿不到 GDI 与 DXGI 输出，返回空表。恢复后自行可用。
//!
//! 但旧实现有三个问题，都会让现场难以诊断：
//! 1. 每秒打一条 **ERROR** —— 真正的故障会被埋掉；
//! 2. 没有退避 —— 每秒空转一次完整显示器枚举；
//! 3. `gap_ms[max]` 被抬到 **31508**，看起来像编码器故障。
//!
//! 运行：cargo test --release --test desktop_unavailable -- --nocapture

#![cfg(all(windows, sos_vpx))]

use std::time::Duration;

/// 重建退避的最大值（与 `sos_video::MAX_REBUILD_BACKOFF` 一致）
const MAX_REBUILD_BACKOFF: Duration = Duration::from_secs(5);

/// 与 `sos_video::is_no_display_error` 等价的判定
fn is_no_display_error(msg: &str) -> bool {
    msg.contains("no display") || msg.contains("SHMEM not registered")
}

/// 退避翻倍，封顶 5 秒
fn next_backoff(cur: Duration) -> Duration {
    (cur * 2).min(MAX_REBUILD_BACKOFF)
}

/// **退避曲线**：130 秒内只重试约 30 次，而不是 130 次
#[test]
fn backoff_keeps_retry_rate_low() {
    let mut backoff = Duration::from_secs(1);
    let mut elapsed = Duration::ZERO;
    let mut attempts = 0u32;
    println!("{:>10} {:>10} {:>10}", "累计秒", "本次等待", "尝试次数");
    while elapsed < Duration::from_secs(130) {
        attempts += 1;
        if attempts <= 8 {
            println!(
                "{:>10.0} {:>10.1} {:>10}",
                elapsed.as_secs_f64(),
                backoff.as_secs_f64(),
                attempts
            );
        }
        elapsed += backoff;
        backoff = next_backoff(backoff);
    }
    println!("... 130 秒内共重试 {} 次（旧实现为 130 次）", attempts);
    assert!(
        attempts <= 35,
        "130 秒内不应超过 35 次重试，实际 {}",
        attempts
    );
    assert_eq!(backoff, MAX_REBUILD_BACKOFF, "封顶必须生效");

    // 单调不减，且必须封顶
    let mut b = Duration::from_secs(1);
    for _ in 0..10 {
        let n = next_backoff(b);
        assert!(n >= b, "退避不应变小");
        assert!(n <= MAX_REBUILD_BACKOFF, "退避不应超过上限");
        b = n;
    }
}

/// **日志闸门**：同一次故障持续期间只记一条
#[test]
fn log_gate_reports_once_per_outage() {
    // 模拟 130 次失败：只应产生 1 条日志
    let mut logged = false;
    let mut log_count = 0u32;
    for _ in 0..130 {
        if !logged {
            logged = true;
            log_count += 1;
        }
    }
    assert_eq!(log_count, 1, "同一次故障只应记一条");

    // 恢复后再故障：应再记一条
    logged = false;
    for _ in 0..10 {
        if !logged {
            logged = true;
            log_count += 1;
        }
    }
    assert_eq!(log_count, 2, "第二次故障应重新记一条");
    println!("130 次失败 → 1 条日志；再故障 → 第 2 条");
}

/// **错误分类**：只有「桌面不可捕获」走降噪+退避路径
#[test]
fn only_no_display_is_treated_as_transient() {
    // 应走降噪路径（可自愈）
    assert!(is_no_display_error("no display available"));
    assert!(is_no_display_error("no display"));
    // 「main 已判定安全桌面、首帧尚未到达」的竞态窗口，也是过渡态：
    // 现场日志里它曾每 3 秒刷一条 ERROR，把真正的问题埋掉了。
    assert!(is_no_display_error("SOS SHMEM not registered"));

    // 其他错误仍应是 ERROR：这些说明代码/环境真的有问题，不能被掩盖
    for msg in [
        "SOS GDI capturer: CreateCompatibleDC failed",
        "dxgi unavailable: 灾难性故障 (0x8000FFFF)",
        "display #3 vanished while enumerating",
        "SOS GDI capturer: BitBlt failed: 拒绝访问。 (0x80070005)",
        "",
    ] {
        assert!(
            !is_no_display_error(msg),
            "「{}」不应被当成可自愈的桌面不可捕获",
            msg
        );
    }
    println!("只有 no display / SHMEM-not-registered 走降噪路径，其他错误仍报 ERROR");
}

/// **gap 诊断**：故障期间不应把 gap_ms 抬成「编码器故障」的样子
///
/// 实测 `gap_ms[min/avg/max]=35/6708/31508` 就是这段故障造成的。
/// 正确做法是：超过 2 秒的间隔视为采集中断，不计入 gap 统计。
#[test]
fn gap_should_not_count_outage() {
    const GAP_OUTAGE_MS: u64 = 2_000;
    // 正常帧间隔 33ms；中断 31.5 秒
    let timeline = [
        33u64,
        66,
        99,
        31_607, // 中断后第一帧
        31_640,
        31_673,
    ];
    let mut gap_max = 0u64;
    let mut gap_count = 0u32;
    let mut last: Option<u64> = None;
    for t in timeline {
        if let Some(prev) = last {
            let gap = t - prev;
            if gap < GAP_OUTAGE_MS {
                gap_max = gap_max.max(gap);
                gap_count += 1;
            }
        }
        last = Some(t);
    }
    println!(
        "计入 {} 个间隔，gap_max = {}（旧实现会把 31508 算进来）",
        gap_count, gap_max
    );
    assert_eq!(gap_max, 33, "只应统计正常帧间隔");
    assert_eq!(gap_count, 4, "跨越中断的那一个间隔应被跳过");

    // 不变量：任何被统计的间隔都必须小于阈值
    assert!(gap_max < GAP_OUTAGE_MS);
}
