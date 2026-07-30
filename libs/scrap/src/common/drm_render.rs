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
// cached EGLImages, which leaks a GPU context per capture session until the process is out of
// memory. The raw
// ctx pointer makes `RenderConverter` !Send/!Sync, which enforces that at the type
// level.

use super::drmtap_dl::{self, drmtap_ctx, drmtap_dmabuf_desc, drmtap_frame_info, DrmtapLib};
use super::Pixfmt;
use hbb_common::log;
use std::ffi::CString;
use std::io;
use std::os::fd::RawFd;

// The geometry/size limits and pixel fourccs are SHARED with the export side, declared once in
// `drm_reader`: they are the guards both halves of the trust boundary rely on to agree about what
// data they will touch, so a private copy here could silently drift from the privileged side's.
// libdrmtap normalizes the EGL path to XRGB8888, but we read `frame.format` per frame so a
// CPU-fallback convert that keeps the source channel order is still presented correctly.
use super::drm_reader::{
    DRM_FORMAT_ABGR8888, DRM_FORMAT_ARGB8888, DRM_FORMAT_XBGR8888, DRM_FORMAT_XRGB8888,
    MAX_DIM, MAX_FRAME_BYTES,
};

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
    /// Open an unprivileged DRM render-node convert context. `node` is the render node
    /// of the GPU that exports the scanout (from the service's display list); `None` or
    /// an empty/invalid path falls back to libdrmtap auto-selection. It opens no KMS
    /// card, spawns no helper, and needs no elevated capability. Returns `None` when
    /// libdrmtap is unavailable or too old to carry the split convert symbols (the
    /// loader refuses anything below 0.4.10 outright), or when no render node could be
    /// opened (a locked-down seat with no `/dev/dri/renderD*` access) — the caller
    /// then degrades to the service-side CPU convert / PipeWire path. MUST be called
    /// on the thread that will later `convert()` and drop it.
    pub fn open_render(node: Option<&str>) -> Option<RenderConverter> {
        let lib = drmtap_dl::get()?;
        let open_render = lib.open_render;
        // The service names the render node of the GPU that EXPORTS the scanout, which
        // is the only device guaranteed to understand its tiling modifier; auto-select
        // (NULL) is the fallback when it cannot. Same /dev/dri gate the capture device
        // gets: the path arrives over IPC, and while the peer is root, a converter that
        // opens whatever path it is handed is a needless widening.
        let node_cstr = match node.filter(|n| !n.is_empty()) {
            None => None,
            // Open the CANONICAL path the gate resolved, not the string that arrived over IPC:
            // opening the original would re-walk its symlinks after the check (see
            // device_under_dev_dri).
            Some(n) => match super::drm_reader::device_under_dev_dri(n) {
                None => {
                    log::warn!("drm: render node {n:?} is not under /dev/dri; auto-selecting");
                    None
                }
                Some(canonical) => canonical.to_str().and_then(|s| CString::new(s).ok()),
            },
        };
        // SAFETY: `open_render` is a resolved C entry point; `node_cstr` outlives the
        // call, and NULL requests auto-selection of a render node.
        let ctx = unsafe {
            open_render(node_cstr.as_ref().map_or(std::ptr::null(), |c| c.as_ptr()))
        };
        if ctx.is_null() {
            log::info!(
                "drmtap_open_render({}) failed; no usable DRM render node",
                node_cstr.as_ref().map_or("NULL".to_owned(), |c| format!("{c:?}"))
            );
            return None;
        }
        match node_cstr {
            Some(c) => log::info!(
                "drm: opened unprivileged convert context on the exporting GPU ({c:?})"
            ),
            None => log::info!(
                "drm: opened unprivileged render-node convert context (auto-selected)"
            ),
        }
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
        // Bound the INCOMING descriptor, not just the frame libdrmtap hands back. Every field here
        // was deserialized from the `_drm` wire, and while the producer is authenticated as root
        // (`connect_drm` refuses a non-root peer) and libdrmtap has validated the fd against the
        // descriptor since 0.4.12, this side already bounds the export descriptor symmetrically --
        // the two halves of the split should agree about what they will touch before the C sees it,
        // not after.
        //
        // offsets/pitches are the interesting ones: they address plane ranges inside the dma-buf, so
        // a malformed pair is what would reach past the buffer. Bound each populated plane's extent
        // the same way the frame path is bounded.
        {
            let (w, h) = (desc.width, desc.height);
            if w == 0 || h == 0 || w > MAX_DIM || h > MAX_DIM {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("drm: refusing a dma-buf descriptor with geometry {w}x{h}"),
                ));
            }
            // REJECT out of range, do not clamp -- and write the normalized value back, so the
            // count the C reads is the count this side actually bounded. Clamping validated planes
            // 0..3 and then handed libdrmtap the raw wire value, so a descriptor claiming 7 planes
            // passed a check for 4. The pinned libdrmtap does refuse >4 itself (drm_grab.c,
            // `num_planes > 4` -> error), so this was not an overflow today; but the whole point of
            // this block, as stated above, is that the two halves of the split agree about what
            // they will touch BEFORE the C sees it, and that only holds if the number travelling
            // with the descriptor is the validated one. It also stops this side depending on an
            // internal check in a library pinned from another repo. Mirrors what the EXPORT half
            // already does (drm_reader.rs `grab_desc`: reject >4, then assign the normalized count).
            let planes = if desc.num_planes == 0 { 1 } else { desc.num_planes };
            if planes > 4 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "drm: refusing a dma-buf descriptor with num_planes {} (1..=4)",
                        desc.num_planes
                    ),
                ));
            }
            desc.num_planes = planes;
            let planes = planes as usize;
            for p in 0..planes {
                let extent = (desc.pitches[p] as usize)
                    .checked_mul(h as usize)
                    .and_then(|rows| rows.checked_add(desc.offsets[p] as usize));
                match extent {
                    Some(end) if end <= MAX_FRAME_BYTES => {}
                    other => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!(
                                "drm: refusing dma-buf plane {p} (offset {} pitch {} over {h} rows -> {other:?}, cap {MAX_FRAME_BYTES})",
                                desc.offsets[p], desc.pitches[p]
                            ),
                        ));
                    }
                }
            }
        }
        // Always bound: a libdrmtap without the split convert symbols never loads.
        let convert_dmabuf = self.lib.convert_dmabuf;
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
                // Every other invalid frame_info property in this function is a hard error
                // that lets the caller fall back to PipeWire; an output format this build
                // cannot interpret must be one too. Presenting it as BGRA would pass the
                // stride checks (a 64bpp output still satisfies stride >= w*4) and encode
                // garbage instead of degrading.
                other => {
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!("drmtap_convert_dmabuf produced an unsupported output fourcc {other:#010x}"),
                    ));
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
