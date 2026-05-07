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
// Platform idle query – returns how long the user has been idle.
// Returns None on Wayland, headless environments, or unsupported platforms.
// ---------------------------------------------------------------------------

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
// Use dlopen to load libX11 + libXss at runtime so the binary does not have a
// hard link-time dependency on libXss-dev.  On Wayland-only systems
// XOpenDisplay returns NULL and we fall back gracefully.
#[cfg(target_os = "linux")]
fn platform_idle_duration_impl() -> Option<Duration> {
    x11_idle_ms().map(Duration::from_millis)
}

#[cfg(target_os = "linux")]
fn x11_idle_ms() -> Option<u64> {
    use std::ffi::CString;
    use std::ptr;

    // Load libraries at runtime.  libXss depends on libX11, both must be open.
    let lib_x11 = unsafe { libc::dlopen(b"libX11.so.6\0".as_ptr() as _, libc::RTLD_LAZY | libc::RTLD_GLOBAL) };
    if lib_x11.is_null() {
        return None;
    }
    let lib_xss = unsafe { libc::dlopen(b"libXss.so.1\0".as_ptr() as _, libc::RTLD_LAZY) };
    if lib_xss.is_null() {
        unsafe { libc::dlclose(lib_x11) };
        return None;
    }

    // Symbol types matching the XScreenSaver C API.
    type XOpenDisplayFn = unsafe extern "C" fn(*const libc::c_char) -> *mut libc::c_void;
    type XCloseDisplayFn = unsafe extern "C" fn(*mut libc::c_void) -> libc::c_int;
    type XDefaultRootWindowFn = unsafe extern "C" fn(*mut libc::c_void) -> libc::c_ulong;
    type XFreeFn = unsafe extern "C" fn(*mut libc::c_void) -> libc::c_int;
    type XScreenSaverAllocInfoFn = unsafe extern "C" fn() -> *mut XSSInfo;
    type XScreenSaverQueryInfoFn = unsafe extern "C" fn(
        *mut libc::c_void,
        libc::c_ulong,
        *mut XSSInfo,
    ) -> libc::c_int;

    #[repr(C)]
    struct XSSInfo {
        window: libc::c_ulong,
        state: libc::c_int,
        kind: libc::c_int,
        til_or_since: libc::c_ulong,
        idle: libc::c_ulong, // idle time in milliseconds
        event_mask: libc::c_ulong,
    }

    macro_rules! sym {
        ($lib:expr, $name:literal, $ty:ty) => {{
            let s = CString::new($name).ok()?;
            let ptr = unsafe { libc::dlsym($lib, s.as_ptr()) };
            if ptr.is_null() {
                return None;
            }
            unsafe { std::mem::transmute::<*mut libc::c_void, $ty>(ptr) }
        }};
    }

    let x_open   = sym!(lib_x11, "XOpenDisplay",         XOpenDisplayFn);
    let x_close  = sym!(lib_x11, "XCloseDisplay",        XCloseDisplayFn);
    let x_root   = sym!(lib_x11, "XDefaultRootWindow",   XDefaultRootWindowFn);
    let x_free   = sym!(lib_x11, "XFree",                XFreeFn);
    let xss_alloc = sym!(lib_xss, "XScreenSaverAllocInfo",  XScreenSaverAllocInfoFn);
    let xss_query = sym!(lib_xss, "XScreenSaverQueryInfo", XScreenSaverQueryInfoFn);

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
        let idle_ms = if ok != 0 { Some(unsafe { (*info).idle as u64 }) } else { None };
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

// ---- macOS -----------------------------------------------------------------
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
