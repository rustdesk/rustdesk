use super::vpx::*;
use std::os::raw::c_int;

extern "C" {
    // seems libyuv uses reverse byte order compared with our view

    pub fn ARGBRotate(
        src_argb: *const u8,
        src_stride_argb: c_int,
        dst_argb: *mut u8,
        dst_stride_argb: c_int,
        src_width: c_int,
        src_height: c_int,
        mode: c_int,
    ) -> c_int;

    pub fn ARGBMirror(
        src_argb: *const u8,
        src_stride_argb: c_int,
        dst_argb: *mut u8,
        dst_stride_argb: c_int,
        width: c_int,
        height: c_int,
    ) -> c_int;

    pub fn ARGBToI420(
        src_bgra: *const u8,
        src_stride_bgra: c_int,
        dst_y: *mut u8,
        dst_stride_y: c_int,
        dst_u: *mut u8,
        dst_stride_u: c_int,
        dst_v: *mut u8,
        dst_stride_v: c_int,
        width: c_int,
        height: c_int,
    ) -> c_int;

    pub fn ABGRToI420(
        src_rgba: *const u8,
        src_stride_rgba: c_int,
        dst_y: *mut u8,
        dst_stride_y: c_int,
        dst_u: *mut u8,
        dst_stride_u: c_int,
        dst_v: *mut u8,
        dst_stride_v: c_int,
        width: c_int,
        height: c_int,
    ) -> c_int;

    pub fn ARGBToNV12(
        src_bgra: *const u8,
        src_stride_bgra: c_int,
        dst_y: *mut u8,
        dst_stride_y: c_int,
        dst_uv: *mut u8,
        dst_stride_uv: c_int,
        width: c_int,
        height: c_int,
    ) -> c_int;

    pub fn NV12ToI420(
        src_y: *const u8,
        src_stride_y: c_int,
        src_uv: *const u8,
        src_stride_uv: c_int,
        dst_y: *mut u8,
        dst_stride_y: c_int,
        dst_u: *mut u8,
        dst_stride_u: c_int,
        dst_v: *mut u8,
        dst_stride_v: c_int,
        width: c_int,
        height: c_int,
    ) -> c_int;

    // I420ToRGB24: RGB little endian (bgr in memory)
    // I420ToRaw: RGB big endian (rgb in memory) to RGBA.
    pub fn I420ToRAW(
        src_y: *const u8,
        src_stride_y: c_int,
        src_u: *const u8,
        src_stride_u: c_int,
        src_v: *const u8,
        src_stride_v: c_int,
        dst_rgba: *mut u8,
        dst_stride_raw: c_int,
        width: c_int,
        height: c_int,
    ) -> c_int;

    pub fn I420ToARGB(
        src_y: *const u8,
        src_stride_y: c_int,
        src_u: *const u8,
        src_stride_u: c_int,
        src_v: *const u8,
        src_stride_v: c_int,
        dst_rgba: *mut u8,
        dst_stride_rgba: c_int,
        width: c_int,
        height: c_int,
    ) -> c_int;

    pub fn I420ToABGR(
        src_y: *const u8,
        src_stride_y: c_int,
        src_u: *const u8,
        src_stride_u: c_int,
        src_v: *const u8,
        src_stride_v: c_int,
        dst_rgba: *mut u8,
        dst_stride_rgba: c_int,
        width: c_int,
        height: c_int,
    ) -> c_int;

    pub fn NV12ToARGB(
        src_y: *const u8,
        src_stride_y: c_int,
        src_uv: *const u8,
        src_stride_uv: c_int,
        dst_rgba: *mut u8,
        dst_stride_rgba: c_int,
        width: c_int,
        height: c_int,
    ) -> c_int;

    pub fn NV12ToABGR(
        src_y: *const u8,
        src_stride_y: c_int,
        src_uv: *const u8,
        src_stride_uv: c_int,
        dst_rgba: *mut u8,
        dst_stride_rgba: c_int,
        width: c_int,
        height: c_int,
    ) -> c_int;
}

// https://github.com/webmproject/libvpx/blob/master/vpx/src/vpx_image.c
#[inline]
fn get_vpx_i420_stride(
    width: usize,
    height: usize,
    stride_align: usize,
) -> (usize, usize, usize, usize, usize, usize) {
    let mut img = Default::default();
    unsafe {
        vpx_img_wrap(
            &mut img,
            vpx_img_fmt::VPX_IMG_FMT_I420,
            width as _,
            height as _,
            stride_align as _,
            0x1 as _,
        );
    }
    (
        img.w as _,
        img.h as _,
        img.stride[0] as _,
        img.stride[1] as _,
        img.planes[1] as usize - img.planes[0] as usize,
        img.planes[2] as usize - img.planes[0] as usize,
    )
}

pub fn i420_to_rgb(width: usize, height: usize, src: &[u8], dst: &mut Vec<u8>) {
    let (_, _, src_stride_y, src_stride_uv, u, v) =
        get_vpx_i420_stride(width, height, super::STRIDE_ALIGN);
    let src_y = src.as_ptr();
    let src_u = src[u..].as_ptr();
    let src_v = src[v..].as_ptr();
    dst.resize(width * height * 3, 0);
    unsafe {
        super::I420ToRAW(
            src_y,
            src_stride_y as _,
            src_u,
            src_stride_uv as _,
            src_v,
            src_stride_uv as _,
            dst.as_mut_ptr(),
            (width * 3) as _,
            width as _,
            height as _,
        );
    };
}

pub fn bgra_to_i420(width: usize, height: usize, src: &[u8], dst: &mut Vec<u8>) {
    let (_, h, dst_stride_y, dst_stride_uv, u, v) =
        get_vpx_i420_stride(width, height, super::STRIDE_ALIGN);
    dst.resize(h * dst_stride_y * 2, 0); // waste some memory to ensure memory safety
    let dst_y = dst.as_mut_ptr();
    let dst_u = dst[u..].as_mut_ptr();
    let dst_v = dst[v..].as_mut_ptr();
    unsafe {
        ARGBToI420(
            src.as_ptr(),
            (src.len() / height) as _,
            dst_y,
            dst_stride_y as _,
            dst_u,
            dst_stride_uv as _,
            dst_v,
            dst_stride_uv as _,
            width as _,
            height as _,
        );
    }
}

pub fn rgba_to_i420(width: usize, height: usize, src: &[u8], dst: &mut Vec<u8>) {
    let (_, h, dst_stride_y, dst_stride_uv, u, v) =
        get_vpx_i420_stride(width, height, super::STRIDE_ALIGN);
    dst.resize(h * dst_stride_y * 2, 0); // waste some memory to ensure memory safety
    let dst_y = dst.as_mut_ptr();
    let dst_u = dst[u..].as_mut_ptr();
    let dst_v = dst[v..].as_mut_ptr();
    unsafe {
        ABGRToI420(
            src.as_ptr(),
            (src.len() / height) as _,
            dst_y,
            dst_stride_y as _,
            dst_u,
            dst_stride_uv as _,
            dst_v,
            dst_stride_uv as _,
            width as _,
            height as _,
        );
    }
}

pub unsafe fn nv12_to_i420(
    src_y: *const u8,
    src_stride_y: c_int,
    src_uv: *const u8,
    src_stride_uv: c_int,
    width: usize,
    height: usize,
    dst: &mut Vec<u8>,
) {
    let (_, h, dst_stride_y, dst_stride_uv, u, v) =
        get_vpx_i420_stride(width, height, super::STRIDE_ALIGN);
    dst.resize(h * dst_stride_y * 2, 0); // waste some memory to ensure memory safety
    let dst_y = dst.as_mut_ptr();
    let dst_u = dst[u..].as_mut_ptr();
    let dst_v = dst[v..].as_mut_ptr();
    NV12ToI420(
        src_y,
        src_stride_y,
        src_uv,
        src_stride_uv,
        dst_y,
        dst_stride_y as _,
        dst_u,
        dst_stride_uv as _,
        dst_v,
        dst_stride_uv as _,
        width as _,
        height as _,
    );
}

#[cfg(feature = "hwcodec")]
pub mod hw {
    use hbb_common::{anyhow::anyhow, ResultType};
    use crate::ImageFormat;
    #[cfg(target_os = "windows")]
    use hwcodec::{ffmpeg::ffmpeg_linesize_offset_length, AVPixelFormat};

    pub fn hw_bgra_to_i420(
        width: usize,
        height: usize,
        stride: &[i32],
        offset: &[i32],
        length: i32,
        src: &[u8],
        dst: &mut Vec<u8>,
    ) {
        let stride_y = stride[0] as usize;
        let stride_u = stride[1] as usize;
        let stride_v = stride[2] as usize;
        let offset_u = offset[0] as usize;
        let offset_v = offset[1] as usize;

        dst.resize(length as _, 0);
        let dst_y = dst.as_mut_ptr();
        let dst_u = dst[offset_u..].as_mut_ptr();
        let dst_v = dst[offset_v..].as_mut_ptr();
        unsafe {
            super::ARGBToI420(
                src.as_ptr(),
                (src.len() / height) as _,
                dst_y,
                stride_y as _,
                dst_u,
                stride_u as _,
                dst_v,
                stride_v as _,
                width as _,
                height as _,
            );
        }
    }

    pub fn hw_bgra_to_nv12(
        width: usize,
        height: usize,
        stride: &[i32],
        offset: &[i32],
        length: i32,
        src: &[u8],
        dst: &mut Vec<u8>,
    ) {
        let stride_y = stride[0] as usize;
        let stride_uv = stride[1] as usize;
        let offset_uv = offset[0] as usize;

        dst.resize(length as _, 0);
        let dst_y = dst.as_mut_ptr();
        let dst_uv = dst[offset_uv..].as_mut_ptr();
        unsafe {
            super::ARGBToNV12(
                src.as_ptr(),
                (src.len() / height) as _,
                dst_y,
                stride_y as _,
                dst_uv,
                stride_uv as _,
                width as _,
                height as _,
            );
        }
    }

    #[cfg(target_os = "windows")]
    pub fn hw_nv12_to(
        fmt: ImageFormat,
        width: usize,
        height: usize,
        src_y: &[u8],
        src_uv: &[u8],
        src_stride_y: usize,
        src_stride_uv: usize,
        dst: &mut Vec<u8>,
        i420: &mut Vec<u8>,
        align: usize,
    ) -> ResultType<()> {
        let nv12_stride_y = src_stride_y;
        let nv12_stride_uv = src_stride_uv;
        if let Ok((linesize_i420, offset_i420, i420_len)) =
            ffmpeg_linesize_offset_length(AVPixelFormat::AV_PIX_FMT_YUV420P, width, height, align)
        {
            dst.resize(width * height * 4, 0);
            let i420_stride_y = linesize_i420[0];
            let i420_stride_u = linesize_i420[1];
            let i420_stride_v = linesize_i420[2];
            i420.resize(i420_len as _, 0);

            unsafe {
                let i420_offset_y = i420.as_ptr().add(0) as _;
                let i420_offset_u = i420.as_ptr().add(offset_i420[0] as _) as _;
                let i420_offset_v = i420.as_ptr().add(offset_i420[1] as _) as _;
                super::NV12ToI420(
                    src_y.as_ptr(),
                    nv12_stride_y as _,
                    src_uv.as_ptr(),
                    nv12_stride_uv as _,
                    i420_offset_y,
                    i420_stride_y,
                    i420_offset_u,
                    i420_stride_u,
                    i420_offset_v,
                    i420_stride_v,
                    width as _,
                    height as _,
                );
                match fmt {
                    ImageFormat::ARGB => {
                        super::I420ToARGB(
                            i420_offset_y,
                            i420_stride_y,
                            i420_offset_u,
                            i420_stride_u,
                            i420_offset_v,
                            i420_stride_v,
                            dst.as_mut_ptr(),
                            (width * 4) as _,
                            width as _,
                            height as _,
                        );
                    }
                    ImageFormat::ABGR => {
                        super::I420ToABGR(
                            i420_offset_y,
                            i420_stride_y,
                            i420_offset_u,
                            i420_stride_u,
                            i420_offset_v,
                            i420_stride_v,
                            dst.as_mut_ptr(),
                            (width * 4) as _,
                            width as _,
                            height as _,
                        );
                    }
                    _ => {
                        return Err(anyhow!("unsupported image format"));
                    }
                }
                return Ok(());
            };
        }
        return Err(anyhow!("get linesize offset failed"));
    }

    #[cfg(not(target_os = "windows"))]
    pub fn hw_nv12_to(
        fmt: ImageFormat,
        width: usize,
        height: usize,
        src_y: &[u8],
        src_uv: &[u8],
        src_stride_y: usize,
        src_stride_uv: usize,
        dst: &mut Vec<u8>,
        _i420: &mut Vec<u8>,
        _align: usize,
    ) -> ResultType<()> {
        dst.resize(width * height * 4, 0);
        unsafe {
            match fmt {
                ImageFormat::ARGB => {
                    match super::NV12ToARGB(
                        src_y.as_ptr(),
                        src_stride_y as _,
                        src_uv.as_ptr(),
                        src_stride_uv as _,
                        dst.as_mut_ptr(),
                        (width * 4) as _,
                        width as _,
                        height as _,
                    ) {
                        0 => Ok(()),
                        _ => Err(anyhow!("NV12ToARGB failed")),
                    }
                }
                ImageFormat::ABGR => {
                    match super::NV12ToABGR(
                        src_y.as_ptr(),
                        src_stride_y as _,
                        src_uv.as_ptr(),
                        src_stride_uv as _,
                        dst.as_mut_ptr(),
                        (width * 4) as _,
                        width as _,
                        height as _,
                    ) {
                        0 => Ok(()),
                        _ => Err(anyhow!("NV12ToABGR failed")),
                    }
                }
                _ => {
                    Err(anyhow!("unsupported image format"))
                }
            }
        }
    }

    pub fn hw_i420_to(
        fmt: ImageFormat,
        width: usize,
        height: usize,
        src_y: &[u8],
        src_u: &[u8],
        src_v: &[u8],
        src_stride_y: usize,
        src_stride_u: usize,
        src_stride_v: usize,
        dst: &mut Vec<u8>,
    ) {
        let src_y = src_y.as_ptr();
        let src_u = src_u.as_ptr();
        let src_v = src_v.as_ptr();
        dst.resize(width * height * 4, 0);
        unsafe {
            match fmt {
                ImageFormat::ARGB => {
                    super::I420ToARGB(
                        src_y,
                        src_stride_y as _,
                        src_u,
                        src_stride_u as _,
                        src_v,
                        src_stride_v as _,
                        dst.as_mut_ptr(),
                        (width * 4) as _,
                        width as _,
                        height as _,
                    );
                }
                ImageFormat::ABGR => {
                    super::I420ToABGR(
                        src_y,
                        src_stride_y as _,
                        src_u,
                        src_stride_u as _,
                        src_v,
                        src_stride_v as _,
                        dst.as_mut_ptr(),
                        (width * 4) as _,
                        width as _,
                        height as _,
                    );
                }
                _ => {
                }
            }
        };
    }
}
