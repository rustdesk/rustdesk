use crate::platform::screen_frame::Style;
use std::{
    mem,
    ptr::{null, null_mut},
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
};
use winapi::{
    shared::{
        minwindef::{BOOL, DWORD, FALSE, HINSTANCE, LPARAM, LRESULT, TRUE, UINT, WPARAM},
        windef::{HDC, HMONITOR, HWINEVENTHOOK, HWND, LPRECT, RECT},
        winerror::ERROR_CLASS_ALREADY_EXISTS,
    },
    um::{
        errhandlingapi::GetLastError,
        libloaderapi::GetModuleHandleW,
        processthreadsapi::GetCurrentThreadId,
        wingdi::{CombineRgn, CreateRectRgn, CreateSolidBrush, DeleteObject, RGB, RGN_DIFF},
        winuser::{
            BeginPaint, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, EndPaint,
            EnumDisplayMonitors, FillRect, GetClientRect, GetMessageW, GetMonitorInfoW,
            GetClassNameA, GetDesktopWindow, GetWindow, GetWindowLongW, IsWindowVisible,
            MonitorFromRect, MonitorFromWindow, MONITORINFOEXW, MONITOR_DEFAULTTONEAREST, PeekMessageW, InvalidateRect,
            PostThreadMessageW, RegisterClassW, SetLayeredWindowAttributes,
            SetWindowDisplayAffinity, SetWindowPos, SetWindowRgn, TranslateMessage, HTTRANSPARENT, HWND_TOPMOST,
            SetWinEventHook, UnhookWinEvent, EVENT_OBJECT_REORDER,
            GW_HWNDPREV, GWL_EXSTYLE, OBJID_CLIENT, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
            LWA_ALPHA, MA_NOACTIVATE, MSG, PAINTSTRUCT, PM_NOREMOVE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
            SWP_SHOWWINDOW, WM_APP, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_ERASEBKGND,
            WM_MOUSEACTIVATE, WM_NCHITTEST, WM_PAINT, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
            WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
        },
    },
};

const WM_SCREEN_FRAME_UPDATE: UINT = WM_APP + 1;
const WM_SCREEN_FRAME_REBUILD: UINT = WM_APP + 2;
const WM_SCREEN_FRAME_COLORS: UINT = WM_APP + 3;
const WM_SCREEN_FRAME_RESTACK: UINT = WM_APP + 4;
const WDA_EXCLUDEFROMCAPTURE: DWORD = 0x0000_0011;
const CLASS_NAME: &[u16] = &[
    82, 117, 115, 116, 68, 101, 115, 107, 83, 99, 114, 101, 101, 110, 70, 114, 97, 109, 101, 0,
];

static REQUESTED_VISIBLE: AtomicBool = AtomicBool::new(false);
static WORKER_RUNNING: AtomicBool = AtomicBool::new(false);
static REBUILD_QUEUED: AtomicBool = AtomicBool::new(false);
static WORKER_THREAD_ID: AtomicU32 = AtomicU32::new(0);

pub(crate) fn set_visible(visible: bool) {
    let changed = REQUESTED_VISIBLE.swap(visible, Ordering::AcqRel) != visible;
    if visible {
        ensure_worker();
    }
    if changed {
        notify_worker(WM_SCREEN_FRAME_UPDATE);
    } else if visible {
        // A remaining session can change the union of viewed displays without hiding the frame.
        refresh_colors();
    }
}

fn ensure_worker() {
    if WORKER_RUNNING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }
    if let Err(err) = std::thread::Builder::new()
        .name("screen-frame".to_owned())
        .spawn(worker_main)
    {
        WORKER_RUNNING.store(false, Ordering::Release);
        hbb_common::throttled_log!(
            std::time::Duration::from_secs(5),
            error,
            "Failed to start screen frame thread: {err}"
        );
    }
}

fn notify_worker(message: UINT) -> bool {
    let thread_id = WORKER_THREAD_ID.load(Ordering::Acquire);
    if thread_id == 0 {
        return false;
    }
    if unsafe { PostThreadMessageW(thread_id, message, 0, 0) } == FALSE {
        let error = unsafe { GetLastError() };
        hbb_common::throttled_log!(
            std::time::Duration::from_secs(5),
            debug,
            "Failed to notify screen frame thread: {error}"
        );
        return false;
    }
    true
}

fn request_rebuild() {
    if !REBUILD_QUEUED.swap(true, Ordering::AcqRel) && !notify_worker(WM_SCREEN_FRAME_REBUILD) {
        REBUILD_QUEUED.store(false, Ordering::Release);
    }
}

pub(crate) fn refresh() {
    if REQUESTED_VISIBLE.load(Ordering::Acquire) {
        request_rebuild();
    }
}

pub(crate) fn refresh_colors() {
    if REQUESTED_VISIBLE.load(Ordering::Acquire) {
        notify_worker(WM_SCREEN_FRAME_COLORS);
    }
}

fn worker_main() {
    unsafe {
        let mut message: MSG = mem::zeroed();
        PeekMessageW(&mut message, null_mut(), 0, 0, PM_NOREMOVE);
        WORKER_THREAD_ID.store(GetCurrentThreadId(), Ordering::Release);

        let Some(instance) = register_window_class() else {
            WORKER_THREAD_ID.store(0, Ordering::Release);
            WORKER_RUNNING.store(false, Ordering::Release);
            return;
        };
        // Explorer can reorder the taskbar after a display layout change.
        // Observe completed desktop Z-order changes instead of polling or racing
        // the taskbar on a timer. Own-process events cannot feed back into the hook.
        let reorder_hook = SetWinEventHook(
            EVENT_OBJECT_REORDER, EVENT_OBJECT_REORDER, null_mut(),
            Some(desktop_reordered), 0, 0, WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        );
        if reorder_hook.is_null() {
            hbb_common::throttled_log!(std::time::Duration::from_secs(5), warn,
                "Failed to watch screen frame Z-order: {}", GetLastError());
        }
        let mut windows = Vec::new();
        apply_requested_state(instance, &mut windows);

        loop {
            let result = GetMessageW(&mut message, null_mut(), 0, 0);
            if result <= 0 {
                if result < 0 {
                    let error = GetLastError();
                    hbb_common::throttled_log!(
                        std::time::Duration::from_secs(5),
                        error,
                        "Screen frame message loop failed: {error}"
                    );
                }
                break;
            }
            if message.hwnd.is_null() {
                match message.message {
                    WM_SCREEN_FRAME_UPDATE => apply_requested_state(instance, &mut windows),
                    WM_SCREEN_FRAME_REBUILD => {
                        REBUILD_QUEUED.store(false, Ordering::Release);
                        apply_requested_state(instance, &mut windows);
                    }
                    WM_SCREEN_FRAME_RESTACK => keep_above_taskbar(&windows),
                    WM_SCREEN_FRAME_COLORS => {
                        // Repaint the same HWNDs to avoid recreating borders when subscriptions change.
                        for &window in &windows {
                            if InvalidateRect(window, null(), FALSE) == FALSE {
                                hbb_common::throttled_log!(std::time::Duration::from_secs(5), warn,
                                    "Failed to recolor screen frame: {}", GetLastError());
                            }
                        }
                    }
                    _ => {}
                }
            } else {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        if !reorder_hook.is_null() && UnhookWinEvent(reorder_hook) == FALSE {
            hbb_common::throttled_log!(std::time::Duration::from_secs(5), warn,
                "Failed to stop watching screen frame Z-order: {}", GetLastError());
        }
        destroy_windows(&mut windows);
    }
    WORKER_THREAD_ID.store(0, Ordering::Release);
    REBUILD_QUEUED.store(false, Ordering::Release);
    WORKER_RUNNING.store(false, Ordering::Release);
}

unsafe extern "system" fn desktop_reordered(
    _hook: HWINEVENTHOOK,
    _event: DWORD,
    window: HWND,
    object: i32,
    child: i32,
    _thread: DWORD,
    _time: DWORD,
) {
    // The desktop reports top-level window reordering through its client object;
    // ignore child controls and do no window queries while the frame is hidden.
    if REQUESTED_VISIBLE.load(Ordering::Acquire)
        && object == OBJID_CLIENT && child == 0 && window == GetDesktopWindow()
    {
        notify_worker(WM_SCREEN_FRAME_RESTACK);
    }
}

unsafe fn keep_above_taskbar(windows: &[HWND]) {
    for &window in windows {
        let mut raise = GetWindowLongW(window, GWL_EXSTYLE) & WS_EX_TOPMOST as i32 == 0;
        let monitor = MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST);
        let mut above = GetWindow(window, GW_HWNDPREV);
        // The Z-order can change while it is inspected; keep traversal bounded.
        for _ in 0..1024 {
            if above.is_null() || raise {
                break;
            }
            let mut class = [0u8; 32];
            let len = GetClassNameA(above, class.as_mut_ptr() as _, class.len() as i32);
            if matches!(&class[..len as usize], b"Shell_TrayWnd" | b"Shell_SecondaryTrayWnd")
                && IsWindowVisible(above) != FALSE
                && MonitorFromWindow(above, MONITOR_DEFAULTTONEAREST) == monitor
            {
                raise = true;
                break;
            }
            above = GetWindow(above, GW_HWNDPREV);
        }
        // Only repair an actual loss of TOPMOST or taskbar occlusion. Reordering
        // unrelated windows must not keep raising the frame or stealing focus.
        if raise && SetWindowPos(window, HWND_TOPMOST, 0, 0, 0, 0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) == FALSE
        {
            hbb_common::throttled_log!(std::time::Duration::from_secs(5), warn,
                "Failed to restore screen frame Z-order: {}", GetLastError());
        }
    }
}

unsafe fn register_window_class() -> Option<HINSTANCE> {
    let instance = GetModuleHandleW(null());
    if instance.is_null() {
        let error = GetLastError();
        hbb_common::throttled_log!(
            std::time::Duration::from_secs(5),
            error,
            "Failed to get the screen frame module: {error}"
        );
        return None;
    }
    let mut window_class: WNDCLASSW = mem::zeroed();
    window_class.lpfnWndProc = Some(window_proc);
    window_class.hInstance = instance;
    window_class.lpszClassName = CLASS_NAME.as_ptr();
    if RegisterClassW(&window_class) == 0 {
        let error = GetLastError();
        if error != ERROR_CLASS_ALREADY_EXISTS {
            hbb_common::throttled_log!(
                std::time::Duration::from_secs(5),
                error,
                "Failed to register the screen frame window: {error}"
            );
            return None;
        }
    }
    Some(instance)
}

unsafe fn apply_requested_state(instance: HINSTANCE, windows: &mut Vec<HWND>) {
    if REQUESTED_VISIBLE.load(Ordering::Acquire) {
        create_windows(instance, windows);
    } else {
        destroy_windows(windows);
    }
}

unsafe fn create_windows(instance: HINSTANCE, windows: &mut Vec<HWND>) {
    let mut monitors: Vec<RECT> = Vec::new();
    if EnumDisplayMonitors(
        null_mut(),
        null(),
        Some(enum_monitor),
        &mut monitors as *mut Vec<RECT> as LPARAM,
    ) == FALSE
    {
        let error = GetLastError();
        hbb_common::throttled_log!(
            std::time::Duration::from_secs(5),
            error,
            "Failed to enumerate displays for screen frame: {error}"
        );
        return;
    }

    // Keep HWNDs and their capture exclusion across layout changes instead of
    // briefly removing every border. Windows can follow the current enumeration
    // order: painting resolves colors from their current monitor, not a cached ID.
    let mut used = 0;
    for monitor in monitors {
        let style = style_for_monitor(MonitorFromRect(&monitor, MONITOR_DEFAULTTONEAREST));
        let width = monitor.right - monitor.left;
        let height = monitor.bottom - monitor.top;
        if width <= 0 || height <= 0 {
            continue;
        }
        let thickness = i32::from(style.width).min(width / 2).min(height / 2);
        if thickness <= 0 {
            continue;
        }
        // A monitor-sized hollow window lets the shell recognize the overlay as
        // fullscreen and place the taskbar behind it. Its region excludes the
        // desktop center, so only the border is painted and hit-tested.
        if let Some(&window) = windows.get(used) {
            if let Err(error) = update_window(window, monitor, &style) {
                hbb_common::throttled_log!(std::time::Duration::from_secs(5), error,
                    "Failed to update a screen frame window: {error}");
                destroy_window(window);
                windows.remove(used);
                continue;
            }
        } else if let Some(window) = create_window(instance, monitor, &style) {
            windows.push(window);
        } else {
            continue;
        }
        used += 1;
    }
    for window in windows.drain(used..) {
        destroy_window(window);
    }
}

unsafe extern "system" fn enum_monitor(
    _monitor: HMONITOR,
    _dc: HDC,
    rect: LPRECT,
    data: LPARAM,
) -> BOOL {
    if rect.is_null() || data == 0 {
        return TRUE;
    }
    let monitors = &mut *(data as *mut Vec<RECT>);
    monitors.push(*rect);
    TRUE
}

unsafe fn create_window(instance: HINSTANCE, rect: RECT, style: &Style) -> Option<HWND> {
    let window = CreateWindowExW(
        // Establish the Z-order group before showing a new display's window;
        // the final SetWindowPos also positions it in that group.
        WS_EX_LAYERED | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT | WS_EX_TOPMOST,
        CLASS_NAME.as_ptr(),
        null(),
        WS_POPUP,
        rect.left,
        rect.top,
        rect.right - rect.left,
        rect.bottom - rect.top,
        null_mut(),
        null_mut(),
        instance,
        null_mut(),
    );
    if window.is_null() {
        let error = GetLastError();
        hbb_common::throttled_log!(
            std::time::Duration::from_secs(5),
            error,
            "Failed to create a screen frame window: {error}"
        );
        return None;
    }
    if let Err(error) = set_border_region(window, rect, style.width) {
        hbb_common::throttled_log!(std::time::Duration::from_secs(5), error,
            "Failed to shape a screen frame window: {error}");
        destroy_window(window);
        return None;
    }
    let alpha = ((100 - u32::from(style.transparency)) * 255 / 100) as u8;
    if SetLayeredWindowAttributes(window, 0, alpha, LWA_ALPHA) == FALSE {
        let error = GetLastError();
        hbb_common::throttled_log!(
            std::time::Duration::from_secs(5),
            error,
            "Failed to paint a screen frame window: {error}"
        );
        destroy_window(window);
        return None;
    }
    // WDA_EXCLUDEFROMCAPTURE requires Windows 10 2004 (build 19041).
    // Older Windows treats it as WDA_MONITOR, which can leave blank/black strips
    // in captures. Skip exclusion there and keep the local border visible even
    // if exclusion fails. This version gate is independent of GDI/DXGI selection.
    if base::platform::windows::is_windows_version_or_greater(10, 0, 19041, 0, 0)
        && SetWindowDisplayAffinity(window, WDA_EXCLUDEFROMCAPTURE) == FALSE
    {
        let error = GetLastError();
        hbb_common::throttled_log!(
            std::time::Duration::from_secs(5),
            debug,
            "Failed to exclude a screen frame window from capture: {error}"
        );
    }
    if SetWindowPos(
        window,
        HWND_TOPMOST,
        rect.left,
        rect.top,
        rect.right - rect.left,
        rect.bottom - rect.top,
        SWP_NOACTIVATE | SWP_SHOWWINDOW,
    ) == FALSE
    {
        let error = GetLastError();
        hbb_common::throttled_log!(
            std::time::Duration::from_secs(5),
            error,
            "Failed to show a screen frame window: {error}"
        );
        destroy_window(window);
        return None;
    }
    Some(window)
}

unsafe fn update_window(window: HWND, rect: RECT, style: &Style) -> Result<(), &'static str> {
    // The region uses window-local coordinates and does not scale with the HWND.
    // Update it as well as the bounds, including when only border width changes.
    set_border_region(window, rect, style.width)?;
    let alpha = ((100 - u32::from(style.transparency)) * 255 / 100) as u8;
    if SetLayeredWindowAttributes(window, 0, alpha, LWA_ALPHA) == FALSE {
        return Err("SetLayeredWindowAttributes failed");
    }
    if SetWindowPos(
        window,
        null_mut(),
        rect.left,
        rect.top,
        rect.right - rect.left,
        rect.bottom - rect.top,
        SWP_NOACTIVATE | SWP_NOZORDER,
    ) == FALSE
    {
        return Err("SetWindowPos failed");
    }
    // The HWND may now belong to another monitor, and an unchanged rectangle can
    // still need repainting after width/color changes. Do not reuse its old pixels.
    if InvalidateRect(window, null(), FALSE) == FALSE {
        return Err("InvalidateRect failed");
    }
    Ok(())
}

unsafe fn set_border_region(window: HWND, rect: RECT, width: u8) -> Result<(), &'static str> {
    let w = rect.right - rect.left;
    let h = rect.bottom - rect.top;
    let thickness = i32::from(width).min(w / 2).min(h / 2);
    let outer = CreateRectRgn(0, 0, w, h);
    if outer.is_null() {
        return Err("CreateRectRgn failed for the monitor bounds");
    }
    let inner = CreateRectRgn(thickness, thickness, w - thickness, h - thickness);
    if inner.is_null() {
        DeleteObject(outer as _);
        return Err("CreateRectRgn failed for the desktop center");
    }
    let combined = CombineRgn(outer, outer, inner, RGN_DIFF);
    DeleteObject(inner as _);
    if combined == 0 {
        DeleteObject(outer as _);
        return Err("CombineRgn failed");
    }
    if SetWindowRgn(window, outer, FALSE) == 0 {
        DeleteObject(outer as _);
        return Err("SetWindowRgn failed");
    }
    // Windows owns the region after SetWindowRgn succeeds.
    Ok(())
}

unsafe fn destroy_windows(windows: &mut Vec<HWND>) {
    for window in windows.drain(..) {
        destroy_window(window);
    }
}

unsafe fn destroy_window(window: HWND) {
    if DestroyWindow(window) == FALSE {
        let error = GetLastError();
        hbb_common::throttled_log!(
            std::time::Duration::from_secs(5),
            debug,
            "Failed to destroy a screen frame window: {error}"
        );
    }
}

unsafe extern "system" fn window_proc(
    window: HWND,
    message: UINT,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    match message {
        WM_NCHITTEST => HTTRANSPARENT,
        WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
        WM_ERASEBKGND => 1,
        WM_DISPLAYCHANGE | WM_DPICHANGED => {
            request_rebuild();
            0
        }
        WM_PAINT => {
            let mut paint: PAINTSTRUCT = mem::zeroed();
            let dc = BeginPaint(window, &mut paint);
            let mut rect: RECT = mem::zeroed();
            if !dc.is_null() && GetClientRect(window, &mut rect) != FALSE {
                let color = style_for_monitor(MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST)).color;
                let brush = CreateSolidBrush(RGB(color[0], color[1], color[2]));
                if !brush.is_null() {
                    FillRect(dc, &rect, brush);
                    DeleteObject(brush as _);
                }
            }
            EndPaint(window, &paint);
            0
        }
        _ => DefWindowProcW(window, message, w_param, l_param),
    }
}

unsafe fn style_for_monitor(monitor: HMONITOR) -> Style {
    let mut info: MONITORINFOEXW = mem::zeroed();
    info.cbSize = mem::size_of::<MONITORINFOEXW>() as _;
    if GetMonitorInfoW(monitor, &mut info as *mut _ as _) == FALSE {
        hbb_common::throttled_log!(std::time::Duration::from_secs(5), warn,
            "Failed to identify screen frame monitor: {}", GetLastError());
        return Style::load();
    }
    let end = info.szDevice.iter().position(|&c| c == 0).unwrap_or(info.szDevice.len());
    Style::for_display(&String::from_utf16_lossy(&info.szDevice[..end]))
}
