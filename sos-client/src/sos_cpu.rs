//! 进程级 CPU 使用率采样，替代上游的 PDH 实现。
//!
//! # 为什么不用上游的 `start_cpu_performance_monitor()`
//!
//! 上游走 PDH：`PdhOpenQuery` + `PdhAddEnglishCounter("\Processor(_total)\% Processor Time")`
//! + `PdhCollectQueryDataEx` 每 2 秒采一次，结果放在全局队列里。
//!
//! 换掉它的理由是结构性的，不是性能：
//!
//! - PDH 查询对象是有状态的全局资源，句柄泄漏或计数器集合被禁用后
//!   会**永久**不再产出数据，而上游没有重建路径；
//! - 首次采集需要两个样本才有有效值，上游为此额外挂了一个补种线程。
//!
//! # 关于「41ms / 性能计数器损坏」的错误结论（已撤回）
//!
//! 早期版本的本文档声称现场机器每次 PDH 查询消耗 41ms 的 `WmiPrvSE.exe` CPU、
//! 且性能计数器注册信息已损坏。**这是算错的**：两阶段 A/B 的实际时长不同
//! （16.3s vs 15.0s），却直接拿原始 ms 差除以采集次数。
//! 改用「复用同一个查询 + 基准差分」后实测 **0.0ms**，与健康机器一致。
//! 因此不要为此执行 `lodctr /R`（全机操作，有害无益）。
//!
//! # 这里怎么做
//!
//! `GetSystemTimes` 是单次内核调用，返回**自开机以来**的
//! idle / kernel / user 累计时间。两次采样求差即可算出这段时间内的
//! CPU 占用。特点：
//!
//! - 不经过 PDH、不经过 WMI、不依赖任何 provider 或注册表性能数据；
//! - 不持有需要生命周期管理的查询对象，没有失效后不可恢复的状态；
//! - 与任务管理器的数据源一致（`kernel` 含 `idle`，计算时要减掉）。
//!
//! 采样线程每 2 秒跑一次，与上游 PDH 的 `SAMPLE_INTERVAL` 保持一致，
//! 使 `cpu_usage_one_minute()` 的语义不变。

#![cfg(windows)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::Threading::GetSystemTimes;

/// 采样周期（与上游 PDH 的 `SAMPLE_INTERVAL: DWORD = 2` 一致）
const SAMPLE_INTERVAL: Duration = Duration::from_secs(2);

/// 保留多少个样本用于「最近一分钟」的平均值。
/// 30 个 × 2 秒 = 60 秒，与上游 `queue.len() == 31` 的窗口等价。
const WINDOW_SAMPLES: usize = 30;

/// 最近一分钟的平均 CPU 使用率（百分比，0-100），以及它的采样时刻
static CPU_USAGE_ONE_MINUTE: Mutex<Option<(f64, Instant)>> = Mutex::new(None);
/// 监视线程是否已经启动
static STARTED: AtomicBool = AtomicBool::new(false);

/// 把 `FILETIME` 还原成 100ns 计数
fn ft_to_u64(f: FILETIME) -> u64 {
    ((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64
}

/// `(idle, kernel, user)` 三个累计时间，单位 100ns
fn system_times() -> Option<(u64, u64, u64)> {
    let mut idle = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    unsafe {
        GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)).ok()?;
    }
    Some((ft_to_u64(idle), ft_to_u64(kernel), ft_to_u64(user)))
}

/// 计算两次采样之间的 CPU 使用率（百分比，0-100）
///
/// 关键点：`GetSystemTimes` 的 **kernel 时间包含 idle 时间**，
/// 所以忙碌时间 = (kernel_delta - idle_delta) + user_delta，
/// 总时间 = kernel_delta + user_delta。漏掉这个减法会把空闲也算成忙碌，
/// 结果永远接近 100%。
///
/// `pub` 供 `tests/sos_cpu.rs` 直接验证纯计算部分（不需要真实采样）。
pub fn usage_between(prev: (u64, u64, u64), cur: (u64, u64, u64)) -> Option<f64> {
    let idle_d = cur.0.checked_sub(prev.0)? as f64;
    let kern_d = cur.1.checked_sub(prev.1)? as f64;
    let user_d = cur.2.checked_sub(prev.2)? as f64;
    let total = kern_d + user_d;
    if total <= 0.0 {
        return None;
    }
    let busy = (kern_d - idle_d) + user_d;
    Some((busy / total * 100.0).clamp(0.0, 100.0))
}

/// 启动 CPU 使用率采样线程（进程内只启动一次）。
///
/// 与上游 `start_cpu_performance_monitor()` 的调用位置一致，
/// 但完全不使用 PDH。
pub fn start_cpu_monitor() {
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::Builder::new()
        .name("sos-cpu".into())
        .spawn(|| {
            let mut prev = match system_times() {
                Some(t) => t,
                None => {
                    log::warn!("[CPU] GetSystemTimes failed, CPU usage unavailable");
                    return;
                }
            };
            // 首个样本需要一个间隔才有意义，先取一次差值再进入循环
            let mut window: Vec<f64> = Vec::with_capacity(WINDOW_SAMPLES);
            loop {
                std::thread::sleep(SAMPLE_INTERVAL);
                let Some(cur) = system_times() else {
                    log::warn!("[CPU] GetSystemTimes failed mid-flight");
                    continue;
                };
                if let Some(pct) = usage_between(prev, cur) {
                    if window.len() == WINDOW_SAMPLES {
                        window.remove(0);
                    }
                    window.push(pct);
                    let avg = window.iter().sum::<f64>() / window.len() as f64;
                    if let Ok(mut slot) = CPU_USAGE_ONE_MINUTE.lock() {
                        *slot = Some((avg, Instant::now()));
                    }
                    // 同时写进上游缓存：`scrap::codec::codec_thread_num()` 等
                    // 上游代码读的是 `hbb_common::platform::windows::cpu_uage_one_minute()`。
                    // 我们自己不用它（sos_vpx 的线程数由 recommended_threads 决定），
                    // 但库内其它调用点应当看到真实值而不是 None。
                    hbb_common::platform::windows::sync_cpu_usage(Some(avg));
                }
                prev = cur;
            }
        })
        .ok();
}

/// 最近一分钟的平均 CPU 使用率；超过 30 秒没更新则视为不可用（与上游一致）。
///
/// 生产路径不读它（采样线程自己维护窗口，并把结果同步进上游缓存），
/// 仅供 `tests/cpu_usage.rs` / `tests/sos_cpu.rs` 验证采样线程确实在产出。
#[allow(dead_code)]
pub fn cpu_usage_one_minute() -> Option<f64> {
    let guard = CPU_USAGE_ONE_MINUTE.lock().ok()?;
    match *guard {
        Some((v, at)) if at.elapsed().as_secs() < 30 => Some(v),
        _ => None,
    }
}


