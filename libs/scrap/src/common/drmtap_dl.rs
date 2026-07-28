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

// A capturable DRM device from `drmtap_list_devices` (libdrmtap >= 0.4.15).
// Mirrors `drmtap_device` in include/drmtap.h EXACTLY (field order + widths);
// its layout is FROZEN there for the same reason as drmtap_dmabuf_desc (written
// into caller-owned storage). `path`/`render_node`/`driver` are NUL-terminated.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct drmtap_device {
    pub path: [c_char; 64],
    pub render_node: [c_char; 64],
    pub driver: [c_char; 32],
    pub display_count: u32,
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
// libdrmtap >= 0.4.15. Enumerates every DRM device with KMS resources, so a
// multi-GPU host can open one context per device instead of only advertising the
// first card's displays. Bound as an Option; `None` on an older .so.
type FnListDevices = unsafe extern "C" fn(*mut drmtap_device, c_int) -> c_int;
type FnGrabMapped = unsafe extern "C" fn(*mut drmtap_ctx, *mut drmtap_frame_info) -> c_int;
type FnFrameRelease = unsafe extern "C" fn(*mut drmtap_ctx, *mut drmtap_frame_info);
type FnGetCursor = unsafe extern "C" fn(*mut drmtap_ctx, *mut drmtap_cursor_info) -> c_int;
type FnCursorRelease = unsafe extern "C" fn(*mut drmtap_ctx, *mut drmtap_cursor_info);
// Split-capture entry points (libdrmtap >= 0.4.10). REQUIRED (see below).
// `grab_desc` runs on the privileged export side; `open_render`/`convert_dmabuf`
// on the unprivileged converter side.
type FnGrabDesc =
    unsafe extern "C" fn(*mut drmtap_ctx, *mut drmtap_dmabuf_desc, *mut drmtap_frame_info) -> c_int;
type FnOpenRender = unsafe extern "C" fn(*const c_char) -> *mut drmtap_ctx;
// libdrmtap >= 0.4.15. Names the render node of the device a context is bound to,
// so the exporter can tell the converter which GPU to bind to instead of leaving
// it to auto-selection. Returns a ctx-owned string, or NULL if it has none.
type FnRenderNode = unsafe extern "C" fn(*mut drmtap_ctx) -> *const c_char;
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
    // libdrmtap >= 0.4.15; `None` on an older .so (the service then enumerates a
    // single auto-detected device, exactly as before).
    pub list_devices: Option<FnListDevices>,
    pub grab_mapped: FnGrabMapped,
    pub frame_release: FnFrameRelease,
    pub get_cursor: FnGetCursor,
    pub cursor_release: FnCursorRelease,
    // Split-capture symbols (libdrmtap >= 0.4.10). Not optional: a library that
    // cannot do the split is refused at load time (see `abi_accepted`), so these
    // are plain pointers and the type system carries the guarantee that no
    // caller can silently take an in-process-convert path instead.
    // Root needs `grab_desc`; the unprivileged converter needs
    // `open_render` + `convert_dmabuf`.
    pub grab_desc: FnGrabDesc,
    pub open_render: FnOpenRender,
    pub convert_dmabuf: FnConvertDmabuf,
    // libdrmtap >= 0.4.15; `None` on an older .so, where the converter keeps
    // relying on `open_render(NULL)` auto-selection exactly as before.
    pub render_node: Option<FnRenderNode>,
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

// Lowest (minor, patch) this build accepts. The project is still 0.x, so the
// major alone bounds nothing: every release it has ever made reports major 0,
// and comparing only that accepts a library from before the split existed.
//
// 0.4.10 is the oldest release with the WHOLE split API: `drmtap_open_render`
// and `drmtap_convert_dmabuf` arrived in 0.4.9, `drmtap_grab_desc` in 0.4.10.
// That is the oldest library that can serve the architecture this code
// implements, where the privileged process exports the scanout dma-buf and NEVER
// converts, so it never loads libEGL/libGLESv2. Below it the only way to capture
// is the in-process convert, in the ROOT service, which is precisely the
// property the split exists to remove: treat such a library as unusable and fall
// back to PipeWire/portal rather than quietly pull the vendor GL stack into the
// privileged process because a stale file happened to be on the load path.
//
// The mirrored `#[repr(C)]` layouts above are unchanged across 0.4.9..0.4.15
// (verified field by field against include/drmtap.h at both ends), so the floor
// costs no compatibility that was real.
const DRMTAP_MIN_MINOR_PATCH: (c_int, c_int) = (4, 10);

/// Whether a library reporting `major.minor.patch` may be loaded. Pure, so the
/// version rule is unit-testable without an .so to dlopen: the major must match
/// exactly (struct layouts track it) and (minor, patch) must be at or above the
/// floor that provides the split-capture API.
fn abi_accepted(major: c_int, minor: c_int, patch: c_int) -> bool {
    major == DRMTAP_ABI_MAJOR && (minor, patch) >= DRMTAP_MIN_MINOR_PATCH
}

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
            // Resolve what we ACTUALLY opened, for the absolute candidate only. That name is a soname
            // symlink, so a second file declaring the same soname beside the packaged one (an upgrade
            // leftover, a hand-built .so) can end up being the one it points at, and then the path we
            // asked for tells the reader nothing about which library is loaded. ONLY for an absolute
            // name: `dlopen` does not search the process CWD for a bare soname (it uses DT_RUNPATH,
            // LD_LIBRARY_PATH, the ld.so cache, the default dirs), while `canonicalize` resolves a
            // relative name against the CWD, so canonicalizing a fallback could name a same-named file
            // the loader never touched. For those we keep logging the plain name.
            let real = std::path::Path::new(name)
                .is_absolute()
                .then(|| std::fs::canonicalize(name).ok())
                .flatten();
            // every symbol is required; a missing one means an incompatible .so,
            // so bail to None and let the caller fall back to PipeWire.
            let version: FnVersion = *lib.get(b"drmtap_version").ok()?;
            // Call it once at load time: this smoke-checks that the .so responds
            // through the resolved entry point *and* lets us reject a rebuilt
            // library whose ABI (struct layout) no longer matches the #[repr(C)]
            // definitions above. Resolving symbols alone would not catch that.
            let v = version();
            let (major, minor, patch) = ((v >> 16) & 0xff, (v >> 8) & 0xff, v & 0xff);
            if !abi_accepted(major, minor, patch) {
                let why = if major != DRMTAP_ABI_MAJOR {
                    "the struct layouts this build mirrors track the ABI major, so reading a \
                     frame descriptor through a mismatched one would mis-decode it"
                } else {
                    "it predates the split-capture API, so its only capture path converts \
                     in-process, which in the root service means loading the GL stack there"
                };
                let (min_minor, min_patch) = DRMTAP_MIN_MINOR_PATCH;
                log::warn!(
                    "libdrmtap {name} reports v{major}.{minor}.{patch}, which this build cannot \
                     use (needs ABI major {DRMTAP_ABI_MAJOR}, at least \
                     v{DRMTAP_ABI_MAJOR}.{min_minor}.{min_patch}): {why}. Refusing to load; \
                     falling back to PipeWire/portal."
                );
                return None;
            }
            let open: FnOpen = *lib.get(b"drmtap_open").ok()?;
            let close: FnClose = *lib.get(b"drmtap_close").ok()?;
            let list_displays: FnListDisplays = *lib.get(b"drmtap_list_displays").ok()?;
            let list_devices: Option<FnListDevices> =
                lib.get(b"drmtap_list_devices").ok().map(|s| *s);
            let grab_mapped: FnGrabMapped = *lib.get(b"drmtap_grab_mapped").ok()?;
            let frame_release: FnFrameRelease = *lib.get(b"drmtap_frame_release").ok()?;
            let get_cursor: FnGetCursor = *lib.get(b"drmtap_get_cursor").ok()?;
            let cursor_release: FnCursorRelease = *lib.get(b"drmtap_cursor_release").ok()?;
            // Split-capture symbols are required too. The version floor above already turns
            // away the libraries that predate them; requiring the symbols as well covers what
            // the floor cannot see, a library that REPORTS a new enough version without
            // carrying the API. That is not hypothetical (see the stale-build note below), and
            // it fails the same way: no split export means the only capture path left runs the
            // convert in the root service. Both refusals disable DRM capture and fall back to
            // PipeWire/portal, which is the outcome we want.
            // Resolved as a group and reported by name rather than through a bare `?`, so the
            // log says which symbols are absent instead of the generic "dlopen failed" line,
            // which would send whoever reads it hunting for a missing file.
            let grab: Option<FnGrabDesc> = lib.get(b"drmtap_grab_desc").ok().map(|s| *s);
            let open_r: Option<FnOpenRender> = lib.get(b"drmtap_open_render").ok().map(|s| *s);
            let conv: Option<FnConvertDmabuf> =
                lib.get(b"drmtap_convert_dmabuf").ok().map(|s| *s);
            let (grab_desc, open_render, convert_dmabuf) = match (grab, open_r, conv) {
                (Some(g), Some(o), Some(c)) => (g, o, c),
                (grab, open_r, conv) => {
                    let mut missing = Vec::new();
                    if grab.is_none() {
                        missing.push("drmtap_grab_desc");
                    }
                    if open_r.is_none() {
                        missing.push("drmtap_open_render");
                    }
                    if conv.is_none() {
                        missing.push("drmtap_convert_dmabuf");
                    }
                    log::warn!(
                        "libdrmtap {name} reports v{major}.{minor}.{patch} but does not export \
                         {}: it is a stale or pre-release build, not the version it claims. \
                         Refusing to load; falling back to PipeWire/portal.",
                        missing.join(", ")
                    );
                    return None;
                }
            };
            let render_node: Option<FnRenderNode> =
                lib.get(b"drmtap_render_node").ok().map(|s| *s);
            // Log the load only now that every required symbol resolved: this function still returns
            // None on a missing one, and announcing success first would print "libdrmtap loaded"
            // followed by "libdrmtap not available" for the same library.
            let loaded_from = real
                .as_ref()
                .map_or_else(|| name.to_owned(), |p| p.display().to_string());
            if loaded_from == name {
                log::info!("libdrmtap loaded: {name} (v{major}.{minor}.{patch})");
            } else {
                log::info!("libdrmtap loaded: {name} -> {loaded_from} (v{major}.{minor}.{patch})");
            }
            // A library can REPORT a version whose symbols it does not actually have, and that is not
            // hypothetical: the multi-GPU accessors landed after an earlier build had already stamped
            // itself 0.4.15, so a stale copy of that build keeps claiming 0.4.15 while lacking them.
            // It degrades SILENTLY (the service stops naming the exporting GPU, so the converter is
            // left guessing), so say it out loud and name the file, because the version alone lies.
            let (no_node, no_devices) = (render_node.is_none(), list_devices.is_none());
            if (minor, patch) >= (4, 15) && (no_node || no_devices) {
                let missing = if no_node && no_devices {
                    "drmtap_render_node and drmtap_list_devices"
                } else if no_node {
                    "drmtap_render_node"
                } else {
                    "drmtap_list_devices"
                };
                // Name only the capability each absent symbol actually costs.
                let effect = if no_node && no_devices {
                    "Multi-GPU display enumeration and exporting-GPU selection stay disabled."
                } else if no_node {
                    "Exporting-GPU selection stays disabled."
                } else {
                    "Multi-GPU display enumeration stays disabled."
                };
                log::warn!(
                    "libdrmtap at {loaded_from} reports v{major}.{minor}.{patch} but is missing \
                     {missing}: it is a stale or pre-release build. Check what the soname symlink \
                     points at and remove any leftover libdrmtap.so.0* beside it. {effect}"
                );
            }
            Some(DrmtapLib {
                _lib: lib,
                open,
                close,
                list_displays,
                list_devices,
                grab_mapped,
                frame_release,
                get_cursor,
                cursor_release,
                grab_desc,
                open_render,
                convert_dmabuf,
                render_node,
                version: (major, minor, patch),
            })
        }
    }
}

static DRMTAP_LIB: OnceLock<Option<DrmtapLib>> = OnceLock::new();

/// Returns the loaded libdrmtap, or None if the .so (or one of its runtime deps)
/// is not present, or is too old to serve the split-capture architecture (see
/// `abi_accepted`). Loaded once; a failure is remembered (no repeated dlopen).
pub fn get() -> Option<&'static DrmtapLib> {
    DRMTAP_LIB
        .get_or_init(|| {
            let lib = DrmtapLib::load();
            if lib.is_none() {
                // Deliberately not "dlopen failed": the load also declines a library that opens
                // fine but is too old or does not carry the split API, and each of those paths
                // has already said so, with the file name, at warn level.
                log::info!("libdrmtap not available or not usable; DRM capture disabled");
            }
            lib
        })
        .as_ref()
}

#[cfg(test)]
mod tests {
    use super::{abi_accepted, DRMTAP_ABI_MAJOR, DRMTAP_MIN_MINOR_PATCH};

    #[test]
    fn abi_gate_rejects_a_library_from_before_the_split() {
        // The releases that predate drmtap_grab_desc. Accepting any of these means the
        // privileged service has no export-only path and converts in-process, which is
        // the whole thing the split was built to prevent. 0.4.9 is in the list on
        // purpose: it introduced the convert half of the split but not the export half,
        // so it cannot serve the privileged side either.
        for (minor, patch) in [(3, 3), (4, 0), (4, 8), (4, 9)] {
            assert!(
                !abi_accepted(DRMTAP_ABI_MAJOR, minor, patch),
                "v0.{minor}.{patch} predates the split-capture API and must be refused"
            );
        }
    }

    #[test]
    fn abi_gate_accepts_the_floor_and_every_release_above_it() {
        let (min_minor, min_patch) = DRMTAP_MIN_MINOR_PATCH;
        assert!(abi_accepted(DRMTAP_ABI_MAJOR, min_minor, min_patch));
        // 0.4.15 is what the deb ships today; the later ones guard against a floor
        // comparison that only ever looks at `patch` (0.5.0 must pass, 0.4.15 too).
        for (minor, patch) in [(4, 15), (4, 200), (5, 0), (9, 9)] {
            assert!(
                abi_accepted(DRMTAP_ABI_MAJOR, minor, patch),
                "v0.{minor}.{patch} is at or above the floor and must be accepted"
            );
        }
    }

    #[test]
    fn abi_gate_rejects_another_major_in_both_directions() {
        // The mirrored #[repr(C)] layouts track the major, so a newer one is as unsafe
        // to read through as an older one, however high its minor.
        assert!(!abi_accepted(DRMTAP_ABI_MAJOR + 1, 0, 0));
        assert!(!abi_accepted(DRMTAP_ABI_MAJOR + 1, 99, 99));
    }
}
