// Service-side DRM/KMS read engine, in the ROOT `--service`: libdrmtap reads the scanout in-process (direct mode). The DRM_DEVICE env is not consulted here.

use super::drmtap_dl::{
    self, drmtap_config, drmtap_ctx, drmtap_cursor_info, drmtap_device, drmtap_display,
    drmtap_dmabuf_desc, drmtap_frame_info, DrmtapLib,
};
use hbb_common::log;
use std::ffi::CString;
use std::os::raw::c_int;
use std::io;
use std::os::fd::{FromRawFd, OwnedFd};

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

/// Where a cursor bitmap's hotspot most likely is, from its opaque pixels alone: the top-left of
/// the opaque bounding box for an arrow, its centre for an ELONGATED shape (an I-beam), in either
/// direction. Only meaningful on an UPRIGHT sprite - a rotated arrow's tip is some other corner of
/// its box.
///
/// The aspect test is symmetric because a text cursor is not always the vertical one. Adwaita's
/// `vertical-text` shape is a horizontal I-beam: at 24 px its opaque box is 20x9 and the theme's
/// own hotspot is (12, 11), i.e. its centre. A tall-only test calls that shape an arrow and returns
/// (2, 6), which is 11.2 px from the truth; the centre is 1.4 px away. Checked against all 35
/// shapes of the installed theme at 24 px: making the test symmetric moves exactly that one shape
/// and leaves the other 34 byte-identical, arrows included.
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

static LAST_PROVENANCE: std::sync::atomic::AtomicI8 = std::sync::atomic::AtomicI8::new(-1);

/// Say once, and again only if it changes, where the hotspot is coming from. Without this the
/// three states are indistinguishable on a running box, which makes the decision unverifiable
/// anywhere but a unit test.
fn note_hotspot_provenance(answered: Option<bool>, hot_measured: bool) {
    let now = provenance_code(answered);
    let last = LAST_PROVENANCE.swap(now, std::sync::atomic::Ordering::Relaxed);
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
    if bh > bw * 2 || bw > bh * 2 {
        ((minx + maxx) / 2, (miny + maxy) / 2)
    } else {
        (minx, miny)
    }
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
                note_hotspot_provenance(answered, hot_measured);
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
