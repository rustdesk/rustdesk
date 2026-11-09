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

    /// Returns context-owned linear pixels valid ONLY until the next `convert()`; row stride is `len / height`.
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
        // before the call. libdrmtap OWNS `frame.data`: no release/free from this side (drmtap.h).
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

    /// `convert()` straight into `buf`, saving the copy out of the context; `buf` ends up holding
    /// exactly the frame, row stride `len / height`.
    pub fn convert_into(
        &mut self,
        desc: &mut drmtap_dmabuf_desc,
        received_fd: RawFd,
        buf: &mut Vec<u8>,
    ) -> io::Result<(u32, u32, Pixfmt)> {
        // The CPU deswizzle keeps the source pitch; every other path writes width * 4.
        let need = (desc.pitches[0] as usize)
            .max((desc.width as usize).saturating_mul(4))
            .saturating_mul(desc.height as usize);
        if need > MAX_FRAME_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "drm: refusing a {}x{} dma-buf with pitch {}",
                    desc.width, desc.height, desc.pitches[0]
                ),
            ));
        }
        buf.resize(need, 0);
        let (set_output, ctx) = (self.lib.set_output_buffer, self.ctx);
        // SAFETY: `buf` is not touched until the destination is detached below. If libdrmtap
        // refuses it, the frame lands in the context buffer and is copied out as before.
        unsafe { set_output(ctx, buf.as_mut_ptr().cast(), buf.len()) };
        let converted = self.convert(desc, received_fd);
        // SAFETY: NULL only detaches the destination; libdrmtap never frees it.
        unsafe { set_output(ctx, std::ptr::null_mut(), 0) };
        let (data, w, h, fmt) = converted?;
        if data.as_ptr() == buf.as_ptr() {
            let len = data.len();
            buf.truncate(len);
        } else {
            buf.clear();
            buf.extend_from_slice(data);
        }
        Ok((w, h, fmt))
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
mod convert_into_tests {
    use super::*;
    use std::ffi::c_void;

    struct Mock {
        dst: *mut c_void,
        given: *mut c_void,
        own: Vec<u8>,
        use_dst: bool,
    }

    unsafe extern "C" fn set_output(ctx: *mut drmtap_ctx, dst: *mut c_void, _: usize) -> i32 {
        let mock = &mut *ctx.cast::<Mock>();
        mock.dst = dst;
        if !dst.is_null() {
            mock.given = dst;
        }
        0
    }

    /// A 2x2 frame of 7s, written into the destination when `use_dst`, else into its own buffer.
    unsafe extern "C" fn convert(
        ctx: *mut drmtap_ctx,
        _: *const drmtap_dmabuf_desc,
        frame: *mut drmtap_frame_info,
    ) -> i32 {
        let mock = &mut *ctx.cast::<Mock>();
        let out: *mut u8 = if mock.use_dst && !mock.dst.is_null() {
            mock.dst.cast()
        } else {
            mock.own.as_mut_ptr()
        };
        std::ptr::write_bytes(out, 7, 16);
        (*frame).data = out.cast();
        (*frame).width = 2;
        (*frame).height = 2;
        (*frame).stride = 8;
        (*frame).format = DRM_FORMAT_XRGB8888;
        0
    }

    fn convert_with(use_dst: bool, buf: &mut Vec<u8>) -> Box<Mock> {
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
        let mock = Box::into_raw(Box::new(Mock {
            dst: std::ptr::null_mut(),
            given: std::ptr::null_mut(),
            own: vec![0; 16],
            use_dst,
        }));
        let mut conv = RenderConverter {
            lib: Box::leak(Box::new(lib)),
            ctx: mock.cast(),
        };
        let mut d = drmtap_dmabuf_desc {
            width: 2,
            height: 2,
            pitches: [8, 0, 0, 0],
            ..Default::default()
        };
        let res = conv.convert_into(&mut d, -1, buf).unwrap();
        assert!(matches!(res, (2, 2, Pixfmt::BGRA)));
        drop(conv);
        unsafe { Box::from_raw(mock) }
    }

    #[test]
    fn converts_into_the_callers_buffer() {
        let mut buf = vec![0; 64];
        let allocation = buf.as_mut_ptr().cast::<c_void>();
        let mock = convert_with(true, &mut buf);
        assert_eq!(mock.given, allocation);
        assert!(mock.dst.is_null());
        assert_eq!(buf, [7; 16]);
    }

    #[test]
    fn copies_a_frame_the_library_kept() {
        let mut buf = Vec::new();
        convert_with(false, &mut buf);
        assert_eq!(buf, [7; 16]);
    }
}
