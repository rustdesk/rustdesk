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
//! indicator instead. See `docs/multi-control-master-progress.md`.

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
    super::multi_control::enabled() && Config::get_option(keys::OPTION_MULTI_CONTROL_OVERLAY) == "Y"
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
/// Cheap to call on every input batch: it throttles itself, and it hides the overlay again
/// when the set becomes empty or the option is turned off.
pub fn update(cursors: &[DrawCursor]) {
    let wanted = enabled() && !cursors.is_empty();
    let mut published = PUBLISHED.lock().unwrap();
    if let Some(last) = published.last {
        // Always let the last state through, otherwise a controller that stops moving
        // would leave a stale marker behind.
        if last.elapsed() < PUBLISH_INTERVAL && wanted == published.active {
            return;
        }
    }
    published.last = Some(Instant::now());
    published.cursors = cursors.to_vec();
    let was_active = published.active;
    published.active = wanted;
    let cursors = published.cursors.clone();
    drop(published);
    if !wanted {
        if was_active {
            imp::hide();
        }
        return;
    }
    imp::draw(&cursors);
}

/// Hides the overlay, e.g. when the mode is turned off or the last session ends.
pub fn stop() {
    let mut published = PUBLISHED.lock().unwrap();
    published.active = false;
    published.last = None;
    published.cursors.clear();
    drop(published);
    imp::hide();
}

/// Whether the cursors of the remote controllers are currently drawn.
pub fn is_active() -> bool {
    PUBLISHED.lock().unwrap().active
}

#[cfg(windows)]
mod imp {
    use super::DrawCursor;
    use std::{mem, sync::Mutex};
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows::Win32::Graphics::Gdi::{
        BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FillRect, GetStockObject,
        InvalidateRect, SelectObject, SetBkMode, SetTextColor, TextOutW, UpdateWindow,
        DEFAULT_GUI_FONT, HBRUSH, HGDIOBJ, PAINTSTRUCT, TRANSPARENT,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetSystemMetrics,
        RegisterClassW, SetLayeredWindowAttributes, SetWindowDisplayAffinity, ShowWindow,
        TranslateMessage, CS_HREDRAW, CS_VREDRAW, LWA_ALPHA, LWA_COLORKEY, SM_CXVIRTUALSCREEN,
        SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SW_HIDE, SW_SHOWNA,
        SYSTEM_METRICS_INDEX, WDA_EXCLUDEFROMCAPTURE, WM_DESTROY, WM_PAINT, WNDCLASSW,
        WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT,
        WS_POPUP,
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

    lazy_static::lazy_static! {
        static ref CURSORS: Mutex<Vec<DrawCursor>> = Mutex::new(Vec::new());
        static ref THREAD: Mutex<Option<isize>> = Mutex::new(None);
    }

    /// Shows the markers, starting the window thread on first use.
    pub fn draw(cursors: &[DrawCursor]) {
        {
            let mut shared = CURSORS.lock().unwrap();
            *shared = cursors.to_vec();
        }
        let raw = *THREAD.lock().unwrap();
        match raw {
            Some(raw) => {
                let hwnd = HWND(raw as *mut core::ffi::c_void);
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
        if let Some(raw) = *THREAD.lock().unwrap() {
            let hwnd = HWND(raw as *mut core::ffi::c_void);
            unsafe {
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
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
        let mut guard = THREAD.lock().unwrap();
        if guard.is_some() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel::<isize>();
        if std::thread::Builder::new()
            .name("multi-control-overlay".to_owned())
            .spawn(move || run(tx))
            .is_err()
        {
            hbb_common::log::error!("failed to start the multi-control overlay thread");
            return;
        }
        match rx.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(raw) => {
                *guard = Some(raw);
                let hwnd = HWND(raw as *mut core::ffi::c_void);
                repaint(hwnd);
            }
            Err(err) => hbb_common::log::error!("the multi-control overlay did not start: {}", err),
        }
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
                    hbb_common::log::error!("failed to create the multi-control overlay: {}", err);
                    return;
                }
            };
            // Color-key transparency: everything painted in KEY_COLOR lets the desktop
            // through, and WM_NCHITTEST is never reached because the window is
            // WS_EX_TRANSPARENT.
            let _ = SetLayeredWindowAttributes(
                hwnd,
                windows::Win32::Foundation::COLORREF(KEY_COLOR),
                255,
                LWA_COLORKEY | LWA_ALPHA,
            );
            // Keep the markers out of the screen capture, so the controllers do not see
            // them twice (they already get the cursor of every peer over the video).
            if let Err(err) = SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE) {
                hbb_common::log::info!(
                    "the multi-control overlay may appear in the screen capture: {}",
                    err
                );
            }
            let _ = tx.send(hwnd.0 as isize);
            let mut message = mem::zeroed();
            while GetMessageW(&mut message, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
                if message.message == WM_DESTROY {
                    break;
                }
            }
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

    pub fn draw(_cursors: &[DrawCursor]) {}

    pub fn hide() {}
}
