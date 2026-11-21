// Unprivileged half of the split DRM/KMS capture path: the root `--service` exports a scanout
// dma-buf fd + descriptor, this side imports it and EGL-detiles. libEGL/libGLESv2 are dlopen'd
// in the UNPRIVILEGED process on this path; the root service loads them only if it falls back to
// its own CPU-mapped grab (`drmtap_grab_mapped`).

use super::drmtap_dl::{self, drmtap_ctx, drmtap_dmabuf_desc, drmtap_frame_info, DrmtapLib};
use super::Pixfmt;
use hbb_common::log;
use std::ffi::CString;
use std::io;
use std::os::fd::RawFd;

use super::drm_reader::{
    DRM_FORMAT_ABGR8888, DRM_FORMAT_ARGB8888, DRM_FORMAT_XBGR8888, DRM_FORMAT_XRGB8888,
    MAX_DIM, MAX_FRAME_BYTES,
};

/// Unprivileged DRM render-node convert context. !Send/!Sync via the raw ctx pointer: the context
/// and libdrmtap's thread-local EGL state must be created, used (`convert`) and closed on ONE thread.
pub struct RenderConverter {
    lib: &'static DrmtapLib,
    ctx: *mut drmtap_ctx,
}

impl RenderConverter {
    /// Convert into a recycled consumer buffer, clearing the borrowed destination before returning.
    pub fn convert_into(
        &mut self,
        desc: &mut drmtap_dmabuf_desc,
        received_fd: RawFd,
        buf: &mut Vec<u8>,
    ) -> io::Result<(u32, u32, Pixfmt)> {
        let set_output = self.lib.set_output_buffer;
        // CPU conversions can preserve the source pitch; EGL returns tightly packed RGBA.
        let needed = (desc.pitches[0] as usize)
            .max((desc.width as usize).saturating_mul(4))
            .checked_mul(desc.height as usize)
            .filter(|&n| n > 0 && n <= MAX_FRAME_BYTES);
        let Some(needed) = needed.filter(|_| {
            desc.width > 0 && desc.height > 0 && desc.width <= MAX_DIM && desc.height <= MAX_DIM
        }) else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid DRM output size",
            ));
        };
        buf.try_reserve(needed.saturating_sub(buf.len()))
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        buf.resize(needed, 0);
        let dst = buf.as_mut_ptr();
        // SAFETY: the destination is initialized, bounded and independent of the scanout. It is
        // neither moved nor resized during conversion, and is unset before any return below.
        let rc = unsafe { set_output(self.ctx, dst.cast(), buf.len()) };
        if rc != 0 {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("set DRM output: {rc}"),
            ));
        }
        let ctx = self.ctx;
        let converted = self.convert(desc, received_fd);
        // SAFETY: clearing the destination does not invalidate the returned pixels. Do it before
        // the consumer can resize or recycle its buffer, including when conversion failed.
        let rc = unsafe { set_output(ctx, std::ptr::null_mut(), 0) };
        if rc != 0 {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("clear DRM output: {rc}"),
            ));
        }
        match converted {
            Ok((data, w, h, fmt)) => {
                if data.as_ptr() == dst {
                    let len = data.len();
                    if len > buf.len() {
                        Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "DRM output exceeds destination",
                        ))
                    } else {
                        buf.truncate(len);
                        Ok((w, h, fmt))
                    }
                } else {
                    // Defensive fallback if the library returns storage other than
                    // the configured destination.
                    buf.clear();
                    buf.extend_from_slice(data);
                    Ok((w, h, fmt))
                }
            }
            Err(err) => Err(err),
        }
    }

    /// `node` is the render node of the GPU that exports the scanout; `None`/invalid path falls back to libdrmtap auto-selection.
    pub fn open_render(node: Option<&str>) -> Option<RenderConverter> {
        let lib = drmtap_dl::get()?;
        let open_render = lib.open_render;
        let node_cstr = match node.filter(|n| !n.is_empty()) {
            None => None,
            // Open the CANONICAL path the gate resolved: opening the IPC string would re-walk its symlinks after the check.
            Some(n) => match super::drm_reader::device_under_dev_dri(n) {
                None => {
                    log::warn!("drm: render node {n:?} is not under /dev/dri; auto-selecting");
                    None
                }
                Some(canonical) => canonical.to_str().and_then(|s| CString::new(s).ok()),
            },
        };
        // SAFETY: resolved C entry point; `node_cstr` outlives the call, NULL requests auto-selection.
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

    /// Returns linear pixels valid ONLY until the next conversion; row stride is `len / height`.
    pub fn convert(
        &mut self,
        desc: &mut drmtap_dmabuf_desc,
        received_fd: RawFd,
    ) -> io::Result<(&[u8], u32, u32, Pixfmt)> {
        {
            let (w, h) = (desc.width, desc.height);
            if w == 0 || h == 0 || w > MAX_DIM || h > MAX_DIM {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("drm: refusing a dma-buf descriptor with geometry {w}x{h}"),
                ));
            }
            // Reject, do not clamp, and write the normalized count back so the C reads the count bounded here.
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
        let convert_dmabuf = self.lib.convert_dmabuf;
        // LOAD-BEARING: the fd the exporter serialized was process-local; -1 means reuse the cached import for `fb_id`.
        desc.dma_buf_fd = received_fd;
        // SAFETY: self.ctx is a valid render context; `desc` is fully initialized; `frame` is zeroed
        // before the call. frame.data belongs to the context or its configured destination;
        // neither is released/freed through the returned frame (drmtap.h).
        unsafe {
            let mut frame: drmtap_frame_info = std::mem::zeroed();
            let ret = convert_dmabuf(self.ctx, &*desc as *const drmtap_dmabuf_desc, &mut frame);
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
            // A stride below 32bpp under-sizes the row and, read as BGRA downstream, discloses adjacent memory.
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
            let pixfmt = match frame.format {
                DRM_FORMAT_XRGB8888 | DRM_FORMAT_ARGB8888 => Pixfmt::BGRA,
                DRM_FORMAT_XBGR8888 | DRM_FORMAT_ABGR8888 => Pixfmt::RGBA,
                // Unset by an older convert -> libdrmtap's normalized BGRA.
                0 => Pixfmt::BGRA,
                other => {
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!("drmtap_convert_dmabuf produced an unsupported output fourcc {other:#010x}"),
                    ));
                }
            };
            let data = std::slice::from_raw_parts(frame.data as *const u8, len);
            Ok((data, w, h, pixfmt))
        }
    }
}

impl Drop for RenderConverter {
    fn drop(&mut self) {
        if !self.ctx.is_null() {
            // SAFETY: ctx came from drmtap_open_render and is non-null; the !Send ctx pointer keeps
            // this drop on the thread that created and used it (thread-local EGL + cached imports).
            unsafe { (self.lib.close)(self.ctx) };
            self.ctx = std::ptr::null_mut();
        }
    }
}

#[cfg(test)]
mod output_buffer_tests {
    use super::*;
    use std::ffi::c_void;

    struct Context {
        dst: *mut u8,
        len: usize,
        pixels: Vec<u8>,
        output_stride: u32,
        error: i32,
        use_non_destination: bool,
    }

    unsafe extern "C" fn set_output(ctx: *mut drmtap_ctx, dst: *mut c_void, len: usize) -> i32 {
        let ctx = &mut *ctx.cast::<Context>();
        ctx.dst = dst.cast();
        ctx.len = len;
        0
    }

    /// Model caller-owned output; the defensive test deliberately returns nonconforming storage.
    unsafe extern "C" fn convert(
        ctx: *mut drmtap_ctx,
        desc: *const drmtap_dmabuf_desc,
        frame: *mut drmtap_frame_info,
    ) -> i32 {
        let ctx = &mut *ctx.cast::<Context>();
        if ctx.error != 0 {
            return -ctx.error;
        }
        let data = if !ctx.use_non_destination {
            if ctx.dst.is_null() || ctx.len < ctx.pixels.len() {
                return -hbb_common::libc::ENOSPC;
            }
            std::ptr::copy_nonoverlapping(ctx.pixels.as_ptr(), ctx.dst, ctx.pixels.len());
            ctx.dst
        } else {
            ctx.pixels.as_mut_ptr()
        };
        *frame = std::mem::zeroed();
        (*frame).data = data.cast();
        (*frame).width = (*desc).width;
        (*frame).height = (*desc).height;
        (*frame).stride = ctx.output_stride;
        (*frame).format = DRM_FORMAT_ABGR8888;
        0
    }

    fn library() -> &'static DrmtapLib {
        unsafe extern "C" fn grab(_: *mut drmtap_ctx, _: *mut drmtap_frame_info) -> i32 {
            -1
        }
        unsafe extern "C" fn release(_: *mut drmtap_ctx, _: *mut drmtap_frame_info) {}
        unsafe extern "C" fn desc(
            _: *mut drmtap_ctx,
            _: *mut drmtap_dmabuf_desc,
            _: *mut drmtap_frame_info,
        ) -> i32 {
            -1
        }
        unsafe extern "C" fn rotation(_: *mut drmtap_ctx, _: *mut u32) -> i32 {
            -1
        }
        let mut lib = DrmtapLib::fake(grab, release, desc, rotation);
        lib.convert_dmabuf = convert;
        lib.set_output_buffer = set_output;
        Box::leak(Box::new(lib))
    }

    fn descriptor() -> drmtap_dmabuf_desc {
        drmtap_dmabuf_desc {
            width: 2,
            height: 2,
            pitches: [12, 0, 0, 0],
            ..Default::default()
        }
    }

    fn context() -> Context {
        Context {
            dst: std::ptr::null_mut(),
            len: 0,
            pixels: vec![37; 24],
            output_stride: 12,
            error: 0,
            use_non_destination: false,
        }
    }

    #[test]
    fn converts_into_reusable_destination() {
        let mut ctx = context();
        let mut conv = RenderConverter {
            lib: library(),
            ctx: (&mut ctx as *mut Context).cast(),
        };
        let mut buf = vec![0; 24];
        let allocation = buf.as_ptr();
        let (w, h, fmt) = conv.convert_into(&mut descriptor(), -1, &mut buf).unwrap();
        assert_eq!((w, h), (2, 2));
        assert!(matches!(fmt, Pixfmt::RGBA));
        assert_eq!(buf, [37; 24]);
        assert_eq!(buf.as_ptr(), allocation);
        // The C API must not retain a borrow of a buffer the consumer now owns.
        assert!(ctx.dst.is_null());
        ctx.pixels.fill(83);
        conv.convert_into(&mut descriptor(), -1, &mut buf).unwrap();
        assert_eq!(buf, [83; 24]);
    }

    /// A nonconforming mock exercises the defensive copy; supported libraries use the destination.
    #[test]
    fn copies_non_destination_frames() {
        let mut ctx = context();
        ctx.use_non_destination = true;
        let mut conv = RenderConverter {
            lib: library(),
            ctx: (&mut ctx as *mut Context).cast(),
        };
        let mut buf = Vec::new();
        conv.convert_into(&mut descriptor(), -1, &mut buf).unwrap();
        assert_eq!(buf, [37; 24]);
        ctx.pixels.fill(0);
        assert_eq!(buf, [37; 24]);
        assert!(ctx.dst.is_null());
    }

    #[test]
    fn packed_output_reuses_destination_across_resolution_changes() {
        let mut ctx = context();
        let mut conv = RenderConverter {
            lib: library(),
            ctx: (&mut ctx as *mut Context).cast(),
        };
        let mut buf = vec![0; 24];
        let mut allocation = buf.as_ptr();
        for width in [2, 4, 2, 2] {
            let mut desc = descriptor();
            desc.width = width;
            desc.pitches[0] = width * 4 + 4;
            ctx.output_stride = width * 4;
            ctx.pixels = (0..width * 4 * desc.height).map(|n| n as u8).collect();
            let (w, h, _) = conv.convert_into(&mut desc, -1, &mut buf).unwrap();
            assert_eq!((w, h), (width, 2));
            assert_eq!(buf, ctx.pixels);
            assert_eq!(buf.len(), (width * 8) as usize);
            if width == 2 {
                assert_eq!(buf.as_ptr(), allocation);
            }
            allocation = buf.as_ptr();
            assert!(ctx.dst.is_null());
        }
    }

    #[test]
    fn releases_destination_on_failed_conversion() {
        let mut ctx = context();
        let mut conv = RenderConverter {
            lib: library(),
            ctx: (&mut ctx as *mut Context).cast(),
        };
        for error in [hbb_common::libc::EAGAIN, hbb_common::libc::EIO] {
            ctx.error = error;
            let mut buf = Vec::new();
            let err = conv
                .convert_into(&mut descriptor(), -1, &mut buf)
                .unwrap_err();
            assert_eq!(
                err.kind(),
                if error == hbb_common::libc::EAGAIN {
                    io::ErrorKind::WouldBlock
                } else {
                    io::ErrorKind::Other
                }
            );
            assert!(ctx.dst.is_null());
            drop(buf);
            ctx.error = 0;
            let mut next = Vec::new();
            conv.convert_into(&mut descriptor(), -1, &mut next).unwrap();
            assert_eq!(next, [37; 24]);
        }
    }
}
