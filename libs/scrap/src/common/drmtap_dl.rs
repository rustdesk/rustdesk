// Runtime loader for libdrmtap.so (the DRM/KMS capture engine), loaded via
// dlopen instead of static-linked. This keeps the main rustdesk binary free of
// hard libdrm/libEGL/libGLESv2 dependencies: the .so is only opened when the
// drm capture path is actually used, and if it (or one of its deps) is missing
// the load fails cleanly and the caller falls back to PipeWire/portal. The .so
// is shipped only in the opt-in unattended-wayland package.
//
// The privileged read runs in-process in whatever process opens it. When that
// process already holds CAP_SYS_ADMIN (the root --service) libdrmtap reads the
// scanout directly, without forking the setcap helper (see do_grab() in the C).
//
// Mirrors the graceful-load pattern of libs/libxdo-sys-stub.

use hbb_common::{libloading::Library, log};
use std::os::raw::{c_char, c_int, c_void};
use std::sync::OnceLock;

// ---- C ABI structs (must match libdrmtap include/drmtap.h / libdrmtap-sys) ----

#[repr(C)]
pub struct drmtap_ctx {
    _private: [u8; 0],
}

#[repr(C)]
pub struct drmtap_config {
    pub device_path: *const c_char, // NULL = auto-detect /dev/dri/card*
    pub crtc_id: u32,               // 0 = auto-select first active CRTC
    pub helper_path: *const c_char, // only consulted if a helper is needed (never, when root)
    pub debug: c_int,
}

impl Default for drmtap_config {
    fn default() -> Self {
        Self {
            device_path: std::ptr::null(),
            crtc_id: 0,
            helper_path: std::ptr::null(),
            debug: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct drmtap_display {
    pub crtc_id: u32,
    pub connector_id: u32,
    pub name: [c_char; 32],
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub refresh_hz: u32,
    pub active: c_int,
}

#[repr(C)]
pub struct drmtap_frame_info {
    pub data: *mut c_void,
    pub dma_buf_fd: c_int,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub format: u32,
    pub modifier: u64,
    pub fb_id: u32,
    pub _priv: *mut c_void,
}

#[repr(C)]
pub struct drmtap_cursor_info {
    pub x: i32,
    pub y: i32,
    pub hot_x: i32,
    pub hot_y: i32,
    pub width: u32,
    pub height: u32,
    pub pixels: *mut u32,
    pub visible: c_int,
    pub _priv: *mut c_void,
}

// ---- resolved symbol typedefs ----

type FnVersion = unsafe extern "C" fn() -> c_int;
type FnOpen = unsafe extern "C" fn(*const drmtap_config) -> *mut drmtap_ctx;
type FnClose = unsafe extern "C" fn(*mut drmtap_ctx);
type FnListDisplays = unsafe extern "C" fn(*mut drmtap_ctx, *mut drmtap_display, c_int) -> c_int;
type FnGrabMapped = unsafe extern "C" fn(*mut drmtap_ctx, *mut drmtap_frame_info) -> c_int;
type FnFrameRelease = unsafe extern "C" fn(*mut drmtap_ctx, *mut drmtap_frame_info);
type FnGetCursor = unsafe extern "C" fn(*mut drmtap_ctx, *mut drmtap_cursor_info) -> c_int;
type FnCursorRelease = unsafe extern "C" fn(*mut drmtap_ctx, *mut drmtap_cursor_info);

/// The dlopen'd libdrmtap with its resolved entry points. The `Library` is kept
/// alive for the process lifetime (this lives in a `OnceLock`), so the raw fn
/// pointers stay valid.
pub struct DrmtapLib {
    _lib: Library,
    pub open: FnOpen,
    pub close: FnClose,
    pub list_displays: FnListDisplays,
    pub grab_mapped: FnGrabMapped,
    pub frame_release: FnFrameRelease,
    pub get_cursor: FnGetCursor,
    pub cursor_release: FnCursorRelease,
}

// SAFETY: the resolved fn pointers are plain C entry points with no interior
// mutability; libdrmtap contexts are used single-threaded by the caller. The
// Library handle is never moved out. Matches how libxdo-sys-stub treats XdoLib.
unsafe impl Send for DrmtapLib {}
unsafe impl Sync for DrmtapLib {}

// The #[repr(C)] struct layouts above track libdrmtap's ABI *major* version,
// which in turn tracks the `.so.0` soname. drmtap_version() packs the semver as
// (major << 16) | (minor << 8) | patch. A major mismatch means the structs may
// be laid out differently, so we refuse the library rather than read through a
// mismatched layout. Minor/patch bumps are additive and remain compatible.
const DRMTAP_ABI_MAJOR: c_int = 0;

impl DrmtapLib {
    fn load() -> Option<Self> {
        // soname first (what a packaged .so installs), then the dev symlink.
        const LIB_NAMES: [&str; 2] = ["libdrmtap.so.0", "libdrmtap.so"];
        unsafe {
            let (lib, name) = LIB_NAMES
                .iter()
                .find_map(|n| Library::new(n).ok().map(|l| (l, *n)))?;
            // every symbol is required; a missing one means an incompatible .so,
            // so bail to None and let the caller fall back to PipeWire.
            let version: FnVersion = *lib.get(b"drmtap_version").ok()?;
            // Call it once at load time: this smoke-checks that the .so responds
            // through the resolved entry point *and* lets us reject a rebuilt
            // library whose ABI (struct layout) no longer matches the #[repr(C)]
            // definitions above. Resolving symbols alone would not catch that.
            let v = version();
            let (major, minor, patch) = ((v >> 16) & 0xff, (v >> 8) & 0xff, v & 0xff);
            if major != DRMTAP_ABI_MAJOR {
                log::warn!(
                    "libdrmtap {name} reports ABI major {major} (v{major}.{minor}.{patch}), \
                     expected {DRMTAP_ABI_MAJOR}; refusing to load to avoid struct-layout \
                     mismatch (falling back to PipeWire/portal)"
                );
                return None;
            }
            log::info!("libdrmtap loaded: {name} (v{major}.{minor}.{patch})");
            let open: FnOpen = *lib.get(b"drmtap_open").ok()?;
            let close: FnClose = *lib.get(b"drmtap_close").ok()?;
            let list_displays: FnListDisplays = *lib.get(b"drmtap_list_displays").ok()?;
            let grab_mapped: FnGrabMapped = *lib.get(b"drmtap_grab_mapped").ok()?;
            let frame_release: FnFrameRelease = *lib.get(b"drmtap_frame_release").ok()?;
            let get_cursor: FnGetCursor = *lib.get(b"drmtap_get_cursor").ok()?;
            let cursor_release: FnCursorRelease = *lib.get(b"drmtap_cursor_release").ok()?;
            Some(DrmtapLib {
                _lib: lib,
                open,
                close,
                list_displays,
                grab_mapped,
                frame_release,
                get_cursor,
                cursor_release,
            })
        }
    }
}

static DRMTAP_LIB: OnceLock<Option<DrmtapLib>> = OnceLock::new();

/// Returns the loaded libdrmtap, or None if the .so (or one of its runtime deps)
/// is not present. Loaded once; a failure is remembered (no repeated dlopen).
pub fn get() -> Option<&'static DrmtapLib> {
    DRMTAP_LIB
        .get_or_init(|| {
            let lib = DrmtapLib::load();
            if lib.is_none() {
                log::info!("libdrmtap not available (dlopen failed); DRM capture disabled");
            }
            lib
        })
        .as_ref()
}
