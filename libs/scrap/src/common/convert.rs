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
