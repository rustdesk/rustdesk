// Service-side DRM/KMS read engine, in the ROOT `--service`: libdrmtap reads the scanout in-process (direct mode). The DRM_DEVICE env is not consulted here.

use super::drmtap_dl::{
    self, drmtap_config, drmtap_ctx, drmtap_cursor_info, drmtap_device, drmtap_display,
    drmtap_dmabuf_desc, drmtap_frame_info, DrmtapLib,
};
use hbb_common::log;
use std::ffi::CString;
use std::io;
use std::os::fd::{FromRawFd, OwnedFd};
use std::os::raw::c_int;

// Trust-boundary limits and formats `drm_render` (the unprivileged converter) imports: two copies that drift apart would weaken one side.
// 16384 covers 8K+ with headroom; anything larger is rejected as a bogus/hostile geometry.
pub(crate) const MAX_DIM: u32 = 16384;
// 256 MiB covers an 8K BGRA frame (7680x4320x4 ~= 127 MiB) with margin.
pub(crate) const MAX_FRAME_BYTES: usize = 256 * 1024 * 1024;
// XRGB/ARGB are little-endian B,G,R,{X,A} in memory == `Pixfmt::BGRA`; XBGR/ABGR are R,G,B,{X,A} == `Pixfmt::RGBA`.
pub(crate) const DRM_FORMAT_XRGB8888: u32 = 0x3432_5258; // 'XR24'
pub(crate) const DRM_FORMAT_ARGB8888: u32 = 0x3432_5241; // 'AR24'
pub(crate) const DRM_FORMAT_XBGR8888: u32 = 0x3432_4258; // 'XB24'
pub(crate) const DRM_FORMAT_ABGR8888: u32 = 0x3432_4241; // 'AB24'

/// Cursor id published when the plane reports the cursor hidden, so the id changes and, where the DRM cursor is authoritative, the client drops the last shape.
pub const HIDDEN_CURSOR_ID: u64 = u64::MAX;

pub struct CursorSnapshot {
    pub id: u64,
    pub width: u32,
    pub height: u32,
    pub hotx: i32,
    pub hoty: i32,
    /// Whether the hotspot came from the plane's HOTSPOT_X/Y properties. Those exist on
    /// para-virtualised drivers only; on bare metal it is `infer_hotspot`'s guess, made on the
    /// bitmap as scanned out - which on a rotated output is the compositor's pre-rotated sprite,
    /// so the consumer must re-guess on the upright one.
    pub hot_measured: bool,
    pub colors: Vec<u8>,
}

/// Fold everything that makes a cursor a DIFFERENT cursor into its id.
///
/// The producer dedupes by comparing this against the last one it sent, so anything left out is
/// something a change in which the consumer will never be told about. Geometry and the hotspot are
/// the obvious ones: identical pixels at a new size or hotspot are a new shape.
///
/// `hot_measured` belongs here too, and that is less obvious. It is not metadata about the cursor,
/// it selects what the consumer DOES with it: false means rotate the bitmap and re-infer a
/// hotspot, true means map the supplied point. So a sample whose pixels and `(0, 0)` hotspot are
/// unchanged but whose provenance flipped is a different cursor as far as the client is concerned,
/// and without this it would be deduped away and never sent.
fn cursor_id(hash: u64, width: u32, height: u32, hotx: i32, hoty: i32, hot_measured: bool) -> u64 {
    let mut id = hash;
    for v in [
        width as u64,
        height as u64,
        hotx as u32 as u64,
        hoty as u32 as u64,
        hot_measured as u64,
    ] {
        id ^= v;
        id = id.wrapping_mul(1099511628211);
    }
    id
}

/// Encodes a provenance answer for the one-shot log below. -1 is "nothing reported yet".
fn provenance_code(answered: Option<bool>) -> i8 {
    match answered {
        None => 0,
        Some(false) => 1,
        Some(true) => 2,
    }
}

/// Whether this sample's provenance is worth a log line, given what was last reported.
///
/// Its own function because the obvious version of this is a line per cursor read, and a cursor is
/// read far more often than it changes. A per-sample log here was a real defect in libdrmtap for
/// the same reason. So: the first sample of a run, and thereafter only a genuine change of state.
fn provenance_is_news(last: i8, now: i8) -> bool {
    last != now
}

impl DrmReader {
    /// Say once, and again only if it changes, where the hotspot is coming from. Without this the
    /// three states are indistinguishable on a running box, which makes the decision unverifiable
    /// anywhere but a unit test.
    fn note_hotspot_provenance(&mut self, answered: Option<bool>, hot_measured: bool) {
        let now = provenance_code(answered);
        let last = std::mem::replace(&mut self.last_provenance, now);
        if !provenance_is_news(last, now) {
            return;
        }
        match answered {
            Some(true) => log::info!(
                "drm: cursor hotspot provenance: the driver published HOTSPOT_X/Y, using its value"
            ),
            Some(false) => log::info!(
                "drm: cursor hotspot provenance: the plane exposes no HOTSPOT_X/Y, inferring the \
                 hotspot from the bitmap"
            ),
            None => log::info!(
                "drm: cursor hotspot provenance: unavailable (libdrmtap older than 0.5.6, or a \
                 pre-0.5.6 privileged helper); falling back to the coordinate heuristic, which says \
                 hot_measured={hot_measured}"
            ),
        }
    }
}

/// Whether `hot_x`/`hot_y` are the driver's answer rather than a guess.
///
/// `answered` is what `drmtap_cursor_hotspot_valid` said: `Some(v)` when it answered, `None` when
/// the symbol is absent (a library older than 0.5.6) or it reported that nothing recorded an
/// answer for this sample.
///
/// When it answered, that is the answer, full stop - including `Some(true)` on a `(0, 0)` hotspot,
/// which is the whole reason the entry point exists: a para-virtualized driver really can put the
/// hotspot at the top-left corner, and the coordinates alone cannot tell that apart from a plane
/// that exposes no HOTSPOT_X/Y at all. When it could not answer, fall back to the old test. That
/// test is wrong in both directions, but it is what this code did before, and it keeps a deployed
/// older `.so` behaving exactly as it used to instead of changing under it.
fn hot_measured_from(answered: Option<bool>, hot_x: i32, hot_y: i32) -> bool {
    match answered {
        Some(measured) => measured,
        None => hot_x != 0 || hot_y != 0,
    }
}

/// A mirror has to agree on this many of every 100 pixels of the union to count as symmetry.
/// Swept against master over two theme versions: below 86 the rule starts moving shapes whose
/// corner guess was already right (`grabbing`), and from 92 up the directional cursors of the
/// older theme stop being recognised and regress. 86 to 91 is the interval that holds on both,
/// so this sits in the middle of it.
const MIRROR_AGREEMENT_PERCENT: u64 = 88;

/// Guess the click point of a cursor bitmap, for the drivers that publish no hotspot.
///
/// Three facts drive it, in order of how much of the shape they read.
///
/// A shape that is mirror symmetric about one of the box's mid-lines has no distinguished end on
/// that axis: a crosshair, an I-beam, a two-headed resize arrow. On such an axis the click point
/// is the middle.
///
/// A shape that mirrors onto itself only under a half turn or about a DIAGONAL is a two-headed or
/// corner resize cursor. Those are a pointer plus the marker of the edge being resized, and the
/// theme puts the click point on that marker, which is the heavier end. The heavier end is also
/// the only thing that separates `e-resize` from `w-resize`: they are the same picture reflected.
///
/// Everything else keeps the top-left corner of the opaque box, because cursors are DRAWN
/// pointing up and to the left, so that corner is where an arrow's tip is. That is a fact about
/// the sprite in its own upright frame, and the reason this guess is only meaningful when taken
/// there rather than on a sprite the compositor pre-rotated.
///
/// Measured against the hotspots the theme declares, which is the value a compositor programs
/// into HOTSPOT_X/Y where the property exists, and against what master delivers over the angles
/// master handles (0, 90, 270). adwaita-icon-theme 50 at 24 px: mean error per shape 12.08 px
/// down to 4.20, better on 83 of the 105 (shape, angle) cases and worse on 4. The same holds at
/// 48 px and on adwaita-icon-theme 46.
///
/// Two shapes stay further out than master and no bitmap rule reaches them: `help`, whose click
/// point the theme puts on the dot of its question mark, and `alias` by half a pixel. That is why
/// a hotspot the driver publishes is always preferred to this, and why rustdesk#16122 recovers it
/// exactly from the injected pointer instead of guessing.
pub fn infer_hotspot(rgba: &[u8], w: usize, h: usize) -> (i32, i32) {
    let (mut minx, mut miny, mut maxx, mut maxy) = (w as i32, h as i32, -1i32, -1i32);
    for (i, px) in rgba.chunks_exact(4).take(w * h).enumerate() {
        if px[3] >= 128 {
            let (x, y) = ((i % w) as i32, (i / w) as i32);
            minx = minx.min(x);
            maxx = maxx.max(x);
            miny = miny.min(y);
            maxy = maxy.max(y);
        }
    }
    if maxx < minx || maxy < miny {
        return (0, 0);
    }
    let (bw, bh) = (maxx - minx + 1, maxy - miny + 1);
    let (cx, cy) = ((minx + maxx) / 2, (miny + maxy) / 2);
    // An elongated box keeps its centre whichever way it lies (rustdesk#16242): a bar has no
    // corner to speak of. Kept ahead of the mirror test so it still covers an elongated shape
    // that is NOT symmetric, which the theme happens not to contain but a custom cursor can.
    if bh > bw * 2 || bw > bh * 2 {
        return (cx, cy);
    }
    let opaque = |x: i32, y: i32| {
        rgba.get((y as usize * w + x as usize) * 4 + 3)
            .is_some_and(|&a| a >= 128)
    };
    let agrees = |i: u64, u: u64| u == 0 || i * 100 >= u * MIRROR_AGREEMENT_PERCENT;
    // One pass over the opaque box, comparing it with its own three mirrors, and the mass of each
    // half while we are here.
    let (mut ix, mut ux) = (0u64, 0u64);
    let (mut iy, mut uy) = (0u64, 0u64);
    let (mut ih, mut uh) = (0u64, 0u64);
    let (mut left, mut right, mut top, mut bottom) = (0u64, 0u64, 0u64, 0u64);
    let (half_w, half_h) = (bw / 2, bh / 2);
    for y in miny..=maxy {
        for x in minx..=maxx {
            let (fx, fy) = (maxx - (x - minx), maxy - (y - miny));
            let here = opaque(x, y);
            for (other, (i, u)) in [
                (opaque(fx, y), (&mut ix, &mut ux)),
                (opaque(x, fy), (&mut iy, &mut uy)),
                (opaque(fx, fy), (&mut ih, &mut uh)),
            ] {
                *i += u64::from(here && other);
                *u += u64::from(here || other);
            }
            if here {
                if x - minx < half_w {
                    left += 1;
                } else if maxx - x < half_w {
                    right += 1;
                }
                if y - miny < half_h {
                    top += 1;
                } else if maxy - y < half_h {
                    bottom += 1;
                }
            }
        }
    }
    // Symmetric under a half turn and the centre is the only point the shape cannot tell from
    // itself, whichever the per-axis answers are: a two-headed diagonal arrow mirrors to the
    // other diagonal on each axis alone, so neither axis reads as symmetric.
    if agrees(ih, uh) {
        return (cx, cy);
    }
    let diagonal = mirrors_about_a_diagonal(&opaque, minx, miny, bw, bh, &agrees);
    // An axis is DIRECTIONAL when the shape is a pointer plus an edge marker rather than a plain
    // arrow: the perpendicular axis mirrors (the n/e/s/w resize cursors), or the shape mirrors
    // about a diagonal (the corner ones). The marker is the end the click point belongs to, and
    // it is the heavier end, which is the only thing that tells `e-resize` from `w-resize` - they
    // are the same picture reflected. Everything else keeps the up-left corner.
    let side = |heavier_is_max: bool, lo: i32, hi: i32| if heavier_is_max { hi } else { lo };
    (
        if agrees(ix, ux) {
            cx
        } else if agrees(iy, uy) || diagonal {
            side(right > left, minx, maxx)
        } else {
            minx
        },
        if agrees(iy, uy) {
            cy
        } else if agrees(ix, ux) || diagonal {
            side(bottom > top, miny, maxy)
        } else {
            miny
        },
    )
}

/// Whether the opaque box mirrors onto itself about either of its diagonals.
///
/// A diagonal reflection of a `bw x bh` box is a `bh x bw` box, so the two can only be compared
/// inside a square, and where the box is not square the answer depends on where in that square it
/// is placed: the corner resize cursors are drawn against one corner, and padding away from that
/// corner moves the shape off the diagonal it is symmetric about. `ne-resize` scores 0.62 anchored
/// top-left and 0.98 anchored bottom-right; `nw-resize` is the other way round. So both anchorings
/// are tried and the best answer wins, which is what "the shape is symmetric" means once the
/// reflection is allowed to be shifted back into place.
fn mirrors_about_a_diagonal(
    opaque: &impl Fn(i32, i32) -> bool,
    minx: i32,
    miny: i32,
    bw: i32,
    bh: i32,
    agrees: &impl Fn(u64, u64) -> bool,
) -> bool {
    let k = bw.max(bh);
    for (dx, dy) in [(0, 0), (k - bw, k - bh)] {
        // The box placed at (dx, dy) inside a k x k square, empty everywhere else.
        let cell = |r: i32, c: i32| {
            let (x, y) = (c - dx, r - dy);
            (0..bw).contains(&x) && (0..bh).contains(&y) && opaque(minx + x, miny + y)
        };
        let (mut id, mut ud, mut ia, mut ua) = (0u64, 0u64, 0u64, 0u64);
        for r in 0..k {
            for c in 0..k {
                let here = cell(r, c);
                for (other, (i, u)) in [
                    (cell(c, r), (&mut id, &mut ud)),
                    (cell(k - 1 - c, k - 1 - r), (&mut ia, &mut ua)),
                ] {
                    *i += u64::from(here && other);
                    *u += u64::from(here || other);
                }
            }
        }
        if agrees(id, ud) || agrees(ia, ua) {
            return true;
        }
    }
    false
}

/// One enumerated DRM display, physical geometry only (the server overlays the Wayland logical origin/scale where it can match one).
pub struct DisplaySnapshot {
    pub name: String,
    pub crtc_id: u32,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub active: bool,
}

pub struct DrmDevice {
    pub path: String,
    /// Render node, or empty if this device has none.
    pub render_node: String,
    pub display_count: u32,
}

/// Copy a fixed C char array into a `String`, stopping at the first NUL WITHIN the array, so a
/// field libdrmtap failed to terminate cannot read past it.
fn cstr_field(buf: &[std::os::raw::c_char]) -> String {
    // SAFETY: c_char and u8 share size/alignment; the slice is the exact length of `buf`.
    let bytes: &[u8] =
        unsafe { std::slice::from_raw_parts(buf.as_ptr() as *const u8, buf.len()) };
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// Enumerate every DRM device with KMS resources. `None` = unavailable, too old, or failed (the caller then scans /dev/dri/card* itself); empty `Vec` = none found.
pub fn list_devices() -> Option<Vec<DrmDevice>> {
    let lib = drmtap_dl::get()?;
    let f = lib.list_devices?;
    const MAX: usize = 16;
    let mut raw: [drmtap_device; MAX] = unsafe { std::mem::zeroed() };
    // SAFETY: `raw` is MAX valid, zeroed drmtap_device slots; the call fills up to MAX and returns the count.
    let n = unsafe { f(raw.as_mut_ptr(), MAX as std::os::raw::c_int) };
    if n < 0 {
        log::warn!("drmtap_list_devices failed ({n}); using single-device auto-detect");
        return None;
    }
    let n = (n as usize).min(MAX);
    Some(
        raw[..n]
            .iter()
            .map(|d| DrmDevice {
                path: cstr_field(&d.path),
                render_node: cstr_field(&d.render_node),
                display_count: d.display_count,
            })
            .collect(),
    )
}

/// The CANONICAL path, when `path` canonicalizes to a node directly under /dev/dri/, else `None`.
/// Callers must open the value returned: opening the original re-resolves every symlink component after the check.
pub(super) fn device_under_dev_dri(path: &str) -> Option<std::path::PathBuf> {
    let p = std::fs::canonicalize(path).ok()?;
    if p.parent() == Some(std::path::Path::new("/dev/dri")) {
        Some(p)
    } else {
        None
    }
}

/// An open DRM read context. Not Send/Sync deliberately (the raw ctx is used on one thread).
pub struct DrmReader {
    lib: &'static DrmtapLib,
    ctx: *mut drmtap_ctx,
    buf: Vec<u8>,
    /// Last provenance reported by THIS reader, for the one-shot log. Per reader and not global:
    /// several readers run at once, one per captured display, and their provenance can legitimately
    /// differ and be stable -- a virtio-gpu output publishes HOTSPOT_X/Y while an i915 one on the
    /// same host does not. A single global would read those two steady streams as an endless
    /// alternation and log on every sample, which is the per-frame logging this was written to
    /// avoid. -1 is "nothing reported yet".
    last_provenance: i8,
}

impl DrmReader {
    /// Open the DRM device. `device = None` auto-detects, `Some(path)` is realpath-gated to /dev/dri/. `crtc_id = 0` auto-selects the first active CRTC.
    pub fn open(device: Option<&str>, crtc_id: u32) -> Option<DrmReader> {
        let lib = drmtap_dl::get()?;
        let device_cstr = match device {
            None => None,
            Some(d) => {
                let Some(canonical) = device_under_dev_dri(d) else {
                    log::warn!("DRM device {d:?} is not under /dev/dri; refusing to open");
                    return None;
                };
                match canonical.to_str().and_then(|s| CString::new(s).ok()) {
                    Some(c) => Some(c),
                    None => return None,
                }
            }
        };
        let cfg = drmtap_config {
            device_path: device_cstr.as_ref().map_or(std::ptr::null(), |c| c.as_ptr()),
            crtc_id,
            helper_path: std::ptr::null(),
            debug: 0,
        };
        // SAFETY: cfg is a valid struct; device_cstr outlives this call.
        let ctx = unsafe { (lib.open)(&cfg) };
        drop(device_cstr);
        if ctx.is_null() {
            log::info!("drmtap_open failed; DRM capture unavailable");
            return None;
        }
        Some(DrmReader {
            last_provenance: -1,
            lib,
            ctx,
            buf: Vec::new(),
        })
    }

    /// Grab one frame, tightly packed as BGRA (`w*4*h` bytes), into the internal buffer; valid until the next grab.
    pub fn grab(&mut self) -> io::Result<(&[u8], usize, usize)> {
        // SAFETY: ctx is valid; frame is zeroed before the call. The frame is released on every return path that OWNS one: a failing
        // `drmtap_grab_mapped` leaves nothing to release, and releasing anyway would be a double free.
        unsafe {
            let mut frame: drmtap_frame_info = std::mem::zeroed();
            let ret = (self.lib.grab_mapped)(self.ctx, &mut frame);
            if ret < 0 {
                let errno = -ret;
                if errno == hbb_common::libc::EAGAIN
                    || errno == hbb_common::libc::EBUSY
                    || errno == hbb_common::libc::EINTR
                {
                    return Err(io::ErrorKind::WouldBlock.into());
                }
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("drmtap_grab_mapped failed: errno {errno}"),
                ));
            }
            if frame.data.is_null() || frame.width == 0 || frame.height == 0 {
                (self.lib.frame_release)(self.ctx, &mut frame);
                return Err(io::ErrorKind::WouldBlock.into());
            }
            let w = frame.width;
            let h = frame.height;
            let stride = frame.stride as usize;
            // The row copy reads w*4 bytes from a source only stride*height bytes: reject sub-32bpp / insane geometry to avoid an OOB read.
            if w > MAX_DIM || h > MAX_DIM || stride < (w as usize) * 4 {
                log::warn!(
                    "DRM scanout not 32-bit BGRA-compatible ({w}x{h} stride {stride} fourcc {:#010x}); falling back",
                    frame.format
                );
                (self.lib.frame_release)(self.ctx, &mut frame);
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "unsupported DRM scanout format",
                ));
            }
            // XBGR8888 passes the stride check but, labeled BGRA downstream, would ship red and blue swapped; a zero fourcc falls through to the stride invariant (kept for libdrmtap builds that do not set it).
            if frame.format != 0
                && frame.format != DRM_FORMAT_XRGB8888
                && frame.format != DRM_FORMAT_ARGB8888
            {
                log::warn!(
                    "DRM scanout fourcc {:#010x} is not BGRA-compatible; falling back",
                    frame.format
                );
                (self.lib.frame_release)(self.ctx, &mut frame);
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "unsupported DRM scanout format",
                ));
            }
            let (w, h) = (w as usize, h as usize);
            let frame_size = match w.checked_mul(4).and_then(|x| x.checked_mul(h)) {
                Some(sz) if sz > 0 && sz <= MAX_FRAME_BYTES => sz,
                other => {
                    log::warn!(
                        "DRM scanout geometry {w}x{h} yields an out-of-range frame ({other:?} bytes); falling back"
                    );
                    (self.lib.frame_release)(self.ctx, &mut frame);
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        "DRM scanout frame too large",
                    ));
                }
            };
            // Bound the SOURCE extent too: the row loop reads up to (h-1)*stride + w*4, and `y * stride` can overflow.
            match stride.checked_mul(h) {
                Some(sz) if sz > 0 && sz <= MAX_FRAME_BYTES => {}
                other => {
                    log::warn!(
                        "DRM scanout stride {stride} x {h} rows is out of range ({other:?} bytes); falling back"
                    );
                    (self.lib.frame_release)(self.ctx, &mut frame);
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        "DRM scanout stride out of range",
                    ));
                }
            }
            if self.buf.len() != frame_size {
                self.buf.resize(frame_size, 0);
            }
            let src = frame.data as *const u8;
            let dst = self.buf.as_mut_ptr();
            if stride == w * 4 {
                std::ptr::copy_nonoverlapping(src, dst, frame_size);
            } else {
                for y in 0..h {
                    std::ptr::copy_nonoverlapping(src.add(y * stride), dst.add(y * w * 4), w * 4);
                }
            }
            (self.lib.frame_release)(self.ctx, &mut frame);
            Ok((&self.buf, w, h))
        }
    }

    /// Render node of the GPU this reader captures from, so the converter binds to the device that EXPORTS the scanout:
    /// importing across vendors can fail on an incompatible tiling modifier. `None` if the symbol is absent or the device is display-only.
    pub fn render_node(&mut self) -> Option<String> {
        let f = self.lib.render_node?;
        // SAFETY: self.ctx is valid; the returned pointer is owned by the context and stays valid until it is closed.
        let ptr = unsafe { f(self.ctx) };
        if ptr.is_null() {
            return None;
        }
        unsafe { std::ffi::CStr::from_ptr(ptr) }
            .to_str()
            .ok()
            .map(|s| s.to_owned())
    }

    /// Zero-copy EXPORT grab: fills a `drmtap_dmabuf_desc` (dma-buf fd, plane layout, HDR metadata) WITHOUT mapping, detiling or copying pixels, so on this
    /// path the root process never loads libEGL/libGLESv2. The exported fd is READ-ONLY (libdrmtap drops `DRM_RDWR` and `dup` shares that open file
    /// description), so the `--server` that receives it can map the scanout but never write the live framebuffer. Validation here is METADATA ONLY.
    pub fn grab_desc(&mut self) -> io::Result<(OwnedFd, drmtap_dmabuf_desc)> {
        let grab_desc = self.lib.grab_desc;
        // SAFETY: self.ctx is valid; desc/frame are zeroed before the call. Only paths that reach a populated frame release it: on `-EINVAL`
        // libdrmtap returns before allocating, a failed inner grab has already cleaned up, and on `-ENOTSUP` libdrmtap releases the frame itself.
        unsafe {
            let mut desc: drmtap_dmabuf_desc = std::mem::zeroed();
            let mut frame: drmtap_frame_info = std::mem::zeroed();
            let ret = grab_desc(self.ctx, &mut desc, &mut frame);
            if ret < 0 {
                let errno = -ret;
                if errno == hbb_common::libc::EAGAIN
                    || errno == hbb_common::libc::EBUSY
                    || errno == hbb_common::libc::EINTR
                {
                    return Err(io::ErrorKind::WouldBlock.into());
                }
                if errno == hbb_common::libc::ENOTSUP {
                    // A distinct error so the caller degrades to the mapped/PipeWire path instead of tight-looping a rebuild.
                    return Err(io::Error::new(
                        io::ErrorKind::Unsupported,
                        "drmtap_grab_desc: no transferable dma-buf (ENOTSUP)",
                    ));
                }
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("drmtap_grab_desc failed: errno {errno}"),
                ));
            }
            // `desc.dma_buf_fd` is the canonical fd (what split_capture.c sends); `frame` owns it too and `frame_release` closes the library's copy.
            let raw_fd = if desc.dma_buf_fd >= 0 {
                desc.dma_buf_fd
            } else {
                frame.dma_buf_fd
            };
            if raw_fd < 0 {
                (self.lib.frame_release)(self.ctx, &mut frame);
                return Err(io::ErrorKind::WouldBlock.into());
            }
            let w = desc.width;
            let h = desc.height;
            if w == 0 || h == 0 || w > MAX_DIM || h > MAX_DIM {
                (self.lib.frame_release)(self.ctx, &mut frame);
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("DRM scanout geometry {w}x{h} out of range"),
                ));
            }
            // No fourcc gate here: the converter handles every format libdrmtap supports, and gating here dropped convertible scanouts such as XR30.
            let planes = if desc.num_planes == 0 { 1 } else { desc.num_planes };
            if planes > 4 {
                (self.lib.frame_release)(self.ctx, &mut frame);
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("DRM scanout num_planes {} out of range (1..=4)", desc.num_planes),
                ));
            }
            for p in 0..(planes as usize) {
                let extent = (desc.pitches[p] as usize)
                    .checked_mul(h as usize)
                    .and_then(|rows| rows.checked_add(desc.offsets[p] as usize));
                match extent {
                    Some(end) if end <= MAX_FRAME_BYTES => {}
                    other => {
                        (self.lib.frame_release)(self.ctx, &mut frame);
                        return Err(io::Error::new(
                            io::ErrorKind::Other,
                            format!(
                                "DRM scanout plane {p} out of range (offset {} pitch {} over {h} rows -> {other:?}, cap {MAX_FRAME_BYTES})",
                                desc.offsets[p], desc.pitches[p]
                            ),
                        ));
                    }
                }
            }
            // dup BEFORE releasing the frame: after release the library may recycle its handle, while an independent fd on the same open dma-buf
            // keeps the buffer alive for the peer. F_DUPFD_CLOEXEC, not dup(): `dup` never copies close-on-exec and this root service forks elsewhere.
            let dup_fd = hbb_common::libc::fcntl(raw_fd, hbb_common::libc::F_DUPFD_CLOEXEC, 0);
            if dup_fd < 0 {
                let e = io::Error::last_os_error();
                (self.lib.frame_release)(self.ctx, &mut frame);
                return Err(e);
            }
            let owned = OwnedFd::from_raw_fd(dup_fd);
            (self.lib.frame_release)(self.ctx, &mut frame);
            desc.num_planes = planes;
            desc.dma_buf_fd = -1;
            Ok((owned, desc))
        }
    }

    /// Read the hardware cursor plane: the hidden sentinel when the plane reports the cursor invisible, the real shape when visible, and `None` when the read fails.
    pub fn cursor(&mut self) -> Option<CursorSnapshot> {
        // SAFETY: ctx valid; c zeroed; released on EVERY path after a successful get_cursor. Only a failed get_cursor returns without releasing, because then there is nothing to release.
        unsafe {
            let mut c: drmtap_cursor_info = std::mem::zeroed();
            let cret = (self.lib.get_cursor)(self.ctx, &mut c);
            if cret != 0 {
                return None;
            }
            let out = if c.visible == 0 {
                Some(CursorSnapshot {
                    id: HIDDEN_CURSOR_ID,
                    width: 1,
                    height: 1,
                    hotx: 0,
                    hoty: 0,
                    hot_measured: false,
                    colors: vec![0, 0, 0, 0],
                })
            } else if !c.pixels.is_null()
                && c.width > 0
                && c.height > 0
                && (c.width as i64) * (c.height as i64) <= 256 * 256
            {
                let cw = c.width as i32;
                let ch = c.height as i32;
                let n = (cw * ch) as usize;
                let src = std::slice::from_raw_parts(c.pixels, n);
                let mut hash: u64 = 1469598103934665603;
                let mut colors = Vec::with_capacity(n * 4);
                for &p in src.iter() {
                    colors.push(((p >> 16) & 0xff) as u8);
                    colors.push(((p >> 8) & 0xff) as u8);
                    colors.push((p & 0xff) as u8);
                    colors.push(((p >> 24) & 0xff) as u8);
                    hash ^= p as u64;
                    hash = hash.wrapping_mul(1099511628211);
                }
                // Ask the library where the hotspot came from instead of guessing from the
                // coordinates. `hot_x/hot_y == (0, 0)` means two opposite things - the plane
                // exposes no HOTSPOT_X/Y (every bare-metal driver), or it exposes them and the
                // driver's answer IS the top-left corner - and the old test below cannot separate
                // them: it overrides a real (0, 0) measurement with a guess from the bitmap, and
                // on a driver without the properties it would trust a hotspot nobody published.
                // Asked BEFORE cursor_release, which is what owns this sample.
                let answered = self.lib.cursor_hotspot_valid.and_then(|f| {
                    let mut valid: c_int = 0;
                    // 0 = answered; -ENOTSUP = nothing recorded an answer for this sample, which
                    // is what a cursor read through a pre-0.5.6 privileged helper produces.
                    (f(&c, &mut valid) == 0).then(|| valid != 0)
                });
                let hot_measured = hot_measured_from(answered, c.hot_x, c.hot_y);
                self.note_hotspot_provenance(answered, hot_measured);
                let (hotx, hoty) = if hot_measured {
                    (c.hot_x, c.hot_y)
                } else {
                    infer_hotspot(&colors, cw as usize, ch as usize)
                };
                // Fold geometry + hotspot into the id: identical pixels with a changed size or
                // hotspot must count as a new shape, otherwise drm_capture_worker suppresses the
                // update (it dedupes by id) and the client keeps rendering the stale cursor.
                let id = cursor_id(hash, cw as u32, ch as u32, hotx, hoty, hot_measured);
                Some(CursorSnapshot {
                    id,
                    width: cw as u32,
                    height: ch as u32,
                    hotx,
                    hoty,
                    hot_measured,
                    colors,
                })
            } else {
                None
            };
            (self.lib.cursor_release)(self.ctx, &mut c);
            out
        }
    }

    pub fn displays(&mut self) -> Vec<DisplaySnapshot> {
        // SAFETY: ctx valid; raw is a zeroed, correctly-sized array; count is clamped to the buffer before indexing.
        unsafe {
            let mut raw = vec![std::mem::zeroed::<drmtap_display>(); 16];
            let cap = raw.len() as i32;
            let n = (self.lib.list_displays)(self.ctx, raw.as_mut_ptr(), cap);
            if n <= 0 {
                return Vec::new();
            }
            let count = (n as usize).min(raw.len());
            (0..count)
                .map(|i| {
                    let name_bytes: Vec<u8> = raw[i]
                        .name
                        .iter()
                        .take_while(|&&ch| ch != 0)
                        .map(|&ch| ch as u8)
                        .collect();
                    DisplaySnapshot {
                        name: String::from_utf8_lossy(&name_bytes).to_string(),
                        crtc_id: raw[i].crtc_id,
                        x: raw[i].x as i32,
                        y: raw[i].y as i32,
                        width: raw[i].width,
                        height: raw[i].height,
                        active: raw[i].active != 0,
                    }
                })
                .collect()
        }
    }
}

impl Drop for DrmReader {
    fn drop(&mut self) {
        if !self.ctx.is_null() {
            // SAFETY: ctx came from drmtap_open and is non-null.
            unsafe { (self.lib.close)(self.ctx) };
            self.ctx = std::ptr::null_mut();
        }
    }
}

#[cfg(test)]
mod hotspot_provenance_tests {
    use super::{cursor_id, hot_measured_from, provenance_code, provenance_is_news};

    /// The log this makes observable is one line per CHANGE, not one per cursor read. A cursor is
    /// read far more often than its provenance changes, and a per-sample line here would be the
    /// same defect libdrmtap already had once.
    #[test]
    fn provenance_is_logged_once_and_then_only_on_a_change() {
        let (none, guessed, measured) = (
            provenance_code(None),
            provenance_code(Some(false)),
            provenance_code(Some(true)),
        );
        assert_ne!(none, guessed);
        assert_ne!(guessed, measured);
        assert_ne!(none, measured);
        // First sample of a run: -1 is the unset sentinel, so anything is news.
        assert!(provenance_is_news(-1, measured));
        // The same state again is not.
        assert!(!provenance_is_news(measured, measured));
        assert!(!provenance_is_news(guessed, guessed));
        // A real transition is.
        assert!(provenance_is_news(guessed, measured));
        assert!(provenance_is_news(measured, none));
    }

    /// The dedup key has to move when the provenance does, or the consumer is never told that the
    /// same picture now means something different. This is the transition that produces it: the
    /// properties become readable, the driver's answer is the corner, and the bitmap is unchanged.
    #[test]
    fn provenance_changes_the_cursor_identity() {
        let guessed = cursor_id(0xabc, 24, 24, 0, 0, false);
        let measured = cursor_id(0xabc, 24, 24, 0, 0, true);
        assert_ne!(
            guessed, measured,
            "same pixels, same (0, 0), different meaning: the producer must not dedupe this away"
        );
        // And the pre-existing parts still count, so this did not trade one blind spot for another.
        assert_ne!(cursor_id(0xabc, 24, 24, 0, 0, true), cursor_id(0xabc, 32, 24, 0, 0, true));
        assert_ne!(cursor_id(0xabc, 24, 24, 0, 0, true), cursor_id(0xabc, 24, 24, 1, 0, true));
        assert_ne!(cursor_id(0xabc, 24, 24, 0, 0, true), cursor_id(0xdef, 24, 24, 0, 0, true));
    }

    /// The case the whole entry point exists for, and the one the old heuristic got backwards: a
    /// para-virtualized driver that really publishes the hotspot at the image's top-left corner.
    /// `(0, 0)` measured is still measured, and re-inferring one from the bitmap would move a
    /// cursor the driver had already placed.
    #[test]
    fn a_measured_zero_hotspot_is_measured() {
        assert!(hot_measured_from(Some(true), 0, 0));
    }

    /// And the mirror: when the library says it was NOT measured, that is the answer even if the
    /// coordinates happen to be non-zero. The old test would have trusted them.
    #[test]
    fn the_librarys_no_beats_the_coordinates() {
        assert!(!hot_measured_from(Some(false), 12, 11));
        assert!(!hot_measured_from(Some(false), 0, 0));
    }

    /// No answer available: either the symbol is absent (a library older than 0.5.6) or nothing
    /// recorded one for this sample (a cursor read through an older privileged helper). Neither
    /// is "it was a guess", so the pre-existing behaviour is kept rather than resolved either way
    /// -- an older deployed .so must keep behaving as it used to.
    #[test]
    fn without_an_answer_the_old_heuristic_stands() {
        assert!(!hot_measured_from(None, 0, 0));
        assert!(hot_measured_from(None, 12, 11));
        assert!(hot_measured_from(None, 0, 5));
    }
}

#[cfg(test)]
mod hotspot_guess_tests {
    use super::infer_hotspot;

    /// A sprite from an ASCII picture: `#` is opaque, anything else transparent. Written this way
    /// because every case here is about a SHAPE, and a shape is unreadable as a byte array.
    fn sprite(rows: &[&str]) -> (Vec<u8>, usize, usize) {
        let (h, w) = (rows.len(), rows[0].len());
        let mut px = vec![0u8; w * h * 4];
        for (y, row) in rows.iter().enumerate() {
            assert_eq!(row.len(), w, "ragged picture");
            for (x, c) in row.bytes().enumerate() {
                if c == b'#' {
                    px[(y * w + x) * 4 + 3] = 255;
                }
            }
        }
        (px, w, h)
    }

    /// An arrow keeps the top-left corner of its box. Cursors are drawn pointing up and left, so
    /// that corner IS the tip - and it is a fact about the sprite upright, which is why the
    /// consumer re-takes this guess after turning a rotated sprite back.
    #[test]
    fn an_arrow_points_at_the_corner_of_its_box() {
        let (px, w, h) = sprite(&[
            ".........",
            ".#.......",
            ".##......",
            ".###.....",
            ".####....",
            ".#####...",
            ".###.....",
            ".#..##...",
            "......##.",
        ]);
        assert_eq!(infer_hotspot(&px, w, h), (1, 1));
    }

    /// A shape that mirrors onto itself about the box's mid-lines has no tip to point with, so
    /// the corner names nothing and the middle is the only answer the shape supports. This is the
    /// crosshair/cell/resize family, 17 of the installed theme's 35 shapes.
    #[test]
    fn a_symmetric_shape_is_held_by_its_middle() {
        let (px, w, h) = sprite(&[
            "....#....",
            "....#....",
            "....#....",
            "....#....",
            "#########",
            "....#....",
            "....#....",
            "....#....",
            "....#....",
        ]);
        assert_eq!(infer_hotspot(&px, w, h), (4, 4));
    }

    /// Each axis is decided by its OWN mirror. This shape is symmetric left-to-right and not
    /// top-to-bottom, which is the single-direction resize family: the click point is centred
    /// across the axis that mirrors and stays at the edge on the axis that does not.
    #[test]
    fn one_symmetric_axis_centres_that_axis_only() {
        let (px, w, h) = sprite(&[
            "....#....",
            "...###...",
            "..#####..",
            ".#######.",
            "....#....",
            "....#....",
            "....#....",
            "....#....",
            "....#....",
        ]);
        assert_eq!(infer_hotspot(&px, w, h), (4, 0));
    }

    /// A two-headed DIAGONAL arrow mirrors onto the other diagonal about either axis alone, so
    /// neither axis reads as symmetric - and it is still a shape with no tip. The half turn is
    /// what catches it.
    #[test]
    fn a_diagonal_two_headed_arrow_is_held_by_its_middle_too() {
        let (px, w, h) = sprite(&[
            "###......",
            "##.......",
            "#.#......",
            "...#.....",
            "....#....",
            ".....#...",
            "......#.#",
            ".......##",
            "......###",
        ]);
        let (hx, hy) = infer_hotspot(&px, w, h);
        assert_eq!((hx, hy), (4, 4), "a half turn maps this shape onto itself");
    }

    /// A single-direction resize cursor is an arrow plus the bar of the edge it resizes, and the
    /// theme puts the click point on the BAR. The bar is the heavy end, and it is the only thing
    /// that separates `e-resize` from `w-resize`: they are the same picture reflected, so a rule
    /// that always took the up-left corner had to be wrong for one of the two.
    #[test]
    fn a_directional_resize_cursor_is_held_by_its_bar() {
        let (px, w, h) = sprite(&[
            "......####",
            "..##..####",
            ".###..####",
            "####..####",
            ".###..####",
            "..##..####",
            "......####",
        ]);
        assert_eq!(infer_hotspot(&px, w, h), (9, 3));
        // The mirror image has to answer the other side, or the rule is not reading the shape.
        let (px, w, h) = sprite(&[
            "####......",
            "####..##..",
            "####..###.",
            "####..####",
            "####..###.",
            "####..##..",
            "####......",
        ]);
        assert_eq!(infer_hotspot(&px, w, h), (0, 3));
    }

    /// A corner resize cursor is a bracket hugging one corner, and it mirrors onto itself about a
    /// DIAGONAL rather than about either axis, so neither per-axis test sees it. The click point
    /// is the inner corner of the bracket, which is the heavy end on both axes.
    ///
    /// A real one's box is usually not square, and a diagonal reflection only lines up inside a
    /// square, so which corner of that square the box is placed against decides the answer:
    /// `ne-resize` scores 0.62 anchored one way and 0.98 the other, and `nw-resize` is the
    /// reverse. Both anchorings are therefore tried. That case cannot be built as a small
    /// picture - a non-square box can never be exactly diagonal-symmetric - so it is covered by
    /// the theme corpus instead, where dropping the second anchoring sends `ne-resize` back past
    /// master.
    #[test]
    fn a_corner_bracket_is_held_by_its_inner_corner() {
        let (px, w, h) = sprite(&[
            ".....##", ".....##", ".....##", ".....##", ".....##", "#######", "#######",
        ]);
        assert_eq!(infer_hotspot(&px, w, h), (6, 6));
    }

    /// The elongation rule stays ahead of the mirror test, so a bar that is NOT symmetric still
    /// keeps its centre. Nothing in the installed theme is shaped like this - its two I-beams are
    /// both symmetric - but a custom cursor can be, and this is the rustdesk#16242 round-one fix.
    #[test]
    fn an_elongated_shape_keeps_its_centre_even_when_it_is_lopsided() {
        let (px, w, h) = sprite(&["##########.", "#####......", "##########."]);
        assert_eq!(infer_hotspot(&px, w, h), (4, 1));
    }

    /// Nothing opaque is not a shape, and must not read as a box at the origin with a hotspot in
    /// the middle of it.
    #[test]
    fn a_blank_sprite_has_no_hotspot_to_guess() {
        let (px, w, h) = sprite(&["...", "...", "..."]);
        assert_eq!(infer_hotspot(&px, w, h), (0, 0));
    }
}
