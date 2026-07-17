// Service-side DRM/KMS read engine. Runs in the ROOT `--service`, which already
// holds CAP_SYS_ADMIN, so libdrmtap reads the scanout in-process (direct mode,
// no helper fork, no setcap). Loaded via the dlopen loader (drmtap_dl) so the
// main binary has no hard libdrm/EGL dependency.
//
// SECURITY (direct-mode mitigation): the scanout parse now runs in the root
// service with no seccomp cage, so we do NOT honor an untrusted device path.
// The caller passes either None (libdrmtap auto-detects /dev/dri/card* by a
// hardcoded pattern) or an explicit path that we realpath-gate to /dev/dri/
// before opening. The DRM_DEVICE env is intentionally NOT consulted here.

use super::drmtap_dl::{
    self, drmtap_config, drmtap_ctx, drmtap_cursor_info, drmtap_display, drmtap_frame_info,
    DrmtapLib,
};
use hbb_common::log;
use std::ffi::CString;
use std::io;

// Largest scanout we will copy; also bounds w*4*h against overflow. 16384 covers
// 8K+ with headroom; anything larger is rejected as a bogus/hostile geometry.
const MAX_DIM: u32 = 16384;

/// Sentinel cursor id published when the plane reports the cursor hidden, so the
/// id changes and the client drops the last shape. Distinct from any real hash.
pub const HIDDEN_CURSOR_ID: u64 = u64::MAX;

/// A hardware-cursor snapshot to ship to the server (RGBA colors).
pub struct CursorSnapshot {
    pub id: u64,
    pub width: u32,
    pub height: u32,
    pub hotx: i32,
    pub hoty: i32,
    pub colors: Vec<u8>,
}

/// One enumerated DRM display (physical geometry only; the server augments with
/// the Wayland logical geometry/scale, which needs the user session).
pub struct DisplaySnapshot {
    pub name: String,
    pub crtc_id: u32,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub active: bool,
}

/// Returns true only if `path` canonicalizes to a node directly under /dev/dri/.
/// This is the realpath gate the libdrmtap helper applied but the in-process
/// (direct) path does not, so the service must apply it itself.
fn device_under_dev_dri(path: &str) -> bool {
    match std::fs::canonicalize(path) {
        Ok(p) => p.parent().map_or(false, |d| d == std::path::Path::new("/dev/dri")),
        Err(_) => false,
    }
}

/// An open DRM read context. Not Send/Sync deliberately (the raw ctx is used on
/// one thread, like the old Capturer).
pub struct DrmReader {
    lib: &'static DrmtapLib,
    ctx: *mut drmtap_ctx,
    // grow-once packed-BGRA scratch buffer (preallocated model): resized up to the
    // frame size and never shrunk.
    buf: Vec<u8>,
}

impl DrmReader {
    /// Open the DRM device. `device = None` auto-detects (safe); `Some(path)` is
    /// realpath-gated to /dev/dri/. `crtc_id = 0` auto-selects the first active
    /// CRTC (primary); a non-zero value targets that specific CRTC/display (from
    /// `displays()`). Returns None if libdrmtap is unavailable (dlopen failed),
    /// the device is not allowed, or the open failed — the caller then falls back
    /// to PipeWire/portal.
    pub fn open(device: Option<&str>, crtc_id: u32) -> Option<DrmReader> {
        let lib = drmtap_dl::get()?;
        let device_cstr = match device {
            None => None,
            Some(d) => {
                if !device_under_dev_dri(d) {
                    log::warn!("DRM device {d:?} is not under /dev/dri; refusing to open");
                    return None;
                }
                match CString::new(d) {
                    Ok(c) => Some(c),
                    Err(_) => return None, // interior NUL
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

    /// Grab one frame and copy it, tightly packed as BGRA (`w*4*h` bytes), into
    /// the internal buffer. Returns (width, height). The returned slice is valid
    /// until the next grab. A non-32bpp scanout, an oversized/degenerate
    /// geometry, or a stride < w*4 is rejected with a hard error so the caller
    /// falls back to PipeWire (see the codex format finding). Errno failures map
    /// to WouldBlock (retry) or a hard error (tear down) as in the old path.
    pub fn grab(&mut self) -> io::Result<(&[u8], usize, usize)> {
        // SAFETY: self.ctx is a valid context; frame is zeroed before the call
        // and released on every path.
        unsafe {
            let mut frame: drmtap_frame_info = std::mem::zeroed();
            let ret = (self.lib.grab_mapped)(self.ctx, &mut frame);
            if ret < 0 {
                let errno = -ret;
                // Transient contention (compositor mid page-flip, device momentarily
                // busy, interrupted syscall) -> retry rather than tear the stream down.
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
            // 4-bytes-per-pixel-per-row invariant: the row copy reads w*4 bytes
            // from a source that is only stride*height bytes. Reject sub-32bpp /
            // insane geometry to avoid an OOB read (heap disclosure to the peer).
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
            // Byte-order guard: libdrmtap normalizes the scanout to a BGRA-compatible 32-bit layout
            // (XRGB/ARGB8888 = little-endian B,G,R,{X,A} in memory). A different 32-bit order such as
            // XBGR8888 passes the stride check above but, labeled BGRA downstream, would ship with red
            // and blue swapped — so reject any fourcc we cannot present as BGRA. A zero/unknown fourcc
            // falls through to the stride invariant (kept for libdrmtap builds that do not set it).
            const DRM_FORMAT_XRGB8888: u32 = 0x3432_5258; // 'XR24'
            const DRM_FORMAT_ARGB8888: u32 = 0x3432_5241; // 'AR24'
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
            // Bound the reusable buffer: a malformed or hostile scanout geometry (e.g. 16384x16384)
            // would otherwise resize to gigabytes and, with several concurrent readers, OOM the root
            // --service. 256 MiB covers an 8K BGRA scanout (7680x4320x4 ~= 127 MiB) with margin;
            // anything larger (or an overflow) is rejected as unsupported. checked_mul guards the
            // multiply on 32-bit usize too.
            const MAX_FRAME_BYTES: usize = 256 * 1024 * 1024;
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

    /// Read the hardware cursor plane. Returns a hidden sentinel when the plane
    /// reports the cursor invisible, the real shape when visible, or None on a
    /// read error / unsupported cursor. Ported from the old drm.rs update_cursor.
    pub fn cursor(&mut self) -> Option<CursorSnapshot> {
        // SAFETY: ctx valid; c zeroed before the call; released only on success.
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
                let (mut minx, mut miny, mut maxx, mut maxy) = (cw, ch, -1i32, -1i32);
                for (i, &p) in src.iter().enumerate() {
                    let a = ((p >> 24) & 0xff) as u8;
                    let r = ((p >> 16) & 0xff) as u8;
                    let g = ((p >> 8) & 0xff) as u8;
                    let b = (p & 0xff) as u8;
                    colors.push(r);
                    colors.push(g);
                    colors.push(b);
                    colors.push(a);
                    hash ^= p as u64;
                    hash = hash.wrapping_mul(1099511628211);
                    if a >= 128 {
                        let x = (i as i32) % cw;
                        let y = (i as i32) / cw;
                        if x < minx { minx = x; }
                        if x > maxx { maxx = x; }
                        if y < miny { miny = y; }
                        if y > maxy { maxy = y; }
                    }
                }
                let (hotx, hoty) = if c.hot_x != 0 || c.hot_y != 0 {
                    (c.hot_x, c.hot_y)
                } else if maxx >= minx && maxy >= miny {
                    let (bw, bh) = (maxx - minx + 1, maxy - miny + 1);
                    if bh > bw * 2 {
                        ((minx + maxx) / 2, (miny + maxy) / 2)
                    } else {
                        (minx, miny)
                    }
                } else {
                    (0, 0)
                };
                // Fold geometry + hotspot into the id: a cursor with identical pixels but a changed
                // size or hotspot must count as a new shape, otherwise drm_capture_worker suppresses
                // the update (it dedupes by id) and the client keeps rendering the stale cursor.
                let mut id = hash;
                for v in [cw as u32 as u64, ch as u32 as u64, hotx as u32 as u64, hoty as u32 as u64] {
                    id ^= v;
                    id = id.wrapping_mul(1099511628211);
                }
                Some(CursorSnapshot {
                    id,
                    width: cw as u32,
                    height: ch as u32,
                    hotx,
                    hoty,
                    colors,
                })
            } else {
                None
            };
            (self.lib.cursor_release)(self.ctx, &mut c);
            out
        }
    }

    /// Enumerate the connected DRM displays (physical geometry). The buffer holds
    /// up to 16 connectors (the old path truncated at 8); the raw list is shipped
    /// to the server, which does primary selection + Wayland logical geometry.
    pub fn displays(&mut self) -> Vec<DisplaySnapshot> {
        // SAFETY: ctx valid; raw is a zeroed, correctly-sized array; count is
        // clamped to the buffer before indexing.
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
