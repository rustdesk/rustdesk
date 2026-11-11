//! RustDesk SOS 精简受控端 — 主入口
//!
//! 基于 `hbb_common` 库构建，仅支持 Windows。
//! 单一可执行文件，通过命令行参数切换运行模式：
//!
//! ```text
//! rustdesk-sos.exe               → 主进程模式（托盘 + 全部服务）
//! rustdesk-sos.exe --service     → SYSTEM 子进程模式（UAC 安全桌面捕获）
//! ```
//!
//! 每个参数都有短选项：`-d` `-m` `-r` `-k` `-p` `-s`（见 [`Cli`]）。
//!
//! 主进程必须以管理员权限运行；它用 winlogon 令牌把自身以 SYSTEM 身份
//! 再拉起一份 `--service` 实例，后者只负责在安全桌面出现时往共享内存写帧。
//!
//! Release 构建使用 Windows GUI 子系统（默认无控制台窗口）。
//! 传入 `--debug` 时分配控制台，显示日志输出。

// Release 构建使用 Windows GUI 子系统，不显示控制台窗口
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// ── 模块地图 ──
//
// 全部平铺在 `src/` 下并用 `sos_` 前缀，是为了让 `tests/*.rs` 能用
// `#[path = "../src/sos_xxx.rs"] mod sos_xxx;` 直接复用单个模块做单测
// （测试里的 `crate::sos_xxx` 因此也能解析）。移动文件会连带改动
// 30+ 个测试的 `#[path]`，所以这里只做**逻辑分组**、不动文件位置。
//
//   启动 / 进程
//     sos_bootstrap   日志初始化（主进程 / SYSTEM 子进程各一套文件）
//     sos_config      注册表配置 + CLI 合并后的运行时配置
//     sos_constants   全局常量与 Windows 对象名
//     sos_system      SYSTEM 子进程的启动与服务循环
//     sos_tray        托盘图标与菜单
//     sos_cpu         GetSystemTimes CPU 采样（替代上游 PDH）
//
//   捕获
//     sos_capture     SYSTEM 子进程的采集线程（写 SHMEM）
//     sos_dxgi        自建 DXGI Desktop Duplication 采集器
//     sos_gdi         自建 GDI 采集器（含安全桌面自检/降级/状态图）
//     sos_display     显示器枚举与尺寸查询
//     sos_uac         安全桌面 / 会话连接状态判定
//
//   视频
//     sos_video       每显示器的采集→转换→编码→发送循环
//     sos_vpx         自建 libvpx 编码器与码率/档位策略
//     sos_qos         RTT/帧率/码率/画质比（多显示器）
//
//   网络
//     sos_connection    会话主循环、能力协商、消息分派
//     sos_rendezvous    信令服务器交互
//     sos_relay         中继地址解析
//     sos_file_transfer 文件传输
//     sos_pipe          命名管道（主进程 ↔ SYSTEM 子进程）
//     sos_shmem         共享内存（帧数据通道）
//
//   输入
//     sos_input       远程鼠标/键盘注入

// ── 启动 / 进程 ──
mod sos_bootstrap;
mod sos_config;
mod sos_constants;
#[cfg(windows)]
mod sos_cpu;
mod sos_system;
mod sos_tray;

// ── 捕获 ──
// `sos_gdi` / `sos_uac` 不依赖 libvpx：前者是纯 GDI 采集，后者只查桌面状态。
// `sos_capture`（SYSTEM 子进程的采集线程）要用它们，而那条路径与编码器无关，
// 所以不能跟着 `sos_vpx` 一起被关掉。
mod sos_capture;
#[cfg(windows)]
mod sos_display;
#[cfg(sos_vpx)]
mod sos_dxgi;
mod sos_gdi;
mod sos_uac;

// ── 视频 ──
#[cfg(sos_vpx)]
mod sos_qos;
#[cfg(sos_vpx)]
mod sos_video;
#[cfg(sos_vpx)]
mod sos_vpx;

// ── 网络 ──
mod sos_connection;
mod sos_file_transfer;
mod sos_pipe;
mod sos_relay;
mod sos_rendezvous;
mod sos_shmem;

// ── 输入 ──
mod sos_input;

// 自建视频链路（`sos_video` / `sos_vpx` / `sos_qos`）依赖由 vcpkg 的 libvpx
// 生成的绑定，缺少时 `build.rs` 不会置 `sos_vpx`。此时 `sos_connection`
// 与 `main` 里的视频调用点都无法编译 —— 与其让编译器抛一堆「找不到模块」，
// 不如在这里直接说明原因。
#[cfg(not(sos_vpx))]
compile_error!(
    "libvpx bindings are required: install vcpkg and build vpx (x64-windows-static), \
     then ensure VCPKG_ROOT is set before `cargo build`"
);

use clap::Parser;
use hbb_common::ResultType;
use std::io::Write;

// ── SYSTEM 子进程看门狗参数 ──
//
// 命名沿用 `sos_video.rs` 约定的 `*_UPTIME` / `*_BACKOFF` / `MAX_*`：
// 单位在名字里，不带单位的数值不写常量。

/// 子进程「活着多久才算正常启动」。低于它视为启动期失败。
const SUBPROC_MIN_UPTIME: std::time::Duration = std::time::Duration::from_secs(10);

/// 启动期失败后首次重启前的等待，之后逐次翻倍。
const SUBPROC_RESTART_BACKOFF: std::time::Duration = std::time::Duration::from_secs(1);

/// 重启退避上限（也决定了最大翻倍次数：1s→30s 约 5 次）。
const SUBPROC_MAX_RESTART_DELAY: std::time::Duration = std::time::Duration::from_secs(30);

/// 退避指数的上限。`1 << 5 = 32`，配合 1s 基数恰好触到 30s 封顶，
/// 再大的移位只会溢出而无意义。
const SUBPROC_MAX_BACKOFF_SHIFT: u32 = 5;

/// 桌面切换钩子不可用时的轮询间隔。
///
/// `SosWaitForDesktopSwitch` 在钩子注册失败时会**立刻**返回 -1，
/// 调用方若直接在循环里重试就变成忙等 —— 实测这种状态下单个线程能吃掉
/// 整个核心（4 核机器上正好 25%），并且每轮还附带一次全进程快照。
const UAC_FALLBACK_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(1);

/// 安全桌面激活后等待 SYSTEM 子进程首帧的单次时长。
const SECURE_READY_WAIT_MS: u32 = 500;

/// watcher 线程等待桌面切换的**上限**。
///
/// 钩子（`EVENT_SYSTEM_DESKTOPSWITCH`）只作为「加速器」：事件到了立刻醒，
/// 没事件也必须每秒醒一次重新评估。
///
/// 现场日志证明只靠事件会失效：机器在 570s 左右锁屏，`sos_video` 在
/// 583s 已经判定安全桌面，而 watcher 一直静默到 606s（客户端断开时才收到
/// 事件）—— 25 秒内 SYSTEM 采集从未启动，控制端全程黑屏。
///
/// 会话被锁 / RDP 断开后，本会话不再收到 WinEvent，这是可复现的行为，
/// 不是偶发丢事件。所以这里的上限是**必须**的，不能退回 `INFINITE`。
const SECURE_POLL_INTERVAL_MS: u32 = 1000;

/// RustDesk SOS 精简受控端
#[derive(Parser)]
#[command(
    name = "rustdesk-sos",
    version,
    about = "RustDesk SOS 精简受控端 - 单文件远程协助客户端"
)]
struct Cli {
    /// 显示控制台窗口并输出调试日志
    #[arg(short = 'd', long, default_value_t = false)]
    debug: bool,

    /// 管理服务器地址（信令/ID 服务器），如 hbbs.example.com
    #[arg(short = 'm', long, default_value = "")]
    manager: String,

    /// 中继服务器地址（可选，未指定时由管理服务器自动下发）。
    /// 可用逗号分隔多个地址，按顺序逐个尝试。
    #[arg(short = 'r', long, default_value = "")]
    relay: String,

    /// 管理服务器公钥（Base64，可选，使用自定义服务器时必填）
    #[arg(short = 'k', long, default_value = "")]
    key: String,

    /// 固定密码（传入后永久使用，不断开刷新；不传入则每次连接后自动更换）
    #[arg(short = 'p', long, default_value = "")]
    password: String,

    /// 启动模式开关（内部使用：主进程以 SYSTEM 身份拉起自身时传入）
    #[arg(short = 's', long, default_value_t = false)]
    service: bool,

    /// 命名内核对象使用会话内命名空间（内部使用）
    ///
    /// 主进程启用 `SeCreateGlobalPrivilege` 失败时会退回会话内命名，
    /// 并通过本开关告知子进程，避免两侧名字不一致导致连不上。
    #[arg(long, default_value_t = false)]
    no_global: bool,
}

#[tokio::main]
async fn main() -> ResultType<()> {
    // 捕获所有 panic，输出到 stderr 防止静默崩溃
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let loc = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        let payload = info.payload();
        let msg = if let Some(s) = payload.downcast_ref::<&str>() {
            *s
        } else if let Some(s) = payload.downcast_ref::<String>() {
            s.as_str()
        } else {
            "unknown payload"
        };
        eprintln!("!!! PANIC at {}: {}", loc, msg);
        default_hook(info);
    }));

    let cli = Cli::parse();

    // ── 控制台管理 ──
    // Release 构建使用 GUI 子系统（无控制台），仅 --debug 时显示控制台。
    // AttachConsole 挂接父控制台后标准句柄仍是无效的，需要手动通过
    // CreateFile("CONOUT$") 打开控制台并用 SetStdHandle 重定向。
    #[cfg(windows)]
    if cli.debug && !cli.service {
        use windows::core::PCWSTR;
        use windows::Win32::Storage::FileSystem::*;
        use windows::Win32::System::Console::*;
        unsafe {
            let has_parent = AttachConsole(ATTACH_PARENT_PROCESS).is_ok();
            if !has_parent {
                let _ = AllocConsole();
            }
            // 打开控制台的输出设备，重定向 stdout/stderr
            // CreateFileW 的 dwDesiredAccess 是 u32，不能用 FILE_GENERIC_WRITE（类型不匹配）
            const GENERIC_WRITE: u32 = 0x40000000;
            const GENERIC_READ: u32 = 0x80000000;
            let con_out: Vec<u16> = "CONOUT$\0".encode_utf16().collect();
            let handle = CreateFileW(
                PCWSTR(con_out.as_ptr()),
                GENERIC_WRITE | GENERIC_READ,
                FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES(0),
                None,
            );
            if let Ok(h) = handle {
                let _ = SetStdHandle(STD_OUTPUT_HANDLE, h);
                let _ = SetStdHandle(STD_ERROR_HANDLE, h);
            }
        }
        // 确保 Rust stdio 重定向到新句柄
        let _ = std::io::stdout().flush();
        let _ = std::io::stderr().flush();
    }

    let log_level = if cli.debug { "info" } else { "warn" };
    // 初始化日志（所有模式都需要）
    if cli.service {
        if cli.debug {
            // SYSTEM 子进程：追加写入 rustdesk-sos-system.log
            sos_bootstrap::init_system_logger(log_level);
        } else {
            sos_bootstrap::init_logger(log_level);
        }
    } else if cli.debug {
        // 主进程 --debug：同时写控制台与 exe 同目录的 rustdesk-sos.log
        sos_bootstrap::init_debug_logger(log_level);
    } else {
        sos_bootstrap::init_logger(log_level);
    }

    // ── 模式分发：服务模式 ──
    if cli.service {
        // 与主进程保持一致：主进程启用全局命名空间失败时会传 --no-global
        sos_shmem::set_global_namespace(!cli.no_global);
        return sos_system::run_service();
    }

    // ── 主进程模式 ──

    // 0. 硬性检查：必须以管理员权限运行
    if !sos_bootstrap::is_admin() {
        sos_bootstrap::show_admin_required_and_exit();
    }

    // 0b. 单实例保护。必须放在创建任何**全局命名对象**之前
    //     （管道 \\.\pipe\sos_input、共享内存 RustDeskSOS_sos、捕获启停事件）。
    //     否则第二个实例会抢同一个管道，两个 SYSTEM 子进程互踢，
    //     看门狗把失败的重启刷成高频循环 —— 详见 acquire_single_instance 的文档。
    #[cfg(windows)]
    if !sos_bootstrap::acquire_single_instance() {
        sos_bootstrap::show_already_running_and_exit(cli.debug);
    }

    log::info!(
        "{} v{} starting...",
        sos_constants::APP_NAME,
        sos_constants::VERSION
    );

    // 1a. 输出主进程令牌诊断
    #[cfg(windows)]
    sos_system::log_token_info("MAIN_INIT");

    // 2. 读取/初始化注册表配置
    //    ID 优先级: 注册表 > MAC 地址自动生成
    //    管理服务器优先级: --manager > 注册表 > 默认
    let config = sos_config::SosConfig::from_cli_and_registry(
        &cli.manager,
        &cli.relay,
        &cli.key,
        &cli.password,
    )?;
    log::info!("Device ID: {}", config.id);
    log::info!("Rendezvous server: {}", config.rendezvous_server);

    // CPU 使用率采样。
    //
    // 这里**不用**上游的 `start_cpu_performance_monitor()`：它走 PDH 的
    // `\Processor(_total)\% Processor Time`，需要 `PdhCollectQueryDataEx`
    // 定时采集，会进 pdh.dll → perfos.dll 的查询链路。
    //
    // （早期版本在这里写了「每次查询 41ms / 计数器注册损坏」，那是**算错的**：
    //  两阶段 A/B 的实际时长不同（16.3s vs 15.0s），却直接拿原始 ms 差除以
    //  采集次数。做了基准差分后实测为 0.0ms，两台机器一致。
    //  PDH 本身没有问题，不要为此执行 `lodctr /R`。）
    //
    // 换掉它的真实理由只剩两个，与那 41ms 无关：
    //   1. `GetSystemTimes` 是单次内核调用，不碰 pdh.dll 的查询对象，
    //      也没有「查询句柄泄漏就永久失效」这类失效模式；
    //   2. 上游实现把结果放在全局队列里，还需要额外的补种线程。
    //
    // 采样周期仍为 2 秒，并继续把结果同步进上游缓存，
    // 保证任何依赖 `cpu_uage_one_minute()` 的代码行为不变。
    #[cfg(windows)]
    crate::sos_cpu::start_cpu_monitor();

    // 编码资源配置诊断。
    //
    // 注意：不能再用 `scrap::codec_thread_num()` 作为参考值。它的算法是
    // `min(cpu/2, available_memory/2)`，而 `available_memory` 是整数 GB，
    // 4GB 虚拟机上报 1G 时 `1/2 = 0`，于是恒定返回 1 线程。
    // 我们的自建编码器不用这个启发式，实测线程数对内存几乎无影响，
    // 真正的开销是分辨率决定的基础工作集（见 sos_vpx 的实测表）。
    #[cfg(sos_vpx)]
    {
        let threads = crate::sos_vpx::SosVpxConfig::recommended_threads();
        log::info!(
            "encoder: threads={} cpus={} total_mem={}MB avail_mem={}MB (scrap heuristic would say {})",
            threads,
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(0),
            crate::sos_vpx::total_phys_mb(),
            crate::sos_vpx::avail_phys_mb(),
            scrap::codec::codec_thread_num(64),
        );
    }

    // 密码策略：
    //   --password 传入 → 永久固定，不保存注册表，不断开刷新
    //   未传入       → 自动生成临时密码，每次连接断开后刷新
    let cli_password_provided = !config.password.is_empty();
    let temp_pwd = if cli_password_provided {
        config.password.clone()
    } else {
        sos_config::generate_temp_password()
    };
    sos_config::set_current_password(temp_pwd.clone());
    if cli_password_provided {
        log::info!("Using fixed CLI password (not regenerated on disconnect)");
    } else {
        log::info!(
            "Temporary password (will refresh each connection): {}",
            temp_pwd
        );
    }

    // 4. 创建 SOS 自有 SHMEM（固定名 "sos"，SYSTEM 子进程常驻写帧）
    let _shmem = match sos_shmem::create_shmem() {
        Ok(m) => m,
        Err(e) => {
            log::error!("Failed to create SOS SHMEM: {}", e);
            return Err(e);
        }
    };

    // 创建输入管道服务器（Main → System 输入事件）
    let (pipe_tx, _) =
        sos_pipe::create_pipe_server(sos_constants::INPUT_PIPE, sos_pipe::PIPE_ACCESS_OUTBOUND);
    crate::sos_pipe::set_down_tx(pipe_tx);

    // 创建捕获启停命名事件（替代原 CONTROL_PIPE，统一为内核事件模型）
    let (capture_start_event, capture_stop_event) = unsafe {
        extern "system" {
            fn CreateEventW(sa: *const u8, manual: i32, initial: i32, name: *const u16) -> isize;
        }
        let to_wide = |s: &str| {
            s.encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<u16>>()
        };
        let start = CreateEventW(
            std::ptr::null(),
            0,
            0,
            to_wide(&sos_shmem::scoped_name(sos_constants::CAPTURE_START_EVENT)).as_ptr(),
        );
        let stop = CreateEventW(
            std::ptr::null(),
            0,
            0,
            to_wide(&sos_shmem::scoped_name(sos_constants::CAPTURE_STOP_EVENT)).as_ptr(),
        );
        (start, stop)
    };

    // 5. 以 SYSTEM 身份启动子进程（常驻），获取进程句柄用于看门狗
    #[cfg(windows)]
    let global_ns = sos_shmem::uses_global_namespace();
    #[cfg(windows)]
    let sub_proc_handle = sos_system::launch_system_sub_process(cli.debug, global_ns);

    // 5a. SYSTEM 子进程存活看门狗：基于进程句柄的 WaitForSingleObject 监控
    //     零轮询、零心跳 — 纯事件驱动。子进程死亡时自动重建管道 + 重启子进程。
    #[cfg(windows)]
    if sub_proc_handle != 0 {
        let debug = cli.debug;
        std::thread::spawn(move || {
            // FFI 声明（看门狗专用，避免引入额外 crate 依赖）
            extern "system" {
                fn WaitForSingleObject(h: isize, ms: u32) -> u32;
                fn CloseHandle(h: isize) -> i32;
            }
            const INFINITE: u32 = 0xFFFFFFFF;
            const WAIT_OBJECT_0: u32 = 0;

            let mut current_handle = sub_proc_handle;
            let mut started = std::time::Instant::now();
            let mut rapid_restarts = 0u32;
            loop {
                log::info!(
                    "Watchdog: waiting on sub-process handle 0x{:x}",
                    current_handle
                );
                let ret = unsafe { WaitForSingleObject(current_handle, INFINITE) };
                if ret != WAIT_OBJECT_0 {
                    log::error!("Watchdog: WaitForSingleObject failed, ret={}", ret);
                    break;
                }
                unsafe {
                    CloseHandle(current_handle);
                }
                let lived = started.elapsed();
                log::warn!("SYSTEM sub-process exited after {:?}, restarting...", lived);

                // 子进程活着的时间太短，说明它在**启动阶段**就失败了
                // （拿不到 winlogon 令牌、管道被另一个实例占住、缺少权限……）。
                // 此时立刻重启只会把同一个失败刷成每秒一轮的循环，而每轮
                // 都要 OpenProcess(winlogon) + CreateProcessWithTokenW，
                // 足以让安全软件的进程/令牌防护持续告警。指数退避把频率压下去，
                // 同时也让日志里能看清真正的错误而不再被重启刷屏。
                if lived < SUBPROC_MIN_UPTIME {
                    rapid_restarts = (rapid_restarts + 1).min(SUBPROC_MAX_BACKOFF_SHIFT);
                } else {
                    rapid_restarts = 0;
                }
                if rapid_restarts > 0 {
                    let factor = 1u32 << (rapid_restarts - 1);
                    let delay = (SUBPROC_RESTART_BACKOFF * factor).min(SUBPROC_MAX_RESTART_DELAY);
                    log::warn!(
                        "Watchdog: restart streak={}, backing off {:?} before relaunch",
                        rapid_restarts,
                        delay
                    );
                    std::thread::sleep(delay);
                }

                // 重建输入管道
                let (pt, _) = sos_pipe::create_pipe_server(
                    sos_constants::INPUT_PIPE,
                    sos_pipe::PIPE_ACCESS_OUTBOUND,
                );
                sos_pipe::set_down_tx(pt);

                // 启动新的 SYSTEM 子进程
                let new_handle = sos_system::launch_system_sub_process(
                    debug,
                    crate::sos_shmem::uses_global_namespace(),
                );
                if new_handle == 0 {
                    log::error!("Watchdog: failed to restart sub-process, exiting");
                    break;
                }
                current_handle = new_handle;
                started = std::time::Instant::now();
            }
            log::warn!("Watchdog: exited");
        });
    }

    // 6. 启动 SOS 自建视频链路（替代上游 video_service）
    //    采集 → I420 转换 → 自建 libvpx 编码 → 通道发送。
    //    绕开 scrap 的 codec_thread_num 线程钳制与每帧 flush。
    sos_video::start();
    log::info!("SOS video pipeline started ({})", sos_video::describe());

    // 6a. UAC 监控：SosWaitForDesktopSwitch 内部封装 SetWinEventHook + 消息泵
    //     调用线程阻塞等待桌面切换，零轮询、零 sleep。
    #[cfg(windows)]
    {
        let uac_w = {
            scrap::Display::all()
                .unwrap_or_default()
                .first()
                .map(|d| d.width() as usize)
                .unwrap_or(1280)
        };
        let uac_h = {
            scrap::Display::all()
                .unwrap_or_default()
                .first()
                .map(|d| d.height() as usize)
                .unwrap_or(720)
        };

        // 创建命名自动重置事件：捕获线程写完首帧后 SetEvent
        let event_name_wide: Vec<u16> = sos_shmem::scoped_name(sos_constants::CAPTURE_READY_EVENT)
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let ready_event = unsafe {
            extern "system" {
                fn CreateEventW(
                    sa: *const u8,
                    manual: i32,
                    initial: i32,
                    name: *const u16,
                ) -> isize;
            }
            CreateEventW(std::ptr::null(), 0, 0, event_name_wide.as_ptr())
        };

        std::thread::spawn(move || {
            extern "system" {
                fn WaitForSingleObject(h: isize, ms: u32) -> u32;
                fn SetEvent(h: isize) -> i32;
            }
            const WAIT_OBJECT_0: u32 = 0;

            // 判据是 `sos_uac::secure_desktop()`：
            // 1. 主判据 `desktop_changed()`（本线程桌面名 != 输入桌面名），
            //    与上游一致，锁屏与 UAC 一并覆盖；
            // 2. 兜底 `consent_running()`，也就是自建进程快照。上游这里写的是
            //    `cmd /C tasklist | findstr consent.exe` —— 每次要创建
            //    cmd.exe + tasklist.exe + findstr.exe 并等它们退出，实测单次
            //    205ms；快照版 1-3ms。
            //
            // 为什么这件事不只是"慢"：每次进程创建/销毁都会产生一次
            // 进程实例事件，系统上任何注册了 `Win32_Process` 变更通知的
            // 组件（杀毒、EDR、SCCM/Intune、OEM 管理软件）都会被唤醒，
            // 这些回调在 WmiPrvSE.exe 里执行。桌面切换事件在锁屏/解锁、
            // UAC 弹出、RDP 连接与断开时都会触发，反复拉进程会让
            // WmiPrvSE 的 CPU 占用明显抬头。
            //
            // 注意 `secure_desktop()` 检查的是**调用线程**的桌面，所以这个
            // 线程永远不能调用 `SetThreadDesktop`。
            let mut was_uac = crate::sos_uac::secure_desktop();
            // 是否已经把视频切到「SYSTEM 子进程写 SHMEM」这条通路。
            // 必须等首帧真的到位之后才切 —— 否则视频会取到一张永不更新的
            // 共享内存帧，画面直接冻结（比停在旧画面上更糟）。
            let mut secure_ready = false;
            // 钩子是否处于可用状态。诊断用的坑，但也是节流闸门：
            // 不可用时只打一条日志，之后退化成定时轮询。
            let mut hook_ok = true;
            // 「等待首帧」日志只打一次，避免锁屏长时间期间刷屏
            let mut secure_wait_logged = false;

            // 启动时就已处于安全桌面（开机自启 + 机器锁着）：
            // 这个状态下不会再发生「桌面切换」，钩子永远等不到事件，
            // 所以必须在这里主动请求一次。
            if was_uac {
                log::info!(
                    "[SECURE] 启动时已处于安全桌面 → SetEvent(CAPTURE_START) ({})",
                    crate::sos_uac::desktop_state()
                );
                unsafe { SetEvent(capture_start_event) };
            }

            loop {
                // **永远**用有限超时，不能再用 INFINITE。
                //
                // 钩子只是「加速器」，不能当唯一信号源。只靠事件会漏掉两种
                // 真实情况，而且都发生在最需要采集的时候：
                //
                // 1. 进程启动时就已经是安全桌面 —— 切换事件早就过去了；
                // 2. **会话被锁 / RDP 断开后，本会话不再收到任何 WinEvent**。
                //
                // 现场日志就是第 2 种：`sos_video` 在 583s 已经判定
                // `secure desktop active = true`，而这里的钩子一直静默，
                // 直到 606s 客户端断开后才第一次触发 —— 25 秒内 SHMEM 从没
                // 被注册，控制端全程黑屏（见下面 "SOS SHMEM not registered" 循环）。
                //
                // 代价：每秒一次 `desktop_changed()`（几个句柄操作）+ 最坏
                // 每秒一次进程快照（已限流到 1s）。远小于一次白跑的显示器枚举。
                let sw = unsafe { sos_system::SosWaitForDesktopSwitch(SECURE_POLL_INTERVAL_MS) };
                if sw < 0 {
                    if hook_ok {
                        hook_ok = false;
                        log::error!(
                            "[UAC] 桌面切换钩子不可用，退化为 {:?} 定时轮询（安全桌面仍可检测）",
                            UAC_FALLBACK_POLL_INTERVAL
                        );
                    }
                    std::thread::sleep(UAC_FALLBACK_POLL_INTERVAL);
                } else if !hook_ok {
                    hook_ok = true;
                    log::info!("[UAC] 桌面切换钩子已恢复");
                }

                let uac = crate::sos_uac::secure_desktop();

                if uac && !was_uac {
                    log::info!(
                        "[SECURE] 安全桌面激活（锁屏 / UAC）→ SetEvent(CAPTURE_START) ({})",
                        crate::sos_uac::desktop_state()
                    );
                    was_uac = true;
                    // 通知 SYSTEM 子进程开始往 SHMEM 写帧
                    unsafe { SetEvent(capture_start_event) };
                } else if !uac && was_uac {
                    log::info!(
                        "[SECURE] 安全桌面结束 → SetEvent(CAPTURE_STOP) ({})",
                        crate::sos_uac::desktop_state()
                    );
                    was_uac = false;
                    unsafe { SetEvent(capture_stop_event) };
                    if secure_ready {
                        secure_ready = false;
                        #[cfg(sos_vpx)]
                        sos_video::set_uac_active(false);
                    }
                }

                // 安全桌面期间：等 SYSTEM 子进程的首帧，到了再把视频切过去。
                // 没到就下一轮继续等（子进程那边会重试，不会退出）。
                if uac && !secure_ready {
                    // 重复 SetEvent 是幂等的（子进程侧只是把 active 置 true），
                    // 但可以覆盖「子进程刚被看门狗重启，错过了最初的 START 信号」
                    // 这种情况 —— 否则新子进程会一直暂停在锁屏界面。
                    unsafe { SetEvent(capture_start_event) };
                    let ret = unsafe { WaitForSingleObject(ready_event, SECURE_READY_WAIT_MS) };
                    if ret != WAIT_OBJECT_0 {
                        // 等不到首帧是「子进程没起来 / 采不到 / 在别的 session」
                        // 的直接证据，必须打到 INFO 并带上桌面状态，
                        // 否则现场只能看到控制端黑屏而日志一片安静。
                        if !secure_wait_logged {
                            secure_wait_logged = true;
                            log::warn!(
                                "[SECURE] 已请求 SYSTEM 采集但首帧未到，继续等待（{}）",
                                crate::sos_uac::desktop_state()
                            );
                        }
                        continue;
                    }
                    secure_wait_logged = false;
                    match sos_shmem::SosShmem::open_existing(sos_shmem::SHMEM_NAME) {
                        Ok(shmem) => {
                            sos_shmem::register_shmem(shmem, uac_w, uac_h);
                            #[cfg(sos_vpx)]
                            sos_video::set_uac_active(true);
                            secure_ready = true;
                            log::info!("[SECURE] SYSTEM 子进程首帧就绪 → 切换到共享内存采集");
                        }
                        Err(e) => {
                            log::warn!("[SECURE] SHMEM 打开失败({}), 保持本地采集", e);
                        }
                    }
                } else if !uac {
                    secure_wait_logged = false;
                }
            }
        });
    }

    // 5b. 启动上游 clipboard_service（剪贴板同步，含文件剪贴板）
    log::info!("Starting upstream clipboard service...");
    let clip_svc = librustdesk::server::clipboard_service::new("clipboard".into());
    sos_config::set_clipboard_service(clip_svc);
    log::info!("Clipboard service started (upstream)");

    // 5c. 初始化 cliprdr 上下文（OLE 文件剪贴板引擎，用于远程文件粘贴）
    #[cfg(windows)]
    {
        log::info!("Initializing cliprdr context for file clipboard...");
        clipboard::ContextSend::enable(true);
        if clipboard::ContextSend::is_enabled() {
            log::info!("Cliprdr context initialized successfully");
        } else {
            log::warn!("Cliprdr context initialization failed - file clipboard may not work");
        }
    }

    // 6. 创建托盘（tokio unbounded channel，支持异步 recv，无需轮询）
    let (tray_tx, mut tray_rx) = tokio::sync::mpsc::unbounded_channel::<sos_tray::TrayCommand>();
    let tray_tx_for_rendezvous = tray_tx.clone();
    std::thread::spawn(move || {
        if let Err(e) = sos_tray::run(tray_tx) {
            log::error!("Tray thread error: {}", e);
        }
    });

    // 8. 创建密码刷新通道（仅未传入 --password 时才需要断开后刷新）
    let (pwd_refresh_tx, mut pwd_refresh_rx) = tokio::sync::mpsc::unbounded_channel::<()>();

    // 8. 启动信令注册任务（后台持续运行）
    let rendezvous_config = config.clone();
    let rendezvous_handle = tokio::spawn(async move {
        if let Err(e) =
            sos_rendezvous::run(rendezvous_config, tray_tx_for_rendezvous, pwd_refresh_tx).await
        {
            log::error!("Rendezvous service error: {}", e);
        }
    });

    log::info!(
        "SOS client is ready, ID: {}, waiting for incoming connections...",
        config.id
    );

    // 9. 主事件循环：接受托盘命令 + 密码刷新通知 + 保持运行
    loop {
        tokio::select! {
            // 密码刷新通知：连接断开后重新生成临时密码（仅当未通过 --password 固定时生效）
            Some(()) = pwd_refresh_rx.recv() => {
                if cli_password_provided {
                    log::trace!("CLI password is fixed, ignoring password refresh trigger");
                } else {
                    let new_pwd = sos_config::generate_temp_password();
                    sos_config::set_current_password(new_pwd.clone());
                    log::info!("Connection closed, regenerated temporary password: {}", new_pwd);
                }
            }

            // 托盘命令（事件驱动，零轮询）
            cmd = tray_rx.recv() => {
                match cmd {
                    Some(sos_tray::TrayCommand::Exit) | None => {
                        log::info!("Exit requested via tray");
                        break;
                    }
                    Some(sos_tray::TrayCommand::ShowInfo) => {
                        log::debug!("Show info requested via tray");
                        sos_tray::show_info();
                    }
                }
            }
        }
    }

    // 8. 清理：通知信令任务退出
    rendezvous_handle.abort();

    log::info!("SOS client exiting.");
    Ok(())
}
