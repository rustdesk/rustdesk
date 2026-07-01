// DRM/KMS capture backend for RustDesk — powered by libdrmtap
//
// Reads the compositor's scanout directly from the DRM/KMS subsystem without
// involving xdg-desktop-portal.  libdrmtap-sys statically embeds the C sources
// (no shared library to install) and spawns a privileged helper
// (drmtap-helper, CAP_SYS_ADMIN + seccomp) for capture without running
// rustdesk as root.
//
// Multi-monitor: each Display carries its CRTC id and virtual-FB origin (x, y)
// so the Capturer opens the exact CRTC the user selected, not just the primary.
//
// Tested on:
//   - Intel Meteor Lake (i915) dual 3840×2160 — EGL detiling of the compressed
//     INTEL_4_TILED_MTL_RC_CCS_CC framebuffer modifier
//   - virtio-gpu (QEMU/KVM) — linear framebuffer

use crate::{Frame, TraitCapturer};
use std::{io, time::{Duration, Instant}};
use super::x11::PixelBuffer;
use hbb_common::log;

// FFI bindings to libdrmtap — struct layouts must match drmtap.h exactly!
// Use libdrmtap-sys crate for static linking
use libdrmtap_sys::{
    drmtap_close, drmtap_config, drmtap_ctx, drmtap_cursor_info, drmtap_cursor_release,
    drmtap_display, drmtap_frame_info, drmtap_get_cursor, drmtap_grab_mapped,
    drmtap_frame_release, drmtap_list_displays, drmtap_open,
};
use std::sync::Mutex;

// Latest hardware cursor captured from the DRM cursor plane (via the privileged
// helper). RustDesk's cursor source on Wayland is XFixes, which only reflects
// the X cursor and is stale over native Wayland apps. The DRM cursor plane, in
// contrast, holds the compositor's actual current cursor and updates when the
// shape changes — so we capture it here and feed it to the cursor service.
#[derive(Clone)]
pub struct DrmCursor {
    pub id: u64, // content hash; changes when the cursor shape changes
    pub width: i32,
    pub height: i32,
    pub hotx: i32,
    pub hoty: i32,
    pub colors: Vec<u8>, // RGBA8888
}

static DRM_CURSOR: Mutex<Option<DrmCursor>> = Mutex::new(None);

/// Current hardware cursor captured from the DRM cursor plane, if any.
pub fn drm_cursor() -> Option<DrmCursor> {
    DRM_CURSOR.lock().unwrap().clone()
}

/// Cheap id-only accessor for the ~33ms cursor poll fast path, which only needs
/// the id to detect shape changes — avoids cloning the pixel buffer every poll.
pub fn drm_cursor_id() -> Option<u64> {
    DRM_CURSOR.lock().unwrap().as_ref().map(|c| c.id)
}

pub struct Display {
    name: String,
    // Logical origin in the compositor's coordinate space (from the matching
    // Wayland output). Falls back to the physical CRTC offset when no compositor
    // output matches (e.g. capturing the login screen with no compositor).
    x: i32,
    y: i32,
    // Physical pixel size of the captured framebuffer (DRM mode).
    w: usize,
    h: usize,
    // Logical size + scale (physical/logical) from the matching Wayland output;
    // fall back to physical size and scale 1.0 when unknown.
    logical_w: usize,
    logical_h: usize,
    scale: f64,
    crtc_id: u32,
    primary: bool,
}

// Match a DRM connector to its Wayland output (by connector name) to obtain the
// compositor's LOGICAL geometry and scale factor.
//
// Rationale: video is captured by DRM in PHYSICAL pixels, but RustDesk's Wayland
// input path injects the cursor in the compositor's LOGICAL coordinate space
// (the uinput device range and the per-display `scale`/`origin` reported to the
// peer are all logical). If the DRM backend reported scale 1.0 and the physical
// origin, the client would send physical coordinates while uinput expects logical
// ones, mis-mapping the cursor under fractional/HiDPI scaling or multi-monitor
// layouts. Matching the Wayland output keeps both coordinate systems consistent
// for any client/server configuration.
//
// Falls back to the physical geometry (scale 1.0) when no compositor output
// matches — e.g. an X11 session, or the GDM/SDDM login screen with no compositor.
// Normalize a connector name so DRM and compositor naming line up. DRM exposes
// sub-typed names like "HDMI-A-1" / "DVI-I-1", while compositors (e.g. Mutter)
// often shorten them to "HDMI-1" / "DVI-1". We collapse to "<type>-<index>" by
// dropping any middle segments, and lowercase for a case-insensitive compare.
fn normalize_connector(name: &str) -> String {
    let parts: Vec<&str> = name.split('-').filter(|s| !s.is_empty()).collect();
    match parts.as_slice() {
        [] => name.to_lowercase(),
        [single] => single.to_lowercase(),
        // Keep the leading type and the trailing index, drop the middle (the
        // sub-type letter such as the "A" in "HDMI-A-1").
        [first, .., last] => format!("{}-{}", first, last).to_lowercase(),
    }
}

fn logical_geometry_for(
    name: &str,
    phys_x: i32,
    phys_y: i32,
    phys_w: usize,
    phys_h: usize,
) -> (i32, i32, usize, usize, f64) {
    let displays = crate::wayland::display::get_displays();
    // Prefer an exact name match; fall back to a normalized connector match.
    let want = normalize_connector(name);
    let matched = displays
        .displays
        .iter()
        .find(|d| d.name == name)
        .or_else(|| {
            displays
                .displays
                .iter()
                .find(|d| normalize_connector(&d.name) == want)
        });
    if let Some(d) = matched {
        let (lw, lh) = d.logical_size.unwrap_or((d.width, d.height));
        let lw = (lw.max(1)) as usize;
        let lh = (lh.max(1)) as usize;
        let scale = if d.width > 0 {
            d.width as f64 / lw as f64
        } else {
            1.0
        };
        return (d.x, d.y, lw, lh, scale);
    }
    // No matching compositor output — use physical geometry, no scaling.
    // (e.g. an X11 session, or a login screen with no compositor running.)
    (phys_x, phys_y, phys_w, phys_h, 1.0)
}

/// Index of the display matching the compositor's primary output (by connector
/// name), or 0 if the primary is unknown or doesn't match an enumerated
/// connector. Keeps the DRM primary consistent with the Wayland/compositor
/// primary instead of just libdrmtap's enumeration order.
fn primary_display_index(displays: &[Display]) -> usize {
    #[cfg(feature = "wayland")]
    if let Some(name) = crate::wayland::display::get_primary_monitor() {
        if let Some(idx) = displays.iter().position(|d| d.name == name) {
            return idx;
        }
        log::debug!(
            "DRM: compositor primary '{name}' not among enumerated connectors; using first"
        );
    }
    let _ = displays;
    0
}

impl Display {
    pub fn all() -> io::Result<Vec<Display>> {
        // SAFETY: All FFI calls use valid pointers and check return values.
        // The drmtap context is opened and closed within this function scope.
        unsafe {
            let device_env = std::env::var("DRM_DEVICE").ok();
            // DRM_DEVICE is user-controlled; an interior NUL would make CString::new
            // fail — treat that as unset (null device_path) rather than panicking.
            let device_cstr = device_env
                .as_ref()
                .and_then(|s| std::ffi::CString::new(s.as_str()).ok());

            let cfg = drmtap_config {
                device_path: device_cstr
                    .as_ref()
                    .map(|c| c.as_ptr())
                    .unwrap_or(std::ptr::null()),
                crtc_id: 0,
                helper_path: std::ptr::null(),
                debug: if std::env::var("DRMTAP_DEBUG").is_ok() { 1 } else { 0 },
            };
            let ctx = drmtap_open(&cfg);
            if ctx.is_null() {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "drmtap_open failed",
                ));
            }

            let mut raw_displays = vec![std::mem::zeroed::<drmtap_display>(); 8];
            let cap = raw_displays.len() as i32;
            let n = drmtap_list_displays(ctx, raw_displays.as_mut_ptr(), cap);
            drmtap_close(ctx);

            if n <= 0 {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "No DRM displays found",
                ));
            }

            // drmtap_list_displays returns the total connected-connector count,
            // which may exceed the buffer capacity (only `cap` entries are
            // written). Clamp before indexing so >8 connectors can't read past
            // the end of the Vec.
            let count = (n as usize).min(raw_displays.len());

            let mut idxs: Vec<usize> =
                (0..count).filter(|&i| raw_displays[i].active != 0).collect();
            if idxs.is_empty() {
                // nvidia-drm doesn't flag the connector active via the legacy API
                // even with a live scanout; fall back to all enumerated displays
                // and let the capturer auto-select the active CRTC (crtc_id 0).
                idxs = (0..count).collect();
            }
            let mut displays: Vec<Display> = idxs
                .into_iter()
                .map(|i| {
                    let name_bytes: Vec<u8> = raw_displays[i]
                        .name
                        .iter()
                        .take_while(|&&c| c != 0)
                        .map(|&c| c as u8)
                        .collect();
                    let name = String::from_utf8_lossy(&name_bytes).to_string();
                    let phys_w = raw_displays[i].width as usize;
                    let phys_h = raw_displays[i].height as usize;
                    let (ox, oy, logical_w, logical_h, scale) = logical_geometry_for(
                        &name,
                        raw_displays[i].x as i32,
                        raw_displays[i].y as i32,
                        phys_w,
                        phys_h,
                    );
                    Display {
                        name,
                        x: ox,
                        y: oy,
                        w: phys_w,
                        h: phys_h,
                        logical_w,
                        logical_h,
                        scale,
                        crtc_id: raw_displays[i].crtc_id,
                        primary: false,
                    }
                })
                .collect();

            if displays.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "No active DRM displays",
                ));
            }

            // Mark the primary to match the compositor's primary output (by
            // connector name) rather than libdrmtap's enumeration order. Falls
            // back to the first display if the compositor primary is unknown or
            // its name doesn't match an enumerated connector.
            let primary_idx = primary_display_index(&displays);
            if let Some(d) = displays.get_mut(primary_idx) {
                d.primary = true;
            }

            Ok(displays)
        }
    }

    pub fn primary() -> io::Result<Display> {
        let mut all = Self::all()?;
        let idx = all.iter().position(|d| d.primary).unwrap_or(0);
        Ok(all.remove(idx))
    }

    pub fn width(&self) -> usize { self.w }
    pub fn height(&self) -> usize { self.h }
    pub fn scale(&self) -> f64 { self.scale }
    pub fn logical_width(&self) -> usize { self.logical_w }
    pub fn logical_height(&self) -> usize { self.logical_h }
    pub fn origin(&self) -> (i32, i32) { (self.x, self.y) }
    pub fn is_online(&self) -> bool { true }
    pub fn is_primary(&self) -> bool { self.primary }
    pub fn name(&self) -> String { self.name.clone() }
}

pub struct Capturer {
    ctx: *mut drmtap_ctx,
    w: usize,
    h: usize,
    buffer: Vec<u8>,
    last_fb_id: u32,
    frame_count: u64,
    cursor_tick: u64,
    skip_count: u64,
    last_grab_time: Instant,
}

impl Capturer {
    pub fn new(display: Display) -> io::Result<Capturer> {
        // SAFETY: FFI call to drmtap_open with valid config struct.
        // The returned pointer is checked for null before use.
        unsafe {
            let device_env = std::env::var("DRM_DEVICE").ok();
            // DRM_DEVICE is user-controlled; an interior NUL would make CString::new
            // fail — treat that as unset (null device_path) rather than panicking.
            let device_cstr = device_env
                .as_ref()
                .and_then(|s| std::ffi::CString::new(s.as_str()).ok());

            let cfg = drmtap_config {
                device_path: device_cstr
                    .as_ref()
                    .map(|c| c.as_ptr())
                    .unwrap_or(std::ptr::null()),
                crtc_id: display.crtc_id,
                helper_path: std::ptr::null(),
                debug: if std::env::var("DRMTAP_DEBUG").is_ok() { 1 } else { 0 },
            };
            let ctx = drmtap_open(&cfg);
            if ctx.is_null() {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "drmtap_open failed",
                ));
            }
            Ok(Capturer {
                ctx,
                w: display.w,
                h: display.h,
                buffer: Vec::new(),
                last_fb_id: 0,
                frame_count: 0,
                cursor_tick: 0,
                skip_count: 0,
                last_grab_time: Instant::now(),
            })
        }
    }

    pub fn width(&self) -> usize { self.w }
    pub fn height(&self) -> usize { self.h }
}

impl TraitCapturer for Capturer {
    fn frame<'a>(&'a mut self, timeout: Duration) -> io::Result<Frame<'a>> {
        // SAFETY: All FFI calls use the valid self.ctx pointer (checked non-null
        // in new()). Frame data pointer is validated before dereferencing.
        // drmtap_frame_release is always called before returning.
        unsafe {
            // Rate limit: minimum 16ms between grabs (~60 FPS max)
            let elapsed = self.last_grab_time.elapsed();
            let min_interval = Duration::from_millis(16);
            if elapsed < min_interval {
                std::thread::sleep(min_interval - elapsed);
            }

            let mut frame: drmtap_frame_info = std::mem::zeroed();
            let ret = drmtap_grab_mapped(self.ctx, &mut frame);
            if ret < 0 {
                std::thread::sleep(Duration::from_millis(16));
                return Err(io::ErrorKind::WouldBlock.into());
            }

            if frame.data.is_null() || frame.width == 0 || frame.height == 0 {
                drmtap_frame_release(self.ctx, &mut frame);
                std::thread::sleep(Duration::from_millis(16));
                return Err(io::ErrorKind::WouldBlock.into());
            }

            self.last_grab_time = Instant::now();
            let current_fb_id = frame.fb_id;

            // Poll the hardware cursor plane independently of framebuffer changes.
            // The cursor plane is a separate DRM plane — its shape can change even
            // when the scanout framebuffer is unchanged (e.g. while the desktop is
            // idle but the user is moving the mouse).
            self.cursor_tick += 1;
            if self.cursor_tick % 4 == 0 {
                self.update_cursor();
            }

            // fb_id skip: if framebuffer hasn't changed, skip expensive copy
            if current_fb_id == self.last_fb_id && self.last_fb_id != 0 {
                drmtap_frame_release(self.ctx, &mut frame);
                self.skip_count += 1;
                let sleep_ms = timeout.as_millis().min(33).max(1) as u64;
                std::thread::sleep(Duration::from_millis(sleep_ms));
                return Err(io::ErrorKind::WouldBlock.into());
            }
            self.last_fb_id = current_fb_id;

            let w = frame.width as usize;
            let h = frame.height as usize;
            let stride = frame.stride as usize;
            let frame_size = w * 4 * h;

            if self.buffer.len() != frame_size {
                self.buffer.resize(frame_size, 0);
            }

            let src = frame.data as *const u8;
            if stride == w * 4 {
                std::ptr::copy_nonoverlapping(src, self.buffer.as_mut_ptr(), frame_size);
            } else {
                for y in 0..h {
                    std::ptr::copy_nonoverlapping(
                        src.add(y * stride),
                        self.buffer.as_mut_ptr().add(y * w * 4),
                        w * 4,
                    );
                }
            }

            drmtap_frame_release(self.ctx, &mut frame);

            self.frame_count += 1;
            self.w = w;
            self.h = h;
            Ok(Frame::PixelBuffer(PixelBuffer::new(
                &self.buffer,
                crate::Pixfmt::BGRA,
                w,
                h,
            )))
        }
    }

}

impl Capturer {
    // Capture the hardware cursor from the DRM cursor plane and update DRM_CURSOR.
    // The cursor plane is independent of the scanout framebuffer, so this is called
    // on every cursor_tick even when the framebuffer hasn't changed.
    unsafe fn update_cursor(&mut self) {
        let mut c: drmtap_cursor_info = std::mem::zeroed();
        let cret = drmtap_get_cursor(self.ctx, &mut c);
        if cret == 0
            && c.visible != 0
            && !c.pixels.is_null()
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
                let bw = maxx - minx + 1;
                let bh = maxy - miny + 1;
                if bh > bw * 2 {
                    ((minx + maxx) / 2, (miny + maxy) / 2)
                } else {
                    (minx, miny)
                }
            } else {
                (0, 0)
            };
            let mut lock = DRM_CURSOR.lock().unwrap();
            let changed = lock.as_ref().map_or(true, |old| old.id != hash);
            if changed {
                *lock = Some(DrmCursor {
                    id: hash,
                    width: cw,
                    height: ch,
                    hotx,
                    hoty,
                    colors,
                });
            }
        }
        drmtap_cursor_release(self.ctx, &mut c);
    }
}

impl Drop for Capturer {
    fn drop(&mut self) {
        if !self.ctx.is_null() {
            // SAFETY: ctx was obtained from drmtap_open and is non-null.
            unsafe { drmtap_close(self.ctx); }
            self.ctx = std::ptr::null_mut();
        }
    }
}
