// Runtime loader for libdrmtap.so (the DRM/KMS capture engine), dlopen'd so the binary carries no hard libdrm/libEGL/libGLESv2 dependency.

use hbb_common::{libloading::Library, log};
use std::os::raw::{c_char, c_int, c_void};
use std::sync::OnceLock;

// C ABI structs: must match libdrmtap include/drmtap.h.

#[repr(C)]
pub struct drmtap_ctx {
    _private: [u8; 0],
}

#[repr(C)]
pub struct drmtap_config {
    pub device_path: *const c_char, // NULL = auto-detect /dev/dri/card*
    pub crtc_id: u32,               // 0 = auto-select first active CRTC
    pub helper_path: *const c_char, // only consulted if the direct DRM export is denied (no CAP_SYS_ADMIN)
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

// Descriptor of an externally-supplied scanout DMA-BUF: the privileged exporter fills it via
// `drmtap_grab_desc`; the converter overwrites `dma_buf_fd` with the fd it got via SCM_RIGHTS.
// Mirrors `drmtap_dmabuf_desc` EXACTLY (field order + widths); a mismatch mis-reads CCS/HDR scanouts.
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

// Resolved symbol typedefs.

type FnVersion = unsafe extern "C" fn() -> c_int;
type FnOpen = unsafe extern "C" fn(*const drmtap_config) -> *mut drmtap_ctx;
type FnClose = unsafe extern "C" fn(*mut drmtap_ctx);
type FnListDisplays = unsafe extern "C" fn(*mut drmtap_ctx, *mut drmtap_display, c_int) -> c_int;
type FnListDevices = unsafe extern "C" fn(*mut drmtap_device, c_int) -> c_int;
type FnGrabMapped = unsafe extern "C" fn(*mut drmtap_ctx, *mut drmtap_frame_info) -> c_int;
type FnFrameRelease = unsafe extern "C" fn(*mut drmtap_ctx, *mut drmtap_frame_info);
type FnGetCursor = unsafe extern "C" fn(*mut drmtap_ctx, *mut drmtap_cursor_info) -> c_int;
type FnCursorRelease = unsafe extern "C" fn(*mut drmtap_ctx, *mut drmtap_cursor_info);
// Split-capture entry points (libdrmtap >= 0.4.10), required: `grab_desc` runs on the privileged
// export side, `open_render`/`convert_dmabuf` on the unprivileged converter side.
type FnGrabDesc =
    unsafe extern "C" fn(*mut drmtap_ctx, *mut drmtap_dmabuf_desc, *mut drmtap_frame_info) -> c_int;
type FnOpenRender = unsafe extern "C" fn(*const c_char) -> *mut drmtap_ctx;
// libdrmtap >= 0.4.15; returns a ctx-owned string, or NULL if it has none.
type FnRenderNode = unsafe extern "C" fn(*mut drmtap_ctx) -> *const c_char;
type FnConvertDmabuf =
    unsafe extern "C" fn(*mut drmtap_ctx, *const drmtap_dmabuf_desc, *mut drmtap_frame_info) -> c_int;

/// The dlopen'd libdrmtap; the `Library` is kept alive for the process lifetime, so the raw fn pointers stay valid.
pub struct DrmtapLib {
    _lib: Library,
    pub open: FnOpen,
    pub close: FnClose,
    pub list_displays: FnListDisplays,
    pub list_devices: Option<FnListDevices>,
    pub grab_mapped: FnGrabMapped,
    pub frame_release: FnFrameRelease,
    pub get_cursor: FnGetCursor,
    pub cursor_release: FnCursorRelease,
    pub grab_desc: FnGrabDesc,
    pub open_render: FnOpenRender,
    pub convert_dmabuf: FnConvertDmabuf,
    pub render_node: Option<FnRenderNode>,
    pub version: (c_int, c_int, c_int),
}

// SAFETY: the resolved fn pointers are plain C entry points with no interior mutability;
// libdrmtap contexts are used single-threaded by the caller. The Library handle is never moved out.
unsafe impl Send for DrmtapLib {}
unsafe impl Sync for DrmtapLib {}

const DRMTAP_ABI_MAJOR: c_int = 0;

// Lowest (minor, patch) accepted. 0.5.0 is the floor because it fixes the padded-framebuffer read
// (a scanout whose pitch exceeds width*bpp was decoded at the wrong stride); the whole split API
// has been present since 0.4.10.
const DRMTAP_MIN_MINOR_PATCH: (c_int, c_int) = (5, 0);

// The MINOR series this build's mirrored structs were verified against: libdrmtap's header freezes
// only `drmtap_device` and `drmtap_dmabuf_desc`, so an unverified minor could be read at wrong offsets.
const DRMTAP_ABI_MINOR: c_int = 5;

/// Whether a library reporting `major.minor.patch` may be loaded (major and minor exact, patch at or above the floor).
fn abi_accepted(major: c_int, minor: c_int, patch: c_int) -> bool {
    major == DRMTAP_ABI_MAJOR
        && minor == DRMTAP_ABI_MINOR
        && (minor, patch) >= DRMTAP_MIN_MINOR_PATCH
}

impl DrmtapLib {
    fn load() -> Option<Self> {
        // Absolute path FIRST: the deb bundles the .so privately under /usr/lib/rustdesk and does NOT register that dir with ld.so.
        const INSTALLED: &str = "/usr/lib/rustdesk/libdrmtap.so.0";
        // Bare sonames exist so an unpackaged development build can load a locally built .so from
        // the normal ld.so search path. They are NOT offered when running as root: this is the one
        // place where which file happens to be on the load path decides what gets mapped into the
        // CAP_SYS_ADMIN process, and the packaged service always finds the absolute path first
        // anyway. A root process that reaches the fallback has no bundled library, which is the
        // PipeWire-fallback case, not a reason to search.
        const DEV_ONLY: [&str; 2] = ["libdrmtap.so.0", "libdrmtap.so"];
        let is_root = unsafe { hbb_common::libc::geteuid() } == 0;
        let candidates: Vec<&str> = if is_root {
            vec![INSTALLED]
        } else {
            std::iter::once(INSTALLED).chain(DEV_ONLY).collect()
        };
        unsafe {
            let (lib, name) = candidates
                .iter()
                .find_map(|n| Library::new(*n).ok().map(|l| (l, *n)))?;
            // Canonicalize the absolute candidate only: `dlopen` does not search the CWD for a bare
            // soname, while `canonicalize` resolves a relative name against it.
            let real = std::path::Path::new(name)
                .is_absolute()
                .then(|| std::fs::canonicalize(name).ok())
                .flatten();
            let version: FnVersion = *lib.get(b"drmtap_version").ok()?;
            let v = version();
            let (major, minor, patch) = ((v >> 16) & 0xff, (v >> 8) & 0xff, v & 0xff);
            if !abi_accepted(major, minor, patch) {
                let why = if major != DRMTAP_ABI_MAJOR {
                    "the struct layouts this build mirrors track the ABI major, so reading a \
                     frame descriptor through a mismatched one would mis-decode it"
                } else if minor != DRMTAP_ABI_MINOR {
                    "this build mirrors the struct layouts of one minor and only that one; \
                     under 0.x semver the minor is the breaking axis, so an unverified minor \
                     could be read at the wrong offsets. Widening it is a deliberate act, done \
                     with the layouts re-checked field by field"
                } else {
                    "it predates the split-capture API, so its only capture path converts \
                     in-process, which in the root service means loading the GL stack there"
                };
                let (min_minor, min_patch) = DRMTAP_MIN_MINOR_PATCH;
                log::warn!(
                    "libdrmtap {name} reports v{major}.{minor}.{patch}, which this build cannot \
                     use (needs ABI major {DRMTAP_ABI_MAJOR}, minor {DRMTAP_ABI_MINOR}, at least \
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
            // Log the load only now that every required symbol resolved: this fn still returns None on a missing one.
            let loaded_from = real
                .as_ref()
                .map_or_else(|| name.to_owned(), |p| p.display().to_string());
            if loaded_from == name {
                log::info!("libdrmtap loaded: {name} (v{major}.{minor}.{patch})");
            } else {
                log::info!("libdrmtap loaded: {name} -> {loaded_from} (v{major}.{minor}.{patch})");
            }
            let (no_node, no_devices) = (render_node.is_none(), list_devices.is_none());
            if (minor, patch) >= (4, 15) && (no_node || no_devices) {
                let missing = if no_node && no_devices {
                    "drmtap_render_node and drmtap_list_devices"
                } else if no_node {
                    "drmtap_render_node"
                } else {
                    "drmtap_list_devices"
                };
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

/// The loaded libdrmtap, or None if the .so (or a runtime dep) is absent or its version/exports fall outside the ABI gate. Loaded once; a failure is remembered.
pub fn get() -> Option<&'static DrmtapLib> {
    DRMTAP_LIB
        .get_or_init(|| {
            let lib = DrmtapLib::load();
            if lib.is_none() {
                log::info!("libdrmtap not available or not usable; DRM capture disabled");
            }
            lib
        })
        .as_ref()
}

#[cfg(test)]
mod tests {
    use super::{abi_accepted, DRMTAP_ABI_MAJOR, DRMTAP_ABI_MINOR, DRMTAP_MIN_MINOR_PATCH};

    #[test]
    fn abi_gate_rejects_a_library_from_before_the_split() {
        // These are refused because their MINOR differs from the verified one, which is the only
        // reason the gate needs. Naming the pre-split releases keeps the intent readable, but do
        // not read this as the floor doing the work: see the test below.
        for (minor, patch) in [(3, 3), (4, 0), (4, 8), (4, 9)] {
            assert!(
                !abi_accepted(DRMTAP_ABI_MAJOR, minor, patch),
                "v0.{minor}.{patch} is not the verified minor and must be refused"
            );
        }
    }

    #[test]
    fn the_patch_floor_is_currently_vacuous_and_that_is_deliberate() {
        // With MIN_MINOR_PATCH.0 == DRMTAP_ABI_MINOR the floor can never reject anything: the
        // minor equality already forces `(minor, patch) >= (minor, 0)`. It is kept because it is
        // the mechanism that WOULD do the work the next time a floor lands mid-minor, as (4, 10)
        // did for the split API. This test exists so nobody reads the pre-split test above as
        // evidence that the floor is live -- if that ever matters, this assert is the tripwire.
        let (floor_minor, floor_patch) = DRMTAP_MIN_MINOR_PATCH;
        assert_eq!(
            floor_minor, DRMTAP_ABI_MINOR,
            "the floor is inside the verified minor; a floor in a DIFFERENT minor is unreachable"
        );
        if floor_patch == 0 {
            assert!(
                abi_accepted(DRMTAP_ABI_MAJOR, DRMTAP_ABI_MINOR, 0),
                "patch 0 of the verified minor must be accepted while the floor is 0"
            );
        } else {
            assert!(!abi_accepted(DRMTAP_ABI_MAJOR, DRMTAP_ABI_MINOR, floor_patch - 1));
        }
    }

    #[test]
    fn abi_gate_accepts_the_floor_and_later_patches_of_the_same_minor() {
        let (min_minor, min_patch) = DRMTAP_MIN_MINOR_PATCH;
        assert!(abi_accepted(DRMTAP_ABI_MAJOR, min_minor, min_patch));
        for (minor, patch) in [(DRMTAP_ABI_MINOR, min_patch + 15), (DRMTAP_ABI_MINOR, 200)] {
            assert!(
                abi_accepted(DRMTAP_ABI_MAJOR, minor, patch),
                "v0.{minor}.{patch} is a patch of the verified minor and must be accepted"
            );
        }
    }

    #[test]
    fn abi_gate_rejects_an_unknown_newer_minor() {
        // Relative to DRMTAP_ABI_MINOR, so the next bump cannot leave this test asserting that the
        // NEW verified minor must be refused -- which is what a hardcoded list did before.
        let verified = DRMTAP_ABI_MINOR;
        for (minor, patch) in [
            (verified - 1, 99),
            (verified + 1, 0),
            (verified + 1, 99),
            (verified + 4, 9),
        ] {
            assert!(
                !abi_accepted(DRMTAP_ABI_MAJOR, minor, patch),
                "v0.{minor}.{patch} is an unverified minor and must be refused"
            );
        }
    }

    #[test]
    fn abi_gate_rejects_another_major_in_both_directions() {
        assert!(!abi_accepted(DRMTAP_ABI_MAJOR + 1, 0, 0));
        assert!(!abi_accepted(DRMTAP_ABI_MAJOR + 1, 99, 99));
    }
}
