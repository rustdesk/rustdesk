use super::{PrivacyMode, PrivacyModeState, INVALID_PRIVACY_MODE_CONN_ID};
use crate::platform::windows::{get_user_token, is_root};
use hbb_common::{allow_err, bail, log, ResultType};
use lazy_static::lazy_static;
use std::{
    io::Error,
    mem,
    ptr::{null, null_mut},
    sync::Mutex,
    time::{Duration, Instant},
};
use winapi::{
    shared::{
        minwindef::{BOOL, DWORD, FALSE, HINSTANCE, LPARAM, LRESULT, TRUE, UINT, WPARAM},
        ntdef::NULL,
        windef::{HBRUSH, HDC, HMONITOR, HWND, LPRECT, RECT},
        winerror::ERROR_CLASS_ALREADY_EXISTS,
    },
    um::{
        errhandlingapi::GetLastError,
        handleapi::CloseHandle,
        libloaderapi::GetModuleHandleW,
        processthreadsapi::{
            CreateProcessAsUserW, CreateProcessW, GetExitCodeProcess, TerminateProcess,
            PROCESS_INFORMATION, STARTUPINFOW,
        },
        winbase::{WTSGetActiveConsoleSessionId, DETACHED_PROCESS},
        wingdi::{GetStockObject, BLACK_BRUSH},
        winuser::*,
    },
};

pub(super) const PRIVACY_MODE_IMPL: &str = super::PRIVACY_MODE_IMPL_WIN_PRIVACY_SCREEN;

const HELPER_ARG: &str = "--privacy-screen";
const OVERLAY_WINDOW_NAME: &str = "RustDeskPrivacyScreenWindow";
const OVERLAY_WINDOW_CLASS: &str = "RustDeskPrivacyScreenWindowClass";
const CONTROLLER_WINDOW_NAME: &str = "RustDeskPrivacyScreenController";
const CONTROLLER_WINDOW_CLASS: &str = "RustDeskPrivacyScreenControllerClass";

const WDA_EXCLUDEFROMCAPTURE: DWORD = 0x00000011;
const STILL_ACTIVE: DWORD = 259;
const WM_PRIVACY_SCREEN_REFRESH: UINT = WM_USER + 0x508;

lazy_static! {
    static ref OVERLAY_WINDOWS: Mutex<Vec<isize>> = Mutex::new(Vec::new());
    static ref CONTROLLER_WINDOW: Mutex<isize> = Mutex::new(0);
}

struct WindowHandlers {
    hthread: u64,
    hprocess: u64,
}

impl Drop for WindowHandlers {
    fn drop(&mut self) {
        self.reset();
    }
}

impl WindowHandlers {
    fn reset(&mut self) {
        unsafe {
            if self.hprocess != 0 {
                request_helper_close();
                std::thread::sleep(Duration::from_millis(200));

                let mut exit_code: DWORD = 0;
                if FALSE != GetExitCodeProcess(self.hprocess as _, &mut exit_code as _)
                    && exit_code == STILL_ACTIVE
                    && FALSE == TerminateProcess(self.hprocess as _, 0)
                {
                    log::warn!(
                        "Failed to terminate privacy screen helper, error {}",
                        Error::last_os_error()
                    );
                }
                if FALSE == CloseHandle(self.hprocess as _) {
                    log::warn!(
                        "Failed to close privacy screen helper process handle, error {}",
                        Error::last_os_error()
                    );
                }
                self.hprocess = 0;
            }
            if self.hthread != 0 {
                if FALSE == CloseHandle(self.hthread as _) {
                    log::warn!(
                        "Failed to close privacy screen helper thread handle, error {}",
                        Error::last_os_error()
                    );
                }
                self.hthread = 0;
            }
        }
    }

    fn is_default(&self) -> bool {
        self.hthread == 0 && self.hprocess == 0
    }
}

pub struct PrivacyModeImpl {
    impl_key: String,
    conn_id: i32,
    handlers: WindowHandlers,
}

impl PrivacyModeImpl {
    pub fn new(impl_key: &str) -> Self {
        Self {
            impl_key: impl_key.to_owned(),
            conn_id: INVALID_PRIVACY_MODE_CONN_ID,
            handlers: WindowHandlers {
                hthread: 0,
                hprocess: 0,
            },
        }
    }

    fn start(&mut self) -> ResultType<()> {
        if !self.handlers.is_default() {
            return Ok(());
        }

        let exe_file = std::env::current_exe()?;
        let cmdline = format!("\"{}\" {}", exe_file.to_string_lossy(), HELPER_ARG);

        request_helper_close();
        std::thread::sleep(Duration::from_millis(200));

        let proc_info = if is_root() {
            start_helper_as_active_user(&cmdline)?
        } else {
            start_helper_direct(&cmdline)?
        };

        self.handlers.hthread = proc_info.hThread as _;
        self.handlers.hprocess = proc_info.hProcess as _;
        if wait_find_controller_window(Duration::from_secs(3)).is_null() {
            self.handlers.reset();
            bail!("No privacy screen helper window created");
        }
        self.check_helper_running()
    }

    fn check_helper_running(&mut self) -> ResultType<()> {
        let mut exit_code: DWORD = 0;
        unsafe {
            if FALSE == GetExitCodeProcess(self.handlers.hprocess as _, &mut exit_code as _) {
                bail!(
                    "Failed to get privacy screen helper status, error {}",
                    Error::last_os_error()
                );
            }
        }
        if exit_code != STILL_ACTIVE {
            self.handlers.reset();
            bail!("Privacy screen helper exited with code {}", exit_code);
        }
        Ok(())
    }
}

impl PrivacyMode for PrivacyModeImpl {
    fn is_async_privacy_mode(&self) -> bool {
        false
    }

    fn init(&self) -> ResultType<()> {
        Ok(())
    }

    fn clear(&mut self) {
        allow_err!(self.turn_off_privacy(self.conn_id, None));
    }

    fn turn_on_privacy(&mut self, conn_id: i32) -> ResultType<bool> {
        if self.check_on_conn_id(conn_id)? {
            log::debug!("Privacy screen mode of conn {} is already on", conn_id);
            return Ok(true);
        }

        super::win_input::hook()?;
        if let Err(e) = self.start() {
            allow_err!(super::win_input::unhook());
            bail!("Failed to start privacy screen mode: {}", e);
        }
        self.conn_id = conn_id;
        Ok(true)
    }

    fn turn_off_privacy(
        &mut self,
        conn_id: i32,
        state: Option<PrivacyModeState>,
    ) -> ResultType<()> {
        self.check_off_conn_id(conn_id)?;
        self.handlers.reset();
        super::win_input::unhook()?;

        if self.conn_id != INVALID_PRIVACY_MODE_CONN_ID {
            if let Some(state) = state {
                allow_err!(super::set_privacy_mode_state(
                    conn_id,
                    state,
                    PRIVACY_MODE_IMPL.to_string(),
                    1_000
                ));
            }
            self.conn_id = INVALID_PRIVACY_MODE_CONN_ID;
        }

        Ok(())
    }

    #[inline]
    fn pre_conn_id(&self) -> i32 {
        self.conn_id
    }

    #[inline]
    fn get_impl_key(&self) -> &str {
        &self.impl_key
    }
}

impl Drop for PrivacyModeImpl {
    fn drop(&mut self) {
        if self.conn_id != INVALID_PRIVACY_MODE_CONN_ID {
            allow_err!(self.turn_off_privacy(self.conn_id, None));
        }
    }
}

pub fn is_supported() -> bool {
    super::win_exclude_from_capture::is_supported()
}

pub fn run_helper() -> ResultType<()> {
    if !is_supported() {
        bail!("Privacy screen mode requires Windows 10, version 2004 or newer");
    }
    let instance = unsafe { GetModuleHandleW(null()) };
    if instance.is_null() {
        bail!(
            "Failed to get module handle, error {}",
            Error::last_os_error()
        );
    }

    register_window_classes(instance)?;
    let controller = create_controller_window(instance)?;
    *CONTROLLER_WINDOW.lock().unwrap() = controller as _;

    refresh_overlay_windows(instance)?;

    let mut msg = unsafe { mem::zeroed::<MSG>() };
    unsafe {
        while FALSE != GetMessageW(&mut msg as _, NULL as _, 0, 0) {
            TranslateMessage(&msg as _);
            DispatchMessageW(&msg as _);
        }
    }
    destroy_overlay_windows();
    Ok(())
}

fn start_helper_direct(cmdline: &str) -> ResultType<PROCESS_INFORMATION> {
    let mut start_info = startup_info();
    let mut proc_info = empty_process_info();
    let mut cmd_utf16 = wide(cmdline);
    unsafe {
        if FALSE
            == CreateProcessW(
                null(),
                cmd_utf16.as_mut_ptr(),
                null_mut(),
                null_mut(),
                FALSE,
                DETACHED_PROCESS,
                null_mut(),
                null(),
                &mut start_info as _,
                &mut proc_info as _,
            )
        {
            bail!(
                "Failed to create privacy screen helper process, error {}",
                Error::last_os_error()
            );
        }
    }
    Ok(proc_info)
}

fn start_helper_as_active_user(cmdline: &str) -> ResultType<PROCESS_INFORMATION> {
    let mut start_info = startup_info();
    let mut proc_info = empty_process_info();
    let mut cmd_utf16 = wide(cmdline);
    unsafe {
        let session_id = WTSGetActiveConsoleSessionId();
        let token = get_user_token(session_id, true);
        if token.is_null() {
            bail!("Failed to get token of current user");
        }
        let create_res = CreateProcessAsUserW(
            token,
            null(),
            cmd_utf16.as_mut_ptr(),
            null_mut(),
            null_mut(),
            FALSE,
            DETACHED_PROCESS,
            null_mut(),
            null(),
            &mut start_info as _,
            &mut proc_info as _,
        );
        CloseHandle(token);
        if FALSE == create_res {
            bail!(
                "Failed to create privacy screen helper process as user, error {}",
                Error::last_os_error()
            );
        }
    }
    Ok(proc_info)
}

fn startup_info() -> STARTUPINFOW {
    STARTUPINFOW {
        cb: mem::size_of::<STARTUPINFOW>() as _,
        lpReserved: null_mut(),
        lpDesktop: null_mut(),
        lpTitle: null_mut(),
        dwX: 0,
        dwY: 0,
        dwXSize: 0,
        dwYSize: 0,
        dwXCountChars: 0,
        dwYCountChars: 0,
        dwFillAttribute: 0,
        dwFlags: 0,
        wShowWindow: 0,
        cbReserved2: 0,
        lpReserved2: null_mut(),
        hStdInput: null_mut(),
        hStdOutput: null_mut(),
        hStdError: null_mut(),
    }
}

fn empty_process_info() -> PROCESS_INFORMATION {
    PROCESS_INFORMATION {
        hProcess: null_mut(),
        hThread: null_mut(),
        dwProcessId: 0,
        dwThreadId: 0,
    }
}

fn register_window_classes(instance: HINSTANCE) -> ResultType<()> {
    register_window_class(instance, CONTROLLER_WINDOW_CLASS, Some(controller_wnd_proc))?;
    register_window_class(instance, OVERLAY_WINDOW_CLASS, Some(overlay_wnd_proc))
}

fn register_window_class(
    instance: HINSTANCE,
    class_name: &str,
    wnd_proc: WNDPROC,
) -> ResultType<()> {
    let class_name_w = wide(class_name);
    let wnd_class = WNDCLASSEXW {
        cbSize: mem::size_of::<WNDCLASSEXW>() as _,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: wnd_proc,
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: instance,
        hIcon: null_mut(),
        hCursor: unsafe { LoadCursorW(NULL as _, IDC_ARROW as _) },
        hbrBackground: null_mut(),
        lpszMenuName: null(),
        lpszClassName: class_name_w.as_ptr(),
        hIconSm: null_mut(),
    };
    unsafe {
        if 0 == RegisterClassExW(&wnd_class as _) {
            let code = GetLastError();
            if code != ERROR_CLASS_ALREADY_EXISTS {
                bail!(
                    "Failed to register privacy screen window class {}, error {}",
                    class_name,
                    Error::from_raw_os_error(code as _)
                );
            }
        }
    }
    Ok(())
}

fn create_controller_window(instance: HINSTANCE) -> ResultType<HWND> {
    let class_name = wide(CONTROLLER_WINDOW_CLASS);
    let window_name = wide(CONTROLLER_WINDOW_NAME);
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            class_name.as_ptr(),
            window_name.as_ptr(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            NULL as _,
            NULL as _,
            instance,
            null_mut(),
        )
    };
    if hwnd.is_null() {
        bail!(
            "Failed to create privacy screen controller window, error {}",
            Error::last_os_error()
        );
    }
    Ok(hwnd)
}

fn refresh_overlay_windows(instance: HINSTANCE) -> ResultType<()> {
    destroy_overlay_windows();

    let monitors = enum_monitor_rects()?;
    if monitors.is_empty() {
        bail!("No monitor found for privacy screen mode");
    }

    let mut windows = OVERLAY_WINDOWS.lock().unwrap();
    for rect in monitors {
        let hwnd = create_overlay_window(instance, rect)?;
        windows.push(hwnd as _);
    }
    Ok(())
}

fn destroy_overlay_windows() {
    let mut windows = OVERLAY_WINDOWS.lock().unwrap();
    for hwnd in windows.drain(..) {
        unsafe {
            if !((hwnd as HWND).is_null()) {
                DestroyWindow(hwnd as _);
            }
        }
    }
}

fn request_helper_close() {
    let hwnd = find_controller_window();
    if !hwnd.is_null() {
        unsafe {
            if FALSE == PostMessageW(hwnd, WM_CLOSE, 0, 0) {
                log::warn!(
                    "Failed to close privacy screen helper window, error {}",
                    Error::last_os_error()
                );
            }
        }
    }
}

fn wait_find_controller_window(timeout: Duration) -> HWND {
    let start = Instant::now();
    loop {
        let hwnd = find_controller_window();
        if !hwnd.is_null() {
            return hwnd;
        }
        if start.elapsed() >= timeout {
            return null_mut();
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn find_controller_window() -> HWND {
    let class_name = wide(CONTROLLER_WINDOW_CLASS);
    let window_name = wide(CONTROLLER_WINDOW_NAME);
    unsafe { FindWindowW(class_name.as_ptr(), window_name.as_ptr()) }
}

fn enum_monitor_rects() -> ResultType<Vec<RECT>> {
    let mut monitors = Vec::<RECT>::new();
    unsafe {
        if FALSE
            == EnumDisplayMonitors(
                NULL as _,
                null_mut(),
                Some(enum_monitor_proc),
                &mut monitors as *mut _ as _,
            )
        {
            bail!(
                "Failed to enumerate monitors, error {}",
                Error::last_os_error()
            );
        }
    }
    Ok(monitors)
}

unsafe extern "system" fn enum_monitor_proc(
    hmonitor: HMONITOR,
    _hdc: HDC,
    _rect: LPRECT,
    data: LPARAM,
) -> BOOL {
    let monitors = &mut *(data as *mut Vec<RECT>);
    let mut mi = mem::zeroed::<MONITORINFO>();
    mi.cbSize = mem::size_of::<MONITORINFO>() as _;
    if FALSE != GetMonitorInfoW(hmonitor, &mut mi as _) {
        monitors.push(mi.rcMonitor);
    }
    TRUE
}

fn create_overlay_window(instance: HINSTANCE, rect: RECT) -> ResultType<HWND> {
    let class_name = wide(OVERLAY_WINDOW_CLASS);
    let window_name = wide(OVERLAY_WINDOW_NAME);
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT | WS_EX_LAYERED,
            class_name.as_ptr(),
            window_name.as_ptr(),
            WS_POPUP,
            rect.left,
            rect.top,
            width,
            height,
            NULL as _,
            NULL as _,
            instance,
            null_mut(),
        )
    };
    if hwnd.is_null() {
        bail!(
            "Failed to create privacy screen overlay window, error {}",
            Error::last_os_error()
        );
    }

    unsafe {
        if FALSE == SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA) {
            DestroyWindow(hwnd);
            bail!(
                "Failed to set privacy screen overlay alpha, error {}",
                Error::last_os_error()
            );
        }
        if FALSE == SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE) {
            DestroyWindow(hwnd);
            bail!(
                "Failed to exclude privacy screen overlay from capture, error {}",
                Error::last_os_error()
            );
        }
        if FALSE
            == SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                rect.left,
                rect.top,
                width,
                height,
                SWP_SHOWWINDOW | SWP_NOACTIVATE,
            )
        {
            DestroyWindow(hwnd);
            bail!(
                "Failed to show privacy screen overlay, error {}",
                Error::last_os_error()
            );
        }
        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);
    }

    Ok(hwnd)
}

unsafe extern "system" fn controller_wnd_proc(
    hwnd: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_DISPLAYCHANGE | WM_SETTINGCHANGE => {
            let _ = refresh_overlay_windows(GetModuleHandleW(null()))
                .map_err(|e| log::error!("Failed to refresh privacy screen overlays: {}", e));
            0
        }
        WM_PRIVACY_SCREEN_REFRESH => {
            let _ = refresh_overlay_windows(GetModuleHandleW(null()))
                .map_err(|e| log::error!("Failed to refresh privacy screen overlays: {}", e));
            0
        }
        WM_CLOSE => {
            destroy_overlay_windows();
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe extern "system" fn overlay_wnd_proc(
    hwnd: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_NCHITTEST => HTTRANSPARENT as _,
        WM_ERASEBKGND => {
            fill_black(hwnd, wparam as HDC);
            1
        }
        WM_PAINT => {
            let mut ps = mem::zeroed::<PAINTSTRUCT>();
            let hdc = BeginPaint(hwnd, &mut ps as _);
            if !hdc.is_null() {
                fill_black(hwnd, hdc);
                EndPaint(hwnd, &ps as _);
            }
            0
        }
        WM_DISPLAYCHANGE | WM_SETTINGCHANGE => {
            let controller = *CONTROLLER_WINDOW.lock().unwrap();
            if controller != 0 {
                PostMessageW(controller as _, WM_PRIVACY_SCREEN_REFRESH, 0, 0);
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn fill_black(hwnd: HWND, hdc: HDC) {
    let mut rect = mem::zeroed::<RECT>();
    if FALSE != GetClientRect(hwnd, &mut rect as _) {
        FillRect(
            hdc,
            &rect as _,
            GetStockObject(BLACK_BRUSH as i32) as HBRUSH,
        );
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
