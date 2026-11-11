//! SYSTEM 会话 1 特权模块
//!
//! SYSTEM 子进程入口：管道通信、GDI 帧捕获写 SHMEM、输入事件处理、桌面切换。

use hbb_common::ResultType;

#[cfg(windows)]
extern "C" {
    /// 以 SYSTEM 身份启动进程，可优先选控制台会话。
    /// `prefer_console` 仅在跨会话 IPC（`Global\` 命名）可用时才应为真。
    /// `out_session` / `out_pid` / `out_console_session` 可选，回传诊断信息。
    /// 返回子进程句柄（调用者负责 CloseHandle），失败返回 0。
    pub fn SosRunAsSystemInSession1(
        cmd: *const u16,
        prefer_console: i32,
        out_session: *mut u32,
        out_pid: *mut u32,
        out_console_session: *mut u32,
    ) -> isize;

    /// 阻塞等待下一次桌面切换，或 dwTimeout 毫秒超时。
    /// 返回 1=桌面已切换，0=超时，-1=错误。
    /// 内部使用 SetWinEventHook(WINEVENT_INTOUTCONTEXT) + 消息泵。
    pub fn SosWaitForDesktopSwitch(timeout_ms: u32) -> i32;
}

// ── 令牌诊断 ──

/// 检查当前进程是否以 SYSTEM 身份运行
#[cfg(windows)]
pub fn is_system_process() -> bool {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Security::{
        GetTokenInformation, IsWellKnownSid, TokenUser, WinLocalSystemSid, TOKEN_QUERY, TOKEN_USER,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    let mut token = HANDLE::default();
    unsafe {
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut size = 0u32;
        GetTokenInformation(token, TokenUser, None, 0, &mut size).ok();
        let mut buf = vec![0u8; size as usize];
        if GetTokenInformation(
            token,
            TokenUser,
            Some(buf.as_mut_ptr() as *mut _),
            size,
            &mut size,
        )
        .is_err()
        {
            return false;
        }
        let token_user = &*(buf.as_ptr() as *const TOKEN_USER);
        IsWellKnownSid(token_user.User.Sid, WinLocalSystemSid).as_bool()
    }
}

/// 输出当前进程的令牌/权限诊断信息
#[cfg(windows)]
pub fn log_token_info(stage: &str) {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Security::{
        GetTokenInformation, IsWellKnownSid, TokenElevation, TokenIntegrityLevel, TokenStatistics,
        TokenUser, WinLocalSystemSid, TOKEN_ELEVATION, TOKEN_MANDATORY_LABEL, TOKEN_QUERY,
        TOKEN_STATISTICS, TOKEN_USER,
    };
    use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
    use windows::Win32::System::Threading::{
        GetCurrentProcess, GetCurrentProcessId, OpenProcessToken,
    };

    unsafe {
        let mut session_id = 0u32;
        let pid = GetCurrentProcessId();
        // 失败就保持 0：这只是诊断日志，不该影响启动流程
        let _ = ProcessIdToSessionId(pid, &mut session_id);

        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            log::warn!(
                "[TOKEN:{}] PID={} Session={} CANNOT_OPEN_TOKEN",
                stage,
                pid,
                session_id
            );
            return;
        }

        // User SID + SYSTEM check
        let mut size = 0u32;
        GetTokenInformation(token, TokenUser, None, 0, &mut size).ok();
        let mut buf = vec![0u8; size as usize];
        let user_info = if GetTokenInformation(
            token,
            TokenUser,
            Some(buf.as_mut_ptr() as *mut _),
            size,
            &mut size,
        )
        .is_ok()
        {
            let token_user = &*(buf.as_ptr() as *const TOKEN_USER);
            let is_sys = IsWellKnownSid(token_user.User.Sid, WinLocalSystemSid).as_bool();
            format!("is_system={}", is_sys)
        } else {
            "SID_UNKNOWN".to_string()
        };

        // Integrity level
        let mut il_size = 0u32;
        GetTokenInformation(token, TokenIntegrityLevel, None, 0, &mut il_size).ok();
        let il_str = if il_size > 0 {
            let mut il_buf = vec![0u8; il_size as usize];
            if GetTokenInformation(
                token,
                TokenIntegrityLevel,
                Some(il_buf.as_mut_ptr() as *mut _),
                il_size,
                &mut il_size,
            )
            .is_ok()
            {
                let label = &*(il_buf.as_ptr() as *const TOKEN_MANDATORY_LABEL);
                let sid = label.Label.Sid;
                let sub_auth = windows::Win32::Security::GetSidSubAuthority(sid, 0);
                match *sub_auth {
                    0x0000 => "Untrusted",
                    0x1000 => "Low",
                    0x2000 => "Medium",
                    0x3000 => "High",
                    0x4000 => "System",
                    _ => "Other",
                }
            } else {
                ""
            }
        } else {
            ""
        };
        let il_str = il_str.to_string();

        // Elevation
        let mut elev = TOKEN_ELEVATION::default();
        let mut elev_size = 0u32;
        let elev_str = if GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elev as *mut _ as *mut _),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut elev_size,
        )
        .is_ok()
        {
            if elev.TokenIsElevated != 0 {
                "elevated"
            } else {
                "not_elevated"
            }
        } else {
            "unknown"
        };

        // Token type (primary/impersonation)
        let mut stats = TOKEN_STATISTICS::default();
        let mut stats_size = 0u32;
        let type_str = if GetTokenInformation(
            token,
            TokenStatistics,
            Some(&mut stats as *mut _ as *mut _),
            std::mem::size_of::<TOKEN_STATISTICS>() as u32,
            &mut stats_size,
        )
        .is_ok()
        {
            match stats.TokenType.0 {
                1 => "Primary",
                2 => "Impersonation",
                _ => "Other",
            }
        } else {
            "unknown"
        };

        log::info!(
            "[TOKEN:{}] PID={} Session={} {} Integrity={} Elevation={} Type={}",
            stage,
            pid,
            session_id,
            user_info,
            il_str,
            elev_str,
            type_str,
        );
    }
}

/// 服务模式入口：由主进程以 SYSTEM 身份启动。
pub fn run_service() -> ResultType<()> {
    #[cfg(windows)]
    if !is_system_process() {
        log::info!("Non-SYSTEM process, exiting");
        return Ok(());
    }

    use crate::sos_constants::{CAPTURE_START_EVENT, CAPTURE_STOP_EVENT, INPUT_PIPE};
    use crate::sos_pipe::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    log_token_info("SUB_INIT");

    // 连接输入管道
    log::info!("Connecting to input pipe: {}", INPUT_PIPE);
    let input_pipe = match connect_pipe_client(INPUT_PIPE, 6) {
        Some(h) => h,
        None => {
            log::error!("Failed to connect input pipe");
            return Ok(());
        }
    };

    // 打开捕获启停命名事件（替代原 CONTROL_PIPE，统一为内核事件模型）
    let (start_event, stop_event) = unsafe {
        extern "system" {
            fn OpenEventW(access: u32, inherit: i32, name: *const u16) -> isize;
        }
        const SYNCHRONIZE: u32 = 0x00100000;
        let to_wide = |s: &str| {
            s.encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<u16>>()
        };
        let se = OpenEventW(
            SYNCHRONIZE,
            0,
            to_wide(&crate::sos_shmem::scoped_name(CAPTURE_START_EVENT)).as_ptr(),
        );
        let pe = OpenEventW(
            SYNCHRONIZE,
            0,
            to_wide(&crate::sos_shmem::scoped_name(CAPTURE_STOP_EVENT)).as_ptr(),
        );
        if se == 0 || pe == 0 {
            log::error!("Failed to open capture start/stop events");
            return Ok(());
        }
        (se, pe)
    };

    // 捕获启停标志：初始为 false（暂停）
    let capture_active = Arc::new(AtomicBool::new(false));
    // 捕获线程句柄：事件线程在收到 START 时 unpark 以实现零延迟唤醒
    let cap_thread_handle: Arc<std::sync::Mutex<Option<std::thread::Thread>>> =
        Arc::new(std::sync::Mutex::new(None));

    // 输入线程：读输入管道 → 分派输入事件
    let input_thread = std::thread::spawn(move || {
        loop {
            let data = match unsafe { read_message(input_pipe) } {
                Some(d) => d,
                None => break,
            };
            if data.len() < 5 {
                continue;
            }
            let msg_type = u32::from_ne_bytes([data[0], data[1], data[2], data[3]]);
            if !crate::sos_input::dispatch(msg_type, &data[4..]) {
                return;
            }
        }
        log::warn!("[PIPE] Input pipe disconnected");
        std::process::exit(0);
    });

    // 事件线程：WaitForMultipleObjects 等待启停事件 → 更新 AtomicBool + unpark 捕获线程
    let cap_active = capture_active.clone();
    let cap_handle = cap_thread_handle.clone();
    let event_thread = std::thread::spawn(move || {
        extern "system" {
            fn WaitForMultipleObjects(
                count: u32,
                handles: *const isize,
                wait_all: i32,
                ms: u32,
            ) -> u32;
        }
        const INFINITE: u32 = 0xFFFFFFFF;
        const WAIT_OBJECT_0: u32 = 0;

        let handles = [start_event, stop_event];
        loop {
            let ret = unsafe { WaitForMultipleObjects(2, handles.as_ptr(), 0, INFINITE) };
            if ret == WAIT_OBJECT_0 {
                // CAPTURE_START
                //
                // 主进程在安全桌面期间每秒重复 SetEvent（幂等，用于兜住
                // 「子进程刚被看门狗重启、错过了最初的信号」），所以这里
                // 只在状态**真的变化**时记日志，否则日志会被刷屏。
                if !cap_active.swap(true, Ordering::SeqCst) {
                    log::info!("[EVENT] Received CAPTURE_START");
                }
                if let Some(t) = cap_handle.lock().unwrap().as_ref() {
                    t.unpark();
                }
            } else if ret == WAIT_OBJECT_0 + 1 {
                // CAPTURE_STOP
                if cap_active.swap(false, Ordering::SeqCst) {
                    log::info!("[EVENT] Received CAPTURE_STOP");
                }
            } else {
                log::error!("[EVENT] WaitForMultipleObjects failed, ret={}", ret);
                break;
            }
        }
        cap_active.store(false, Ordering::SeqCst);
        std::process::exit(0);
    });

    // 捕获线程：按需 GDI 捕获
    let cap_thread = std::thread::spawn(move || {
        *cap_thread_handle.lock().unwrap() = Some(std::thread::current());
        crate::sos_capture::run(crate::sos_shmem::SHMEM_NAME, capture_active);
    });

    let _ = input_thread.join();
    let _ = event_thread.join();
    let _ = cap_thread.join();
    Ok(())
}

// ── 主进程中启动 SYSTEM 子进程 ──

/// 以 SYSTEM 身份启动自身副本，**优先放在控制台会话**。
///
/// # 为什么优先控制台会话
///
/// 现场使用方式是「在 RDP 会话里启动 SOS → 退出 RDP → 从控制端连接」。
/// 退出后该会话变成 `WTSDisconnected`，Windows **不再为它合成桌面**，
/// 任何抓屏 API 都拿不到像素（现场日志：`E_ACCESSDENIED` 连续 30 秒）。
/// 控制台会话则始终绑定物理显示输出，即使停在登录界面也在合成画面。
///
/// `global_namespace`：共享内存/命名事件是否在 `Global\` 命名空间里。
/// 若主进程没能启用 `SeCreateGlobalPrivilege`，就必须让子进程留在**同一会话**
/// 并使用会话内名字，因此要把它开关透传下去。
///
pub fn launch_system_sub_process(debug: bool, global_namespace: bool) -> isize {
    #[cfg(windows)]
    {
        let exe = std::env::current_exe()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "rustdesk-sos.exe".into());
        let mut args = String::from("--service");
        if debug {
            args.push_str(" --debug");
        }
        if !global_namespace {
            // 子进程据此把事件/共享内存名去掉 `Global\` 前缀（见 `sos_shmem::scoped_name`）
            args.push_str(" --no-global");
        }
        let cmd = format!("\"{}\" {}", exe, args);
        log::info!(
            "Starting SYSTEM sub-process (global_namespace={}): {}",
            global_namespace,
            cmd
        );

        let cmd_wide: Vec<u16> = cmd.encode_utf16().chain(std::iter::once(0)).collect();
        let mut winlogon_session = 0u32;
        let mut winlogon_pid = 0u32;
        let mut console_session = 0u32;
        let handle = unsafe {
            SosRunAsSystemInSession1(
                cmd_wide.as_ptr(),
                // 只有用 `Global\` 命名时子进程才能跨会话连上 IPC；否则必须同会话
                if global_namespace { 1 } else { 0 },
                &mut winlogon_session,
                &mut winlogon_pid,
                &mut console_session,
            )
        };
        if handle != 0 {
            log::info!(
                "SYSTEM sub-process created via CreateProcessWithTokenW, handle=0x{:x} \
                 winlogon_pid={} session={} console_session={} (main {})",
                handle,
                winlogon_pid,
                winlogon_session,
                console_session,
                crate::sos_uac::desktop_state()
            );
            if global_namespace && winlogon_session != console_session {
                log::warn!(
                    "[SESSION] console session {} unavailable, sub-process runs in session {} — \
                     its screen may differ from the console display",
                    console_session,
                    winlogon_session
                );
            } else if !global_namespace {
                log::warn!(
                    "[SESSION] cross-session IPC unavailable (Global\\ namespace not created), \
                     sub-process stays in session {} — if this session gets disconnected \
                     there will be no picture",
                    winlogon_session
                );
            }
            return handle;
        }
        log::error!("SosRunAsSystemInSession1 failed, fallback to run_as_system");
        match librustdesk::platform::run_as_system(&args) {
            Ok(()) => {
                log::info!("fallback run_as_system succeeded (no handle for monitor)");
                0
            }
            Err(e) => {
                log::error!("fallback also failed: {}", e);
                0
            }
        }
    }
    #[cfg(not(windows))]
    0
}
