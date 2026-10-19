//! Client-side upscalers for decoded remote frames.
//!
//! First resident is a faithful CPU port of Magpie's `FSRCNNX_x2_8-0-4-1`
//! (`Magpie/src/Effects/FSRCNNX/FSRCNNX.hlsl`, weights via
//! `igv/FSRCNN-TensorFlow`, FSRCNN by Dong et al.): luma-only CNN + bilinear
//! chroma, fixed 2x. GPU compute shaders have no hook in the Flutter
//! compositing path, so this runs on the CPU in the video thread and stays
//! fully contained here; callers only see byte buffers and an option key.
//!
//! Cost note: ~5-10k FMAs per input pixel plus six full-resolution passes.
//! The video thread must gate this behind the user toggle and pixel budget;
//! see [`fits_budget`]. The toggle itself is read through the session's
//! `get_toggle_option`, the single source of truth for its encoding.

pub mod fsrcnnx;
#[allow(clippy::excessive_precision)]
mod weights;

/// Fixed scale factor of FSRCNNX. The Magpie effect hard-codes
/// `OUTPUT(WIDTH INPUT_WIDTH*2, HEIGHT INPUT_HEIGHT*2)`.
pub const SCALE: usize = 2;

/// Receptive-field radius: Pass1 5x5 (2) + four 3x3 mappings (4) + Pass6 3x3 (1).
pub const HALO: i32 = 7;

/// Tile width/height (input pixels) for tiled inference. Bounds transient
/// memory to roughly `(TILE + 2*HALO)^2 * 8ch * 3 bufs * 4B` (~2MB).
pub const TILE: usize = 128;

/// Hard cap on input pixels per upscale call. 1920x1080 in -> 3840x2160 out
/// (33MB RGBA) is the largest sane frame: beyond this the memory and the
/// multi-second CPU stall hurt more than the upscale helps. The caller keeps
/// the original frame.
pub const MAX_INPUT_PIXELS: usize = 1920 * 1080;

/// Byte order of an interleaved 4-byte-per-pixel frame buffer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PixelFormat {
    /// Memory order R, G, B, A. Matches libyuv ABGR output, which is what
    /// `scrap::ImageRgb` holds when `ImageFormat::ABGR` is selected.
    Rgba,
    /// Alias of [`PixelFormat::Rgba`]; kept so call sites can name the scrap
    /// format directly. libyuv ABGR bytes are R, G, B, A in memory.
    Abgr,
    /// Memory order B, G, R, A (libyuv ARGB output).
    Argb,
}

impl PixelFormat {
    #[inline]
    fn r_idx(self) -> usize {
        match self {
            PixelFormat::Rgba | PixelFormat::Abgr => 0,
            PixelFormat::Argb => 2,
        }
    }

    #[inline]
    fn g_idx(self) -> usize {
        1
    }

    #[inline]
    fn b_idx(self) -> usize {
        match self {
            PixelFormat::Rgba | PixelFormat::Abgr => 2,
            PixelFormat::Argb => 0,
        }
    }
}

/// Pixel-budget guard. Returns false for empty frames, size mismatches, or
/// frames whose 2x output would exceed a sane allocation.
pub fn fits_budget(w: usize, h: usize, raw_len: usize) -> bool {
    if w == 0 || h == 0 {
        return false;
    }
    match w.checked_mul(h) {
        Some(px) if px <= MAX_INPUT_PIXELS => {}
        _ => return false,
    }
    let out_bytes = w
        .checked_mul(SCALE)
        .and_then(|ow| h.checked_mul(SCALE).and_then(|oh| ow.checked_mul(oh)))
        .and_then(|px| px.checked_mul(4));
    if out_bytes.is_none() {
        return false;
    }
    raw_len == w.saturating_mul(h).saturating_mul(4)
}

/// Upscale an interleaved frame 2x with FSRCNNX (general variant).
/// Returns the upscaled buffer in the same byte order, or `None` when the
/// input fails [`fits_budget`] or a worker thread fails.
pub fn upscale_2x_fsrcnnx(raw: &[u8], w: usize, h: usize, fmt: PixelFormat) -> Option<Vec<u8>> {
    if !fits_budget(w, h, raw.len()) {
        return None;
    }
    fsrcnnx::upscale(raw, w, h, fmt)
}

/// Output dimensions for a given input. `None` on overflow.
pub fn upscaled_dimensions(w: usize, h: usize) -> Option<(usize, usize)> {
    Some((w.checked_mul(SCALE)?, h.checked_mul(SCALE)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: usize, h: usize, r: u8, g: u8, b: u8) -> Vec<u8> {
        let mut v = Vec::with_capacity(w * h * 4);
        for _ in 0..w * h {
            v.extend_from_slice(&[r, g, b, 255]);
        }
        v
    }

    #[test]
    fn rejects_bad_input() {
        assert!(upscale_2x_fsrcnnx(&[], 0, 0, PixelFormat::Rgba).is_none());
        assert!(upscale_2x_fsrcnnx(&[0u8; 12], 1, 1, PixelFormat::Rgba).is_none());
        assert!(upscale_2x_fsrcnnx(&[0u8; 4], 1, 1, PixelFormat::Rgba).is_some());
        assert!(upscaled_dimensions(usize::MAX, 2).is_none());
    }

    #[test]
    fn tiny_frame_upscales_and_stays_deterministic() {
        let src = solid(4, 4, 128, 200, 64);
        let a = upscale_2x_fsrcnnx(&src, 4, 4, PixelFormat::Rgba).expect("fits");
        assert_eq!(a.len(), 8 * 8 * 4);
        let b = upscale_2x_fsrcnnx(&src, 4, 4, PixelFormat::Rgba).expect("fits");
        assert_eq!(a, b);
        assert!(a.chunks_exact(4).all(|p| p[3] == 255));
    }

    #[test]
    fn single_pixel_clamps_without_panic() {
        let src = solid(1, 1, 10, 20, 30);
        let out = upscale_2x_fsrcnnx(&src, 1, 1, PixelFormat::Rgba).expect("fits");
        assert_eq!(out.len(), 2 * 2 * 4);
    }

    #[test]
    fn abgr_and_rgba_agree_on_swizzled_input() {
        let rgba = solid(8, 6, 11, 22, 33);
        let mut abgr = rgba.clone();
        for p in abgr.chunks_exact_mut(4) {
            p.swap(0, 2);
        }
        let from_rgba =
            upscale_2x_fsrcnnx(&rgba, 8, 6, PixelFormat::Rgba).expect("fits");
        let mut from_abgr =
            upscale_2x_fsrcnnx(&abgr, 8, 6, PixelFormat::Argb).expect("fits");
        for p in from_abgr.chunks_exact_mut(4) {
            p.swap(0, 2);
        }
        assert_eq!(from_rgba, from_abgr);
    }

    #[test]
    fn flat_grey_stays_near_grey() {
        let src = solid(16, 16, 128, 128, 128);
        let out = upscale_2x_fsrcnnx(&src, 16, 16, PixelFormat::Rgba).expect("fits");
        let cx = (out.len() / 2) & !3;
        let (r, g, b) = (out[cx], out[cx + 1], out[cx + 2]);
        for c in [r, g, b] {
            assert!(
                (96..=160).contains(&c),
                "flat-field drifted too far: {:?}",
                (r, g, b)
            );
        }
    }

    #[test]
    fn multi_tile_flat_field_has_no_seams() {
        // 288x200 spans 3x2 tiles of 128px, crossing tile boundaries at
        // input x=128/256 and y=128. A flat field must come out bit-uniform
        // well inside the borders; any halo/tiling error shows up here.
        let (w, h) = (288, 200);
        let src = solid(w, h, 100, 150, 200);
        let out = upscale_2x_fsrcnnx(&src, w, h, PixelFormat::Rgba).expect("fits");
        let ow = w * 2;
        let px = |x: usize, y: usize| {
            let o = (y * ow + x) * 4;
            (out[o], out[o + 1], out[o + 2], out[o + 3])
        };
        let first = px(200, 180);
        for y in (160..240).step_by(7) {
            for x in (200..376).step_by(11) {
                assert_eq!(px(x, y), first, "seam at output ({x}, {y})");
            }
        }
    }

    #[test]
    fn nonuniform_image_across_tile_seam_is_exact() {
        // A solid image cannot catch a wrong tap position or a broken tile
        // border, because every pixel carries the same value. 132px crosses
        // the 128px tile boundary, so output x=256 sits on the seam; the
        // expected bytes pin the tap order and the border handling.
        let (w, h) = (132, 4);
        let mut src = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                let o = (y * w + x) * 4;
                src[o] = ((x * 7 + y * 3) % 256) as u8;
                src[o + 1] = ((x * 13 + y * 5) % 256) as u8;
                src[o + 2] = ((x * 29 + y * 11) % 256) as u8;
                src[o + 3] = 255;
            }
        }
        let out = upscale_2x_fsrcnnx(&src, w, h, PixelFormat::Rgba).expect("fits");
        let ow = w * 2;
        let px = |x: usize, y: usize| {
            let o = (y * ow + x) * 4;
            [out[o], out[o + 1], out[o + 2]]
        };
        for (x, y, want) in [
            (10, 1, [34, 63, 140]),
            (130, 3, [202, 81, 100]),
            (254, 0, [119, 111, 91]),  // left of the seam
            (256, 2, [128, 128, 129]), // right of the seam
            (263, 3, [154, 175, 230]), // right edge
        ] {
            assert_eq!(px(x, y), want, "pixel ({x}, {y})");
        }
    }
}
