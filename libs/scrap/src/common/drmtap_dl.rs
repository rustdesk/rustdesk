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

// Descriptor of an externally-supplied scanout DMA-BUF (the split-capture
// contract). Mirrors `drmtap_dmabuf_desc` in libdrmtap include/drmtap.h EXACTLY
// (field order + widths); a mismatch mis-reads CCS/HDR scanouts. The privileged
// exporter fills it in one call via `drmtap_grab_desc`; the unprivileged
// converter receives it over IPC, overwrites `dma_buf_fd` with the fd it got via
// SCM_RIGHTS, and passes it to `drmtap_convert_dmabuf`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct drmtap_dmabuf_desc {
    pub dma_buf_fd: c_int, // scanout DMA-BUF; -1 for an already-imported fb_id
    pub width: u32,
    pub height: u32,
    pub format: u32,           // DRM fourcc of the scanout
    pub modifier: u64,         // DRM format modifier (tiling/compression)
    pub fb_id: u32,            // import-once cache key; 0 disables caching
    pub num_planes: u32,       // used entries in offsets/pitches (1..4); 0 => 1
    pub offsets: [u32; 4],     // per-plane byte offsets (CCS main+aux+clear-color)
    pub pitches: [u32; 4],     // per-plane strides; pitches[0] = main stride
    pub hdr_eotf: u32,         // DRMTAP_EOTF_* (SDR=0, PQ=2, HLG=3)
    pub hdr_max_nits: u32,     // mastering/content peak luminance cd/m2; 0=unknown
}

impl Default for drmtap_dmabuf_desc {
    fn default() -> Self {
        Self {
            dma_buf_fd: -1,
            width: 0,
            height: 0,
            format: 0,
            modifier: 0,
            fb_id: 0,
            num_planes: 0,
            offsets: [0; 4],
            pitches: [0; 4],
            hdr_eotf: 0,
            hdr_max_nits: 0,
        }
    }
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
// Split-capture entry points (libdrmtap >= 0.4.9). Bound OPTIONALLY (see below).
// `grab_desc` runs on the privileged export side; `open_render`/`convert_dmabuf`
// on the unprivileged converter side.
type FnGrabDesc =
    unsafe extern "C" fn(*mut drmtap_ctx, *mut drmtap_dmabuf_desc, *mut drmtap_frame_info) -> c_int;
type FnOpenRender = unsafe extern "C" fn(*const c_char) -> *mut drmtap_ctx;
type FnConvertDmabuf =
    unsafe extern "C" fn(*mut drmtap_ctx, *const drmtap_dmabuf_desc, *mut drmtap_frame_info) -> c_int;

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
    // Split-capture symbols (present only on libdrmtap >= 0.4.9). `None` on an
    // older .so; callers gate on `Some(..)` and fall back to the mapped path.
    // Root needs `grab_desc`; the unprivileged converter needs
    // `open_render` + `convert_dmabuf`.
    pub grab_desc: Option<FnGrabDesc>,
    pub open_render: Option<FnOpenRender>,
    pub convert_dmabuf: Option<FnConvertDmabuf>,
    // Parsed (major, minor, patch) from `drmtap_version()`, for feature gating.
    pub version: (c_int, c_int, c_int),
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
        // Absolute install path FIRST: the deb bundles the .so privately under /usr/lib/rustdesk and
        // deliberately does NOT register that dir in the system-wide ld.so search path (Debian Policy
        // 10.2 forbids a private lib shadowing system libraries for every binary), so the packaged
        // build must resolve it by absolute path. The bare sonames remain as a fallback for a dev build
        // where the .so is reachable via LD_LIBRARY_PATH or a local ldconfig.
        const LIB_NAMES: [&str; 3] = [
            "/usr/lib/rustdesk/libdrmtap.so.0",
            "libdrmtap.so.0",
            "libdrmtap.so",
        ];
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
            // Split-capture symbols are bound OPTIONALLY (not through the `.ok()?`
            // chain above): a pre-0.4.9 .so lacks them, and forcing them here would
            // fail the WHOLE load and silently disable DRM. Each side checks the
            // symbol it needs before taking the split path.
            let grab_desc: Option<FnGrabDesc> = lib.get(b"drmtap_grab_desc").ok().map(|s| *s);
            let open_render: Option<FnOpenRender> = lib.get(b"drmtap_open_render").ok().map(|s| *s);
            let convert_dmabuf: Option<FnConvertDmabuf> =
                lib.get(b"drmtap_convert_dmabuf").ok().map(|s| *s);
            Some(DrmtapLib {
                _lib: lib,
                open,
                close,
                list_displays,
                grab_mapped,
                frame_release,
                get_cursor,
                cursor_release,
                grab_desc,
                open_render,
                convert_dmabuf,
                version: (major, minor, patch),
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
