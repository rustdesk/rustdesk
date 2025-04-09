use super::{CursorData, ResultType};
use crate::{
    common::PORTABLE_APPNAME_RUNTIME_ENV_KEY,
    custom_server::*,
    ipc,
    privacy_mode::win_topmost_window::{self, WIN_TOPMOST_INJECTED_PROCESS_EXE},
};
use hbb_common::{
    allow_err,
    anyhow::anyhow,
    bail,
    config::{self, Config},
    libc::{c_int, wchar_t},
    log,
    message_proto::{DisplayInfo, Resolution, WindowsSession},
    sleep, timeout, tokio,
};
use std::{
    collections::HashMap,
    ffi::{CString, OsString},
    fs,
    io::{self, prelude::*},
    mem,
    os::{raw::c_ulong, windows::process::CommandExt},
    path::*,
    ptr::null_mut,
    sync::{atomic::Ordering, Arc, Mutex},
    time::{Duration, Instant},
};
use wallpaper;
#[cfg(not(debug_assertions))]
use winapi::um::libloaderapi::{LoadLibraryExW, LOAD_LIBRARY_SEARCH_USER_DIRS};
use winapi::{
    ctypes::c_void,
    shared::{minwindef::*, ntdef::NULL, windef::*, winerror::*},
    um::{
        errhandlingapi::GetLastError,
        handleapi::CloseHandle,
        libloaderapi::{GetProcAddress, LoadLibraryExA, LOAD_LIBRARY_SEARCH_SYSTEM32},
        minwinbase::STILL_ACTIVE,
        processthreadsapi::{
            GetCurrentProcess, GetCurrentProcessId, GetExitCodeProcess, OpenProcess,
            OpenProcessToken, ProcessIdToSessionId, PROCESS_INFORMATION, STARTUPINFOW,
        },
        securitybaseapi::GetTokenInformation,
        shellapi::ShellExecuteW,
        sysinfoapi::{GetNativeSystemInfo, SYSTEM_INFO},
        winbase::*,
        wingdi::*,
        winnt::{
            TokenElevation, ES_AWAYMODE_REQUIRED, ES_CONTINUOUS, ES_DISPLAY_REQUIRED,
            ES_SYSTEM_REQUIRED, HANDLE, PROCESS_ALL_ACCESS, PROCESS_QUERY_LIMITED_INFORMATION,
            TOKEN_ELEVATION, TOKEN_QUERY,
        },
        winreg::HKEY_CURRENT_USER,
        winspool::{
            EnumPrintersW, GetDefaultPrinterW, PRINTER_ENUM_CONNECTIONS, PRINTER_ENUM_LOCAL,
            PRINTER_INFO_1W,
        },
        winuser::*,
    },
};
use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
};
use winreg::{enums::*, RegKey};

pub const FLUTTER_RUNNER_WIN32_WINDOW_CLASS: &'static str = "FLUTTER_RUNNER_WIN32_WINDOW"; // main window, install window
pub const EXPLORER_EXE: &'static str = "explorer.exe";
pub const SET_FOREGROUND_WINDOW: &'static str = "SET_FOREGROUND_WINDOW";

const REG_NAME_INSTALL_DESKTOPSHORTCUTS: &str = "DESKTOPSHORTCUTS";
const REG_NAME_INSTALL_STARTMENUSHORTCUTS: &str = "STARTMENUSHORTCUTS";
const REG_NAME_INSTALL_PRINTER: &str = "PRINTER";

pub fn get_focused_display(displays: Vec<DisplayInfo>) -> Option<usize> {
    unsafe {
        let hwnd = GetForegroundWindow();
        let mut rect: RECT = mem::zeroed();
        if GetWindowRect(hwnd, &mut rect as *mut RECT) == 0 {
            return None;
        }
        displays.iter().position(|display| {
            let center_x = rect.left + (rect.right - rect.left) / 2;
            let center_y = rect.top + (rect.bottom - rect.top) / 2;
            center_x >= display.x
                && center_x <= display.x + display.width
                && center_y >= display.y
                && center_y <= display.y + display.height
        })
    }
}

pub fn get_cursor_pos() -> Option<(i32, i32)> {
    unsafe {
        #[allow(invalid_value)]
        let mut out = mem::MaybeUninit::uninit().assume_init();
        if GetCursorPos(&mut out) == FALSE {
            return None;
        }
        return Some((out.x, out.y));
    }
}

pub fn reset_input_cache() {}

pub fn get_cursor() -> ResultType<Option<u64>> {
    unsafe {
        #[allow(invalid_value)]
        let mut ci: CURSORINFO = mem::MaybeUninit::uninit().assume_init();
        ci.cbSize = std::mem::size_of::<CURSORINFO>() as _;
        if crate::portable_service::client::get_cursor_info(&mut ci) == FALSE {
            return Err(io::Error::last_os_error().into());
        }
        if ci.flags & CURSOR_SHOWING == 0 {
            Ok(None)
        } else {
            Ok(Some(ci.hCursor as _))
        }
    }
}

struct IconInfo(ICONINFO);

impl IconInfo {
    fn new(icon: HICON) -> ResultType<Self> {
        unsafe {
            #[allow(invalid_value)]
            let mut ii = mem::MaybeUninit::uninit().assume_init();
            if GetIconInfo(icon, &mut ii) == FALSE {
                Err(io::Error::last_os_error().into())
            } else {
                let ii = Self(ii);
                if ii.0.hbmMask.is_null() {
                    bail!("Cursor bitmap handle is NULL");
                }
                return Ok(ii);
            }
        }
    }

    fn is_color(&self) -> bool {
        !self.0.hbmColor.is_null()
    }
}

impl Drop for IconInfo {
    fn drop(&mut self) {
        unsafe {
            if !self.0.hbmColor.is_null() {
                DeleteObject(self.0.hbmColor as _);
            }
            if !self.0.hbmMask.is_null() {
                DeleteObject(self.0.hbmMask as _);
            }
        }
    }
}

// https://github.com/TurboVNC/tightvnc/blob/a235bae328c12fd1c3aed6f3f034a37a6ffbbd22/vnc_winsrc/winvnc/vncEncoder.cpp
// https://github.com/TigerVNC/tigervnc/blob/master/win/rfb_win32/DeviceFrameBuffer.cxx
pub fn get_cursor_data(hcursor: u64) -> ResultType<CursorData> {
    unsafe {
        let mut ii = IconInfo::new(hcursor as _)?;
        let bm_mask = get_bitmap(ii.0.hbmMask)?;
        let mut width = bm_mask.bmWidth;
        let mut height = if ii.is_color() {
            bm_mask.bmHeight
        } else {
            bm_mask.bmHeight / 2
        };
        let cbits_size = width * height * 4;
        if cbits_size < 16 {
            bail!("Invalid icon: too small"); // solve some crash
        }
        let mut cbits: Vec<u8> = Vec::new();
        cbits.resize(cbits_size as _, 0);
        let mut mbits: Vec<u8> = Vec::new();
        mbits.resize((bm_mask.bmWidthBytes * bm_mask.bmHeight) as _, 0);
        let r = GetBitmapBits(ii.0.hbmMask, mbits.len() as _, mbits.as_mut_ptr() as _);
        if r == 0 {
            bail!("Failed to copy bitmap data");
        }
        if r != (mbits.len() as i32) {
            bail!(
                "Invalid mask cursor buffer size, got {} bytes, expected {}",
                r,
                mbits.len()
            );
        }
        let do_outline;
        if ii.is_color() {
            get_rich_cursor_data(ii.0.hbmColor, width, height, &mut cbits)?;
            do_outline = fix_cursor_mask(
                &mut mbits,
                &mut cbits,
                width as _,
                height as _,
                bm_mask.bmWidthBytes as _,
            );
        } else {
            do_outline = handleMask(
                cbits.as_mut_ptr(),
                mbits.as_ptr(),
                width,
                height,
                bm_mask.bmWidthBytes,
                bm_mask.bmHeight,
            ) > 0;
        }
        if do_outline {
            let mut outline = Vec::new();
            outline.resize(((width + 2) * (height + 2) * 4) as _, 0);
            drawOutline(
                outline.as_mut_ptr(),
                cbits.as_ptr(),
                width,
                height,
                outline.len() as _,
            );
            cbits = outline;
            width += 2;
            height += 2;
            ii.0.xHotspot += 1;
            ii.0.yHotspot += 1;
        }

        Ok(CursorData {
            id: hcursor,
            colors: cbits.into(),
            hotx: ii.0.xHotspot as _,
            hoty: ii.0.yHotspot as _,
            width: width as _,
            height: height as _,
            ..Default::default()
        })
    }
}

#[inline]
fn get_bitmap(handle: HBITMAP) -> ResultType<BITMAP> {
    unsafe {
        let mut bm: BITMAP = mem::zeroed();
        if GetObjectA(
            handle as _,
            std::mem::size_of::<BITMAP>() as _,
            &mut bm as *mut BITMAP as *mut _,
        ) == FALSE
        {
            return Err(io::Error::last_os_error().into());
        }
        if bm.bmPlanes != 1 {
            bail!("Unsupported multi-plane cursor");
        }
        if bm.bmBitsPixel != 1 {
            bail!("Unsupported cursor mask format");
        }
        Ok(bm)
    }
}

struct DC(HDC);

impl DC {
    fn new() -> ResultType<Self> {
        unsafe {
            let dc = GetDC(0 as _);
            if dc.is_null() {
                bail!("Failed to get a drawing context");
            }
            Ok(Self(dc))
        }
    }
}

impl Drop for DC {
    fn drop(&mut self) {
        unsafe {
            if !self.0.is_null() {
                ReleaseDC(0 as _, self.0);
            }
        }
    }
}

struct CompatibleDC(HDC);

impl CompatibleDC {
    fn new(existing: HDC) -> ResultType<Self> {
        unsafe {
            let dc = CreateCompatibleDC(existing);
            if dc.is_null() {
                bail!("Failed to get a compatible drawing context");
            }
            Ok(Self(dc))
        }
    }
}

impl Drop for CompatibleDC {
    fn drop(&mut self) {
        unsafe {
            if !self.0.is_null() {
                DeleteDC(self.0);
            }
        }
    }
}

struct BitmapDC(CompatibleDC, HBITMAP);

impl BitmapDC {
    fn new(hdc: HDC, hbitmap: HBITMAP) -> ResultType<Self> {
        unsafe {
            let dc = CompatibleDC::new(hdc)?;
            let oldbitmap = SelectObject(dc.0, hbitmap as _) as HBITMAP;
            if oldbitmap.is_null() {
                bail!("Failed to select CompatibleDC");
            }
            Ok(Self(dc, oldbitmap))
        }
    }

    fn dc(&self) -> HDC {
        (self.0).0
    }
}

impl Drop for BitmapDC {
    fn drop(&mut self) {
        unsafe {
            if !self.1.is_null() {
                SelectObject((self.0).0, self.1 as _);
            }
        }
    }
}

#[inline]
fn get_rich_cursor_data(
    hbm_color: HBITMAP,
    width: i32,
    height: i32,
    out: &mut Vec<u8>,
) -> ResultType<()> {
    unsafe {
        let dc = DC::new()?;
        let bitmap_dc = BitmapDC::new(dc.0, hbm_color)?;
        if get_di_bits(out.as_mut_ptr(), bitmap_dc.dc(), hbm_color, width, height) > 0 {
            bail!("Failed to get di bits: {}", io::Error::last_os_error());
        }
    }
    Ok(())
}

fn fix_cursor_mask(
    mbits: &mut Vec<u8>,
    cbits: &mut Vec<u8>,
    width: usize,
    height: usize,
    bm_width_bytes: usize,
) -> bool {
    let mut pix_idx = 0;
    for _ in 0..height {
        for _ in 0..width {
            if cbits[pix_idx + 3] != 0 {
                return false;
            }
            pix_idx += 4;
        }
    }

    let packed_width_bytes = (width + 7) >> 3;
    let bm_size = mbits.len();
    let c_size = cbits.len();

    // Pack and invert bitmap data (mbits)
    // borrow from tigervnc
    for y in 0..height {
        for x in 0..packed_width_bytes {
            let a = y * packed_width_bytes + x;
            let b = y * bm_width_bytes + x;
            if a < bm_size && b < bm_size {
                mbits[a] = !mbits[b];
            }
        }
    }

    // Replace "inverted background" bits with black color to ensure
    // cross-platform interoperability. Not beautiful but necessary code.
    // borrow from tigervnc
    let bytes_row = width << 2;
    for y in 0..height {
        let mut bitmask: u8 = 0x80;
        for x in 0..width {
            let mask_idx = y * packed_width_bytes + (x >> 3);
            if mask_idx < bm_size {
                let pix_idx = y * bytes_row + (x << 2);
                if (mbits[mask_idx] & bitmask) == 0 {
                    for b1 in 0..4 {
                        let a = pix_idx + b1;
                        if a < c_size {
                            if cbits[a] != 0 {
                                mbits[mask_idx] ^= bitmask;
                                for b2 in b1..4 {
                                    let b = pix_idx + b2;
                                    if b < c_size {
                                        cbits[b] = 0x00;
                                    }
                                }
                                break;
                            }
                        }
                    }
                }
            }
            bitmask >>= 1;
            if bitmask == 0 {
                bitmask = 0x80;
            }
        }
    }

    // borrow from noVNC
    let mut pix_idx = 0;
    for y in 0..height {
        for x in 0..width {
            let mask_idx = y * packed_width_bytes + (x >> 3);
            let mut alpha = 255;
            if mask_idx < bm_size {
                if (mbits[mask_idx] << (x & 0x7)) & 0x80 == 0 {
                    alpha = 0;
                }
            }
            let a = cbits[pix_idx + 2];
            let b = cbits[pix_idx + 1];
            let c = cbits[pix_idx];
            cbits[pix_idx] = a;
            cbits[pix_idx + 1] = b;
            cbits[pix_idx + 2] = c;
            cbits[pix_idx + 3] = alpha;
            pix_idx += 4;
        }
    }
    return true;
}

define_windows_service!(ffi_service_main, service_main);

fn service_main(arguments: Vec<OsString>) {
    if let Err(e) = run_service(arguments) {
        log::error!("run_service failed: {}", e);
    }
}

pub fn start_os_service() {
    if let Err(e) =
        windows_service::service_dispatcher::start(crate::get_app_name(), ffi_service_main)
    {
        log::error!("start_service failed: {}", e);
    }
}

const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

extern "C" {
    fn get_current_session(rdp: BOOL) -> DWORD;
    fn LaunchProcessWin(
        cmd: *const u16,
        session_id: DWORD,
        as_user: BOOL,
        token_pid: &mut DWORD,
    ) -> HANDLE;
    fn GetSessionUserTokenWin(
        lphUserToken: LPHANDLE,
        dwSessionId: DWORD,
        as_user: BOOL,
        token_pid: &mut DWORD,
    ) -> BOOL;
    fn selectInputDesktop() -> BOOL;
    fn inputDesktopSelected() -> BOOL;
    fn is_windows_server() -> BOOL;
    fn is_windows_10_or_greater() -> BOOL;
    fn handleMask(
        out: *mut u8,
        mask: *const u8,
        width: i32,
        height: i32,
        bmWidthBytes: i32,
        bmHeight: i32,
    ) -> i32;
    fn drawOutline(out: *mut u8, in_: *const u8, width: i32, height: i32, out_size: i32);
    fn get_di_bits(out: *mut u8, dc: HDC, hbmColor: HBITMAP, width: i32, height: i32) -> i32;
    fn blank_screen(v: BOOL);
    fn win32_enable_lowlevel_keyboard(hwnd: HWND) -> i32;
    fn win32_disable_lowlevel_keyboard(hwnd: HWND);
    fn win_stop_system_key_propagate(v: BOOL);
    fn is_win_down() -> BOOL;
    fn is_local_system() -> BOOL;
    fn alloc_console_and_redirect();
    fn is_service_running_w(svc_name: *const u16) -> bool;
}

extern "system" {
    fn BlockInput(v: BOOL) -> BOOL;
}

#[tokio::main(flavor = "current_thread")]
async fn run_service(_arguments: Vec<OsString>) -> ResultType<()> {
    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        log::info!("Got service control event: {:?}", control_event);
        match control_event {
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            ServiceControl::Stop | ServiceControl::Preshutdown | ServiceControl::Shutdown => {
                send_close(crate::POSTFIX_SERVICE).ok();
                ServiceControlHandlerResult::NoError
            }
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    // Register system service event handler
    let status_handle = service_control_handler::register(crate::get_app_name(), event_handler)?;

    let next_status = ServiceStatus {
        // Should match the one from system service registry
        service_type: SERVICE_TYPE,
        // The new state
        current_state: ServiceState::Running,
        // Accept stop events when running
        controls_accepted: ServiceControlAccept::STOP,
        // Used to report an error when starting or stopping only, otherwise must be zero
        exit_code: ServiceExitCode::Win32(0),
        // Only used for pending states, otherwise must be zero
        checkpoint: 0,
        // Only used for pending states, otherwise must be zero
        wait_hint: Duration::default(),
        process_id: None,
    };

    // Tell the system that the service is running now
    status_handle.set_service_status(next_status)?;

    let mut session_id = unsafe { get_current_session(share_rdp()) };
    log::info!("session id {}", session_id);
    let mut h_process = launch_server(session_id, true).await.unwrap_or(NULL);
    let mut incoming = ipc::new_listener(crate::POSTFIX_SERVICE).await?;
    let mut stored_usid = None;
    loop {
        let sids: Vec<_> = get_available_sessions(false)
            .iter()
            .map(|e| e.sid)
            .collect();
        if !sids.contains(&session_id) || !is_share_rdp() {
            let current_active_session = unsafe { get_current_session(share_rdp()) };
            if session_id != current_active_session {
                session_id = current_active_session;
                h_process = launch_server(session_id, true).await.unwrap_or(NULL);
            }
        }
        let res = timeout(super::SERVICE_INTERVAL, incoming.next()).await;
        match res {
            Ok(res) => match res {
                Some(Ok(stream)) => {
                    let mut stream = ipc::Connection::new(stream);
                    if let Ok(Some(data)) = stream.next_timeout(1000).await {
                        match data {
                            ipc::Data::Close => {
                                log::info!("close received");
                                break;
                            }
                            ipc::Data::SAS => {
                                send_sas();
                            }
                            ipc::Data::UserSid(usid) => {
                                if let Some(usid) = usid {
                                    if session_id != usid {
                                        log::info!(
                                            "session changed from {} to {}",
                                            session_id,
                                            usid
                                        );
                                        session_id = usid;
                                        stored_usid = Some(session_id);
                                        h_process =
                                            launch_server(session_id, true).await.unwrap_or(NULL);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            },
            Err(_) => {
                // timeout
                unsafe {
                    let tmp = get_current_session(share_rdp());
                    if tmp == 0xFFFFFFFF {
                        continue;
                    }
                    let mut close_sent = false;
                    if tmp != session_id && stored_usid != Some(session_id) {
                        log::info!("session changed from {} to {}", session_id, tmp);
                        session_id = tmp;
                        send_close_async("").await.ok();
                        close_sent = true;
                    }
                    let mut exit_code: DWORD = 0;
                    if h_process.is_null()
                        || (GetExitCodeProcess(h_process, &mut exit_code) == TRUE
                            && exit_code != STILL_ACTIVE
                            && CloseHandle(h_process) == TRUE)
                    {
                        match launch_server(session_id, !close_sent).await {
                            Ok(ptr) => {
                                h_process = ptr;
                            }
                            Err(err) => {
                                log::error!("Failed to launch server: {}", err);
                            }
                        }
                    }
                }
            }
        }
    }

    if !h_process.is_null() {
        send_close_async("").await.ok();
        unsafe { CloseHandle(h_process) };
    }

    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    Ok(())
}

async fn launch_server(session_id: DWORD, close_first: bool) -> ResultType<HANDLE> {
    if close_first {
        // in case started some elsewhere
        send_close_async("").await.ok();
    }
    let cmd = format!(
        "\"{}\" --server",
        std::env::current_exe()?.to_str().unwrap_or("")
    );
    use std::os::windows::ffi::OsStrExt;
    let wstr: Vec<u16> = std::ffi::OsStr::new(&cmd)
        .encode_wide()
        .chain(Some(0).into_iter())
        .collect();
    let wstr = wstr.as_ptr();
    let mut token_pid = 0;
    let h = unsafe { LaunchProcessWin(wstr, session_id, FALSE, &mut token_pid) };
    if h.is_null() {
        log::error!("Failed to launch server: {}", io::Error::last_os_error());
        if token_pid == 0 {
            log::error!("No process winlogon.exe");
        }
    }
    Ok(h)
}

pub fn run_as_user(arg: Vec<&str>) -> ResultType<Option<std::process::Child>> {
    let cmd = format!(
        "\"{}\" {}",
        std::env::current_exe()?.to_str().unwrap_or(""),
        arg.join(" "),
    );
    let Some(session_id) = get_current_process_session_id() else {
        bail!("Failed to get current process session id");
    };
    use std::os::windows::ffi::OsStrExt;
    let wstr: Vec<u16> = std::ffi::OsStr::new(&cmd)
        .encode_wide()
        .chain(Some(0).into_iter())
        .collect();
    let wstr = wstr.as_ptr();
    let mut token_pid = 0;
    let h = unsafe { LaunchProcessWin(wstr, session_id, TRUE, &mut token_pid) };
    if h.is_null() {
        if token_pid == 0 {
            bail!(
                "Failed to launch {:?} with session id {}: no process {}",
                arg,
                session_id,
                EXPLORER_EXE
            );
        }
        bail!(
            "Failed to launch {:?} with session id {}: {}",
            arg,
            session_id,
            io::Error::last_os_error()
        );
    }
    Ok(None)
}

#[tokio::main(flavor = "current_thread")]
async fn send_close(postfix: &str) -> ResultType<()> {
    send_close_async(postfix).await
}

async fn send_close_async(postfix: &str) -> ResultType<()> {
    ipc::connect(1000, postfix)
        .await?
        .send(&ipc::Data::Close)
        .await?;
    // sleep a while to wait for closing and exit
    sleep(0.1).await;
    Ok(())
}

// https://docs.microsoft.com/en-us/windows/win32/api/sas/nf-sas-sendsas
// https://www.cnblogs.com/doutu/p/4892726.html
pub fn send_sas() {
    #[link(name = "sas")]
    extern "system" {
        pub fn SendSAS(AsUser: BOOL);
    }
    unsafe {
        log::info!("SAS received");
        SendSAS(FALSE);
    }
}

lazy_static::lazy_static! {
    static ref SUPPRESS: Arc<Mutex<Instant>> = Arc::new(Mutex::new(Instant::now()));
}

pub fn desktop_changed() -> bool {
    unsafe { inputDesktopSelected() == FALSE }
}

pub fn try_change_desktop() -> bool {
    unsafe {
        if inputDesktopSelected() == FALSE {
            let res = selectInputDesktop() == TRUE;
            if !res {
                let mut s = SUPPRESS.lock().unwrap();
                if s.elapsed() > std::time::Duration::from_secs(3) {
                    log::error!("Failed to switch desktop: {}", io::Error::last_os_error());
                    *s = Instant::now();
                }
            } else {
                log::info!("Desktop switched");
            }
            return res;
        }
    }
    return false;
}

fn share_rdp() -> BOOL {
    if get_reg("share_rdp") != "false" {
        TRUE
    } else {
        FALSE
    }
}

pub fn is_share_rdp() -> bool {
    share_rdp() == TRUE
}

pub fn set_share_rdp(enable: bool) {
    let (subkey, _, _, _) = get_install_info();
    let cmd = format!(
        "reg add {} /f /v share_rdp /t REG_SZ /d \"{}\"",
        subkey,
        if enable { "true" } else { "false" }
    );
    run_cmds(cmd, false, "share_rdp").ok();
}

pub fn get_current_process_session_id() -> Option<u32> {
    let mut sid = 0;
    if unsafe { ProcessIdToSessionId(GetCurrentProcessId(), &mut sid) == TRUE } {
        Some(sid)
    } else {
        None
    }
}

pub fn is_physical_console_session() -> Option<bool> {
    if let Some(sid) = get_current_process_session_id() {
        let physical_console_session_id = unsafe { get_current_session(FALSE) };
        if physical_console_session_id == u32::MAX {
            return None;
        }
        return Some(physical_console_session_id == sid);
    }
    None
}

pub fn get_active_username() -> String {
    // get_active_user will give console username higher priority
    if let Some(name) = get_current_session_username() {
        return name;
    }
    if !is_root() {
        return crate::username();
    }

    extern "C" {
        fn get_active_user(path: *mut u16, n: u32, rdp: BOOL) -> u32;
    }
    let buff_size = 256;
    let mut buff: Vec<u16> = Vec::with_capacity(buff_size);
    buff.resize(buff_size, 0);
    let n = unsafe { get_active_user(buff.as_mut_ptr(), buff_size as _, share_rdp()) };
    if n == 0 {
        return "".to_owned();
    }
    let sl = unsafe { std::slice::from_raw_parts(buff.as_ptr(), n as _) };
    String::from_utf16(sl)
        .unwrap_or("??".to_owned())
        .trim_end_matches('\0')
        .to_owned()
}

fn get_current_session_username() -> Option<String> {
    let Some(sid) = get_current_process_session_id() else {
        log::error!("get_current_process_session_id failed");
        return None;
    };
    Some(get_session_username(sid))
}

fn get_session_username(session_id: u32) -> String {
    extern "C" {
        fn get_session_user_info(path: *mut u16, n: u32, session_id: u32) -> u32;
    }
    let buff_size = 256;
    let mut buff: Vec<u16> = Vec::with_capacity(buff_size);
    buff.resize(buff_size, 0);
    let n = unsafe { get_session_user_info(buff.as_mut_ptr(), buff_size as _, session_id) };
    if n == 0 {
        return "".to_owned();
    }
    let sl = unsafe { std::slice::from_raw_parts(buff.as_ptr(), n as _) };
    String::from_utf16(sl)
        .unwrap_or("".to_owned())
        .trim_end_matches('\0')
        .to_owned()
}

pub fn get_available_sessions(name: bool) -> Vec<WindowsSession> {
    extern "C" {
        fn get_available_session_ids(buf: *mut wchar_t, buf_size: c_int, include_rdp: bool);
    }
    const BUF_SIZE: c_int = 1024;
    let mut buf: Vec<wchar_t> = vec![0; BUF_SIZE as usize];

    let station_session_id_array = unsafe {
        get_available_session_ids(buf.as_mut_ptr(), BUF_SIZE, true);
        let session_ids = String::from_utf16_lossy(&buf);
        session_ids.trim_matches(char::from(0)).trim().to_string()
    };
    let mut v: Vec<WindowsSession> = vec![];
    // https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-wtsgetactiveconsolesessionid
    let physical_console_sid = unsafe { get_current_session(FALSE) };
    if physical_console_sid != u32::MAX {
        let physical_console_name = if name {
            let physical_console_username = get_session_username(physical_console_sid);
            if physical_console_username.is_empty() {
                "Console".to_owned()
            } else {
                format!("Console: {physical_console_username}")
            }
        } else {
            "".to_owned()
        };
        v.push(WindowsSession {
            sid: physical_console_sid,
            name: physical_console_name,
            ..Default::default()
        });
    }
    // https://learn.microsoft.com/en-us/previous-versions//cc722458(v=technet.10)?redirectedfrom=MSDN
    for type_session_id in station_session_id_array.split(",") {
        let split: Vec<_> = type_session_id.split(":").collect();
        if split.len() == 2 {
            if let Ok(sid) = split[1].parse::<u32>() {
                if !v.iter().any(|e| (*e).sid == sid) {
                    let name = if name {
                        let name = get_session_username(sid);
                        if name.is_empty() {
                            split[0].to_string()
                        } else {
                            format!("{}: {}", split[0], name)
                        }
                    } else {
                        "".to_owned()
                    };
                    v.push(WindowsSession {
                        sid,
                        name,
                        ..Default::default()
                    });
                }
            }
        }
    }
    if name {
        let mut name_count: HashMap<String, usize> = HashMap::new();
        for session in &v {
            *name_count.entry(session.name.clone()).or_insert(0) += 1;
        }
        let current_sid = get_current_process_session_id().unwrap_or_default();
        for e in v.iter_mut() {
            let running = e.sid == current_sid && current_sid != 0;
            if name_count.get(&e.name).map(|v| *v).unwrap_or_default() > 1 {
                e.name = format!("{} (sid = {})", e.name, e.sid);
            }
            if running {
                e.name = format!("{} (running)", e.name);
            }
        }
    }
    v
}

pub fn get_active_user_home() -> Option<PathBuf> {
    let username = get_active_username();
    if !username.is_empty() {
        let drive = std::env::var("SystemDrive").unwrap_or("C:".to_owned());
        let home = PathBuf::from(format!("{}\\Users\\{}", drive, username));
        if home.exists() {
            return Some(home);
        }
    }
    None
}

pub fn is_prelogin() -> bool {
    let Some(username) = get_current_session_username() else {
        return false;
    };
    username.is_empty() || username == "SYSTEM"
}

pub fn is_root() -> bool {
    // https://stackoverflow.com/questions/4023586/correct-way-to-find-out-if-a-service-is-running-as-the-system-user
    unsafe { is_local_system() == TRUE }
}

pub fn lock_screen() {
    extern "system" {
        pub fn LockWorkStation() -> BOOL;
    }
    unsafe {
        LockWorkStation();
    }
}

const IS1: &str = "{54E86BC2-6C85-41F3-A9EB-1A94AC9B1F93}_is1";

fn get_subkey(name: &str, wow: bool) -> String {
    let tmp = format!(
        "HKEY_LOCAL_MACHINE\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{}",
        name
    );
    if wow {
        tmp.replace("Microsoft", "Wow6432Node\\Microsoft")
    } else {
        tmp
    }
}

fn get_valid_subkey() -> String {
    let subkey = get_subkey(IS1, false);
    if !get_reg_of(&subkey, "InstallLocation").is_empty() {
        return subkey;
    }
    let subkey = get_subkey(IS1, true);
    if !get_reg_of(&subkey, "InstallLocation").is_empty() {
        return subkey;
    }
    let app_name = crate::get_app_name();
    let subkey = get_subkey(&app_name, true);
    if !get_reg_of(&subkey, "InstallLocation").is_empty() {
        return subkey;
    }
    return get_subkey(&app_name, false);
}

// Return install options other than InstallLocation.
pub fn get_install_options() -> String {
    let app_name = crate::get_app_name();
    let subkey = format!(".{}", app_name.to_lowercase());
    let mut opts = HashMap::new();

    let desktop_shortcuts = get_reg_of_hkcr(&subkey, REG_NAME_INSTALL_DESKTOPSHORTCUTS);
    if let Some(desktop_shortcuts) = desktop_shortcuts {
        opts.insert(REG_NAME_INSTALL_DESKTOPSHORTCUTS, desktop_shortcuts);
    }
    let start_menu_shortcuts = get_reg_of_hkcr(&subkey, REG_NAME_INSTALL_STARTMENUSHORTCUTS);
    if let Some(start_menu_shortcuts) = start_menu_shortcuts {
        opts.insert(REG_NAME_INSTALL_STARTMENUSHORTCUTS, start_menu_shortcuts);
    }
    let printer = get_reg_of_hkcr(&subkey, REG_NAME_INSTALL_PRINTER);
    if let Some(printer) = printer {
        opts.insert(REG_NAME_INSTALL_PRINTER, printer);
    }
    serde_json::to_string(&opts).unwrap_or("{}".to_owned())
}

// This function return Option<String>, because some registry value may be empty.
fn get_reg_of_hkcr(subkey: &str, name: &str) -> Option<String> {
    let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);
    if let Ok(tmp) = hkcr.open_subkey(subkey.replace("HKEY_CLASSES_ROOT\\", "")) {
        return tmp.get_value(name).ok();
    }
    None
}

pub fn get_install_info() -> (String, String, String, String) {
    get_install_info_with_subkey(get_valid_subkey())
}

fn get_default_install_info() -> (String, String, String, String) {
    get_install_info_with_subkey(get_subkey(&crate::get_app_name(), false))
}

fn get_default_install_path() -> String {
    let mut pf = "C:\\Program Files".to_owned();
    if let Ok(x) = std::env::var("ProgramFiles") {
        if std::path::Path::new(&x).exists() {
            pf = x;
        }
    }
    #[cfg(target_pointer_width = "32")]
    {
        let tmp = pf.replace("Program Files", "Program Files (x86)");
        if std::path::Path::new(&tmp).exists() {
            pf = tmp;
        }
    }
    format!("{}\\{}", pf, crate::get_app_name())
}

pub fn check_update_broker_process() -> ResultType<()> {
    let process_exe = win_topmost_window::INJECTED_PROCESS_EXE;
    let origin_process_exe = win_topmost_window::ORIGIN_PROCESS_EXE;

    let exe_file = std::env::current_exe()?;
    let Some(cur_dir) = exe_file.parent() else {
        bail!("Cannot get parent of current exe file");
    };
    let cur_exe = cur_dir.join(process_exe);

    // Force update broker exe if failed to check modified time.
    let cmds = format!(
        "
        chcp 65001
        taskkill /F /IM {process_exe}
        copy /Y \"{origin_process_exe}\" \"{cur_exe}\"
    ",
        cur_exe = cur_exe.to_string_lossy(),
    );

    if !std::path::Path::new(&cur_exe).exists() {
        run_cmds(cmds, false, "update_broker")?;
        return Ok(());
    }

    let ori_modified = fs::metadata(origin_process_exe)?.modified()?;
    if let Ok(metadata) = fs::metadata(&cur_exe) {
        if let Ok(cur_modified) = metadata.modified() {
            if cur_modified == ori_modified {
                return Ok(());
            } else {
                log::info!(
                    "broker process updated, modify time from {:?} to {:?}",
                    cur_modified,
                    ori_modified
                );
            }
        }
    }

    run_cmds(cmds, false, "update_broker")?;

    Ok(())
}

fn get_install_info_with_subkey(subkey: String) -> (String, String, String, String) {
    let mut path = get_reg_of(&subkey, "InstallLocation");
    if path.is_empty() {
        path = get_default_install_path();
    }
    path = path.trim_end_matches('\\').to_owned();
    let start_menu = format!(
        "%ProgramData%\\Microsoft\\Windows\\Start Menu\\Programs\\{}",
        crate::get_app_name()
    );
    let exe = format!("{}\\{}.exe", path, crate::get_app_name());
    (subkey, path, start_menu, exe)
}

pub fn copy_raw_cmd(src_raw: &str, _raw: &str, _path: &str) -> ResultType<String> {
    let main_raw = format!(
        "XCOPY \"{}\" \"{}\" /Y /E /H /C /I /K /R /Z",
        PathBuf::from(src_raw)
            .parent()
            .ok_or(anyhow!("Can't get parent directory of {src_raw}"))?
            .to_string_lossy()
            .to_string(),
        _path
    );
    return Ok(main_raw);
}

pub fn copy_exe_cmd(src_exe: &str, exe: &str, path: &str) -> ResultType<String> {
    let main_exe = copy_raw_cmd(src_exe, exe, path)?;
    Ok(format!(
        "
        {main_exe}
        copy /Y \"{ORIGIN_PROCESS_EXE}\" \"{path}\\{broker_exe}\"
        ",
        ORIGIN_PROCESS_EXE = win_topmost_window::ORIGIN_PROCESS_EXE,
        broker_exe = win_topmost_window::INJECTED_PROCESS_EXE,
    ))
}

fn get_after_install(
    exe: &str,
    reg_value_start_menu_shortcuts: Option<String>,
    reg_value_desktop_shortcuts: Option<String>,
    reg_value_printer: Option<String>,
) -> String {
    let app_name = crate::get_app_name();
    let ext = app_name.to_lowercase();

    // reg delete HKEY_CURRENT_USER\Software\Classes for
    // https://github.com/rustdesk/rustdesk/commit/f4bdfb6936ae4804fc8ab1cf560db192622ad01a
    // and https://github.com/leanflutter/uni_links_desktop/blob/1b72b0226cec9943ca8a84e244c149773f384e46/lib/src/protocol_registrar_impl_windows.dart#L30
    let hcu = winreg::RegKey::predef(HKEY_CURRENT_USER);
    hcu.delete_subkey_all(format!("Software\\Classes\\{}", exe))
        .ok();

    let desktop_shortcuts = reg_value_desktop_shortcuts
        .map(|v| {
            format!("reg add HKEY_CLASSES_ROOT\\.{ext} /f /v {REG_NAME_INSTALL_DESKTOPSHORTCUTS} /t REG_SZ /d \"{v}\"")
        })
        .unwrap_or_default();
    let start_menu_shortcuts = reg_value_start_menu_shortcuts
        .map(|v| {
            format!(
                "reg add HKEY_CLASSES_ROOT\\.{ext} /f /v {REG_NAME_INSTALL_STARTMENUSHORTCUTS} /t REG_SZ /d \"{v}\""
            )
        })
        .unwrap_or_default();
    let reg_printer = reg_value_printer
        .map(|v| {
            format!(
                "reg add HKEY_CLASSES_ROOT\\.{ext} /f /v {REG_NAME_INSTALL_PRINTER} /t REG_SZ /d \"{v}\""
            )
        })
        .unwrap_or_default();

    format!("
    chcp 65001
    reg add HKEY_CLASSES_ROOT\\.{ext} /f
    {desktop_shortcuts}
    {start_menu_shortcuts}
    {reg_printer}
    reg add HKEY_CLASSES_ROOT\\.{ext}\\DefaultIcon /f
    reg add HKEY_CLASSES_ROOT\\.{ext}\\DefaultIcon /f /ve /t REG_SZ  /d \"\\\"{exe}\\\",0\"
    reg add HKEY_CLASSES_ROOT\\.{ext}\\shell /f
    reg add HKEY_CLASSES_ROOT\\.{ext}\\shell\\open /f
    reg add HKEY_CLASSES_ROOT\\.{ext}\\shell\\open\\command /f
    reg add HKEY_CLASSES_ROOT\\.{ext}\\shell\\open\\command /f /ve /t REG_SZ /d \"\\\"{exe}\\\" --play \\\"%%1\\\"\"
    reg add HKEY_CLASSES_ROOT\\{ext} /f
    reg add HKEY_CLASSES_ROOT\\{ext} /f /v \"URL Protocol\" /t REG_SZ /d \"\"
    reg add HKEY_CLASSES_ROOT\\{ext}\\shell /f
    reg add HKEY_CLASSES_ROOT\\{ext}\\shell\\open /f
    reg add HKEY_CLASSES_ROOT\\{ext}\\shell\\open\\command /f
    reg add HKEY_CLASSES_ROOT\\{ext}\\shell\\open\\command /f /ve /t REG_SZ /d \"\\\"{exe}\\\" \\\"%%1\\\"\"
    netsh advfirewall firewall add rule name=\"{app_name} Service\" dir=out action=allow program=\"{exe}\" enable=yes
    netsh advfirewall firewall add rule name=\"{app_name} Service\" dir=in action=allow program=\"{exe}\" enable=yes
    {create_service}
    reg add HKEY_LOCAL_MACHINE\\Software\\Microsoft\\Windows\\CurrentVersion\\Policies\\System /f /v SoftwareSASGeneration /t REG_DWORD /d 1
    ", create_service=get_create_service(&exe))
}

pub fn install_me(options: &str, path: String, silent: bool, debug: bool) -> ResultType<()> {
    let uninstall_str = get_uninstall(false);
    let mut path = path.trim_end_matches('\\').to_owned();
    let (subkey, _path, start_menu, exe) = get_default_install_info();
    let mut exe = exe;
    if path.is_empty() {
        path = _path;
    } else {
        exe = exe.replace(&_path, &path);
    }
    let mut version_major = "0";
    let mut version_minor = "0";
    let mut version_build = "0";
    let versions: Vec<&str> = crate::VERSION.split(".").collect();
    if versions.len() > 0 {
        version_major = versions[0];
    }
    if versions.len() > 1 {
        version_minor = versions[1];
    }
    if versions.len() > 2 {
        version_build = versions[2];
    }
    let app_name = crate::get_app_name();

    let tmp_path = std::env::temp_dir().to_string_lossy().to_string();
    let mk_shortcut = write_cmds(
        format!(
            "
Set oWS = WScript.CreateObject(\"WScript.Shell\")
sLinkFile = \"{tmp_path}\\{app_name}.lnk\"

Set oLink = oWS.CreateShortcut(sLinkFile)
    oLink.TargetPath = \"{exe}\"
oLink.Save
        "
        ),
        "vbs",
        "mk_shortcut",
    )?
    .to_str()
    .unwrap_or("")
    .to_owned();
    // https://superuser.com/questions/392061/how-to-make-a-shortcut-from-cmd
    let uninstall_shortcut = write_cmds(
        format!(
            "
Set oWS = WScript.CreateObject(\"WScript.Shell\")
sLinkFile = \"{tmp_path}\\Uninstall {app_name}.lnk\"
Set oLink = oWS.CreateShortcut(sLinkFile)
    oLink.TargetPath = \"{exe}\"
    oLink.Arguments = \"--uninstall\"
    oLink.IconLocation = \"msiexec.exe\"
oLink.Save
        "
        ),
        "vbs",
        "uninstall_shortcut",
    )?
    .to_str()
    .unwrap_or("")
    .to_owned();
    let tray_shortcut = get_tray_shortcut(&exe, &tmp_path)?;
    let mut reg_value_desktop_shortcuts = "0".to_owned();
    let mut reg_value_start_menu_shortcuts = "0".to_owned();
    let mut reg_value_printer = "0".to_owned();
    let mut shortcuts = Default::default();
    if options.contains("desktopicon") {
        shortcuts = format!(
            "copy /Y \"{}\\{}.lnk\" \"%PUBLIC%\\Desktop\\\"",
            tmp_path,
            crate::get_app_name()
        );
        reg_value_desktop_shortcuts = "1".to_owned();
    }
    if options.contains("startmenu") {
        shortcuts = format!(
            "{shortcuts}
md \"{start_menu}\"
copy /Y \"{tmp_path}\\{app_name}.lnk\" \"{start_menu}\\\"
copy /Y \"{tmp_path}\\Uninstall {app_name}.lnk\" \"{start_menu}\\\"
     "
        );
        reg_value_start_menu_shortcuts = "1".to_owned();
    }
    let install_printer = options.contains("printer") && crate::platform::is_win_10_or_greater();
    if install_printer {
        reg_value_printer = "1".to_owned();
    }

    let meta = std::fs::symlink_metadata(std::env::current_exe()?)?;
    let size = meta.len() / 1024;
    // https://docs.microsoft.com/zh-cn/windows/win32/msi/uninstall-registry-key?redirectedfrom=MSDNa
    // https://www.windowscentral.com/how-edit-registry-using-command-prompt-windows-10
    // https://www.tenforums.com/tutorials/70903-add-remove-allowed-apps-through-windows-firewall-windows-10-a.html
    // Note: without if exist, the bat may exit in advance on some Windows7 https://github.com/rustdesk/rustdesk/issues/895
    let dels = format!(
        "
if exist \"{mk_shortcut}\" del /f /q \"{mk_shortcut}\"
if exist \"{uninstall_shortcut}\" del /f /q \"{uninstall_shortcut}\"
if exist \"{tray_shortcut}\" del /f /q \"{tray_shortcut}\"
if exist \"{tmp_path}\\{app_name}.lnk\" del /f /q \"{tmp_path}\\{app_name}.lnk\"
if exist \"{tmp_path}\\Uninstall {app_name}.lnk\" del /f /q \"{tmp_path}\\Uninstall {app_name}.lnk\"
if exist \"{tmp_path}\\{app_name} Tray.lnk\" del /f /q \"{tmp_path}\\{app_name} Tray.lnk\"
        "
    );
    let src_exe = std::env::current_exe()?.to_str().unwrap_or("").to_string();

    // potential bug here: if run_cmd cancelled, but config file is changed.
    if let Some(lic) = get_license() {
        Config::set_option("key".into(), lic.key);
        Config::set_option("custom-rendezvous-server".into(), lic.host);
        Config::set_option("api-server".into(), lic.api);
    }

    let tray_shortcuts = if config::is_outgoing_only() {
        "".to_owned()
    } else {
        format!("
cscript \"{tray_shortcut}\"
copy /Y \"{tmp_path}\\{app_name} Tray.lnk\" \"%PROGRAMDATA%\\Microsoft\\Windows\\Start Menu\\Programs\\Startup\\\"
")
    };

    let cmds = format!(
        "
{uninstall_str}
chcp 65001
md \"{path}\"
{copy_exe}
reg add {subkey} /f
reg add {subkey} /f /v DisplayIcon /t REG_SZ /d \"{exe}\"
reg add {subkey} /f /v DisplayName /t REG_SZ /d \"{app_name}\"
reg add {subkey} /f /v DisplayVersion /t REG_SZ /d \"{version}\"
reg add {subkey} /f /v Version /t REG_SZ /d \"{version}\"
reg add {subkey} /f /v BuildDate /t REG_SZ /d \"{build_date}\"
reg add {subkey} /f /v InstallLocation /t REG_SZ /d \"{path}\"
reg add {subkey} /f /v Publisher /t REG_SZ /d \"{app_name}\"
reg add {subkey} /f /v VersionMajor /t REG_DWORD /d {version_major}
reg add {subkey} /f /v VersionMinor /t REG_DWORD /d {version_minor}
reg add {subkey} /f /v VersionBuild /t REG_DWORD /d {version_build}
reg add {subkey} /f /v UninstallString /t REG_SZ /d \"\\\"{exe}\\\" --uninstall\"
reg add {subkey} /f /v EstimatedSize /t REG_DWORD /d {size}
reg add {subkey} /f /v WindowsInstaller /t REG_DWORD /d 0
cscript \"{mk_shortcut}\"
cscript \"{uninstall_shortcut}\"
{tray_shortcuts}
{shortcuts}
copy /Y \"{tmp_path}\\Uninstall {app_name}.lnk\" \"{path}\\\"
{dels}
{import_config}
{after_install}
{sleep}
    ",
        version = crate::VERSION.replace("-", "."),
        build_date = crate::BUILD_DATE,
        after_install = get_after_install(
            &exe,
            Some(reg_value_start_menu_shortcuts),
            Some(reg_value_desktop_shortcuts),
            Some(reg_value_printer)
        ),
        sleep = if debug { "timeout 300" } else { "" },
        dels = if debug { "" } else { &dels },
        copy_exe = copy_exe_cmd(&src_exe, &exe, &path)?,
        import_config = get_import_config(&exe),
    );
    run_cmds(cmds, debug, "install")?;
    if install_printer {
        allow_err!(remote_printer::install_update_printer(
            &crate::get_app_name()
        ));
    }
    run_after_run_cmds(silent);
    Ok(())
}

pub fn run_after_install() -> ResultType<()> {
    let (_, _, _, exe) = get_install_info();
    run_cmds(
        get_after_install(&exe, None, None, None),
        true,
        "after_install",
    )
}

pub fn run_before_uninstall() -> ResultType<()> {
    run_cmds(get_before_uninstall(true), true, "before_install")
}

fn get_before_uninstall(kill_self: bool) -> String {
    let app_name = crate::get_app_name();
    let ext = app_name.to_lowercase();
    let filter = if kill_self {
        "".to_string()
    } else {
        format!(" /FI \"PID ne {}\"", get_current_pid())
    };
    format!(
        "
    chcp 65001
    sc stop {app_name}
    sc delete {app_name}
    taskkill /F /IM {broker_exe}
    taskkill /F /IM {app_name}.exe{filter}
    reg delete HKEY_CLASSES_ROOT\\.{ext} /f
    reg delete HKEY_CLASSES_ROOT\\{ext} /f
    netsh advfirewall firewall delete rule name=\"{app_name} Service\"
    ",
        broker_exe = WIN_TOPMOST_INJECTED_PROCESS_EXE,
    )
}

fn get_uninstall(kill_self: bool) -> String {
    let reg_uninstall_string = get_reg("UninstallString");
    if reg_uninstall_string.to_lowercase().contains("msiexec.exe") {
        return reg_uninstall_string;
    }

    let mut uninstall_cert_cmd = "".to_string();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_path) = exe.to_str() {
            uninstall_cert_cmd = format!("\"{}\" --uninstall-cert", exe_path);
        }
    }
    let (subkey, path, start_menu, _) = get_install_info();
    format!(
        "
    {before_uninstall}
    {uninstall_cert_cmd}
    reg delete {subkey} /f
    {uninstall_amyuni_idd}
    if exist \"{path}\" rd /s /q \"{path}\"
    if exist \"{start_menu}\" rd /s /q \"{start_menu}\"
    if exist \"%PUBLIC%\\Desktop\\{app_name}.lnk\" del /f /q \"%PUBLIC%\\Desktop\\{app_name}.lnk\"
    if exist \"%PROGRAMDATA%\\Microsoft\\Windows\\Start Menu\\Programs\\Startup\\{app_name} Tray.lnk\" del /f /q \"%PROGRAMDATA%\\Microsoft\\Windows\\Start Menu\\Programs\\Startup\\{app_name} Tray.lnk\"
    ",
        before_uninstall=get_before_uninstall(kill_self),
        uninstall_amyuni_idd=get_uninstall_amyuni_idd(),
        app_name = crate::get_app_name(),
    )
}

pub fn uninstall_me(kill_self: bool) -> ResultType<()> {
    if crate::platform::is_win_10_or_greater() {
        remote_printer::uninstall_printer(&crate::get_app_name());
    }
    run_cmds(get_uninstall(kill_self), true, "uninstall")
}

fn write_cmds(cmds: String, ext: &str, tip: &str) -> ResultType<std::path::PathBuf> {
    let mut cmds = cmds;
    let mut tmp = std::env::temp_dir();
    // When dir contains these characters, the bat file will not execute in elevated mode.
    if vec!["&", "@", "^"]
        .drain(..)
        .any(|s| tmp.to_string_lossy().to_string().contains(s))
    {
        if let Ok(dir) = user_accessible_folder() {
            tmp = dir;
        }
    }
    tmp.push(format!("{}_{}.{}", crate::get_app_name(), tip, ext));
    let mut file = std::fs::File::create(&tmp)?;
    if ext == "bat" {
        let tmp2 = get_undone_file(&tmp)?;
        std::fs::File::create(&tmp2).ok();
        cmds = format!(
            "
{cmds}
if exist \"{path}\" del /f /q \"{path}\"
",
            path = tmp2.to_string_lossy()
        );
    }
    // in case cmds mixed with \r\n and \n, make sure all ending with \r\n
    // in some windows, \r\n required for cmd file to run
    cmds = cmds.replace("\r\n", "\n").replace("\n", "\r\n");
    if ext == "vbs" {
        let mut v: Vec<u16> = cmds.encode_utf16().collect();
        // utf8 -> utf16le which vbs support it only
        file.write_all(to_le(&mut v))?;
    } else {
        file.write_all(cmds.as_bytes())?;
    }
    file.sync_all()?;
    return Ok(tmp);
}

fn to_le(v: &mut [u16]) -> &[u8] {
    for b in v.iter_mut() {
        *b = b.to_le()
    }
    unsafe { v.align_to().1 }
}

fn get_undone_file(tmp: &Path) -> ResultType<PathBuf> {
    Ok(tmp.with_file_name(format!(
        "{}.undone",
        tmp.file_name()
            .ok_or(anyhow!("Failed to get filename of {:?}", tmp))?
            .to_string_lossy()
    )))
}

fn run_cmds(cmds: String, show: bool, tip: &str) -> ResultType<()> {
    let tmp = write_cmds(cmds, "bat", tip)?;
    let tmp2 = get_undone_file(&tmp)?;
    let tmp_fn = tmp.to_str().unwrap_or("");
    // https://github.com/rustdesk/rustdesk/issues/6786#issuecomment-1879655410
    // Specify cmd.exe explicitly to avoid the replacement of cmd commands.
    let res = runas::Command::new("cmd.exe")
        .args(&["/C", &tmp_fn])
        .show(show)
        .force_prompt(true)
        .status();
    if !show {
        allow_err!(std::fs::remove_file(tmp));
    }
    let _ = res?;
    if tmp2.exists() {
        allow_err!(std::fs::remove_file(tmp2));
        bail!("{} failed", tip);
    }
    Ok(())
}

pub fn toggle_blank_screen(v: bool) {
    let v = if v { TRUE } else { FALSE };
    unsafe {
        blank_screen(v);
    }
}

pub fn block_input(v: bool) -> (bool, String) {
    let v = if v { TRUE } else { FALSE };
    unsafe {
        if BlockInput(v) == TRUE {
            (true, "".to_owned())
        } else {
            (false, format!("Error: {}", io::Error::last_os_error()))
        }
    }
}

pub fn add_recent_document(path: &str) {
    extern "C" {
        fn AddRecentDocument(path: *const u16);
    }
    use std::os::windows::ffi::OsStrExt;
    let wstr: Vec<u16> = std::ffi::OsStr::new(path)
        .encode_wide()
        .chain(Some(0).into_iter())
        .collect();
    let wstr = wstr.as_ptr();
    unsafe {
        AddRecentDocument(wstr);
    }
}

pub fn is_installed() -> bool {
    let (_, _, _, exe) = get_install_info();
    std::fs::metadata(exe).is_ok()
}

pub fn get_reg(name: &str) -> String {
    let (subkey, _, _, _) = get_install_info();
    get_reg_of(&subkey, name)
}

fn get_reg_of(subkey: &str, name: &str) -> String {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    if let Ok(tmp) = hklm.open_subkey(subkey.replace("HKEY_LOCAL_MACHINE\\", "")) {
        if let Ok(v) = tmp.get_value(name) {
            return v;
        }
    }
    "".to_owned()
}

pub fn get_license_from_exe_name() -> ResultType<CustomServer> {
    let mut exe = std::env::current_exe()?.to_str().unwrap_or("").to_owned();
    // if defined portable appname entry, replace original executable name with it.
    if let Ok(portable_exe) = std::env::var(PORTABLE_APPNAME_RUNTIME_ENV_KEY) {
        exe = portable_exe;
    }
    get_custom_server_from_string(&exe)
}

#[inline]
pub fn is_win_server() -> bool {
    unsafe { is_windows_server() > 0 }
}

#[inline]
pub fn is_win_10_or_greater() -> bool {
    unsafe { is_windows_10_or_greater() > 0 }
}

pub fn bootstrap() -> bool {
    if let Ok(lic) = get_license_from_exe_name() {
        *config::EXE_RENDEZVOUS_SERVER.write().unwrap() = lic.host.clone();
    }

    #[cfg(debug_assertions)]
    {
        true
    }
    #[cfg(not(debug_assertions))]
    {
        // This function will cause `'sciter.dll' was not found neither in PATH nor near the current executable.` when debugging RustDesk.
        set_safe_load_dll()
    }
}

#[cfg(not(debug_assertions))]
fn set_safe_load_dll() -> bool {
    if !unsafe { set_default_dll_directories() } {
        return false;
    }

    // `SetDllDirectoryW` should never fail.
    // https://docs.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-setdlldirectoryw
    if unsafe { SetDllDirectoryW(wide_string("").as_ptr()) == FALSE } {
        eprintln!("SetDllDirectoryW failed: {}", io::Error::last_os_error());
        return false;
    }

    true
}

// https://docs.microsoft.com/en-us/windows/win32/api/libloaderapi/nf-libloaderapi-setdefaultdlldirectories
#[cfg(not(debug_assertions))]
unsafe fn set_default_dll_directories() -> bool {
    let module = LoadLibraryExW(
        wide_string("Kernel32.dll").as_ptr(),
        0 as _,
        LOAD_LIBRARY_SEARCH_SYSTEM32,
    );
    if module.is_null() {
        return false;
    }

    match CString::new("SetDefaultDllDirectories") {
        Err(e) => {
            eprintln!("CString::new failed: {}", e);
            return false;
        }
        Ok(func_name) => {
            let func = GetProcAddress(module, func_name.as_ptr());
            if func.is_null() {
                eprintln!("GetProcAddress failed: {}", io::Error::last_os_error());
                return false;
            }
            type SetDefaultDllDirectories = unsafe extern "system" fn(DWORD) -> BOOL;
            let func: SetDefaultDllDirectories = std::mem::transmute(func);
            if func(LOAD_LIBRARY_SEARCH_SYSTEM32 | LOAD_LIBRARY_SEARCH_USER_DIRS) == FALSE {
                eprintln!(
                    "SetDefaultDllDirectories failed: {}",
                    io::Error::last_os_error()
                );
                return false;
            }
        }
    }
    true
}

pub fn create_shortcut(id: &str) -> ResultType<()> {
    let exe = std::env::current_exe()?.to_str().unwrap_or("").to_owned();
    let shortcut = write_cmds(
        format!(
            "
Set oWS = WScript.CreateObject(\"WScript.Shell\")
strDesktop = oWS.SpecialFolders(\"Desktop\")
Set objFSO = CreateObject(\"Scripting.FileSystemObject\")
sLinkFile = objFSO.BuildPath(strDesktop, \"{id}.lnk\")
Set oLink = oWS.CreateShortcut(sLinkFile)
    oLink.TargetPath = \"{exe}\"
    oLink.Arguments = \"--connect {id}\"
oLink.Save
        "
        ),
        "vbs",
        "connect_shortcut",
    )?
    .to_str()
    .unwrap_or("")
    .to_owned();
    std::process::Command::new("cscript")
        .arg(&shortcut)
        .output()?;
    allow_err!(std::fs::remove_file(shortcut));
    Ok(())
}

pub fn enable_lowlevel_keyboard(hwnd: HWND) {
    let ret = unsafe { win32_enable_lowlevel_keyboard(hwnd) };
    if ret != 0 {
        log::error!("Failure grabbing keyboard");
        return;
    }
}

pub fn disable_lowlevel_keyboard(hwnd: HWND) {
    unsafe { win32_disable_lowlevel_keyboard(hwnd) };
}

pub fn stop_system_key_propagate(v: bool) {
    unsafe { win_stop_system_key_propagate(if v { TRUE } else { FALSE }) };
}

pub fn get_win_key_state() -> bool {
    unsafe { is_win_down() == TRUE }
}

pub fn quit_gui() {
    std::process::exit(0);
    // unsafe { PostQuitMessage(0) }; // some how not work
}

pub fn get_user_token(session_id: u32, as_user: bool) -> HANDLE {
    let mut token = NULL as HANDLE;
    unsafe {
        let mut _token_pid = 0;
        if FALSE
            == GetSessionUserTokenWin(
                &mut token as _,
                session_id,
                if as_user { TRUE } else { FALSE },
                &mut _token_pid,
            )
        {
            NULL as _
        } else {
            token
        }
    }
}

pub fn run_background(exe: &str, arg: &str) -> ResultType<bool> {
    let wexe = wide_string(exe);
    let warg;
    unsafe {
        let ret = ShellExecuteW(
            NULL as _,
            NULL as _,
            wexe.as_ptr() as _,
            if arg.is_empty() {
                NULL as _
            } else {
                warg = wide_string(arg);
                warg.as_ptr() as _
            },
            NULL as _,
            SW_HIDE,
        );
        return Ok(ret as i32 > 32);
    }
}

pub fn run_uac(exe: &str, arg: &str) -> ResultType<bool> {
    let wop = wide_string("runas");
    let wexe = wide_string(exe);
    let warg;
    unsafe {
        let ret = ShellExecuteW(
            NULL as _,
            wop.as_ptr() as _,
            wexe.as_ptr() as _,
            if arg.is_empty() {
                NULL as _
            } else {
                warg = wide_string(arg);
                warg.as_ptr() as _
            },
            NULL as _,
            SW_SHOWNORMAL,
        );
        return Ok(ret as i32 > 32);
    }
}

pub fn check_super_user_permission() -> ResultType<bool> {
    run_uac(
        std::env::current_exe()?
            .to_string_lossy()
            .to_string()
            .as_str(),
        "--version",
    )
}

pub fn elevate(arg: &str) -> ResultType<bool> {
    run_uac(
        std::env::current_exe()?
            .to_string_lossy()
            .to_string()
            .as_str(),
        arg,
    )
}

pub fn run_as_system(arg: &str) -> ResultType<()> {
    let exe = std::env::current_exe()?.to_string_lossy().to_string();
    if impersonate_system::run_as_system(&exe, arg).is_err() {
        bail!(format!("Failed to run {} as system", exe));
    }
    Ok(())
}

pub fn elevate_or_run_as_system(is_setup: bool, is_elevate: bool, is_run_as_system: bool) {
    // avoid possible run recursively due to failed run.
    log::info!(
        "elevate: {} -> {:?}, run_as_system: {} -> {}",
        is_elevate,
        is_elevated(None),
        is_run_as_system,
        crate::username(),
    );
    let arg_elevate = if is_setup {
        "--noinstall --elevate"
    } else {
        "--elevate"
    };
    let arg_run_as_system = if is_setup {
        "--noinstall --run-as-system"
    } else {
        "--run-as-system"
    };
    if is_root() {
        if is_run_as_system {
            log::info!("run portable service");
            crate::portable_service::server::run_portable_service();
        }
    } else {
        match is_elevated(None) {
            Ok(elevated) => {
                if elevated {
                    if !is_run_as_system {
                        if run_as_system(arg_run_as_system).is_ok() {
                            std::process::exit(0);
                        } else {
                            log::error!(
                                "Failed to run as system, error {}",
                                io::Error::last_os_error()
                            );
                        }
                    }
                } else {
                    if !is_elevate {
                        if let Ok(true) = elevate(arg_elevate) {
                            std::process::exit(0);
                        } else {
                            log::error!("Failed to elevate, error {}", io::Error::last_os_error());
                        }
                    }
                }
            }
            Err(_) => log::error!(
                "Failed to get elevation status, error {}",
                io::Error::last_os_error()
            ),
        }
    }
}

pub fn is_elevated(process_id: Option<DWORD>) -> ResultType<bool> {
    use hbb_common::platform::windows::RAIIHandle;
    unsafe {
        let handle: HANDLE = match process_id {
            Some(process_id) => OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, process_id),
            None => GetCurrentProcess(),
        };
        if handle == NULL {
            bail!(
                "Failed to open process, error {}",
                io::Error::last_os_error()
            )
        }
        let _handle = RAIIHandle(handle);
        let mut token: HANDLE = mem::zeroed();
        if OpenProcessToken(handle, TOKEN_QUERY, &mut token) == FALSE {
            bail!(
                "Failed to open process token, error {}",
                io::Error::last_os_error()
            )
        }
        let _token = RAIIHandle(token);
        let mut token_elevation: TOKEN_ELEVATION = mem::zeroed();
        let mut size: DWORD = 0;
        if GetTokenInformation(
            token,
            TokenElevation,
            (&mut token_elevation) as *mut _ as *mut c_void,
            mem::size_of::<TOKEN_ELEVATION>() as _,
            &mut size,
        ) == FALSE
        {
            bail!(
                "Failed to get token information, error {}",
                io::Error::last_os_error()
            )
        }

        Ok(token_elevation.TokenIsElevated != 0)
    }
}

pub fn is_foreground_window_elevated() -> ResultType<bool> {
    unsafe {
        let mut process_id: DWORD = 0;
        GetWindowThreadProcessId(GetForegroundWindow(), &mut process_id);
        if process_id == 0 {
            bail!(
                "Failed to get processId, error {}",
                io::Error::last_os_error()
            )
        }
        is_elevated(Some(process_id))
    }
}

fn get_current_pid() -> u32 {
    unsafe { GetCurrentProcessId() }
}

pub fn get_double_click_time() -> u32 {
    unsafe { GetDoubleClickTime() }
}

pub fn wide_string(s: &str) -> Vec<u16> {
    use std::os::windows::prelude::OsStrExt;
    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(Some(0).into_iter())
        .collect()
}

/// send message to currently shown window
pub fn send_message_to_hnwd(
    class_name: &str,
    window_name: &str,
    dw_data: usize,
    data: &str,
    show_window: bool,
) -> bool {
    unsafe {
        let class_name_utf16 = wide_string(class_name);
        let window_name_utf16 = wide_string(window_name);
        let window = FindWindowW(class_name_utf16.as_ptr(), window_name_utf16.as_ptr());
        if window.is_null() {
            log::warn!("no such window {}:{}", class_name, window_name);
            return false;
        }
        let mut data_struct = COPYDATASTRUCT::default();
        data_struct.dwData = dw_data;
        let mut data_zero: String = data.chars().chain(Some('\0').into_iter()).collect();
        println!("send {:?}", data_zero);
        data_struct.cbData = data_zero.len() as _;
        data_struct.lpData = data_zero.as_mut_ptr() as _;
        SendMessageW(
            window,
            WM_COPYDATA,
            0,
            &data_struct as *const COPYDATASTRUCT as _,
        );
        if show_window {
            ShowWindow(window, SW_NORMAL);
            SetForegroundWindow(window);
        }
    }
    return true;
}

pub fn create_process_with_logon(user: &str, pwd: &str, exe: &str, arg: &str) -> ResultType<()> {
    let last_error_table = HashMap::from([
        (
            ERROR_LOGON_FAILURE,
            "The user name or password is incorrect.",
        ),
        (ERROR_ACCESS_DENIED, "Access is denied."),
    ]);

    unsafe {
        let user_split = user.split("\\").collect::<Vec<&str>>();
        let wuser = wide_string(user_split.get(1).unwrap_or(&user));
        let wpc = wide_string(user_split.get(0).unwrap_or(&""));
        let wpwd = wide_string(pwd);
        let cmd = if arg.is_empty() {
            format!("\"{}\"", exe)
        } else {
            format!("\"{}\" {}", exe, arg)
        };
        let mut wcmd = wide_string(&cmd);
        let mut si: STARTUPINFOW = mem::zeroed();
        si.wShowWindow = SW_HIDE as _;
        si.lpDesktop = NULL as _;
        si.cb = std::mem::size_of::<STARTUPINFOW>() as _;
        si.dwFlags = STARTF_USESHOWWINDOW;
        let mut pi: PROCESS_INFORMATION = mem::zeroed();
        let wexe = wide_string(exe);
        if FALSE
            == CreateProcessWithLogonW(
                wuser.as_ptr(),
                wpc.as_ptr(),
                wpwd.as_ptr(),
                LOGON_WITH_PROFILE,
                wexe.as_ptr(),
                wcmd.as_mut_ptr(),
                CREATE_UNICODE_ENVIRONMENT,
                NULL,
                NULL as _,
                &mut si as *mut STARTUPINFOW,
                &mut pi as *mut PROCESS_INFORMATION,
            )
        {
            let last_error = GetLastError();
            bail!(
                "CreateProcessWithLogonW failed : \"{}\", error {}",
                last_error_table
                    .get(&last_error)
                    .unwrap_or(&"Unknown error"),
                io::Error::from_raw_os_error(last_error as _)
            );
        }
    }
    return Ok(());
}

pub fn set_path_permission(dir: &Path, permission: &str) -> ResultType<()> {
    std::process::Command::new("icacls")
        .arg(dir.as_os_str())
        .arg("/grant")
        .arg(format!("*S-1-1-0:(OI)(CI){}", permission))
        .arg("/T")
        .spawn()?;
    Ok(())
}

#[inline]
fn str_to_device_name(name: &str) -> [u16; 32] {
    let mut device_name: Vec<u16> = wide_string(name);
    if device_name.len() < 32 {
        device_name.resize(32, 0);
    }
    let mut result = [0; 32];
    result.copy_from_slice(&device_name[..32]);
    result
}

pub fn resolutions(name: &str) -> Vec<Resolution> {
    unsafe {
        let mut dm: DEVMODEW = std::mem::zeroed();
        let mut v = vec![];
        let mut num = 0;
        let device_name = str_to_device_name(name);
        loop {
            if EnumDisplaySettingsW(device_name.as_ptr(), num, &mut dm) == 0 {
                break;
            }
            let r = Resolution {
                width: dm.dmPelsWidth as _,
                height: dm.dmPelsHeight as _,
                ..Default::default()
            };
            if !v.contains(&r) {
                v.push(r);
            }
            num += 1;
        }
        v
    }
}

pub fn current_resolution(name: &str) -> ResultType<Resolution> {
    let device_name = str_to_device_name(name);
    unsafe {
        let mut dm: DEVMODEW = std::mem::zeroed();
        dm.dmSize = std::mem::size_of::<DEVMODEW>() as _;
        if EnumDisplaySettingsW(device_name.as_ptr(), ENUM_CURRENT_SETTINGS, &mut dm) == 0 {
            bail!(
                "failed to get current resolution, error {}",
                io::Error::last_os_error()
            );
        }
        let r = Resolution {
            width: dm.dmPelsWidth as _,
            height: dm.dmPelsHeight as _,
            ..Default::default()
        };
        Ok(r)
    }
}

pub(super) fn change_resolution_directly(
    name: &str,
    width: usize,
    height: usize,
) -> ResultType<()> {
    let device_name = str_to_device_name(name);
    unsafe {
        let mut dm: DEVMODEW = std::mem::zeroed();
        dm.dmSize = std::mem::size_of::<DEVMODEW>() as _;
        dm.dmPelsWidth = width as _;
        dm.dmPelsHeight = height as _;
        dm.dmFields = DM_PELSHEIGHT | DM_PELSWIDTH;
        let res = ChangeDisplaySettingsExW(
            device_name.as_ptr(),
            &mut dm,
            NULL as _,
            CDS_UPDATEREGISTRY | CDS_GLOBAL | CDS_RESET,
            NULL,
        );
        if res != DISP_CHANGE_SUCCESSFUL {
            bail!(
                "ChangeDisplaySettingsExW failed, res={}, error {}",
                res,
                io::Error::last_os_error()
            );
        }
        Ok(())
    }
}

pub fn user_accessible_folder() -> ResultType<PathBuf> {
    let disk = std::env::var("SystemDrive").unwrap_or("C:".to_string());
    let dir1 = PathBuf::from(format!("{}\\ProgramData", disk));
    // NOTICE: "C:\Windows\Temp" requires permanent authorization.
    let dir2 = PathBuf::from(format!("{}\\Windows\\Temp", disk));
    let dir;
    if dir1.exists() {
        dir = dir1;
    } else if dir2.exists() {
        dir = dir2;
    } else {
        bail!("no vaild user accessible folder");
    }
    Ok(dir)
}

#[inline]
pub fn uninstall_cert() -> ResultType<()> {
    cert::uninstall_cert()
}

mod cert {
    use hbb_common::ResultType;

    extern "C" {
        fn DeleteRustDeskTestCertsW();
    }
    pub fn uninstall_cert() -> ResultType<()> {
        unsafe {
            DeleteRustDeskTestCertsW();
        }
        Ok(())
    }
}

#[inline]
pub fn get_char_from_vk(vk: u32) -> Option<char> {
    get_char_from_unicode(get_unicode_from_vk(vk)?)
}

pub fn get_char_from_unicode(unicode: u16) -> Option<char> {
    let buff = [unicode];
    if let Some(chr) = String::from_utf16(&buff[..1]).ok()?.chars().next() {
        if chr.is_control() {
            return None;
        } else {
            Some(chr)
        }
    } else {
        None
    }
}

pub fn get_unicode_from_vk(vk: u32) -> Option<u16> {
    const BUF_LEN: i32 = 32;
    let mut buff = [0_u16; BUF_LEN as usize];
    let buff_ptr = buff.as_mut_ptr();
    let len = unsafe {
        let current_window_thread_id = GetWindowThreadProcessId(GetForegroundWindow(), null_mut());
        let layout = GetKeyboardLayout(current_window_thread_id);

        // refs: https://github.com/rustdesk-org/rdev/blob/25a99ce71ab42843ad253dd51e6a35e83e87a8a4/src/windows/keyboard.rs#L115
        let press_state = 129;
        let mut state: [BYTE; 256] = [0; 256];
        let shift_left = rdev::get_modifier(rdev::Key::ShiftLeft);
        let shift_right = rdev::get_modifier(rdev::Key::ShiftRight);
        if shift_left {
            state[VK_LSHIFT as usize] = press_state;
        }
        if shift_right {
            state[VK_RSHIFT as usize] = press_state;
        }
        if shift_left || shift_right {
            state[VK_SHIFT as usize] = press_state;
        }
        ToUnicodeEx(vk, 0x00, &state as _, buff_ptr, BUF_LEN, 0, layout)
    };
    if len == 1 {
        Some(buff[0])
    } else {
        None
    }
}

pub fn is_process_consent_running() -> ResultType<bool> {
    let output = std::process::Command::new("cmd")
        .args(&["/C", "tasklist | findstr consent.exe"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()?;
    Ok(output.status.success() && !output.stdout.is_empty())
}

pub struct WakeLock(u32);
// Failed to compile keepawake-rs on i686
impl WakeLock {
    pub fn new(display: bool, idle: bool, sleep: bool) -> Self {
        let mut flag = ES_CONTINUOUS;
        if display {
            flag |= ES_DISPLAY_REQUIRED;
        }
        if idle {
            flag |= ES_SYSTEM_REQUIRED;
        }
        if sleep {
            flag |= ES_AWAYMODE_REQUIRED;
        }
        unsafe { SetThreadExecutionState(flag) };
        WakeLock(flag)
    }

    pub fn set_display(&mut self, display: bool) -> ResultType<()> {
        let flag = if display {
            self.0 | ES_DISPLAY_REQUIRED
        } else {
            self.0 & !ES_DISPLAY_REQUIRED
        };
        if flag != self.0 {
            unsafe { SetThreadExecutionState(flag) };
            self.0 = flag;
        }
        Ok(())
    }
}

impl Drop for WakeLock {
    fn drop(&mut self) {
        unsafe { SetThreadExecutionState(ES_CONTINUOUS) };
    }
}

pub fn uninstall_service(show_new_window: bool, _: bool) -> bool {
    log::info!("Uninstalling service...");
    let filter = format!(" /FI \"PID ne {}\"", get_current_pid());
    Config::set_option("stop-service".into(), "Y".into());
    let cmds = format!(
        "
    chcp 65001
    sc stop {app_name}
    sc delete {app_name}
    if exist \"%PROGRAMDATA%\\Microsoft\\Windows\\Start Menu\\Programs\\Startup\\{app_name} Tray.lnk\" del /f /q \"%PROGRAMDATA%\\Microsoft\\Windows\\Start Menu\\Programs\\Startup\\{app_name} Tray.lnk\"
    taskkill /F /IM {broker_exe}
    taskkill /F /IM {app_name}.exe{filter}
    ",
        app_name = crate::get_app_name(),
        broker_exe = WIN_TOPMOST_INJECTED_PROCESS_EXE,
    );
    if let Err(err) = run_cmds(cmds, false, "uninstall") {
        Config::set_option("stop-service".into(), "".into());
        log::debug!("{err}");
        return true;
    }
    run_after_run_cmds(!show_new_window);
    std::process::exit(0);
}

pub fn install_service() -> bool {
    log::info!("Installing service...");
    let _installing = crate::platform::InstallingService::new();
    let (_, _, _, exe) = get_install_info();
    let tmp_path = std::env::temp_dir().to_string_lossy().to_string();
    let tray_shortcut = get_tray_shortcut(&exe, &tmp_path).unwrap_or_default();
    let filter = format!(" /FI \"PID ne {}\"", get_current_pid());
    Config::set_option("stop-service".into(), "".into());
    crate::ipc::EXIT_RECV_CLOSE.store(false, Ordering::Relaxed);
    let cmds = format!(
        "
chcp 65001
taskkill /F /IM {app_name}.exe{filter}
cscript \"{tray_shortcut}\"
copy /Y \"{tmp_path}\\{app_name} Tray.lnk\" \"%PROGRAMDATA%\\Microsoft\\Windows\\Start Menu\\Programs\\Startup\\\"
{import_config}
{create_service}
if exist \"{tray_shortcut}\" del /f /q \"{tray_shortcut}\"
    ",
        app_name = crate::get_app_name(),
        import_config = get_import_config(&exe),
        create_service = get_create_service(&exe),
    );
    if let Err(err) = run_cmds(cmds, false, "install") {
        Config::set_option("stop-service".into(), "Y".into());
        crate::ipc::EXIT_RECV_CLOSE.store(true, Ordering::Relaxed);
        log::debug!("{err}");
        return true;
    }
    run_after_run_cmds(false);
    std::process::exit(0);
}

pub fn get_tray_shortcut(exe: &str, tmp_path: &str) -> ResultType<String> {
    Ok(write_cmds(
        format!(
            "
Set oWS = WScript.CreateObject(\"WScript.Shell\")
sLinkFile = \"{tmp_path}\\{app_name} Tray.lnk\"

Set oLink = oWS.CreateShortcut(sLinkFile)
    oLink.TargetPath = \"{exe}\"
    oLink.Arguments = \"--tray\"
oLink.Save
        ",
            app_name = crate::get_app_name(),
        ),
        "vbs",
        "tray_shortcut",
    )?
    .to_str()
    .unwrap_or("")
    .to_owned())
}

fn get_import_config(exe: &str) -> String {
    if config::is_outgoing_only() {
        return "".to_string();
    }
    format!("
sc stop {app_name}
sc delete {app_name}
sc create {app_name} binpath= \"\\\"{exe}\\\" --import-config \\\"{config_path}\\\"\" start= auto DisplayName= \"{app_name} Service\"
sc start {app_name}
sc stop {app_name}
sc delete {app_name}
",
    app_name = crate::get_app_name(),
    config_path=Config::file().to_str().unwrap_or(""),
)
}

fn get_create_service(exe: &str) -> String {
    if config::is_outgoing_only() {
        return "".to_string();
    }
    let stop = Config::get_option("stop-service") == "Y";
    if stop {
        format!("
if exist \"%PROGRAMDATA%\\Microsoft\\Windows\\Start Menu\\Programs\\Startup\\{app_name} Tray.lnk\" del /f /q \"%PROGRAMDATA%\\Microsoft\\Windows\\Start Menu\\Programs\\Startup\\{app_name} Tray.lnk\"
", app_name = crate::get_app_name())
    } else {
        format!("
sc create {app_name} binpath= \"\\\"{exe}\\\" --service\" start= auto DisplayName= \"{app_name} Service\"
sc start {app_name}
",
    app_name = crate::get_app_name())
    }
}

fn run_after_run_cmds(silent: bool) {
    let (_, _, _, exe) = get_install_info();
    if !silent {
        log::debug!("Spawn new window");
        allow_err!(std::process::Command::new("cmd")
            .args(&["/c", "timeout", "/t", "2", "&", &format!("{exe}")])
            .creation_flags(winapi::um::winbase::CREATE_NO_WINDOW)
            .spawn());
    }
    if Config::get_option("stop-service") != "Y" {
        allow_err!(std::process::Command::new(&exe).arg("--tray").spawn());
    }
    std::thread::sleep(std::time::Duration::from_millis(300));
}

#[inline]
pub fn try_kill_broker() {
    allow_err!(std::process::Command::new("cmd")
        .arg("/c")
        .arg(&format!(
            "taskkill /F /IM {}",
            WIN_TOPMOST_INJECTED_PROCESS_EXE
        ))
        .creation_flags(winapi::um::winbase::CREATE_NO_WINDOW)
        .spawn());
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_uninstall_cert() {
        println!("uninstall driver certs: {:?}", cert::uninstall_cert());
    }

    #[test]
    fn test_get_unicode_char_by_vk() {
        let chr = get_char_from_vk(0x41); // VK_A
        assert_eq!(chr, Some('a'));
        let chr = get_char_from_vk(VK_ESCAPE as u32); // VK_ESC
        assert_eq!(chr, None)
    }
}

pub fn message_box(text: &str) {
    let mut text = text.to_owned();
    let nodialog = std::env::var("NO_DIALOG").unwrap_or_default() == "Y";
    if !text.ends_with("!") || nodialog {
        use arboard::Clipboard as ClipboardContext;
        match ClipboardContext::new() {
            Ok(mut ctx) => {
                ctx.set_text(&text).ok();
                if !nodialog {
                    text = format!("{}\n\nAbove text has been copied to clipboard", &text);
                }
            }
            _ => {}
        }
    }
    if nodialog {
        if std::env::var("PRINT_OUT").unwrap_or_default() == "Y" {
            println!("{text}");
        }
        if let Ok(x) = std::env::var("WRITE_TO_FILE") {
            if !x.is_empty() {
                allow_err!(std::fs::write(x, text));
            }
        }
        return;
    }
    let text = text
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<u16>>();
    let caption = "RustDesk Output"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<u16>>();
    unsafe { MessageBoxW(std::ptr::null_mut(), text.as_ptr(), caption.as_ptr(), MB_OK) };
}

pub fn alloc_console() {
    unsafe {
        alloc_console_and_redirect();
    }
}

fn get_license() -> Option<CustomServer> {
    let mut lic: CustomServer = Default::default();
    if let Ok(tmp) = get_license_from_exe_name() {
        lic = tmp;
    } else {
        // for back compatibility from migrating from <= 1.2.1 to 1.2.2
        lic.key = get_reg("Key");
        lic.host = get_reg("Host");
        lic.api = get_reg("Api");
    }
    if lic.key.is_empty() || lic.host.is_empty() {
        return None;
    }
    Some(lic)
}

pub struct WallPaperRemover {
    old_path: String,
}

impl WallPaperRemover {
    pub fn new() -> ResultType<Self> {
        let start = std::time::Instant::now();
        if !Self::need_remove() {
            bail!("already solid color");
        }
        let old_path = match Self::get_recent_wallpaper() {
            Ok(old_path) => old_path,
            Err(e) => {
                log::info!("Failed to get recent wallpaper: {:?}, use fallback", e);
                wallpaper::get().map_err(|e| anyhow!(e.to_string()))?
            }
        };
        Self::set_wallpaper(None)?;
        log::info!(
            "created wallpaper remover,  old_path: {:?},  elapsed: {:?}",
            old_path,
            start.elapsed(),
        );
        Ok(Self { old_path })
    }

    pub fn support() -> bool {
        wallpaper::get().is_ok() || !Self::get_recent_wallpaper().unwrap_or_default().is_empty()
    }

    fn get_recent_wallpaper() -> ResultType<String> {
        // SystemParametersInfoW may return %appdata%\Microsoft\Windows\Themes\TranscodedWallpaper, not real path and may not real cache
        // https://www.makeuseof.com/find-desktop-wallpapers-file-location-windows-11/
        // https://superuser.com/questions/1218413/write-to-current-users-registry-through-a-different-admin-account
        let (hkcu, sid) = if is_root() {
            let sid = get_current_process_session_id().ok_or(anyhow!("failed to get sid"))?;
            (RegKey::predef(HKEY_USERS), format!("{}\\", sid))
        } else {
            (RegKey::predef(HKEY_CURRENT_USER), "".to_string())
        };
        let explorer_key = hkcu.open_subkey_with_flags(
            &format!(
                "{}Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Wallpapers",
                sid
            ),
            KEY_READ,
        )?;
        Ok(explorer_key.get_value("BackgroundHistoryPath0")?)
    }

    fn need_remove() -> bool {
        if let Ok(wallpaper) = wallpaper::get() {
            return !wallpaper.is_empty();
        }
        false
    }

    fn set_wallpaper(path: Option<String>) -> ResultType<()> {
        wallpaper::set_from_path(&path.unwrap_or_default()).map_err(|e| anyhow!(e.to_string()))
    }
}

impl Drop for WallPaperRemover {
    fn drop(&mut self) {
        // If the old background is a slideshow, it will be converted into an image. AnyDesk does the same.
        allow_err!(Self::set_wallpaper(Some(self.old_path.clone())));
    }
}

fn get_uninstall_amyuni_idd() -> String {
    match std::env::current_exe() {
        Ok(path) => format!("\"{}\" --uninstall-amyuni-idd", path.to_str().unwrap_or("")),
        Err(e) => {
            log::warn!("Failed to get current exe path, cannot get command of uninstalling idd, Zzerror: {:?}", e);
            "".to_string()
        }
    }
}

#[inline]
pub fn is_self_service_running() -> bool {
    is_service_running(&crate::get_app_name())
}

pub fn is_service_running(service_name: &str) -> bool {
    unsafe {
        let service_name = wide_string(service_name);
        is_service_running_w(service_name.as_ptr() as _)
    }
}

pub fn is_x64() -> bool {
    const PROCESSOR_ARCHITECTURE_AMD64: u16 = 9;

    let mut sys_info = SYSTEM_INFO::default();
    unsafe {
        GetNativeSystemInfo(&mut sys_info as _);
    }
    unsafe { sys_info.u.s().wProcessorArchitecture == PROCESSOR_ARCHITECTURE_AMD64 }
}

pub fn try_kill_rustdesk_main_window_process() -> ResultType<()> {
    // Kill rustdesk.exe without extra arg, should only be called by --server
    // We can find the exact process which occupies the ipc, see more from https://github.com/winsiderss/systeminformer
    log::info!("try kill rustdesk main window process");
    use hbb_common::sysinfo::System;
    let mut sys = System::new();
    sys.refresh_processes();
    let my_uid = sys
        .process((std::process::id() as usize).into())
        .map(|x| x.user_id())
        .unwrap_or_default();
    let my_pid = std::process::id();
    let app_name = crate::get_app_name().to_lowercase();
    if app_name.is_empty() {
        bail!("app name is empty");
    }
    for (_, p) in sys.processes().iter() {
        let p_name = p.name().to_lowercase();
        // name equal
        if !(p_name == app_name || p_name == app_name.clone() + ".exe") {
            continue;
        }
        // arg more than 1
        if p.cmd().len() < 1 {
            continue;
        }
        // first arg contain app name
        if !p.cmd()[0].to_lowercase().contains(&p_name) {
            continue;
        }
        // only one arg or the second arg is empty uni link
        let is_empty_uni = p.cmd().len() == 2 && crate::common::is_empty_uni_link(&p.cmd()[1]);
        if !(p.cmd().len() == 1 || is_empty_uni) {
            continue;
        }
        // skip self
        if p.pid().as_u32() == my_pid {
            continue;
        }
        // because we call it with --server, so we can check user_id, remove this if call it with user process
        if p.user_id() == my_uid {
            log::info!("user id equal, continue");
            continue;
        }
        log::info!("try kill process: {:?}, pid = {:?}", p.cmd(), p.pid());
        nt_terminate_process(p.pid().as_u32())?;
        log::info!("kill process success: {:?}, pid = {:?}", p.cmd(), p.pid());
        return Ok(());
    }
    bail!("failed to find rustdesk main window process");
}

fn nt_terminate_process(process_id: DWORD) -> ResultType<()> {
    type NtTerminateProcess = unsafe extern "system" fn(HANDLE, DWORD) -> DWORD;
    unsafe {
        let h_module = LoadLibraryExA(
            CString::new("ntdll.dll")?.as_ptr(),
            std::ptr::null_mut(),
            LOAD_LIBRARY_SEARCH_SYSTEM32,
        );
        if !h_module.is_null() {
            let f_nt_terminate_process: NtTerminateProcess = std::mem::transmute(GetProcAddress(
                h_module,
                CString::new("NtTerminateProcess")?.as_ptr(),
            ));
            let h_token = OpenProcess(PROCESS_ALL_ACCESS, 0, process_id);
            if !h_token.is_null() {
                if f_nt_terminate_process(h_token, 1) == 0 {
                    log::info!("terminate process {} success", process_id);
                    CloseHandle(h_token);
                    return Ok(());
                } else {
                    CloseHandle(h_token);
                    bail!("NtTerminateProcess {} failed", process_id);
                }
            } else {
                bail!("OpenProcess {} failed", process_id);
            }
        } else {
            bail!("Failed to load ntdll.dll");
        }
    }
}

pub fn try_set_window_foreground(window: HWND) {
    let env_key = SET_FOREGROUND_WINDOW;
    if let Ok(value) = std::env::var(env_key) {
        if value == "1" {
            unsafe {
                SetForegroundWindow(window);
            }
            std::env::remove_var(env_key);
        }
    }
}

pub mod reg_display_settings {
    use hbb_common::ResultType;
    use serde_derive::{Deserialize, Serialize};
    use std::collections::HashMap;
    use winreg::{enums::*, RegValue};
    const REG_GRAPHICS_DRIVERS_PATH: &str = "SYSTEM\\CurrentControlSet\\Control\\GraphicsDrivers";
    const REG_CONNECTIVITY_PATH: &str = "Connectivity";

    #[derive(Serialize, Deserialize, Debug)]
    pub struct RegRecovery {
        path: String,
        key: String,
        old: (Vec<u8>, isize),
        new: (Vec<u8>, isize),
    }

    pub fn read_reg_connectivity() -> ResultType<HashMap<String, HashMap<String, RegValue>>> {
        let hklm = winreg::RegKey::predef(HKEY_LOCAL_MACHINE);
        let reg_connectivity = hklm.open_subkey_with_flags(
            format!("{}\\{}", REG_GRAPHICS_DRIVERS_PATH, REG_CONNECTIVITY_PATH),
            KEY_READ,
        )?;

        let mut map_connectivity = HashMap::new();
        for key in reg_connectivity.enum_keys() {
            let key = key?;
            let mut map_item = HashMap::new();
            let reg_item = reg_connectivity.open_subkey_with_flags(&key, KEY_READ)?;
            for value in reg_item.enum_values() {
                let (name, value) = value?;
                map_item.insert(name, value);
            }
            map_connectivity.insert(key, map_item);
        }
        Ok(map_connectivity)
    }

    pub fn diff_recent_connectivity(
        map1: HashMap<String, HashMap<String, RegValue>>,
        map2: HashMap<String, HashMap<String, RegValue>>,
    ) -> Option<RegRecovery> {
        for (subkey, map_item2) in map2 {
            if let Some(map_item1) = map1.get(&subkey) {
                let key = "Recent";
                if let Some(value1) = map_item1.get(key) {
                    if let Some(value2) = map_item2.get(key) {
                        if value1 != value2 {
                            return Some(RegRecovery {
                                path: format!(
                                    "{}\\{}\\{}",
                                    REG_GRAPHICS_DRIVERS_PATH, REG_CONNECTIVITY_PATH, subkey
                                ),
                                key: key.to_owned(),
                                old: (value1.bytes.clone(), value1.vtype.clone() as isize),
                                new: (value2.bytes.clone(), value2.vtype.clone() as isize),
                            });
                        }
                    }
                }
            }
        }
        None
    }

    pub fn restore_reg_connectivity(reg_recovery: RegRecovery) -> ResultType<()> {
        let hklm = winreg::RegKey::predef(HKEY_LOCAL_MACHINE);
        let reg_item = hklm.open_subkey_with_flags(&reg_recovery.path, KEY_READ | KEY_WRITE)?;
        let cur_reg_value = reg_item.get_raw_value(&reg_recovery.key)?;
        let new_reg_value = RegValue {
            bytes: reg_recovery.new.0,
            vtype: isize_to_reg_type(reg_recovery.new.1),
        };
        if cur_reg_value != new_reg_value {
            return Ok(());
        }
        let reg_value = RegValue {
            bytes: reg_recovery.old.0,
            vtype: isize_to_reg_type(reg_recovery.old.1),
        };
        reg_item.set_raw_value(&reg_recovery.key, &reg_value)?;
        Ok(())
    }

    #[inline]
    fn isize_to_reg_type(i: isize) -> RegType {
        match i {
            0 => RegType::REG_NONE,
            1 => RegType::REG_SZ,
            2 => RegType::REG_EXPAND_SZ,
            3 => RegType::REG_BINARY,
            4 => RegType::REG_DWORD,
            5 => RegType::REG_DWORD_BIG_ENDIAN,
            6 => RegType::REG_LINK,
            7 => RegType::REG_MULTI_SZ,
            8 => RegType::REG_RESOURCE_LIST,
            9 => RegType::REG_FULL_RESOURCE_DESCRIPTOR,
            10 => RegType::REG_RESOURCE_REQUIREMENTS_LIST,
            11 => RegType::REG_QWORD,
            _ => RegType::REG_NONE,
        }
    }
}

pub fn get_printer_names() -> ResultType<Vec<String>> {
    let mut needed_bytes = 0;
    let mut returned_count = 0;

    unsafe {
        // First call to get required buffer size
        EnumPrintersW(
            PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS,
            std::ptr::null_mut(),
            1,
            std::ptr::null_mut(),
            0,
            &mut needed_bytes,
            &mut returned_count,
        );

        let mut buffer = vec![0u8; needed_bytes as usize];

        if EnumPrintersW(
            PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS,
            std::ptr::null_mut(),
            1,
            buffer.as_mut_ptr() as *mut _,
            needed_bytes,
            &mut needed_bytes,
            &mut returned_count,
        ) == 0
        {
            return Err(anyhow!("Failed to enumerate printers"));
        }

        let ptr = buffer.as_ptr() as *const PRINTER_INFO_1W;
        let printers = std::slice::from_raw_parts(ptr, returned_count as usize);

        Ok(printers
            .iter()
            .filter_map(|p| {
                let name = p.pName;
                if !name.is_null() {
                    let mut len = 0;
                    while len < 500 {
                        if name.add(len).is_null() || *name.add(len) == 0 {
                            break;
                        }
                        len += 1;
                    }
                    if len > 0 && len < 500 {
                        Some(String::from_utf16_lossy(std::slice::from_raw_parts(
                            name, len,
                        )))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect())
    }
}

extern "C" {
    fn PrintXPSRawData(printer_name: *const u16, raw_data: *const u8, data_size: c_ulong) -> DWORD;
}

pub fn send_raw_data_to_printer(printer_name: Option<String>, data: Vec<u8>) -> ResultType<()> {
    let mut printer_name = printer_name.unwrap_or_default();
    if printer_name.is_empty() {
        // use GetDefaultPrinter to get the default printer name
        let mut needed_bytes = 0;
        unsafe {
            GetDefaultPrinterW(std::ptr::null_mut(), &mut needed_bytes);
        }
        if needed_bytes > 0 {
            let mut default_printer_name = vec![0u16; needed_bytes as usize];
            unsafe {
                GetDefaultPrinterW(
                    default_printer_name.as_mut_ptr() as *mut _,
                    &mut needed_bytes,
                );
            }
            printer_name = String::from_utf16_lossy(&default_printer_name[..needed_bytes as usize]);
        }
    } else {
        if let Ok(names) = crate::platform::windows::get_printer_names() {
            if !names.contains(&printer_name) {
                // Don't set the first printer as current printer.
                // It may not be the desired printer.
                bail!("Printer name \"{}\" not found", &printer_name);
            }
        }
    }
    if printer_name.is_empty() {
        return Err(anyhow!("Failed to get printer name"));
    }

    log::info!("Sending data to printer: {}", &printer_name);
    let printer_name = wide_string(&printer_name);
    unsafe {
        let res = PrintXPSRawData(
            printer_name.as_ptr(),
            data.as_ptr() as *const u8,
            data.len() as c_ulong,
        );
        if res != 0 {
            bail!("Failed to send data to the printer, see logs in C:\\Windows\\temp\\test_rustdesk.log for more details.");
        } else {
            log::info!("Successfully sent data to the printer");
        }
    }

    Ok(())
}
