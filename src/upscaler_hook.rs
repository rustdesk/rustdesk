//! Thin bridge between the video thread and the `upscaler` crate.
//!
//! FSRCNNX runs client-side, after decode and before the frame reaches
//! Flutter: it never touches the wire, the encoder, or recordings (those
//! consume the pre-upscale buffer inside the decoder). Only the RGBA
//! pixel-buffer path is upscaled; the `vram` zero-copy texture path and
//! non-RGBA formats pass through untouched.

use base::config::keys;
use hbb_common::log;
use scrap::{ImageFormat, ImageRgb};

use crate::ui_session_interface::{InvokeUiSession, Session};

/// Whether a decoded frame should be upscaled and rendered.
///
/// Always `true` with the toggle off, so the video loop behaves exactly as
/// before. With it on, a non-empty queue means newer frames are already
/// waiting: this one is still decoded (delta codecs need every frame) but
/// skipped for upscaling/display, which bounds latency to the upscaler's pace.
pub fn should_render<T: InvokeUiSession>(session: &Session<T>, queue_empty: bool) -> bool {
    if queue_empty {
        return true;
    }
    !session
        .lc
        .read()
        .map(|lc| lc.get_toggle_option(keys::OPTION_UPSCALER))
        .unwrap_or(false)
}

/// Upscale `rgb` in place when the per-session upscaler toggle is on.
/// No-op unless the frame arrived as a pixel buffer in a supported format.
/// Over-budget frames are left at native resolution.
pub fn maybe_upscale_session_frame<T: InvokeUiSession>(
    session: &Session<T>,
    rgb: &mut ImageRgb,
    pixelbuffer: bool,
) {
    if !pixelbuffer {
        return;
    }
    if !session
        .lc
        .read()
        .map(|lc| lc.get_toggle_option(keys::OPTION_UPSCALER))
        .unwrap_or(false)
    {
        return;
    }
    let fmt = match rgb.fmt() {
        ImageFormat::ABGR => upscaler::PixelFormat::Abgr,
        ImageFormat::ARGB => upscaler::PixelFormat::Argb,
        _ => return,
    };
    let (w, h) = (rgb.w, rgb.h);
    let dims = match upscaler::upscaled_dimensions(w, h) {
        Some(d) => d,
        None => return,
    };
    // Decoder buffers can carry per-row stride padding (the macOS texture
    // path aligns rows to 64 bytes); the upscaler needs densely packed RGBA,
    // so repack a copy first. Afterwards `set_align(1)` makes the decoder
    // emit dense rows, so this copy happens at most until the first upscale
    // lands.
    let dense: Option<Vec<u8>> = if rgb.raw.len() == w.saturating_mul(h).saturating_mul(4) {
        None
    } else {
        match pack_rows(&rgb.raw, w, h) {
            Some(packed) => Some(packed),
            None => {
                log::trace!("FSRCNNX skipped {w}x{h} frame with unexpected stride");
                return;
            }
        }
    };
    let input: &[u8] = dense.as_deref().unwrap_or(&rgb.raw);
    let start = std::time::Instant::now();
    match upscaler::upscale_2x_fsrcnnx(input, w, h, fmt) {
        Some(upscaled) => {
            rgb.raw = upscaled;
            rgb.w = dims.0;
            rgb.h = dims.1;
            // The upscaled buffer is tightly packed; drop any decoder stride
            // padding flag so downstream consumers read it densely.
            rgb.set_align(1);
            if start.elapsed() > std::time::Duration::from_millis(500) {
                hbb_common::throttled_log!(
                    std::time::Duration::from_secs(30),
                    warn,
                    "FSRCNNX upscale of {w}x{h} took {:?}; consider disabling the upscaler for high resolutions",
                    start.elapsed(),
                );
            }
        }
        None => {
            log::trace!("FSRCNNX skipped {w}x{h} frame outside the pixel budget");
        }
    }
}

/// Repack a stride-padded frame into dense RGBA. Returns `None` when the
/// buffer is not exactly `h` rows of at least `w * 4` bytes.
fn pack_rows(raw: &[u8], w: usize, h: usize) -> Option<Vec<u8>> {
    let row = raw.len().checked_div(h)?;
    let need = w.checked_mul(4)?;
    if row.checked_mul(h)? != raw.len() || row < need {
        return None;
    }
    let mut out = Vec::with_capacity(need.saturating_mul(h));
    for y in 0..h {
        out.extend_from_slice(&raw[y * row..y * row + need]);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repacks_padded_rows_and_rejects_garbage() {
        // 3x2 frame with 4 bytes of padding per row.
        let raw: Vec<u8> = (0..2)
            .flat_map(|y| {
                (0..3)
                    .flat_map(move |x| [x as u8, y as u8, 42, 255])
                    .chain([9, 9, 9, 9])
            })
            .collect();
        assert_eq!(raw.len(), 2 * 16);
        let dense = pack_rows(&raw, 3, 2).expect("padded rows repack");
        assert_eq!(dense.len(), 3 * 2 * 4);
        assert_eq!(&dense[0..4], &[0, 0, 42, 255]);
        assert_eq!(&dense[12..16], &[0, 1, 42, 255]);
        assert_eq!(&dense[20..24], &[2, 1, 42, 255]);
        assert!(pack_rows(&raw, 3, 0).is_none());
        assert!(pack_rows(&raw[..raw.len() - 1], 3, 2).is_none());
        assert!(pack_rows(&raw, 5, 2).is_none());
    }
}
