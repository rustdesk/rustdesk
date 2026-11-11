//! `sos_cpu` 的 CPU 使用率计算：`GetSystemTimes` 差值的正确性。
//!
//! ## 为什么要单独测
//!
//! `GetSystemTimes` 返回的三个累计时间有一个容易踩的陷阱：
//! **`kernel` 时间包含 `idle` 时间**（`lpKernelTime` 是「内核模式时间，
//! 含空闲」）。所以忙碌时间必须是
//!
//! ```text
//! busy = (kernel_delta - idle_delta) + user_delta
//! total = kernel_delta + user_delta
//! ```
//!
//! 直接用 `(kernel + user - idle) / (kernel + user)` 是常见写法，但
//! 分母如果也减掉 idle 就会算出 >100%。如果忘了减 idle，
//! 结果永远接近 100%，看起来「CPU 一直满载」——这正是排查此类问题时
//! 最容易得出错误结论的地方。
//!
//! 运行：`cargo test --release --test cpu_usage -- --nocapture`

#![cfg(windows)]

#[path = "../src/sos_cpu.rs"]
mod sos_cpu;

use sos_cpu::usage_between;

#[test]
fn kernel_includes_idle() {
    // 总跨度 100 单位：60 空闲（只体现在 kernel）+ 40 用户态
    // kernel_delta = 60, user_delta = 40
    // busy = (60 - 60) + 40 = 40 → 40%
    let pct = usage_between((0, 0, 0), (60, 60, 40)).expect("valid");
    println!("空闲 60 + 用户 40 → {pct:.2}%");
    assert!((pct - 40.0).abs() < 0.01, "got {pct}");
}

#[test]
fn fully_busy() {
    let pct = usage_between((0, 0, 0), (0, 50, 50)).expect("valid");
    println!("满负载 → {pct:.2}%");
    assert!((pct - 100.0).abs() < 0.01, "got {pct}");
}

#[test]
fn fully_idle() {
    let pct = usage_between((0, 0, 0), (100, 100, 0)).expect("valid");
    println!("全空闲 → {pct:.2}%");
    assert!(pct.abs() < 0.01, "got {pct}");
}

/// 两次采样之间没有时间流逝，或者计数器回绕 → 必须返回 `None`
/// 而不是一个错误的 0% 或 100%
#[test]
fn invalid_deltas_are_none() {
    assert!(usage_between((5, 5, 5), (5, 5, 5)).is_none(), "无流逝时间");
    assert!(usage_between((9, 9, 9), (1, 1, 1)).is_none(), "计数器回绕");
}

/// 结果永远夹在 0-100 之间（idle 增量异常大于 kernel 增量时也不能出负数）
#[test]
fn always_in_range() {
    let pct = usage_between((0, 0, 0), (200, 100, 0)).expect("valid");
    assert_eq!(pct, 0.0, "idle > kernel 时应夹到 0");
}

/// 真实机器上取一次样本，确认识别到的量级合理
#[test]
fn real_sample_is_plausible() {
    use std::time::Duration;
    // 直接调用公开 API：启动采样并等两个周期
    sos_cpu::start_cpu_monitor();
    std::thread::sleep(Duration::from_secs(5));
    match sos_cpu::cpu_usage_one_minute() {
        Some(v) => {
            println!("实测一分钟均值 = {v:.2}%");
            assert!((0.0..=100.0).contains(&v), "越界: {v}");
        }
        None => println!("（采样窗口尚未填满，读不到值；不算失败）"),
    }
}
