use std::time::{Duration, Instant};

pub trait IdleSource {
    fn idle_duration(&self) -> hbb_common::ResultType<Duration>;
}

#[derive(Debug, Clone)]
pub struct ActivityTracker {
    idle_timeout: Duration,
    last_activity: Option<Instant>,
}

impl ActivityTracker {
    pub fn new(idle_timeout: Duration) -> Self {
        Self {
            idle_timeout,
            last_activity: None,
        }
    }

    pub fn is_active(&self, now: Instant) -> bool {
        self.last_activity
            .and_then(|last| now.checked_duration_since(last))
            .is_some_and(|elapsed| elapsed <= self.idle_timeout)
    }

    pub fn record_activity(&mut self, now: Instant) {
        self.last_activity = Some(now);
    }

    pub fn source_is_active(
        source: &impl IdleSource,
        idle_timeout: Duration,
    ) -> hbb_common::ResultType<bool> {
        Ok(source.idle_duration()? <= idle_timeout)
    }
}

// ---------------------------------------------------------------------------
// Platform idle / activity query
// ---------------------------------------------------------------------------

/// Returns `Some(idle_duration)` when a platform idle source is available,
/// or `None` when no reliable idle source can be queried (Wayland-only, etc.).
///
/// On Linux, two strategies are tried in order:
///   1. XScreenSaver extension (XSS) – gives accurate idle time.
///   2. XQueryPointer – falls back to mouse-position polling; a position
///      change since the last call counts as recent activity and returns
///      idle = 0. Callers that check for idle ≤ idle_timeout will correctly
///      detect activity. This strategy only detects mouse movement, not
///      keyboard input, but is sufficient to start/stop recording reliably
///      on desktops where XSS is unavailable.
pub fn platform_idle_duration() -> Option<Duration> {
    platform_idle_duration_impl()
}

// ---- Windows ---------------------------------------------------------------
#[cfg(target_os = "windows")]
fn platform_idle_duration_impl() -> Option<Duration> {
    use std::mem;
    // SAFETY: zero-init LASTINPUTINFO with cbSize set is exactly what Win32 requires.
    unsafe {
        let mut info: winapi::um::winuser::LASTINPUTINFO = mem::zeroed();
        info.cbSize = mem::size_of::<winapi::um::winuser::LASTINPUTINFO>() as u32;
        if winapi::um::winuser::GetLastInputInfo(&mut info) == 0 {
            return None;
        }
        let tick_now = winapi::um::sysinfoapi::GetTickCount();
        let idle_ms = tick_now.wrapping_sub(info.dwTime);
        Some(Duration::from_millis(idle_ms as u64))
    }
}

// ---- Linux -----------------------------------------------------------------
#[cfg(target_os = "linux")]
fn platform_idle_duration_impl() -> Option<Duration> {
    // Strategy 1: XScreenSaver extension (accurate idle time).
    if let Some(ms) = x11_xss_idle_ms() {
        return Some(Duration::from_millis(ms));
    }
    // Strategy 2: Check /proc/interrupts for input activity (keyboard/mouse)
    // combined with XQueryPointer for pointer movement.
    // If either detects activity, return idle = 0.
    let ptr_idle = x11_pointer_idle_duration();
    let kbd_active = proc_interrupts_input_changed();
    match (ptr_idle, kbd_active) {
        (_, true) => Some(Duration::ZERO),               // keyboard activity
        (Some(Duration::ZERO), _) => Some(Duration::ZERO), // mouse moved
        (Some(d), _) => Some(d),                         // no activity
        (None, _) => None,                               // X11 unavailable
    }
}

// ---- Strategy 1: XScreenSaver extension ------------------------------------
#[cfg(target_os = "linux")]
fn x11_xss_idle_ms() -> Option<u64> {
    use std::ffi::CString;
    use std::ptr;

    let lib_x11 = unsafe {
        libc::dlopen(
            b"libX11.so.6\0".as_ptr() as _,
            libc::RTLD_LAZY | libc::RTLD_GLOBAL,
        )
    };
    if lib_x11.is_null() {
        return None;
    }
    let lib_xss =
        unsafe { libc::dlopen(b"libXss.so.1\0".as_ptr() as _, libc::RTLD_LAZY) };
    if lib_xss.is_null() {
        unsafe { libc::dlclose(lib_x11) };
        return None;
    }

    type XOpenDisplayFn = unsafe extern "C" fn(*const libc::c_char) -> *mut libc::c_void;
    type XCloseDisplayFn = unsafe extern "C" fn(*mut libc::c_void) -> libc::c_int;
    type XDefaultRootWindowFn = unsafe extern "C" fn(*mut libc::c_void) -> libc::c_ulong;
    type XFreeFn = unsafe extern "C" fn(*mut libc::c_void) -> libc::c_int;
    type XSSAllocFn = unsafe extern "C" fn() -> *mut XSSInfo;
    type XSSQueryFn =
        unsafe extern "C" fn(*mut libc::c_void, libc::c_ulong, *mut XSSInfo) -> libc::c_int;

    #[repr(C)]
    struct XSSInfo {
        window: libc::c_ulong,
        state: libc::c_int,
        kind: libc::c_int,
        til_or_since: libc::c_ulong,
        idle: libc::c_ulong,
        event_mask: libc::c_ulong,
    }

    macro_rules! sym {
        ($lib:expr, $name:literal, $ty:ty) => {{
            let s = match CString::new($name) {
                Ok(s) => s,
                Err(_) => return None,
            };
            let ptr = unsafe { libc::dlsym($lib, s.as_ptr()) };
            if ptr.is_null() {
                unsafe {
                    libc::dlclose(lib_xss);
                    libc::dlclose(lib_x11);
                }
                return None;
            }
            unsafe { std::mem::transmute::<*mut libc::c_void, $ty>(ptr) }
        }};
    }

    let x_open: XOpenDisplayFn = sym!(lib_x11, "XOpenDisplay", XOpenDisplayFn);
    let x_close: XCloseDisplayFn = sym!(lib_x11, "XCloseDisplay", XCloseDisplayFn);
    let x_root: XDefaultRootWindowFn =
        sym!(lib_x11, "XDefaultRootWindow", XDefaultRootWindowFn);
    let x_free: XFreeFn = sym!(lib_x11, "XFree", XFreeFn);
    let xss_alloc: XSSAllocFn = sym!(lib_xss, "XScreenSaverAllocInfo", XSSAllocFn);
    let xss_query: XSSQueryFn = sym!(lib_xss, "XScreenSaverQueryInfo", XSSQueryFn);

    let result = (|| -> Option<u64> {
        let display = unsafe { x_open(ptr::null()) };
        if display.is_null() {
            return None;
        }
        let info = unsafe { xss_alloc() };
        if info.is_null() {
            unsafe { x_close(display) };
            return None;
        }
        let root = unsafe { x_root(display) };
        let ok = unsafe { xss_query(display, root, info) };
        // ok == 0 means the extension is not supported on this display.
        let idle_ms = if ok != 0 {
            Some(unsafe { (*info).idle as u64 })
        } else {
            None
        };
        unsafe { x_free(info as *mut libc::c_void) };
        unsafe { x_close(display) };
        idle_ms
    })();

    unsafe {
        libc::dlclose(lib_xss);
        libc::dlclose(lib_x11);
    }
    result
}

// ---- Strategy 2: XQueryPointer (mouse-position polling) --------------------
//
// We track the last known pointer position in a thread-local so that
// consecutive calls can detect movement without external state.
#[cfg(target_os = "linux")]
fn x11_pointer_idle_duration() -> Option<Duration> {
    use std::cell::Cell;
    use std::ffi::CString;
    use std::ptr;

    // Previous pointer position, stored per-thread.
    thread_local! {
        static LAST_POS: Cell<(i32, i32)> = Cell::new((i32::MIN, i32::MIN));
    }

    let lib_x11 = unsafe {
        libc::dlopen(
            b"libX11.so.6\0".as_ptr() as _,
            libc::RTLD_LAZY | libc::RTLD_GLOBAL,
        )
    };
    if lib_x11.is_null() {
        return None;
    }

    type XOpenDisplayFn = unsafe extern "C" fn(*const libc::c_char) -> *mut libc::c_void;
    type XCloseDisplayFn = unsafe extern "C" fn(*mut libc::c_void) -> libc::c_int;
    type XDefaultRootWindowFn = unsafe extern "C" fn(*mut libc::c_void) -> libc::c_ulong;
    #[allow(clippy::type_complexity)]
    type XQueryPointerFn = unsafe extern "C" fn(
        *mut libc::c_void,  // display
        libc::c_ulong,       // window
        *mut libc::c_ulong,  // root_return
        *mut libc::c_ulong,  // child_return
        *mut libc::c_int,    // root_x_return
        *mut libc::c_int,    // root_y_return
        *mut libc::c_int,    // win_x_return
        *mut libc::c_int,    // win_y_return
        *mut libc::c_uint,   // mask_return
    ) -> libc::c_int;

    macro_rules! sym2 {
        ($name:literal, $ty:ty) => {{
            let s = match CString::new($name) {
                Ok(s) => s,
                Err(_) => {
                    unsafe { libc::dlclose(lib_x11) };
                    return None;
                }
            };
            let ptr = unsafe { libc::dlsym(lib_x11, s.as_ptr()) };
            if ptr.is_null() {
                unsafe { libc::dlclose(lib_x11) };
                return None;
            }
            unsafe { std::mem::transmute::<*mut libc::c_void, $ty>(ptr) }
        }};
    }

    let x_open: XOpenDisplayFn = sym2!("XOpenDisplay", XOpenDisplayFn);
    let x_close: XCloseDisplayFn = sym2!("XCloseDisplay", XCloseDisplayFn);
    let x_root: XDefaultRootWindowFn = sym2!("XDefaultRootWindow", XDefaultRootWindowFn);
    let x_query_ptr: XQueryPointerFn = sym2!("XQueryPointer", XQueryPointerFn);

    let result = (|| -> Option<Duration> {
        let display = unsafe { x_open(ptr::null()) };
        if display.is_null() {
            return None;
        }
        let root = unsafe { x_root(display) };

        let mut root_ret: libc::c_ulong = 0;
        let mut child_ret: libc::c_ulong = 0;
        let mut root_x: libc::c_int = 0;
        let mut root_y: libc::c_int = 0;
        let mut win_x: libc::c_int = 0;
        let mut win_y: libc::c_int = 0;
        let mut mask: libc::c_uint = 0;

        unsafe {
            x_query_ptr(
                display, root,
                &mut root_ret, &mut child_ret,
                &mut root_x, &mut root_y,
                &mut win_x, &mut win_y,
                &mut mask,
            )
        };
        unsafe { x_close(display) };

        let pos = (root_x, root_y);
        let moved = LAST_POS.with(|last| {
            let prev = last.get();
            let changed = prev != pos && prev != (i32::MIN, i32::MIN);
            last.set(pos);
            changed
        });

        // If the pointer moved, report idle = 0 (user is active).
        // If it didn't move, report a large idle time so callers treat it as idle.
        if moved {
            Some(Duration::ZERO)
        } else {
            // Return a value just above the typical idle_timeout so the
            // service stays in Idle without triggering recording on its own.
            Some(Duration::from_secs(60))
        }
    })();

    unsafe { libc::dlclose(lib_x11) };
    result
}

// ---- Strategy 3: /proc/interrupts input counter ---------------------------
// Reads the interrupt count for keyboard-related interrupt sources (i8042,
// PS/2, USB HID, etc.) and returns true when the count changed since the
// last call.  This is a passive read – no special permissions needed.
#[cfg(target_os = "linux")]
fn proc_interrupts_input_changed() -> bool {
    use std::cell::Cell;
    use std::io::{BufRead, BufReader};

    thread_local! {
        static LAST_COUNT: Cell<u64> = Cell::new(0);
    }

    let count = read_input_interrupt_count();
    if count == 0 {
        return false;
    }

    LAST_COUNT.with(|last| {
        let prev = last.get();
        last.set(count);
        prev != 0 && count != prev
    })
}

#[cfg(target_os = "linux")]
fn read_input_interrupt_count() -> u64 {
    use std::fs::File;
    use std::io::{BufRead, BufReader};

    let file = match File::open("/proc/interrupts") {
        Ok(f) => f,
        Err(_) => return 0,
    };
    let mut total: u64 = 0;
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        // Match lines containing keyboard/input-related interrupt sources.
        let lower = line.to_ascii_lowercase();
        if lower.contains("i8042")
            || lower.contains("keyboard")
            || lower.contains("xhci")
            || lower.contains("ehci")
        {
            // Sum all CPU columns (numbers after the IRQ label).
            for tok in line.split_whitespace().skip(1) {
                if let Ok(n) = tok.parse::<u64>() {
                    total = total.saturating_add(n);
                } else {
                    // Stop at first non-numeric token (device name).
                    break;
                }
            }
        }
    }
    total
}

#[cfg(target_os = "macos")]
fn platform_idle_duration_impl() -> Option<Duration> {
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventSourceSecondsSinceLastEventType(state_id: i32, event_type: u32) -> f64;
    }
    // kCGEventSourceStateCombinedSessionState = 1, kCGAnyInputEventType = u32::MAX
    let secs = unsafe { CGEventSourceSecondsSinceLastEventType(1, u32::MAX) };
    if secs < 0.0 || secs > 86_400.0 * 365.0 {
        None
    } else {
        Some(Duration::from_secs_f64(secs))
    }
}

// ---- unsupported -----------------------------------------------------------
#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn platform_idle_duration_impl() -> Option<Duration> {
    None
}
