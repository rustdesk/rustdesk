#![allow(dead_code)]

pub use scrap::STRIDE_ALIGN;
use scrap::{
    aom::{AomEncoder, AomEncoderConfig},
    codec::{EncoderApi, EncoderCfg},
    EncodeYuvFormat, Pixfmt, VpxEncoder, VpxEncoderConfig, VpxVideoCodecId,
};

// ---------------------------------------------------------------------------
// Resolutions
// ---------------------------------------------------------------------------

pub const RESOLUTIONS: &[(usize, usize, &str)] = &[
    (1280, 720, "720p"),
    (1920, 1080, "1080p"),
    (3840, 2160, "4K"),
];

// ---------------------------------------------------------------------------
// Alignment
// ---------------------------------------------------------------------------

#[inline]
pub fn align_up(x: usize, align: usize) -> usize {
    (x + align - 1) / align * align
}

// ---------------------------------------------------------------------------
// BGRA buffer generation
// ---------------------------------------------------------------------------

pub enum Pattern {
    /// Solid fill — compresses very well, potential fast-path.
    Solid(u8),
    /// Horizontal gradient — varied but predictable.
    Gradient,
    /// Pseudo-random (seeded LCG) — worst case, incompressible.
    Random(u64),
}

pub fn make_bgra(w: usize, h: usize, pattern: &Pattern) -> (Vec<u8>, usize) {
    let stride = w * 4;
    let mut buf = vec![0u8; stride * h];
    fill_bgra(&mut buf, w, h, stride, pattern);
    (buf, stride)
}

/// BGRA buffer with extra stride padding (simulates non-aligned capture).
pub fn make_bgra_strided(w: usize, h: usize, stride: usize, pattern: &Pattern) -> Vec<u8> {
    assert!(stride >= w * 4);
    let mut buf = vec![0u8; stride * h];
    fill_bgra(&mut buf, w, h, stride, pattern);
    buf
}

fn fill_bgra(buf: &mut [u8], w: usize, h: usize, stride: usize, pattern: &Pattern) {
    match pattern {
        Pattern::Solid(v) => {
            for row in 0..h {
                for col in 0..w {
                    let off = row * stride + col * 4;
                    buf[off] = *v;
                    buf[off + 1] = *v;
                    buf[off + 2] = *v;
                    buf[off + 3] = 255;
                }
            }
        }
        Pattern::Gradient => {
            for row in 0..h {
                for col in 0..w {
                    let off = row * stride + col * 4;
                    let v = ((row + col) % 256) as u8;
                    buf[off] = v;
                    buf[off + 1] = v;
                    buf[off + 2] = v;
                    buf[off + 3] = 255;
                }
            }
        }
        Pattern::Random(seed) => {
            let mut s = *seed;
            for row in 0..h {
                for col in 0..w {
                    let off = row * stride + col * 4;
                    for j in 0..4 {
                        s = s
                            .wrapping_mul(6364136223846793005)
                            .wrapping_add(1442695040888963407);
                        buf[off + j] = (s >> 33) as u8;
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// YUV layouts
// ---------------------------------------------------------------------------

pub struct I420Layout {
    pub stride_y: usize,
    pub stride_uv: usize,
    pub y_size: usize,
    pub uv_size: usize,
    pub total: usize,
}

pub fn i420_layout(w: usize, h: usize) -> I420Layout {
    let stride_y = align_up(w, STRIDE_ALIGN);
    let stride_uv = align_up(w / 2, STRIDE_ALIGN);
    let y_size = stride_y * h;
    let uv_size = stride_uv * (h / 2);
    I420Layout {
        stride_y,
        stride_uv,
        y_size,
        uv_size,
        total: y_size + 2 * uv_size,
    }
}

pub fn make_i420(w: usize, h: usize, shift: usize) -> (Vec<u8>, I420Layout) {
    let layout = i420_layout(w, h);
    let mut data = vec![0u8; layout.total];
    for row in 0..h {
        for col in 0..w {
            data[row * layout.stride_y + col] = ((row + col + shift) % 256) as u8;
        }
    }
    for i in layout.y_size..layout.total {
        data[i] = 128;
    }
    (data, layout)
}

pub struct NV12Layout {
    pub stride_y: usize,
    pub stride_uv: usize,
    pub y_size: usize,
    pub uv_size: usize,
    pub total: usize,
}

pub fn nv12_layout(w: usize, h: usize) -> NV12Layout {
    let stride_y = align_up(w, STRIDE_ALIGN);
    let stride_uv = align_up(w, STRIDE_ALIGN);
    let y_size = stride_y * h;
    let uv_size = stride_uv * (h / 2);
    NV12Layout {
        stride_y,
        stride_uv,
        y_size,
        uv_size,
        total: y_size + uv_size,
    }
}

pub fn make_nv12(w: usize, h: usize) -> (Vec<u8>, NV12Layout) {
    let layout = nv12_layout(w, h);
    let mut data = vec![0u8; layout.total];
    for row in 0..h {
        for col in 0..w {
            data[row * layout.stride_y + col] = ((row + col) % 256) as u8;
        }
    }
    for i in layout.y_size..layout.total {
        data[i] = 128;
    }
    (data, layout)
}

pub struct I444Layout {
    pub stride: usize,
    pub plane_size: usize,
    pub total: usize,
}

pub fn i444_layout(w: usize, h: usize) -> I444Layout {
    let stride = align_up(w, STRIDE_ALIGN);
    let plane_size = stride * h;
    I444Layout {
        stride,
        plane_size,
        total: 3 * plane_size,
    }
}

pub fn make_i444(w: usize, h: usize) -> (Vec<u8>, I444Layout) {
    let layout = i444_layout(w, h);
    let mut data = vec![0u8; layout.total];
    for row in 0..h {
        for col in 0..w {
            data[row * layout.stride + col] = ((row + col) % 256) as u8;
        }
    }
    for i in layout.plane_size..layout.total {
        data[i] = 128;
    }
    (data, layout)
}

// ---------------------------------------------------------------------------
// EncodeYuvFormat helpers
// ---------------------------------------------------------------------------

pub fn yuv_format_i420(w: usize, h: usize) -> EncodeYuvFormat {
    let layout = i420_layout(w, h);
    EncodeYuvFormat {
        pixfmt: Pixfmt::I420,
        w,
        h,
        stride: vec![layout.stride_y, layout.stride_uv, layout.stride_uv, 0],
        u: layout.y_size,
        v: layout.y_size + layout.uv_size,
    }
}

pub fn yuv_format_nv12(w: usize, h: usize) -> EncodeYuvFormat {
    let layout = nv12_layout(w, h);
    EncodeYuvFormat {
        pixfmt: Pixfmt::NV12,
        w,
        h,
        stride: vec![layout.stride_y, layout.stride_uv, 0, 0],
        u: layout.y_size,
        v: 0,
    }
}

pub fn yuv_format_i444(w: usize, h: usize) -> EncodeYuvFormat {
    let layout = i444_layout(w, h);
    EncodeYuvFormat {
        pixfmt: Pixfmt::I444,
        w,
        h,
        stride: vec![layout.stride, layout.stride, layout.stride, 0],
        u: layout.plane_size,
        v: 2 * layout.plane_size,
    }
}

// ---------------------------------------------------------------------------
// Pre-encoding helpers (for decode benchmarks)
// ---------------------------------------------------------------------------

pub struct EncodedFrame {
    pub data: Vec<u8>,
    pub key: bool,
    pub pts: i64,
}

pub fn pre_encode_vpx(
    codec: VpxVideoCodecId,
    w: usize,
    h: usize,
    quality: f32,
    n: usize,
) -> Vec<EncodedFrame> {
    let cfg = EncoderCfg::VPX(VpxEncoderConfig {
        width: w as _,
        height: h as _,
        quality,
        codec,
        keyframe_interval: None,
    });
    let mut enc = VpxEncoder::new(cfg, false).unwrap();
    let mut out = Vec::with_capacity(n);
    for i in 0.. {
        let (yuv, _) = make_i420(w, h, i * 3);
        for frame in enc.encode(i as i64, &yuv, STRIDE_ALIGN).unwrap() {
            out.push(EncodedFrame {
                data: frame.data.to_vec(),
                key: frame.key,
                pts: frame.pts,
            });
        }
        for frame in enc.flush().unwrap() {
            out.push(EncodedFrame {
                data: frame.data.to_vec(),
                key: frame.key,
                pts: frame.pts,
            });
        }
        if out.len() >= n {
            break;
        }
    }
    out.truncate(n);
    out
}

pub fn pre_encode_av1(w: usize, h: usize, quality: f32, n: usize) -> Vec<EncodedFrame> {
    let cfg = EncoderCfg::AOM(AomEncoderConfig {
        width: w as _,
        height: h as _,
        quality,
        keyframe_interval: None,
    });
    let mut enc = AomEncoder::new(cfg, false).unwrap();
    let mut out = Vec::with_capacity(n);
    for i in 0.. {
        let (yuv, _) = make_i420(w, h, i * 3);
        for frame in enc.encode(i as i64, &yuv, STRIDE_ALIGN).unwrap() {
            out.push(EncodedFrame {
                data: frame.data.to_vec(),
                key: frame.key,
                pts: frame.pts,
            });
        }
        if out.len() >= n {
            break;
        }
    }
    out.truncate(n);
    out
}
