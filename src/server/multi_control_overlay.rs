//! Cursor overlay on the controlled desktop.
//!
//! The local user sees exactly one pointer, and a borrowing controller really moves it, so
//! without this the only hint of "who moved it" is the connection window. This module
//! paints one marker per controller that has a known position, so the person at the machine
//! can see where each of them points before and while they borrow.
//!
//! It is a display feature only: it never takes input. The window is layered and
//! click-through, never activated, and asked to stay out of the screen capture, so it does
//! not appear in what the controllers see (they get the cursors over the existing
//! whiteboard path) and it cannot swallow a click or steal focus.
//!
//! It must run in the process that owns the desktop. With the installed service that is the
//! helper process, not the one that holds the arbitration state, so the caller publishes
//! through [`update`] and this module decides whether to draw locally or forward.
//!
//! Windows only for now: X11 needs a compositing-friendly ARGB visual and Wayland has no
//! way to draw over another application at all, so both keep the connection-window
//! indicator instead.

use crate::server::multi_control::DrawCursor;
use base::config::keys;
use hbb_common::config::Config;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Fastest repaint the overlay does. Controllers can send moves much faster than a person
/// can see, and the window would only redraw its own frames anyway.
const PUBLISH_INTERVAL: Duration = Duration::from_millis(50);

/// Only `Y` turns the overlay on, and only while the mode itself runs.
pub fn enabled() -> bool {
    SUPPORTED
        && super::multi_control::enabled()
        && Config::get_option(keys::OPTION_MULTI_CONTROL_OVERLAY) == "Y"
}

/// Whether this build can draw the overlay at all.
pub const SUPPORTED: bool = cfg!(windows);

#[derive(Default)]
struct Published {
    last: Option<Instant>,
    cursors: Vec<DrawCursor>,
    active: bool,
}

lazy_static::lazy_static! {
    static ref PUBLISHED: Mutex<Published> = Mutex::new(Published::default());
}

/// Hands the current set of controller cursors to the overlay.
///
/// Cheap to call on every input batch: an unchanged set is not repainted at all, and a
/// change is rate limited, so a controller that keeps still costs nothing.
pub fn update(cursors: &[DrawCursor]) {
    let wanted = enabled() && !cursors.is_empty();
    let wanted_cursors: Vec<DrawCursor> = if wanted { cursors.to_vec() } else { Vec::new() };
    let mut published = PUBLISHED.lock().unwrap();
    if published.active == wanted && published.cursors == wanted_cursors {
        // Nothing to paint: this is the steady state while the controllers keep still.
        return;
    }
    if let Some(last) = published.last {
        if last.elapsed() < PUBLISH_INTERVAL {
            // Too soon, and the change is not lost: the next call carries it.
            return;
        }
    }
    published.last = Some(Instant::now());
    published.active = wanted;
    published.cursors = wanted_cursors;
    let markers: Vec<(i32, i32, i32, bool)> = published
        .cursors
        .iter()
        .map(|c| (c.conn, c.x, c.y, c.borrowing))
        .collect();
    drop(published);
    publish(&markers);
}

/// Draws the markers in this process: called either directly, or by the process that owns
/// the desktop after they arrived from the one holding the arbitration state.
pub fn draw_local(markers: &[(i32, i32, i32, bool)]) {
    {
        let mut shared = imp::markers();
        *shared = markers
            .iter()
            .map(|(conn, x, y, borrowing)| DrawCursor {
                conn: *conn,
                x: *x,
                y: *y,
                borrowing: *borrowing,
            })
            .collect();
    }
    if markers.is_empty() {
        imp::hide();
    } else {
        imp::draw();
    }
}

/// The desktop the markers are relative to, in virtual screen coordinates.
pub fn desktop_rect() -> Option<(i32, i32, u32, u32)> {
    imp::desktop_rect()
}

/// Draws in this process, or forwards to the one that owns the desktop.
fn publish(markers: &[(i32, i32, i32, bool)]) {
    #[cfg(windows)]
    {
        if crate::server::portable_service::client::running() {
            crate::server::portable_service::client::handle_multi_control_cursors(markers);
            return;
        }
    }
    draw_local(markers);
}

/// Hides the overlay everywhere and ends the window, e.g. when the mode is turned off or
/// the option is unchecked.
pub fn stop() {
    {
        let mut published = PUBLISHED.lock().unwrap();
        published.active = false;
        published.last = None;
        published.cursors.clear();
    }
    // The window may live in the process that owns the desktop, so the hide travels the
    // same way the markers did: clearing this process alone would leave the last frame
    // frozen on the other one.
    publish(&[]);
    imp::shutdown();
    // A new attempt may succeed, e.g. once there is an interactive desktop again.
    imp::reset_failure();
}

/// Whether the cursors of the remote controllers are currently drawn.
pub fn is_active() -> bool {
    PUBLISHED.lock().unwrap().active
}

/// Whether anything of the overlay is still alive in this process, even if it is hidden.
/// The window and its thread have to be ended when the option goes off, not only hidden.
pub fn is_started() -> bool {
    imp::is_started()
}

#[cfg(windows)]
mod imp {
    use super::DrawCursor;
    use std::{
        mem,
        sync::{
            atomic::{AtomicBool, Ordering},
            Mutex,
        },
    };
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows::Win32::Graphics::Gdi::{
        BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FillRect, GetStockObject,
        InvalidateRect, SelectObject, SetBkMode, SetTextColor, TextOutW, UpdateWindow,
        DEFAULT_GUI_FONT, HBRUSH, HGDIOBJ, PAINTSTRUCT, TRANSPARENT,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
        GetSystemMetrics, PostMessageW, PostQuitMessage, RegisterClassW,
        SetLayeredWindowAttributes, SetWindowDisplayAffinity, SetWindowPos, ShowWindow,
        TranslateMessage, UnregisterClassW, CS_HREDRAW, CS_VREDRAW, HWND_TOPMOST, LWA_ALPHA,
        LWA_COLORKEY, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
        SWP_NOACTIVATE, SW_HIDE, SW_SHOWNA, SYSTEM_METRICS_INDEX, WDA_EXCLUDEFROMCAPTURE, WM_APP,
        WM_DESTROY, WM_PAINT, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
        WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
    };

    /// The colour that means "transparent" in the layered window. Any colour works, as long
    /// as nothing else in the overlay uses it exactly.
    const KEY_COLOR: u32 = 0x00ff_00ff;

    /// Marker colours, picked by connection so the same controller keeps its colour for as
    /// long as it is connected. The palette is deliberately small and saturated: these
    /// markers sit on top of a desktop of unknown colours.
    const PALETTE: [u32; 6] = [
        0x00e6_1a1a,
        0x001a_6ee6,
        0x001a_e65c,
        0x00e6_b31a,
        0x00a3_1ae6,
        0x001a_c9e6,
    ];

    const MARKER: i32 = 14;
    const RING: i32 = 22;
    /// Tells the window thread to end itself; `WM_APP` and up are free for a program.
    const WM_APP_QUIT: u32 = WM_APP + 1;

    lazy_static::lazy_static! {
        static ref CURSORS: Mutex<Vec<DrawCursor>> = Mutex::new(Vec::new());
        static ref THREAD: Mutex<Option<isize>> = Mutex::new(None);
        /// The desktop the window was sized for, so a display change can be noticed.
        static ref WINDOW_RECT: Mutex<(i32, i32, i32, i32)> = Mutex::new((0, 0, 0, 0));
        /// A window could not be created: without this latch the publish rate would retry
        /// it for as long as the option stays on.
        static ref FAILED: AtomicBool = AtomicBool::new(false);
        /// A thread is on its way to a window, so two callers do not create two of them.
        static ref STARTING: AtomicBool = AtomicBool::new(false);
    }

    /// The markers the window paints; the caller fills it before asking for a repaint.
    pub fn markers() -> std::sync::MutexGuard<'static, Vec<DrawCursor>> {
        CURSORS.lock().unwrap()
    }

    /// The virtual desktop, the space every marker is relative to.
    pub fn desktop_rect() -> Option<(i32, i32, u32, u32)> {
        let (x, y, width, height) = virtual_screen();
        (width > 0 && height > 0).then_some((x, y, width as u32, height as u32))
    }

    /// Shows the markers, starting the window thread on first use.
    pub fn draw() {
        if FAILED.load(Ordering::SeqCst) {
            return;
        }
        let raw = *THREAD.lock().unwrap();
        match raw {
            Some(raw) => {
                let hwnd = HWND(raw as *mut core::ffi::c_void);
                fit_to_desktop(hwnd);
                // The window outlives individual marker sets, so it is only hidden and
                // shown again instead of being recreated.
                unsafe {
                    let _ = ShowWindow(hwnd, SW_SHOWNA);
                }
                repaint(hwnd);
            }
            None => start_thread(),
        }
    }

    /// Hides the markers; the window itself stays for the next time.
    pub fn hide() {
        CURSORS.lock().unwrap().clear();
        let raw = *THREAD.lock().unwrap();
        if let Some(raw) = raw {
            let hwnd = HWND(raw as *mut core::ffi::c_void);
            unsafe {
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
        }
    }

    /// Ends the window and its thread, so nothing of a mode that is off stays behind.
    pub fn shutdown() {
        let raw = *THREAD.lock().unwrap();
        if let Some(raw) = raw {
            let hwnd = HWND(raw as *mut core::ffi::c_void);
            unsafe {
                let _ = PostMessageW(Some(hwnd), WM_APP_QUIT, WPARAM(0), LPARAM(0));
            }
        }
    }

    /// Whether a window, or a failed attempt at one, is still around in this process.
    pub fn is_started() -> bool {
        THREAD.lock().unwrap().is_some() || FAILED.load(Ordering::SeqCst)
    }

    /// Lets a later attempt create a window again.
    pub fn reset_failure() {
        FAILED.store(false, Ordering::SeqCst);
    }

    /// Resizes the window when the desktop changed under it: the markers are relative to
    /// the desktop, so a window that keeps the geometry of an older layout draws every
    /// marker in the wrong place, or clips it away.
    fn fit_to_desktop(hwnd: HWND) {
        let screen = virtual_screen();
        {
            let mut rect = WINDOW_RECT.lock().unwrap();
            if *rect == screen {
                return;
            }
            *rect = screen;
        }
        let (x, y, width, height) = screen;
        unsafe {
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                x,
                y,
                width,
                height,
                SWP_NOACTIVATE,
            );
        }
    }

    fn repaint(hwnd: HWND) {
        unsafe {
            // InvalidateRect only queues the paint; painting here would race the message
            // loop, so the window procedure does the drawing.
            let _ = InvalidateRect(Some(hwnd), None, true);
            let _ = UpdateWindow(hwnd);
        }
    }

    fn start_thread() {
        if STARTING.swap(true, Ordering::SeqCst) {
            return;
        }
        if THREAD.lock().unwrap().is_some() {
            STARTING.store(false, Ordering::SeqCst);
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel::<isize>();
        if std::thread::Builder::new()
            .name("multi-control-overlay".to_owned())
            .spawn(move || run(tx))
            .is_err()
        {
            hbb_common::log::error!("failed to start the multi-control overlay thread");
            STARTING.store(false, Ordering::SeqCst);
            return;
        }
        // The window handle, or 0 when the thread could not create one. The lock is not
        // held while waiting: `hide` and `draw` may be called from other threads.
        match rx.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(0) => {
                FAILED.store(true, Ordering::SeqCst);
                hbb_common::log::warn!("the multi-control overlay could not be created");
            }
            Ok(raw) => {
                *THREAD.lock().unwrap() = Some(raw);
                let hwnd = HWND(raw as *mut core::ffi::c_void);
                fit_to_desktop(hwnd);
                repaint(hwnd);
            }
            Err(err) => {
                FAILED.store(true, Ordering::SeqCst);
                hbb_common::log::error!("the multi-control overlay did not start: {}", err);
            }
        }
        STARTING.store(false, Ordering::SeqCst);
    }

    fn run(tx: std::sync::mpsc::Sender<isize>) {
        unsafe {
            let class_name: Vec<u16> = "RustDeskMultiControlOverlay\0".encode_utf16().collect();
            let window_name: Vec<u16> = "RustDesk\0".encode_utf16().collect();
            let class = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(wnd_proc),
                lpszClassName: PCWSTR(class_name.as_ptr()),
                ..Default::default()
            };
            // A second registration fails harmlessly; the class is what matters.
            RegisterClassW(&class);
            let (x, y, width, height) = virtual_screen();
            let style = WS_EX_LAYERED
                | WS_EX_TRANSPARENT
                | WS_EX_TOOLWINDOW
                | WS_EX_NOACTIVATE
                | WS_EX_TOPMOST;
            let hwnd = match CreateWindowExW(
                style,
                PCWSTR(class_name.as_ptr()),
                PCWSTR(window_name.as_ptr()),
                WS_POPUP,
                x,
                y,
                width,
                height,
                None,
                None,
                None,
                None,
            ) {
                Ok(hwnd) => hwnd,
                Err(err) => {
                    hbb_common::log::warn!("failed to create the multi-control overlay: {}", err);
                    let _ = tx.send(0);
                    return;
                }
            };
            // Color-key transparency: everything painted in KEY_COLOR lets the desktop
            // through, and WM_NCHITTEST is never reached because the window is
            // WS_EX_TRANSPARENT.
            if let Err(err) = SetLayeredWindowAttributes(
                hwnd,
                windows::Win32::Foundation::COLORREF(KEY_COLOR),
                255,
                LWA_COLORKEY | LWA_ALPHA,
            ) {
                // A layered window whose attributes were never set is not displayed at
                // all, so keeping it would only hide the reason.
                hbb_common::log::warn!(
                    "failed to make the multi-control overlay transparent: {}",
                    err
                );
                let _ = DestroyWindow(hwnd);
                let _ = tx.send(0);
                return;
            }
            // Keep the markers out of the screen capture, so the controllers do not see
            // them twice (they already get the cursor of every peer over the video).
            if let Err(err) = SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE) {
                // Without this the markers would be burned into every controller's video,
                // which is worse than not showing them at all.
                hbb_common::log::warn!(
                    "the multi-control overlay would appear in the screen capture: {}",
                    err
                );
                let _ = DestroyWindow(hwnd);
                let _ = tx.send(0);
                return;
            }
            *WINDOW_RECT.lock().unwrap() = (x, y, width, height);
            let _ = tx.send(hwnd.0 as isize);
            let mut message = mem::zeroed();
            while GetMessageW(&mut message, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            // The window normally ends through `WM_APP_QUIT`; this also covers a loop that
            // ended some other way, so neither the window nor its class stays behind.
            let _ = DestroyWindow(hwnd);
            let _ = UnregisterClassW(PCWSTR(class_name.as_ptr()), None);
            *WINDOW_RECT.lock().unwrap() = (0, 0, 0, 0);
            *THREAD.lock().unwrap() = None;
        }
    }

    fn virtual_screen() -> (i32, i32, i32, i32) {
        unsafe {
            let metric = |index: SYSTEM_METRICS_INDEX| GetSystemMetrics(index);
            (
                metric(SM_XVIRTUALSCREEN),
                metric(SM_YVIRTUALSCREEN),
                metric(SM_CXVIRTUALSCREEN),
                metric(SM_CYVIRTUALSCREEN),
            )
        }
    }

    fn color_of(conn: i32) -> u32 {
        PALETTE[(conn.rem_euclid(PALETTE.len() as i32)) as usize]
    }

    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match message {
            WM_PAINT => {
                let mut paint: PAINTSTRUCT = mem::zeroed();
                let dc = BeginPaint(hwnd, &mut paint);
                paint_markers(dc, hwnd);
                let _ = EndPaint(hwnd, &paint);
                LRESULT(0)
            }
            WM_APP_QUIT => {
                // Ends the message loop through WM_DESTROY, on this thread.
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }

    unsafe fn paint_markers(dc: windows::Win32::Graphics::Gdi::HDC, hwnd: HWND) {
        let mut rect = RECT::default();
        let _ = windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut rect);
        let background: HBRUSH = CreateSolidBrush(windows::Win32::Foundation::COLORREF(KEY_COLOR));
        FillRect(dc, &rect, background);
        let _ = DeleteObject(HGDIOBJ(background.0));
        let _ = SetBkMode(dc, TRANSPARENT);
        let font = GetStockObject(DEFAULT_GUI_FONT);
        let old = SelectObject(dc, font);
        let cursors = CURSORS.lock().unwrap().clone();
        for cursor in cursors.iter() {
            let color = color_of(cursor.conn);
            let brush: HBRUSH = CreateSolidBrush(windows::Win32::Foundation::COLORREF(color));
            let old_brush = SelectObject(dc, HGDIOBJ(brush.0));
            if cursor.borrowing {
                // A controller that holds the pointer gets a ring, so the local user can
                // tell it apart from somebody who is only pointing.
                let ring: HBRUSH =
                    CreateSolidBrush(windows::Win32::Foundation::COLORREF(0x00ff_ffff));
                let old_ring = SelectObject(dc, HGDIOBJ(ring.0));
                let _ = windows::Win32::Graphics::Gdi::Ellipse(
                    dc,
                    cursor.x - RING,
                    cursor.y - RING,
                    cursor.x + RING,
                    cursor.y + RING,
                );
                let _ = SelectObject(dc, old_ring);
                let _ = DeleteObject(HGDIOBJ(ring.0));
            }
            let _ = windows::Win32::Graphics::Gdi::Ellipse(
                dc,
                cursor.x - MARKER,
                cursor.y - MARKER,
                cursor.x + MARKER,
                cursor.y + MARKER,
            );
            let _ = SelectObject(dc, old_brush);
            let _ = DeleteObject(HGDIOBJ(brush.0));
            // The label is the connection id: the connection window maps it to the peer's
            // name, so a name does not have to travel to this process as well.
            let label: Vec<u16> = format!("#{}", cursor.conn).encode_utf16().collect();
            let _ = SetTextColor(dc, windows::Win32::Foundation::COLORREF(0x00ff_ffff));
            let _ = TextOutW(dc, cursor.x - 4, cursor.y + MARKER + 2, &label);
        }
        let _ = SelectObject(dc, old);
    }
}

#[cfg(not(windows))]
mod imp {
    use super::DrawCursor;

    /// No overlay implementation for this platform yet, so nothing is ever drawn: Wayland
    /// has no way to draw over another application, and X11 needs an ARGB visual plus an
    /// empty input shape that is not written yet.
    pub fn markers() -> std::sync::MutexGuard<'static, Vec<DrawCursor>> {
        lazy_static::lazy_static! {
            static ref MARKERS: std::sync::Mutex<Vec<DrawCursor>> = std::sync::Mutex::new(Vec::new());
        }
        MARKERS.lock().unwrap()
    }

    pub fn desktop_rect() -> Option<(i32, i32, u32, u32)> {
        None
    }

    pub fn draw() {}

    pub fn hide() {}

    pub fn shutdown() {}

    pub fn is_started() -> bool {
        false
    }

    pub fn reset_failure() {}
}
