// Unprivileged (`--server`) render-side converter for the split DRM/KMS capture
// path. This is the OTHER half of the split introduced with libdrmtap >= 0.4.9:
// the root `--service` now only EXPORTS a scanout dma-buf fd + a small metadata
// descriptor (see `drm_reader::grab_desc`), and THIS side imports that fd and does
// the EGL detile / RGBA convert. Because the convert lives here, libEGL/libGLESv2
// are dlopen'd in the UNPRIVILEGED process, never in the privileged root service.
//
// A `RenderConverter` wraps a `drmtap_open_render(NULL)` render-node context and
// converts one imported dma-buf per `convert()` call. The EGL context and the
// import-once EGLImage cache it holds are THREAD-LOCAL inside libdrmtap: the
// context MUST be created, used (`convert`), and closed (`drop`) on the SAME
// thread (the consumer's `recv_thread`). Dropping it off-thread would strand the
// cached EGLImages — the exact leak class behind the 0.4.8 OOM regression. The raw
// ctx pointer makes `RenderConverter` !Send/!Sync, which enforces that at the type
// level.

use super::drmtap_dl::{self, drmtap_ctx, drmtap_dmabuf_desc, drmtap_frame_info, DrmtapLib};
use super::Pixfmt;
use hbb_common::log;
use std::io;
use std::os::fd::RawFd;

// DRM fourccs of the 32-bit linear formats libdrmtap's convert can emit. XRGB/ARGB
// are little-endian B,G,R,{X,A} in memory == our `Pixfmt::BGRA`; XBGR/ABGR are
// R,G,B,{X,A} == `Pixfmt::RGBA`. libdrmtap normalizes the EGL path to XRGB8888, but
// we read `frame.format` per frame so a CPU-fallback convert that keeps the source
// channel order is still presented correctly (not hardcoded BGRA).
const DRM_FORMAT_XRGB8888: u32 = 0x3432_5258; // 'XR24'
const DRM_FORMAT_ARGB8888: u32 = 0x3432_5241; // 'AR24'
const DRM_FORMAT_XBGR8888: u32 = 0x3432_4258; // 'XB24'
const DRM_FORMAT_ABGR8888: u32 = 0x3432_4241; // 'AB24'

// Same geometry / size guards as the export side (`drm_reader`), applied to the
// convert OUTPUT so a malformed `frame_info` cannot make us build an out-of-range
// slice from the context-owned pointer. 16384 covers 8K+; 256 MiB covers an 8K
// BGRA frame (7680x4320x4 ~= 127 MiB) with margin.
const MAX_DIM: u32 = 16384;
const MAX_FRAME_BYTES: usize = 256 * 1024 * 1024;

/// An unprivileged DRM render-node convert context (`drmtap_open_render`). Imports a
/// scanout dma-buf (received over SCM_RIGHTS) and EGL-detiles it to linear pixels.
/// Deliberately !Send/!Sync (the raw ctx pointer): the context and libdrmtap's
/// thread-local EGL state must stay on ONE thread for the context's whole life
/// (create + convert + close).
pub struct RenderConverter {
    lib: &'static DrmtapLib,
    ctx: *mut drmtap_ctx,
}

impl RenderConverter {
    /// Open an unprivileged DRM render-node convert context (`drmtap_open_render(NULL)`
    /// auto-selects a render node — it opens no KMS card, spawns no helper, and needs
    /// no elevated capability). Returns `None` when libdrmtap is unavailable, the split
    /// convert symbols are missing (a pre-0.4.9 `.so`), or no render node could be
    /// opened (a locked-down seat with no `/dev/dri/renderD*` access) — the caller then
    /// degrades to the CPU-mapped / PipeWire path. MUST be called on the thread that
    /// will later `convert()` and drop it.
    pub fn open_render() -> Option<RenderConverter> {
        let lib = drmtap_dl::get()?;
        // The converter needs BOTH split symbols; bail (so the caller degrades) if either
        // is absent, rather than open a ctx we could never convert with.
        let open_render = lib.open_render?;
        if lib.convert_dmabuf.is_none() {
            log::info!(
                "libdrmtap exposes drmtap_open_render but not drmtap_convert_dmabuf; \
                 cannot convert dma-buf frames (old .so)"
            );
            return None;
        }
        // SAFETY: `open_render` is a resolved C entry point; NULL requests auto-selection
        // of a render node.
        let ctx = unsafe { open_render(std::ptr::null()) };
        if ctx.is_null() {
            log::info!("drmtap_open_render(NULL) failed; no usable DRM render node");
            return None;
        }
        log::info!("drm: opened unprivileged render-node convert context");
        Some(RenderConverter { lib, ctx })
    }

    /// Import + convert one scanout dma-buf. `desc` is the descriptor rebuilt from the
    /// wire `DmabufDesc`; `received_fd` is the fd number this process obtained via
    /// SCM_RIGHTS (or `-1` for an import-once cache hit, where libdrmtap reuses the
    /// EGLImage it already holds for `desc.fb_id`). The fd is written into
    /// `desc.dma_buf_fd` before the call (LOAD-BEARING: the integer the exporter
    /// serialized was process-local and never crossed the wire).
    ///
    /// On success returns a borrow of the CONTEXT-OWNED linear pixels plus the frame
    /// width/height and the `Pixfmt` read from `frame.format`. The slice covers
    /// `stride * height` bytes, so the caller can recover the (possibly padded) row
    /// stride as `data.len() / height`. It is valid ONLY until the next `convert()`
    /// (or drop) — do NOT free it and do NOT call `drmtap_frame_release` on it
    /// (libdrmtap owns it). The `&mut self` borrow keeps the slice from outliving the
    /// next convert; copy it out (into the latest-wins slot) before the next call.
    pub fn convert(
        &mut self,
        desc: &mut drmtap_dmabuf_desc,
        received_fd: RawFd,
    ) -> io::Result<(&[u8], u32, u32, Pixfmt)> {
        let convert_dmabuf = self.lib.convert_dmabuf.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::Unsupported,
                "libdrmtap too old: drmtap_convert_dmabuf unavailable (need >= 0.4.9)",
            )
        })?;
        // Overwrite the descriptor's fd with the one THIS process received (split_capture.c
        // does the same at recv time). -1 means "reuse the cached import for `fb_id`".
        desc.dma_buf_fd = received_fd;
        // SAFETY: self.ctx is a valid render context; `desc` points to a fully-initialized
        // descriptor; `frame` is zeroed before the call. libdrmtap OWNS the returned
        // `frame.data` (no release/free from this side, per drmtap.h).
        unsafe {
            let mut frame: drmtap_frame_info = std::mem::zeroed();
            let ret = convert_dmabuf(self.ctx, &*desc as *const drmtap_dmabuf_desc, &mut frame);
            if ret < 0 {
                let errno = -ret;
                // Transient contention (device busy, interrupted syscall) -> retry rather
                // than tear the stream down.
                if errno == hbb_common::libc::EAGAIN
                    || errno == hbb_common::libc::EBUSY
                    || errno == hbb_common::libc::EINTR
                {
                    return Err(io::ErrorKind::WouldBlock.into());
                }
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("drmtap_convert_dmabuf failed: errno {errno}"),
                ));
            }
            if frame.data.is_null() || frame.width == 0 || frame.height == 0 || frame.stride == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "drmtap_convert_dmabuf produced an empty frame",
                ));
            }
            let w = frame.width;
            let h = frame.height;
            let stride = frame.stride as usize;
            // Guard the slice we are about to build from a hostile/garbage `frame_info`:
            // reject an insane geometry or a stride below 32bpp (would under-size the row
            // and, read as BGRA downstream, disclose adjacent memory).
            if w > MAX_DIM || h > MAX_DIM || stride < (w as usize) * 4 {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!(
                        "drmtap_convert_dmabuf bad geometry {w}x{h} stride {stride} fourcc {:#010x}",
                        frame.format
                    ),
                ));
            }
            let len = match stride.checked_mul(h as usize) {
                Some(sz) if sz > 0 && sz <= MAX_FRAME_BYTES => sz,
                other => {
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!("drmtap_convert_dmabuf frame size out of range ({other:?} bytes)"),
                    ));
                }
            };
            // Channel order from the ACTUAL convert output (do NOT hardcode BGRA): the EGL
            // path normalizes to XRGB8888 (BGRA), but reading it keeps any other emitted
            // order labeled correctly for the encoder.
            let pixfmt = match frame.format {
                DRM_FORMAT_XRGB8888 | DRM_FORMAT_ARGB8888 => Pixfmt::BGRA,
                DRM_FORMAT_XBGR8888 | DRM_FORMAT_ABGR8888 => Pixfmt::RGBA,
                // Unset by an older convert -> libdrmtap's normalized BGRA.
                0 => Pixfmt::BGRA,
                other => {
                    log::debug!(
                        "drm: convert output fourcc {other:#010x} unrecognized; presenting as BGRA"
                    );
                    Pixfmt::BGRA
                }
            };
            // Borrow the context-owned pixels. The returned lifetime is tied to `&mut self`
            // (elision), so the borrow cannot outlive the next `convert()` that would
            // overwrite these bytes.
            let data = std::slice::from_raw_parts(frame.data as *const u8, len);
            Ok((data, w, h, pixfmt))
        }
    }
}

impl Drop for RenderConverter {
    fn drop(&mut self) {
        if !self.ctx.is_null() {
            // SAFETY: ctx came from drmtap_open_render and is non-null. This MUST run on the
            // same thread that created and used it (thread-local EGL state + cached imports);
            // guaranteed because the !Send ctx pointer keeps the whole `RenderConverter` on
            // the owning `recv_thread`, where it is also dropped.
            unsafe { (self.lib.close)(self.ctx) };
            self.ctx = std::ptr::null_mut();
        }
    }
}
