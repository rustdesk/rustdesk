//! Experimental SDR DMA-BUF input for VAAPI. Unsupported capture metadata stays
//! on the existing pixel path; any GPU encode failure disables the experiment.
use super::{
    convert, drm_render::RenderConverter, drmtap_dl::drmtap_dmabuf_desc, EncodeYuvFormat,
    PixelBuffer, TraitPixelBuffer,
};
use hbb_common::{anyhow::anyhow, log, ResultType};
use std::{
    os::fd::AsRawFd,
    sync::atomic::{AtomicBool, Ordering},
};

pub use ::hwcodec::ffmpeg_ram::encode::DmaBufFrame;
static FAILED: AtomicBool = AtomicBool::new(false);

pub fn enabled() -> bool {
    std::env::var("RUSTDESK_DRM_VAAPI").as_deref() == Ok("1") && !FAILED.load(Ordering::Relaxed)
}

/// Carry the explicit opt-in through sudo implementations that do not preserve
/// the service environment when starting the unprivileged server.
pub fn service_env() -> Option<(&'static str, String)> {
    std::env::var("RUSTDESK_DRM_VAAPI")
        .ok()
        .filter(|value| value == "1")
        .map(|value| ("RUSTDESK_DRM_VAAPI", value))
}

pub fn disable(reason: &str) {
    if !FAILED.swap(true, Ordering::Relaxed) {
        log::warn!("drm: disabling experimental GPU encoding; retaining CPU capture/VAAPI encoding: {reason}");
    }
}

/// Read back only when a screenshot or fallback explicitly requires CPU pixels.
fn with_pixels<T>(
    frame: &DmaBufFrame,
    f: impl FnOnce(&PixelBuffer<'_>) -> ResultType<T>,
) -> ResultType<T> {
    let mut converter = RenderConverter::open_render(Some(&frame.render_node))
        .ok_or_else(|| anyhow!("could not open DRM screenshot/fallback renderer"))?;
    let mut desc = drmtap_dmabuf_desc {
        dma_buf_fd: frame.fd.as_raw_fd(),
        width: frame.width,
        height: frame.height,
        format: frame.format,
        modifier: frame.modifier,
        fb_id: 0,
        num_planes: frame.num_planes,
        offsets: frame.offsets,
        pitches: frame.pitches,
        hdr_eotf: 0,
        hdr_max_nits: 0,
    };
    let (data, w, h, fmt) = converter.convert(&mut desc, frame.fd.as_raw_fd())?;
    if (w, h) != (frame.width, frame.height) {
        return Err(anyhow!("GPU readback changed geometry"));
    }
    f(&PixelBuffer::new(data, fmt, w as usize, h as usize))
}

pub fn rgba(frame: &DmaBufFrame) -> ResultType<Vec<u8>> {
    with_pixels(frame, rgba_pixels)
}

fn rgba_pixels(pixels: &PixelBuffer<'_>) -> ResultType<Vec<u8>> {
    let mut rgba = Vec::new();
    convert::convert(pixels, super::Pixfmt::RGBA, &mut rgba)?;
    let row_bytes = pixels.width() * 4;
    let stride = pixels.stride()[0];
    if stride == row_bytes {
        return Ok(rgba);
    }
    // Screenshot consumers require packed rows even when a linear scanout has padding.
    Ok(rgba
        .chunks_exact(stride)
        .flat_map(|row| row[..row_bytes].iter().copied())
        .collect())
}

pub fn fallback_yuv(frame: &DmaBufFrame, fmt: EncodeYuvFormat) -> ResultType<Vec<u8>> {
    with_pixels(frame, |pixels| {
        let mut yuv = Vec::new();
        convert::convert_to_yuv(pixels, fmt, &mut yuv, &mut Vec::new())?;
        Ok(yuv)
    })
}

/// Limit the initial GPU experiment to packed SDR RGB. HDR keeps tone mapping in
/// libdrmtap; rotation is checked by the capturer before handing over a frame.
pub fn supports_format(format: u32, hdr_eotf: u32) -> bool {
    hdr_eotf == 0
        && matches!(
            format,
            0x3432_5258 | 0x3432_5241 | 0x3432_4258 | 0x3432_4241 | // XR24/AR24/XB24/AB24
        0x3033_5258 | 0x3033_5241 | 0x3033_4258 | 0x3033_4241
        ) // XR30/AR30/XB30/AB30
}

pub fn render_node(path: &str) -> Option<std::path::PathBuf> {
    let node = super::drm_reader::device_under_dev_dri(path)?;
    let name = node.file_name()?.to_str()?.strip_prefix("renderD")?;
    (!name.is_empty() && name.bytes().all(|c| c.is_ascii_digit())).then_some(node)
}

#[cfg(test)]
mod drm_gpu_tests {
    use super::*;

    #[test]
    fn hdr_and_non_rgb_inputs_retain_the_existing_conversion_path() {
        for format in [0x3432_5258, 0x3432_4241, 0x3033_5258, 0x3033_4241] {
            assert!(supports_format(format, 0));
            assert!(!supports_format(format, 2));
            assert!(!supports_format(format, 3));
        }
        assert!(!supports_format(0x3231_564e, 0)); // NV12 requires different colour handling.
        assert!(!supports_format(0, 0));
    }

    #[test]
    fn encoder_input_preserves_the_owned_descriptor_without_reading_pixels() {
        use super::super::{EncodeInput, Frame, Pixfmt};
        use std::os::fd::AsRawFd;
        let frame = DmaBufFrame {
            fd: std::fs::File::open("/dev/null").unwrap().into(),
            render_node: "/dev/dri/renderD128".into(),
            width: 2,
            height: 2,
            format: 0x3432_5258,
            num_planes: 1,
            offsets: [0; 4],
            pitches: [8, 0, 0, 0],
            modifier: 0,
        };
        let fd = frame.fd.as_raw_fd();
        let captured = Frame::DmaBuf(frame);
        let mut yuv = vec![9];
        let fmt = EncodeYuvFormat {
            pixfmt: Pixfmt::NV12,
            w: 2,
            h: 2,
            stride: vec![2, 2],
            u: 4,
            v: 0,
        };
        match captured.to(fmt, &mut yuv, &mut Vec::new()).unwrap() {
            EncodeInput::DmaBuf(frame) => {
                assert_eq!(
                    (frame.width, frame.height, frame.fd.as_raw_fd()),
                    (2, 2, fd)
                );
            }
            _ => panic!("GPU-capable encoder did not receive its DMA-BUF"),
        }
        assert_eq!(yuv, [9]);
    }
    #[test]
    fn screenshots_return_packed_rgba_for_padded_scanouts() {
        for fmt in [super::super::Pixfmt::BGRA, super::super::Pixfmt::RGBA] {
            let pixels = [
                1, 2, 3, 255, 4, 5, 6, 255, 99, 99, 99, 99, 7, 8, 9, 255, 10, 11, 12, 255, 99, 99,
                99, 99,
            ];
            let input = PixelBuffer::new(&pixels, fmt, 2, 2);
            let expected = if fmt == super::super::Pixfmt::BGRA {
                vec![3, 2, 1, 255, 6, 5, 4, 255, 9, 8, 7, 255, 12, 11, 10, 255]
            } else {
                vec![1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255]
            };
            assert_eq!(rgba_pixels(&input).unwrap(), expected);
        }
    }
}
