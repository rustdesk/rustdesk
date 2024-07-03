#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(improper_ctypes)]
#![allow(dead_code)]

include!(concat!(env!("OUT_DIR"), "/yuv_ffi.rs"));

#[cfg(not(target_os = "ios"))]
use crate::PixelBuffer;
use crate::{generate_call_macro, EncodeYuvFormat, TraitPixelBuffer};
use hbb_common::{bail, log, ResultType};

generate_call_macro!(call_yuv, false);

#[cfg(not(target_os = "ios"))]
pub fn convert_to_yuv(
    captured: &PixelBuffer,
    dst_fmt: EncodeYuvFormat,
    dst: &mut Vec<u8>,
    mid_data: &mut Vec<u8>,
) -> ResultType<()> {
    let src = captured.data();
    let src_stride = captured.stride();
    let src_pixfmt = captured.pixfmt();
    let src_width = captured.width();
    let src_height = captured.height();
    if src_width > dst_fmt.w || src_height > dst_fmt.h {
        bail!(
            "src rect > dst rect: ({src_width}, {src_height}) > ({},{})",
            dst_fmt.w,
            dst_fmt.h
        );
    }
    if src_pixfmt == crate::Pixfmt::BGRA
        || src_pixfmt == crate::Pixfmt::RGBA
        || src_pixfmt == crate::Pixfmt::RGB565LE
    {
        // stride is calculated, not real, so we need to check it
        if src_stride[0] < src_width * src_pixfmt.bytes_per_pixel() {
            bail!(
                "src_stride too small: {} < {}",
                src_stride[0],
                src_width * src_pixfmt.bytes_per_pixel()
            );
        }
        if src.len() < src_stride[0] * src_height {
            bail!(
                "wrong src len, {} < {} * {}",
                src.len(),
                src_stride[0],
                src_height
            );
        }
    }
    let align = |x: usize| (x + 63) / 64 * 64;
    let unsupported = format!(
        "unsupported pixfmt conversion: {src_pixfmt:?} -> {:?}",
        dst_fmt.pixfmt
    );

    match (src_pixfmt, dst_fmt.pixfmt) {
        (crate::Pixfmt::BGRA, crate::Pixfmt::I420)
        | (crate::Pixfmt::RGBA, crate::Pixfmt::I420)
        | (crate::Pixfmt::RGB565LE, crate::Pixfmt::I420) => {
            let dst_stride_y = dst_fmt.stride[0];
            let dst_stride_uv = dst_fmt.stride[1];
            dst.resize(dst_fmt.h * dst_stride_y * 2, 0); // waste some memory to ensure memory safety
            let dst_y = dst.as_mut_ptr();
            let dst_u = dst[dst_fmt.u..].as_mut_ptr();
            let dst_v = dst[dst_fmt.v..].as_mut_ptr();
            let f = match src_pixfmt {
                crate::Pixfmt::BGRA => ARGBToI420,
                crate::Pixfmt::RGBA => ABGRToI420,
                crate::Pixfmt::RGB565LE => RGB565ToI420,
                _ => bail!(unsupported),
            };
            call_yuv!(f(
                src.as_ptr(),
                src_stride[0] as _,
                dst_y,
                dst_stride_y as _,
                dst_u,
                dst_stride_uv as _,
                dst_v,
                dst_stride_uv as _,
                src_width as _,
                src_height as _,
            ));
        }
        (crate::Pixfmt::BGRA, crate::Pixfmt::NV12)
        | (crate::Pixfmt::RGBA, crate::Pixfmt::NV12)
        | (crate::Pixfmt::RGB565LE, crate::Pixfmt::NV12) => {
            let dst_stride_y = dst_fmt.stride[0];
            let dst_stride_uv = dst_fmt.stride[1];
            dst.resize(
                align(dst_fmt.h) * (align(dst_stride_y) + align(dst_stride_uv / 2)),
                0,
            );
            let dst_y = dst.as_mut_ptr();
            let dst_uv = dst[dst_fmt.u..].as_mut_ptr();
            let (input, input_stride) = match src_pixfmt {
                crate::Pixfmt::BGRA => (src.as_ptr(), src_stride[0]),
                crate::Pixfmt::RGBA => (src.as_ptr(), src_stride[0]),
                crate::Pixfmt::RGB565LE => {
                    let mid_stride = src_width * 4;
                    mid_data.resize(mid_stride * src_height, 0);
                    call_yuv!(RGB565ToARGB(
                        src.as_ptr(),
                        src_stride[0] as _,
                        mid_data.as_mut_ptr(),
                        mid_stride as _,
                        src_width as _,
                        src_height as _,
                    ));
                    (mid_data.as_ptr(), mid_stride)
                }
                _ => bail!(unsupported),
            };
            let f = match src_pixfmt {
                crate::Pixfmt::BGRA => ARGBToNV12,
                crate::Pixfmt::RGBA => ABGRToNV12,
                crate::Pixfmt::RGB565LE => ARGBToNV12,
                _ => bail!(unsupported),
            };
            call_yuv!(f(
                input,
                input_stride as _,
                dst_y,
                dst_stride_y as _,
                dst_uv,
                dst_stride_uv as _,
                src_width as _,
                src_height as _,
            ));
        }
        (crate::Pixfmt::BGRA, crate::Pixfmt::I444)
        | (crate::Pixfmt::RGBA, crate::Pixfmt::I444)
        | (crate::Pixfmt::RGB565LE, crate::Pixfmt::I444) => {
            let dst_stride_y = dst_fmt.stride[0];
            let dst_stride_u = dst_fmt.stride[1];
            let dst_stride_v = dst_fmt.stride[2];
            dst.resize(
                align(dst_fmt.h)
                    * (align(dst_stride_y) + align(dst_stride_u) + align(dst_stride_v)),
                0,
            );
            let dst_y = dst.as_mut_ptr();
            let dst_u = dst[dst_fmt.u..].as_mut_ptr();
            let dst_v = dst[dst_fmt.v..].as_mut_ptr();
            let (input, input_stride) = match src_pixfmt {
                crate::Pixfmt::BGRA => (src.as_ptr(), src_stride[0]),
                crate::Pixfmt::RGBA => {
                    mid_data.resize(src.len(), 0);
                    call_yuv!(ABGRToARGB(
                        src.as_ptr(),
                        src_stride[0] as _,
                        mid_data.as_mut_ptr(),
                        src_stride[0] as _,
                        src_width as _,
                        src_height as _,
                    ));
                    (mid_data.as_ptr(), src_stride[0])
                }
                crate::Pixfmt::RGB565LE => {
                    let mid_stride = src_width * 4;
                    mid_data.resize(mid_stride * src_height, 0);
                    call_yuv!(RGB565ToARGB(
                        src.as_ptr(),
                        src_stride[0] as _,
                        mid_data.as_mut_ptr(),
                        mid_stride as _,
                        src_width as _,
                        src_height as _,
                    ));
                    (mid_data.as_ptr(), mid_stride)
                }
                _ => bail!(unsupported),
            };

            call_yuv!(ARGBToI444(
                input,
                input_stride as _,
                dst_y,
                dst_stride_y as _,
                dst_u,
                dst_stride_u as _,
                dst_v,
                dst_stride_v as _,
                src_width as _,
                src_height as _,
            ));
        }
        _ => {
            bail!(unsupported);
        }
    }
    Ok(())
}
