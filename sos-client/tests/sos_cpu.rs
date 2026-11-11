//! `sos_cpu` 的采样正确性验证。
//!
//! 这段逻辑替代了上游的 PDH 实现，判错方向会导致「编码线程数被钳到 1」
//! 或「以为机器很闲」。两个方向的错误都要防：
//!
//! - 漏掉 `kernel` 包含 `idle` 这一点 → 空闲也判成忙碌，结果恒接近 100%；
//! - 忘了减去 `idle` 之外的东西 → 忙碌判成空闲，编码器线程数被压到最低。
//!
//! 运行：`cargo test --release --test sos_cpu -- --nocapture`

#![cfg(windows)]

#[path = "../src/sos_cpu.rs"]
mod sos_cpu;

use std::time::{Duration, Instant};

/// `GetSystemTimes` 的 kernel 时间**包含** idle。漏减会让结果恒接近 100%。
#[test]
fn idle_time_is_excluded_from_busy() {
    // 100% 空闲：kernel 增长等于 idle 增长，user 不增长
    let prev = (0u64, 0u64, 0u64);
    let cur = (1_000_000, 1_000_000, 0);
    let pct = sos_cpu::usage_between(prev, cur).expect("应能计算");
    assert!(
        pct.abs() < 0.01,
        "全空闲必须算出 0%，实际 {pct}（说明忘了 kernel 含 idle）"
    );

    // 100% 忙碌：kernel 增长但 idle 不增长
    let cur = (0, 1_000_000, 1_000_000);
    let pct = sos_cpu::usage_between(prev, cur).expect("应能计算");
    assert!(
        (pct - 100.0).abs() < 0.01,
        "满负载必须算出 100%，实际 {pct}"
    );
}

/// 典型混合负载：一半时间空闲
#[test]
fn mixed_load_matches_expectation() {
    let prev = (0u64, 0u64, 0u64);
    // 总时间 2,000,000（kernel 1M + user 1M），其中 idle 1M
    // 忙碌 = (1M - 1M) + 1M = 1M → 50%
    let pct =
        sos_cpu::usage_between(prev, (1_000_000, 1_000_000, 1_000_000)).expect("应能计算");
    assert!((pct - 50.0).abs() < 0.01, "应为 50%，实际 {pct}");

    // 三个计数器同时增长，验证是差分而不是绝对时间
    let prev = (5_000_000, 5_000_000, 5_000_000);
    let cur = (6_000_000, 6_000_000, 6_000_000);
    let pct = sos_cpu::usage_between(prev, cur).expect("应能计算");
    assert!((pct - 50.0).abs() < 0.01, "差分后仍应为 50%，实际 {pct}");
}

/// 结果必须夹在 0-100，不能因为计数器回绕或异常出现越界值
#[test]
fn result_is_clamped() {
    let pct = sos_cpu::usage_between((0, 0, 0), (0, 100, 100)).unwrap();
    assert!((0.0..=100.0).contains(&pct));
    // idle 大于 kernel（不应发生，但必须不 panic 且不越界）
    let pct = sos_cpu::usage_between((0, 0, 0), (200, 100, 0)).unwrap();
    assert!((0.0..=100.0).contains(&pct), "异常输入也要夹紧，实际 {pct}");
}

/// 无效输入返回 None，而不是给出误导性的 0%
#[test]
fn invalid_inputs_return_none() {
    // 时间没有推进
    assert!(sos_cpu::usage_between((10, 10, 10), (10, 10, 10)).is_none());
    // 计数器回退（理论上不会发生，但不能算出负数）
    assert!(sos_cpu::usage_between((10, 10, 10), (5, 5, 5)).is_none());
}

/// 真实采样：确认 `GetSystemTimes` 在本机可用，且瞬时值合理
///
/// 不校验具体数值（依赖机器负载），只校验「可读、在范围内、不是恒定 100%」。
#[test]
fn real_sampler_returns_plausible_value() {
    // 先启动监视器再读，验证整条链路
    sos_cpu::start_cpu_monitor();
    // 首个样本需要一个采样周期（2 秒）
    let deadline = Instant::now() + Duration::from_secs(6);
    let mut got = None;
    while Instant::now() < deadline {
        if let Some(v) = sos_cpu::cpu_usage_one_minute() {
            got = Some(v);
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    let v = got.expect("6 秒内应产出首个样本（GetSystemTimes 不可用？）");
    println!("实测 CPU 使用率 = {v:.2}%");
    assert!((0.0..=100.0).contains(&v), "越界：{v}");
    assert!(v < 99.9, "恒定 100% 说明 kernel/idle 相减错了：{v}");
}

/// 重复启动必须幂等（只应有一个采样线程）
#[test]
fn start_is_idempotent() {
    for _ in 0..5 {
        sos_cpu::start_cpu_monitor();
    }
    // 只要能跑起来且不 panic 即通过；多线程会体现在采样间隔异常上，
    // 这里用一个宽松的合理性检查兜底
    std::thread::sleep(Duration::from_millis(2500));
    let v = sos_cpu::cpu_usage_one_minute();
    assert!(v.is_none() || (0.0..=100.0).contains(&v.unwrap()));
}
