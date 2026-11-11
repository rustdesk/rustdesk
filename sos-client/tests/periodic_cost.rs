//! 测量采集循环里两个「周期性检查」的真实代价。
//!
//! 背景：`[VIDEO:PERF] it` 比 `spf` 稳定多出约 20ms，而 cap/conv/enc/send 都很小。
//! 嫌疑集中在每秒执行一次的检查上：
//! - 显示器尺寸检测曾用 `Display::all()`（创建 DXGI 工厂 + 枚举适配器）
//! - `is_process_consent_running()`（枚举进程）
//!
//! 运行：cargo test --release --test periodic_cost -- --nocapture --test-threads=1

#![cfg(windows)]

#[path = "../src/sos_uac.rs"]
mod sos_uac;

use std::time::{Duration, Instant};

fn bench<F: FnMut()>(name: &str, iters: u32, mut f: F) {
    // 预热，避免把首次初始化的代价算进去
    f();
    let start = Instant::now();
    for _ in 0..iters {
        f();
    }
    let per = start.elapsed().as_secs_f64() * 1000.0 / iters as f64;
    println!("{name:<40} {per:>8.2} ms/次");
}

#[test]
fn periodic_check_cost() {
    println!("=== 周期性检查耗时（每秒执行一次的代价）===");

    bench("Display::all()（原显示器检测）", 20, || {
        let d = scrap::Display::all();
        std::hint::black_box(d.map(|v| v.len()).unwrap_or(0));
    });

    bench("GetSystemMetrics（新显示器检测）", 20, || {
        use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
        let (w, h) = unsafe { (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)) };
        std::hint::black_box(w as u64 + h as u64);
    });

    bench("is_process_consent_running()（上游）", 20, || {
        let r = librustdesk::platform::windows::is_process_consent_running();
        std::hint::black_box(r.map(|b| b as u64).unwrap_or(0));
    });

    bench("sos_uac::consent_running()（自建快照）", 200, || {
        std::hint::black_box(sos_uac::consent_running() as u64);
    });

    bench("desktop_changed()", 20, || {
        std::hint::black_box(librustdesk::platform::windows::desktop_changed());
    });

    // 换算成「每帧摊销」：30fps 下每秒 30 轮
    println!();
    println!("以上每项都是每秒执行一次；30fps 时每帧摊销 = 单次耗时 / 30。");

    // 上游实现的副作用比耗时更值得注意：每次调用都会拉起
    // cmd.exe + tasklist.exe + findstr.exe 三个进程。每次进程创建/销毁
    // 都会产生一次 WMI `Win32_Process` 实例事件，注册了该通知的组件
    // （杀毒 / EDR / SCCM / OEM 管理软件）会在 WmiPrvSE.exe 里被唤醒。
    // 所以这个调用在高频路径上是「越用越慢」的放大器，不只是 205ms。
    println!();
    println!("注意：上游 is_process_consent_running() 每次创建 3 个子进程；");
    println!("      高频调用会放大系统级 WMI 事件处理开销（WmiPrvSE.exe）。");
    let _ = Duration::ZERO;
}
