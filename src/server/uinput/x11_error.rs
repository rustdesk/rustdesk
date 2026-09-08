use hbb_common::{libc, log, x11::xlib};

// Server extension numbers are nonnegative; this entry is local to the probe.
const ERROR_EXTENSION: i32 = -1;
const XI_BAD_DEVICE: i32 = 0;

pub(super) fn with_display(check: impl FnOnce(*mut xlib::Display) -> bool) -> bool {
    let display = unsafe { xlib::XOpenDisplay(std::ptr::null()) };
    if display.is_null() {
        return false;
    }
    let mut failed = false;
    // XCloseDisplay frees extension nodes with free(), not Rust's allocator.
    let data =
        unsafe { libc::calloc(1, std::mem::size_of::<xlib::XExtData>()) }.cast::<xlib::XExtData>();
    if data.is_null() {
        log::error!("Failed to allocate X11 smooth-scroll error context");
        unsafe { xlib::XCloseDisplay(display) };
        return false;
    }
    unsafe {
        (*data).number = ERROR_EXTENSION;
        (*data).private_data = std::ptr::addr_of_mut!(failed).cast();
        // The x11 binding omits XExtData* from this callback's C signature.
        (*data).free_private = Some(std::mem::transmute::<
            unsafe extern "C" fn(*mut xlib::XExtData) -> i32,
            unsafe extern "C" fn() -> i32,
        >(keep_error_flag));
        xlib::XAddToExtensionList(xlib::XEHeadOfExtensionList(display.cast()), data);
    }
    let ready = check(display);
    // Keep the error context alive while XCloseDisplay flushes XCloseDevice errors.
    unsafe { xlib::XCloseDisplay(display) };
    // libXi can return MappingSuccess after a protocol error, so check the recorded error too.
    ready && !failed
}

unsafe extern "C" fn keep_error_flag(_data: *mut xlib::XExtData) -> i32 {
    // The flag lives on with_display's stack, not in Xlib-owned storage.
    xlib::Success.into()
}

pub(super) unsafe fn install(display: *mut xlib::Display, error_base: i32) {
    // This Display belongs only to the readiness probe. A wire-error hook avoids
    // replacing XSetErrorHandler, which is shared with every other X11 thread.
    for error_code in [
        error_base + XI_BAD_DEVICE,
        i32::from(xlib::BadValue),
        i32::from(xlib::BadMatch),
    ] {
        xlib::XESetWireToError(display, error_code, Some(handle_error));
    }
}

unsafe extern "C" fn handle_error(
    display: *mut xlib::Display,
    event: *mut xlib::XErrorEvent,
    _wire_error: *mut xlib::xError,
) -> i32 {
    // Wire-error callbacks hold the Display lock. XFindContext would deadlock here.
    let data =
        xlib::XFindOnExtensionList(xlib::XEHeadOfExtensionList(display.cast()), ERROR_EXTENSION);
    if !data.is_null() && !(*data).private_data.is_null() {
        *(*data).private_data.cast::<bool>() = true;
    } else {
        log::error!("Missing X11 smooth-scroll error context");
    }
    let event = &*event;
    log::error!(
        "X11 smooth-scroll setup failed: error={}, request={}:{}, resource={}",
        event.error_code,
        event.request_code,
        event.minor_code,
        event.resourceid
    );
    xlib::False
}
