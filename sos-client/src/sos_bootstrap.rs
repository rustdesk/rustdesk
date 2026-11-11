//! SOS 启动引导模块
//!
//! 处理管理员权限检查、服务启动、信令注册初始化。

/// 检查当前进程是否以管理员权限运行
#[cfg(windows)]
pub fn is_admin() -> bool {
    use windows::Win32::Security::{
        GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    let mut token = windows::Win32::Foundation::HANDLE::default();
    unsafe {
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
    }

    let mut elevation = TOKEN_ELEVATION::default();
    let mut return_length = 0u32;
    unsafe {
        if GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut return_length,
        )
        .is_err()
        {
            return false;
        }
    }
    elevation.TokenIsElevated != 0
}

/// 非管理员权限时弹窗报错并退出
pub fn show_admin_required_and_exit() -> ! {
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
        let title: Vec<u16> = "权限不足\0".encode_utf16().collect();
        let msg: Vec<u16> =
            "RustDesk SOS 必须以管理员权限运行。\n\n请右键点击程序 → \"以管理员身份运行\"，\n或在命令行中使用管理员权限启动。\0"
                .encode_utf16()
                .collect();
        unsafe {
            MessageBoxW(
                None,
                windows::core::PCWSTR(msg.as_ptr()),
                windows::core::PCWSTR(title.as_ptr()),
                MB_ICONERROR | MB_OK,
            );
        }
    }
    std::process::exit(1);
}

// ── 单实例保护 ──

/// 全局互斥体句柄。必须活到进程结束：一旦关闭，互斥体就随之消失，
/// 之后启动的实例会误判自己是唯一实例。进程退出时由内核回收。
#[cfg(windows)]
static INSTANCE_MUTEX: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

/// 抢占「本机唯一主进程」互斥体。返回 `true` 表示可以继续启动。
///
/// 为什么必须有这道闸：主进程会创建一批**全局命名对象**
/// （`\\.\pipe\sos_input`、共享内存 `RustDeskSOS_sos`、捕获启停事件），
/// 并为每个实例启动一个 SYSTEM 子进程。没有这道闸时第二个实例照常启动，
/// 两个 SYSTEM 子进程抢同一个管道，抢输的那个在 `connect_pipe_client`
/// 上失败后毫秒级退出，主进程看门狗立刻把它重启 —— 形成
/// `OpenProcess(winlogon, PROCESS_ALL_ACCESS) → CreateProcessWithTokenW`
/// 的高频循环，每一轮都会唤醒安全软件的进程/令牌防护。
///
/// 管道名在 `\\.\pipe\` 全局命名空间，所以互斥体也用 `Global\`：
/// 不同登录会话各自跑一个实例同样会撞管道。
#[cfg(windows)]
pub fn acquire_single_instance() -> bool {
    extern "system" {
        fn CreateMutexW(sa: *const u8, initial_owner: i32, name: *const u16) -> isize;
        fn CloseHandle(h: isize) -> i32;
        fn GetLastError() -> u32;
        fn SetLastError(code: u32);
    }
    const ERROR_ALREADY_EXISTS: u32 = 183;

    let name: Vec<u16> = "Global\\RustDeskSOS_main\0".encode_utf16().collect();
    unsafe {
        // CreateMutexW 成功时不会清零 last error，必须自己置 0，
        // 否则可能读到上一次调用残留的 ERROR_ALREADY_EXISTS 而误判。
        SetLastError(0);
        let h = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
        if h == 0 {
            // 创建失败（极端情况：权限不足）不应该阻止程序启动 ——
            // 单实例是保护措施，不是功能前提。
            log::warn!("[INSTANCE] CreateMutexW failed: {}", GetLastError());
            return true;
        }
        if GetLastError() == ERROR_ALREADY_EXISTS {
            CloseHandle(h);
            return false;
        }
        INSTANCE_MUTEX.store(h, std::sync::atomic::Ordering::SeqCst);
        true
    }
}

/// 已有实例在运行时提示并退出。
///
/// 退出码用 0：重复启动不是错误，计划任务、服务管理器之类不应据此报失败。
pub fn show_already_running_and_exit(debug: bool) -> ! {
    log::warn!("Another rustdesk-sos instance owns the single-instance mutex, exiting");
    #[cfg(windows)]
    if !debug {
        use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONINFORMATION, MB_OK};
        let title: Vec<u16> = "RustDesk SOS\0".encode_utf16().collect();
        let msg: Vec<u16> =
            "RustDesk SOS 已在运行，无需重复启动。\n\n（如需重启，请先在托盘退出原实例。）\0"
                .encode_utf16()
                .collect();
        unsafe {
            MessageBoxW(
                None,
                windows::core::PCWSTR(msg.as_ptr()),
                windows::core::PCWSTR(title.as_ptr()),
                MB_ICONINFORMATION | MB_OK,
            );
        }
    }
    std::process::exit(0);
}

/// 初始化日志系统（控制台输出，用于主进程 --debug 模式）
pub fn init_logger(level: &str) {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(level))
        .format_timestamp_millis()
        .try_init();
}

/// SYSTEM 子进程日志文件名（追加写入，保留历史）
#[cfg(windows)]
const SYSTEM_LOG_FILE: &str = "rustdesk-sos-system.log";

/// 主进程 `--debug` 日志文件名（每次运行覆盖，便于定位当次问题）
#[cfg(windows)]
const DEBUG_LOG_FILE: &str = "rustdesk-sos.log";

#[cfg(windows)]
fn parse_level(level: &str) -> log::LevelFilter {
    match level {
        "trace" => log::LevelFilter::Trace,
        "debug" => log::LevelFilter::Debug,
        "info" => log::LevelFilter::Info,
        "warn" => log::LevelFilter::Warn,
        "error" => log::LevelFilter::Error,
        _ => log::LevelFilter::Info,
    }
}

/// 初始化 SYSTEM 子进程日志（写入 exe 同目录的 rustdesk-sos-system.log）
#[cfg(windows)]
pub fn init_system_logger(level: &str) {
    init_file_logger(level, SYSTEM_LOG_FILE, true);
}

/// 初始化主进程 `--debug` 日志。
///
/// 与控制台并行：日志同时写入 exe 同目录的 `rustdesk-sos.log` 与 stderr，
/// 这样即使控制台被关闭或输出被截断，也能事后查看完整记录。
/// 每次启动覆盖旧文件，避免多次调试的记录混在一起。
pub fn init_debug_logger(level: &str) {
    #[cfg(windows)]
    init_file_logger(level, DEBUG_LOG_FILE, false);
    #[cfg(not(windows))]
    init_logger(level);
}

/// 通用文件日志：写入 exe 同目录，同时镜像到 stderr。
///
/// `append = true` 用于长期运行的服务日志，`false` 用于单次调试的运行日志。
#[cfg(windows)]
fn init_file_logger(level: &str, file_name: &str, append: bool) {
    let level_filter = parse_level(level);

    let log_path = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join(file_name)))
        .unwrap_or_else(|| std::path::PathBuf::from(file_name));

    let mut opts = std::fs::OpenOptions::new();
    opts.create(true);
    if append {
        opts.append(true);
    } else {
        opts.write(true).truncate(true);
    }

    let file = match opts.open(&log_path) {
        Ok(f) => f,
        Err(e) => {
            // 无法写文件时退化到 env_logger（stderr）
            init_logger(level);
            log::warn!("Cannot open log file {}: {}", log_path.display(), e);
            return;
        }
    };

    // 日志绝不能在业务线程上做同步 I/O。
    //
    // 旧实现是「全局 Mutex<File> + 每行 flush + 同步写 stderr」，
    // 在可用内存只有 1G、控制台窗口被选中/滚动的机器上，
    // 一次 log::info! 能卡住数秒：rendezvous 循环从 5s 拖到 11.8s，
    // [VIDEO:PERF] 从 5s 拖到 12.5s，整个进程看起来像死机。
    //
    // 现在：格式化后 try_send 给后台线程，队列满就丢弃，永不阻塞调用方。
    let (tx, rx) = std::sync::mpsc::sync_channel::<LogLine>(LOG_QUEUE_CAPACITY);
    let logger = FileLogger {
        level: level_filter,
        tx,
        dropped: std::sync::atomic::AtomicU64::new(0),
    };

    let spawned = std::thread::Builder::new()
        .name("sos-log".into())
        .spawn(move || writer_loop(file, rx));

    if spawned.is_err() {
        // 起不了后台线程就退化到 env_logger，至少还有控制台输出
        init_logger(level);
        log::warn!("Cannot spawn log writer thread, falling back to stderr");
        return;
    }

    log::set_boxed_logger(Box::new(logger)).ok();
    log::set_max_level(level_filter);
    log::info!("Logger initialized: {}", log_path.display());
}

/// 日志队列容量。足够吸收突发，又不会在内存紧张时占用过多。
#[cfg(windows)]
const LOG_QUEUE_CAPACITY: usize = 8192;

/// 传给后台写入线程的一条记录
#[cfg(windows)]
struct LogLine {
    text: String,
    /// warn/error 立刻落盘，避免崩溃时丢掉关键信息
    urgent: bool,
}

#[cfg(windows)]
struct FileLogger {
    level: log::LevelFilter,
    tx: std::sync::mpsc::SyncSender<LogLine>,
    /// 队列满而丢弃的条数，下次成功入队时补一条提示
    dropped: std::sync::atomic::AtomicU64,
}

#[cfg(windows)]
impl log::Log for FileLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let mut text = format!(
            "[{}.{:03}] [{:5}] {}\n",
            ts.as_secs(),
            ts.subsec_millis(),
            record.level(),
            record.args(),
        );
        let dropped = self.dropped.swap(0, std::sync::atomic::Ordering::Relaxed);
        if dropped > 0 {
            text.insert_str(
                0,
                &format!("[WARN ] dropped {} log lines (queue full)\n", dropped),
            );
        }
        let urgent = record.level() <= log::Level::Warn;
        // try_send：队列满时丢弃而不是等待。宁可丢日志，也不能拖慢业务线程。
        if self.tx.try_send(LogLine { text, urgent }).is_err() {
            self.dropped
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }

    fn flush(&self) {
        // 后台线程会周期性 flush，这里不等待，避免反向阻塞调用方
    }
}

/// 后台写日志：批量写文件，周期性 flush，同时镜像到 stderr。
#[cfg(windows)]
fn writer_loop(file: std::fs::File, rx: std::sync::mpsc::Receiver<LogLine>) {
    use std::io::Write;
    use std::sync::mpsc::RecvTimeoutError;

    let mut out = std::io::BufWriter::with_capacity(64 * 1024, file);
    let mut stderr = std::io::stderr();

    loop {
        let first = match rx.recv_timeout(std::time::Duration::from_millis(200)) {
            Ok(line) => line,
            Err(RecvTimeoutError::Timeout) => {
                // 空闲时落盘，最多丢 200ms 的日志
                let _ = out.flush();
                let _ = stderr.flush();
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => {
                let _ = out.flush();
                let _ = stderr.flush();
                break;
            }
        };

        let mut urgent = first.urgent;
        write_line(&mut out, &mut stderr, &first.text);
        // 一次取走队列里已有的记录，减少系统调用
        while let Ok(line) = rx.try_recv() {
            urgent |= line.urgent;
            write_line(&mut out, &mut stderr, &line.text);
        }
        if urgent {
            let _ = out.flush();
            let _ = stderr.flush();
        }
    }
}

#[cfg(windows)]
fn write_line(out: &mut impl std::io::Write, stderr: &mut impl std::io::Write, text: &str) {
    let _ = out.write_all(text.as_bytes());
    // 控制台镜像失败不影响文件日志
    let _ = stderr.write_all(text.as_bytes());
}

// is_system_process, log_token_info, run_capture_shmem 已移入 sos_system 模块
