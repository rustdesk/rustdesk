//! 便宜的 UAC 安全桌面检测。
//!
//! 不能直接用 `librustdesk::platform::windows::is_process_consent_running()`：
//! 它的实现是 `cmd /C tasklist | findstr consent.exe`，每次调用都要
//! 创建 cmd.exe + tasklist.exe + findstr.exe 三个进程。实测单次 205ms，
//! 而采集循环每秒调用一次 —— 等于每秒给视频链路插入一次 200ms 的完全停顿。
//!
//! 这里改用 `CreateToolhelp32Snapshot` 直接遍历进程快照，单次约 1-3ms。

#![cfg(windows)]

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};

/// 目标进程名（UAC 提权对话框）
const CONSENT_EXE: &str = "consent.exe";

/// 是否有 UAC 提权对话框在运行
pub fn consent_running() -> bool {
    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            // 拿不到快照时保守地认为没有 UAC，避免误触发重建
            return false;
        };
        // 用 guard 保证句柄一定被关闭
        let _guard = SnapshotGuard(snapshot);

        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snapshot, &mut entry).is_err() {
            return false;
        }
        loop {
            if exe_name_matches(&entry.szExeFile) {
                return true;
            }
            if Process32NextW(snapshot, &mut entry).is_err() {
                return false;
            }
        }
    }
}

struct SnapshotGuard(HANDLE);

impl Drop for SnapshotGuard {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.0) };
    }
}

/// 定长 UTF-16 缓冲区大小写不敏感地匹配进程名
fn exe_name_matches(buf: &[u16]) -> bool {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    if len != CONSENT_EXE.len() {
        return false;
    }
    buf[..len]
        .iter()
        .zip(CONSENT_EXE.bytes())
        .all(|(&c, b)| (c as u8).eq_ignore_ascii_case(&b))
}

/// 是否处于「安全桌面」（锁屏 / UAC / 其它非默认桌面）。
///
/// # 为什么不只用 `consent_running()`
///
/// `consent.exe` 只代表 **UAC 提权对话框**。锁屏界面是另一个桌面
/// （`Winlogon`），此时根本没有 `consent.exe`，只看进程名会漏掉 ——
/// 表现为「锁屏后画面冻结在最后一帧」。
///
/// 上游的判据更物理（`src/platform/windows.cc::inputDesktopSelected`）：
/// 比较**本线程所在桌面名**与**当前输入桌面名**。
///
/// ```text
/// GetUserObjectInformation(GetThreadDesktop(GetCurrentThreadId()), UOI_NAME)
/// GetUserObjectInformation(OpenInputDesktop(...),                    UOI_NAME)
/// 两者不同 → desktop_changed() == TRUE
/// ```
///
/// 这条判据同时覆盖锁屏与 UAC，且 `OpenInputDesktop` 失败时也返回 TRUE
/// （保守方向：宁可当作安全桌面）。上游在输入注入、视频服务、portable
/// service 采集三处都用它。
///
/// 注意它检查的是**调用线程**的桌面：本函数必须由从未 `SetThreadDesktop`
/// 过的线程调用，否则线程一旦切到 `Winlogon` 就会永远认为「没变」。
pub fn secure_desktop() -> bool {
    if librustdesk::platform::windows::desktop_changed() {
        return true;
    }
    // 交叉验证：`desktop_changed()` 依赖 `OpenInputDesktop` 的成败，
    // 极少数环境（受限窗口站、某些远程会话）可能恒为 FALSE。
    // 此时仍用进程快照兜住 UAC 这一种可判定情形。
    consent_running_throttled()
}

/// `consent_running()` 的调用间隔下限。
///
/// 主判据 `desktop_changed()` 已经是纯句柄操作（十几微秒），能瞬时发现 UAC；
/// 进程快照只是兜底，单次 1-3ms，没有必要每次函数调用都做一遍 ——
/// 桌面状态是秒级变化的。
const CONSENT_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(1);

static CONSENT_CACHE: std::sync::LazyLock<std::sync::Mutex<Option<(std::time::Instant, bool)>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(None));

fn consent_running_throttled() -> bool {
    let mut cache = match CONSENT_CACHE.lock() {
        Ok(c) => c,
        Err(e) => e.into_inner(),
    };
    if let Some((at, val)) = *cache {
        if at.elapsed() < CONSENT_CACHE_TTL {
            return val;
        }
    }
    let val = consent_running();
    *cache = Some((std::time::Instant::now(), val));
    val
}

/// 会话连接状态（`WTSConnectState`）。
///
/// # 为什么必须区分「锁屏」和「会话断开」
///
/// 两者在桌面层面看起来一样（`input_desktop` 都是 `Winlogon`），但**采集
/// 可行性完全不同**：
///
/// | 状态 | 画面 | 能否采集 |
/// |---|---|---|
/// | `Active` + 锁屏 | 登录界面**在渲染** | 能（切到 Winlogon 桌面） |
/// | `Disconnected`（RDP 断开） | **根本没有 framebuffer** | **不能，任何 API 都不行** |
///
/// 现场日志就是第二种：用户在 RDP 里启动 SOS → 断开 RDP → 从控制端连接。
/// 断开后会话变成 `WTSDisconnected`，Windows **不再为该会话合成桌面**，
/// 于是 `BitBlt` 在 Winlogon 与 Default 两个桌面上都返回 `E_ACCESSDENIED`，
/// `DXGI` 也没有输出。等用户重新连上 RDP（画面恢复合成）的瞬间，
/// 同一个 `CreateDCW(L"DISPLAY")` + 同一个 `CAPTUREBLT` 立刻就成功了 ——
/// 日志里 412.366 那一行正是如此。
///
/// 所以它不是一个可以「换种抓法」解决的问题，而是必须**如实告知**控制端。
pub fn session_state() -> SessionState {
    // `WTS_CONNECT_STATE` 的取值（`WTS_CONNECTSTATE_CLASS`）：
    // 0=Active 1=Connected 2=ConnectQuery 3=Shadow 4=Disconnected
    // 5=Idle 6=Listen 7=Reset 8=Down 9=Init
    extern "system" {
        fn WTSQuerySessionInformationW(
            server: isize,
            session_id: u32,
            info_class: i32,
            buffer: *mut *mut u16,
            bytes_returned: *mut u32,
        ) -> i32;
        fn WTSFreeMemory(p: *mut std::ffi::c_void);
    }
    const WTS_CURRENT_SERVER_HANDLE: isize = 0;
    const WTS_CURRENT_SESSION: u32 = 0xFFFF_FFFF;
    const WTS_CONNECT_STATE: i32 = 8;

    let mut buf: *mut u16 = std::ptr::null_mut();
    let mut bytes = 0u32;
    let ok = unsafe {
        WTSQuerySessionInformationW(
            WTS_CURRENT_SERVER_HANDLE,
            WTS_CURRENT_SESSION,
            WTS_CONNECT_STATE,
            &mut buf,
            &mut bytes,
        )
    };
    if ok == 0 || buf.is_null() || bytes < 4 {
        // 查不到时不能误报成「断开」—— 那会让正常会话被标成不可采集
        return SessionState::Unknown;
    }
    let raw = unsafe { *(buf as *const i32) };
    unsafe { WTSFreeMemory(buf as *mut std::ffi::c_void) };
    match raw {
        0 => SessionState::Active,
        1 | 2 => SessionState::Connected,
        3 => SessionState::Shadow,
        4 => SessionState::Disconnected,
        5 => SessionState::Idle,
        _ => SessionState::Unknown,
    }
}

/// 会话连接状态，见 [`session_state`]。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SessionState {
    /// 前台活动会话
    Active,
    /// 已连接但不在前台（切换用户 / 控制台被 RDP 顶掉）
    Connected,
    /// 被远程控制（RDP 会话镜像）
    Shadow,
    /// **会话已断开**（RDP 断开）：没有 framebuffer，采集不可能成功
    Disconnected,
    /// 空闲（尚未登录）
    Idle,
    Unknown,
}

impl SessionState {
    /// 该状态下屏幕内容是否可能被采集到。
    ///
    /// `Disconnected` 是硬性的「否」：Windows 不为断开的会话合成桌面。
    pub fn may_have_surface(self) -> bool {
        !matches!(self, SessionState::Disconnected)
    }

    pub fn label(self) -> &'static str {
        match self {
            SessionState::Active => "Active",
            SessionState::Connected => "Connected",
            SessionState::Shadow => "Shadow(远程控制中)",
            SessionState::Disconnected => "Disconnected(会话已断开)",
            SessionState::Idle => "Idle",
            SessionState::Unknown => "Unknown",
        }
    }
}

/// 供现场日志使用的桌面状态摘要（`session=2 thread_desktop=Default input_desktop=Winlogon`）。
///
/// 现场排查锁屏问题时，最能定性的就是这三样东西：
/// 主进程在哪个 session、它的线程挂在哪个桌面、当前输入桌面叫什么。
/// 只有 `input_desktop` 是 `Winlogon` 才说明真的锁屏了；
/// `input_desktop=<open failed 5>` 则说明我们是**无权**读取输入桌面
/// （非 SYSTEM），这在锁屏时是正常的。
///
/// 只在状态变化时调用，不进热路径。
pub fn desktop_state() -> String {
    // 用最原始的 `extern "system"` 声明，避免绑定 windows crate 的
    // 主题化 API 名字（`UOI_NAME` / `USER_OBJECT_INFORMATION_INDEX` 在各版本间变过）。
    extern "system" {
        fn GetCurrentProcessId() -> u32;
        fn GetCurrentThreadId() -> u32;
        fn GetThreadDesktop(thread_id: u32) -> isize;
        fn OpenInputDesktop(flags: u32, inherit: i32, access: u32) -> isize;
        fn CloseDesktop(desktop: isize) -> i32;
        fn GetUserObjectInformationW(
            obj: isize,
            index: i32,
            info: *mut u16,
            len: u32,
            needed: *mut u32,
        ) -> i32;
        fn ProcessIdToSessionId(pid: u32, session: *mut u32) -> i32;
    }
    const UOI_NAME: i32 = 2;
    const DESKTOP_READOBJECTS: u32 = 0x0001;
    const DESKTOP_SWITCHDESKTOP: u32 = 0x0100;

    unsafe fn name_of(h: isize) -> Option<String> {
        if h == 0 {
            return None;
        }
        let mut buf = [0u16; 256];
        let mut needed = 0u32;
        let ok = GetUserObjectInformationW(
            h,
            UOI_NAME,
            buf.as_mut_ptr(),
            (buf.len() * 2) as u32,
            &mut needed,
        );
        if ok == 0 {
            return None;
        }
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        Some(String::from_utf16_lossy(&buf[..len]))
    }

    unsafe {
        let mut session = 0u32;
        ProcessIdToSessionId(GetCurrentProcessId(), &mut session);

        // 注意：`GetThreadDesktop` 返回的句柄**不许** CloseDesktop。
        let thread_desk =
            name_of(GetThreadDesktop(GetCurrentThreadId())).unwrap_or_else(|| "?".to_string());

        let input = OpenInputDesktop(0, 0, DESKTOP_READOBJECTS | DESKTOP_SWITCHDESKTOP);
        let input_desk = match name_of(input) {
            Some(n) => n,
            None => format!(
                "<open failed {}>",
                std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
            ),
        };
        if input != 0 {
            CloseDesktop(input);
        }

        format!(
            "session={} state={} thread_desktop={} input_desktop={}",
            session,
            session_state().label(),
            thread_desk,
            input_desk
        )
    }
}
