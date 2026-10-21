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

/// Frames to skip in a row before rendering one anyway, so that a queue which
/// never drains cannot leave the display frozen.
const MAX_SKIPPED_FRAMES: u32 = 10;

/// Per-session render-decimation state for the video thread.
pub struct RenderGate {
    skipped: u32,
}

impl RenderGate {
    pub const fn new() -> Self {
        Self { skipped: 0 }
    }

    /// Whether a decoded frame should be upscaled and rendered.
    ///
    /// Always `true` when the upscaler is off or the frame could not be
    /// upscaled anyway (GPU texture, unsupported format, over-budget buffer),
    /// so those paths render exactly as before. Otherwise, a non-empty queue
    /// means newer frames are already waiting: this one is still decoded (delta
    /// codecs need every frame) but skipped. After [`MAX_SKIPPED_FRAMES`] in a
    /// row one is rendered regardless, which bounds both latency and the
    /// longest possible freeze under sustained overload.
    pub fn should_render<T: InvokeUiSession>(
        &mut self,
        session: &Session<T>,
        queue_empty: bool,
        rgb: &ImageRgb,
        pixelbuffer: bool,
    ) -> bool {
        let upscaler_on = session
            .lc
            .read()
            .map(|lc| lc.get_toggle_option(keys::OPTION_UPSCALER))
            .unwrap_or(false);
        let may_skip = !queue_empty && upscaler_on && upscale_target(rgb, pixelbuffer).is_some();
        self.next(may_skip)
    }

    fn next(&mut self, may_skip: bool) -> bool {
        if !may_skip || self.skipped >= MAX_SKIPPED_FRAMES {
            self.skipped = 0;
            return true;
        }
        self.skipped += 1;
        false
    }
}

/// The upscaled output format and dimensions for this frame, or `None` when the
/// upscaler cannot process it.
fn upscale_target(
    rgb: &ImageRgb,
    pixelbuffer: bool,
) -> Option<(upscaler::PixelFormat, (usize, usize))> {
    if !pixelbuffer {
        return None;
    }
    let fmt = match rgb.fmt() {
        ImageFormat::ABGR => upscaler::PixelFormat::Abgr,
        ImageFormat::ARGB => upscaler::PixelFormat::Argb,
        _ => return None,
    };
    let dims = upscaler::upscaled_dimensions(rgb.w, rgb.h)?;
    Some((fmt, dims))
}

/// Upscale `rgb` in place when the per-session upscaler toggle is on.
/// No-op unless the frame arrived as a pixel buffer in a supported format.
/// Over-budget frames are left at native resolution.
pub fn maybe_upscale_session_frame<T: InvokeUiSession>(
    session: &Session<T>,
    rgb: &mut ImageRgb,
    pixelbuffer: bool,
) {
    if !session
        .lc
        .read()
        .map(|lc| lc.get_toggle_option(keys::OPTION_UPSCALER))
        .unwrap_or(false)
    {
        return;
    }
    let (w, h) = (rgb.w, rgb.h);
    let (fmt, dims) = match upscale_target(rgb, pixelbuffer) {
        Some(target) => target,
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
    fn render_gate_never_starves_the_display() {
        let mut gate = RenderGate { skipped: 0 };
        // Frames the upscaler cannot process always render.
        assert!(gate.next(false));
        // Stale frames are skipped, but not indefinitely.
        for _ in 0..MAX_SKIPPED_FRAMES {
            assert!(!gate.next(true));
        }
        assert!(gate.next(true), "must render even if the queue never drains");
        assert!(!gate.next(true), "counter resets after the forced render");
    }

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
