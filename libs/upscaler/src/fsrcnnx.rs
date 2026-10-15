//! Faithful CPU port of Magpie `FSRCNNX_x2_8-0-4-1` (weights via
//! `igv/FSRCNN-TensorFlow`, FSRCNN by Dong et al.).
//!
//! Topology (see `Magpie/src/Effects/FSRCNNX/FSRCNNX.hlsl`):
//! Pass1 5x5 1ch->8ch (linear) -> Pass2/3/4 3x3 8ch->8ch (PReLU) ->
//! Pass5 3x3 8ch->8ch (PReLU) + 1x1 8ch->8ch with residual add of Pass1
//! output (PReLU) -> Pass6 3x3 8ch->4 luma sub-pixels (linear), recombined
//! with bilinearly upsampled chroma via the exact `rgb2uv` / `yuv2rgb`
//! matrices from the shader.
//!
//! Sampling matches the shader's samplers: intermediate taps use
//! clamp-to-edge nearest (`POINT`), the chroma fetch uses bilinear
//! (`LINEAR`), both clamped. MulAdd semantics are
//! `acc[j] += sum_i x[i] * M[i][j]` with row-major literals, which is how
//! `tools/extract.py` folded the `MF4x4` constants into the tables.

use crate::{
    weights::{
        P1_BIAS, P1_TAP_DX, P1_TAP_DY, P1_W, P2_BIAS, P2_SLOPE, P2_W, P3_BIAS, P3_SLOPE, P3_W,
        P4_BIAS, P4_SLOPE, P4_W, P5_C_BIAS, P5_C_SLOPE, P5_C_W, P5_T_BIAS, P5_T_SLOPE, P5_T_W,
        P6_BIAS, P6_W,
    },
    PixelFormat, HALO, TILE,
};

const SPATIAL_DX: [i32; 9] = [-1, -1, -1, 0, 0, 0, 1, 1, 1];
const SPATIAL_DY: [i32; 9] = [-1, 0, 1, -1, 0, 1, -1, 0, 1];

const RGB2UV: [[f32; 3]; 2] = [[-0.169, -0.331, 0.5], [0.5, -0.419, -0.081]];
const YUV2RGB: [[f32; 3]; 3] = [
    [1.0, -0.00093, 1.401687],
    [1.0, -0.3437, -0.71417],
    [1.0, 1.77216, 0.00099],
];

#[inline]
fn clamp_i(x: i32, lo: i32, hi: i32) -> i32 {
    if x < lo {
        lo
    } else if x > hi {
        hi
    } else {
        x
    }
}

#[inline]
fn prelu(x: f32, slope: f32) -> f32 {
    if x >= 0.0 {
        x
    } else {
        x * slope
    }
}

fn conv3x3_prelu(
    input: &[f32],
    pw: usize,
    ph: usize,
    bias: &[f32; 8],
    slope: &[f32; 8],
    w: &[[[f32; 8]; 9]; 8],
    output: &mut [f32],
) {
    let w_last = pw as i32 - 1;
    let h_last = ph as i32 - 1;
    for y in 0..ph {
        for x in 0..pw {
            let mut acc = *bias;
            for s in 0..9 {
                let sx = clamp_i(x as i32 + SPATIAL_DX[s], 0, w_last) as usize;
                let sy = clamp_i(y as i32 + SPATIAL_DY[s], 0, h_last) as usize;
                let base = (sy * pw + sx) * 8;
                for o in 0..8 {
                    let row = &w[o][s];
                    acc[o] += input[base] * row[0]
                        + input[base + 1] * row[1]
                        + input[base + 2] * row[2]
                        + input[base + 3] * row[3]
                        + input[base + 4] * row[4]
                        + input[base + 5] * row[5]
                        + input[base + 6] * row[6]
                        + input[base + 7] * row[7];
                }
            }
            let ob = (y * pw + x) * 8;
            for o in 0..8 {
                output[ob + o] = prelu(acc[o], slope[o]);
            }
        }
    }
}

#[inline]
fn bilinear_channel(raw: &[u8], w: usize, h: usize, stride_idx: [usize; 3], fx: f32, fy: f32) -> [f32; 3] {
    let x0 = fx.floor() as i32;
    let y0 = fy.floor() as i32;
    let tx = fx - x0 as f32;
    let ty = fy - y0 as f32;
    let x0c = clamp_i(x0, 0, w as i32 - 1) as usize;
    let x1c = clamp_i(x0 + 1, 0, w as i32 - 1) as usize;
    let y0c = clamp_i(y0, 0, h as i32 - 1) as usize;
    let y1c = clamp_i(y0 + 1, 0, h as i32 - 1) as usize;
    let mut out = [0.0f32; 3];
    for c in 0..3 {
        let i = stride_idx[c];
        let p00 = raw[(y0c * w + x0c) * 4 + i] as f32 / 255.0;
        let p10 = raw[(y0c * w + x1c) * 4 + i] as f32 / 255.0;
        let p01 = raw[(y1c * w + x0c) * 4 + i] as f32 / 255.0;
        let p11 = raw[(y1c * w + x1c) * 4 + i] as f32 / 255.0;
        out[c] = (p00 * (1.0 - tx) + p10 * tx) * (1.0 - ty) + (p01 * (1.0 - tx) + p11 * tx) * ty;
    }
    out
}

pub(crate) fn upscale(raw: &[u8], w: usize, h: usize, fmt: PixelFormat) -> Option<Vec<u8>> {
    let out_w = w * 2;
    let out_h = h * 2;
    let mut out = vec![0u8; out_w * out_h * 4];
    let chan = [fmt.r_idx(), fmt.g_idx(), fmt.b_idx()];

    let mut luma = vec![0.0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let p = &raw[(y * w + x) * 4..(y * w + x) * 4 + 4];
            let r = p[chan[0]] as f32 / 255.0;
            let g = p[chan[1]] as f32 / 255.0;
            let b = p[chan[2]] as f32 / 255.0;
            luma[y * w + x] = 0.299 * r + 0.587 * g + 0.114 * b;
        }
    }

    // Tiles are independent given the shared luma plane, so they upscale
    // concurrently. Output is deterministic regardless of thread count:
    // every tile reads the same inputs and owns its pixels.
    let mut tiles = Vec::new();
    let mut ty = 0;
    while ty < h {
        let th = TILE.min(h - ty);
        let mut tx = 0;
        while tx < w {
            let tw = TILE.min(w - tx);
            tiles.push((tx, ty, tw, th));
            tx += tw;
        }
        ty += th;
    }
    let parallelism = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let n_threads = parallelism.min(tiles.len()).max(1);
    if n_threads <= 1 {
        let mut feat = max_patch_buf();
        let mut buf_a = max_patch_buf();
        let mut buf_b = max_patch_buf();
        let mut tile_buf = vec![0u8; MAX_TILE_BOX];
        process_tiles(
            raw, w, h, &luma, &tiles, &mut out, out_w, chan, &mut feat, &mut buf_a,
            &mut buf_b, &mut tile_buf,
        );
        return Some(out);
    }
    // One tile box per tile; the main thread blits them after the scope, so
    // worker threads never share mutable output.
    let per_thread = tiles.len().div_ceil(n_threads);
    let raw: &[u8] = raw;
    let luma: &[f32] = &luma;
    let mut failed = false;
    std::thread::scope(|s| {
        let mut handles = Vec::with_capacity(n_threads);
        for chunk in tiles.chunks(per_thread) {
            handles.push(s.spawn(move || {
                let mut feat = max_patch_buf();
                let mut buf_a = max_patch_buf();
                let mut buf_b = max_patch_buf();
                let mut tile_buf = vec![0u8; MAX_TILE_BOX];
                let mut done = Vec::with_capacity(chunk.len());
                for &(tx, ty, tw, th) in chunk.iter() {
                    let box_len = (tw * 2) * (th * 2) * 4;
                    process_tile(
                        raw, w, h, luma, tx, ty, tw, th, chan, &mut feat, &mut buf_a,
                        &mut buf_b, &mut tile_buf[..box_len],
                    );
                    done.push(((tx, ty, tw, th), tile_buf[..box_len].to_vec()));
                }
                done
            }));
        }
        for handle in handles {
            match handle.join() {
                Ok(done) => {
                    for ((tx, ty, tw, th), tile) in done {
                        blit_tile(&mut out, out_w, tx, ty, tw, th, &tile);
                    }
                }
                // A failed worker has no pixels to contribute; drop the frame
                // (the next one follows shortly) rather than showing a partial
                // image. The hook treats this like an over-budget frame.
                Err(_) => {
                    failed = true;
                    break;
                }
            }
        }
    });
    if failed {
        return None;
    }
    Some(out)
}

/// Upscale one tile into `tile_out`, a `(2*tw) x (2*th)` RGBA box.
/// `feat`, `buf_a`, `buf_b` are thread-local scratch for the largest patch.
/// Deterministic: same inputs always produce the same box.
#[allow(clippy::too_many_arguments)]
fn process_tile(
    raw: &[u8],
    w: usize,
    h: usize,
    luma: &[f32],
    tx: usize,
    ty: usize,
    tw: usize,
    th: usize,
    chan: [usize; 3],
    feat: &mut [f32],
    buf_a: &mut [f32],
    buf_b: &mut [f32],
    tile_out: &mut [u8],
) {
    let halo = HALO as usize;
    let out_tw = tw * 2;
    let px0 = tx.saturating_sub(halo);
    let py0 = ty.saturating_sub(halo);
    let px1 = (tx + tw + halo).min(w);
    let py1 = (ty + th + halo).min(h);
    let pw = px1 - px0;
    let ph = py1 - py0;

    let w_last = w as i32 - 1;
    let h_last = h as i32 - 1;
    let inv_out_w = 1.0 / (w * 2) as f32;
    let inv_out_h = 1.0 / (h * 2) as f32;

    // Pass1: 5x5 luma -> 8ch, global clamped reads (exact, no seam error).
    for ly in 0..ph {
        let gy = (py0 + ly) as i32;
        for lx in 0..pw {
            let gx = (px0 + lx) as i32;
            let mut acc = P1_BIAS;
            for k in 0..25 {
                let sx = clamp_i(gx + P1_TAP_DX[k], 0, w_last) as usize;
                let sy = clamp_i(gy + P1_TAP_DY[k], 0, h_last) as usize;
                let s = luma[sy * w + sx];
                let col = &P1_W;
                for o in 0..8 {
                    acc[o] += s * col[o][k];
                }
            }
            let ob = (ly * pw + lx) * 8;
            feat[ob..ob + 8].copy_from_slice(&acc);
        }
    }

    // Pass2..4: 3x3 mappings with PReLU, ping-pong between buf_a / buf_b.
    conv3x3_prelu(&feat[..pw * ph * 8], pw, ph, &P2_BIAS, &P2_SLOPE, &P2_W, buf_a);
    conv3x3_prelu(&buf_a[..pw * ph * 8], pw, ph, &P3_BIAS, &P3_SLOPE, &P3_W, buf_b);
    conv3x3_prelu(&buf_b[..pw * ph * 8], pw, ph, &P4_BIAS, &P4_SLOPE, &P4_W, buf_a);

    // Pass5a: 3x3 -> c in buf_b; Pass5b: 1x1 + P1 residual -> t in feat.
    conv3x3_prelu(&buf_a[..pw * ph * 8], pw, ph, &P5_C_BIAS, &P5_C_SLOPE, &P5_C_W, buf_b);
    {
        let (c_buf, t_buf) = (&buf_b[..pw * ph * 8], &mut feat[..pw * ph * 8]);
        let mut c_row = [0.0f32; 8];
        let mut f_row = [0.0f32; 8];
        for i in 0..pw * ph {
            c_row.copy_from_slice(&c_buf[i * 8..i * 8 + 8]);
            f_row.copy_from_slice(&t_buf[i * 8..i * 8 + 8]);
            let mut t1 = P5_T_BIAS;
            for o in 0..8 {
                let row = &P5_T_W[o];
                t1[o] += c_row[0] * row[0]
                    + c_row[1] * row[1]
                    + c_row[2] * row[2]
                    + c_row[3] * row[3]
                    + c_row[4] * row[4]
                    + c_row[5] * row[5]
                    + c_row[6] * row[6]
                    + c_row[7] * row[7];
            }
            for o in 0..8 {
                t1[o] += f_row[o];
                t1[o] = prelu(t1[o], P5_T_SLOPE[o]);
            }
            t_buf[i * 8..i * 8 + 8].copy_from_slice(&t1);
        }
    }

    // Pass6 + chroma recombine, only for the tile's own pixels.
    let pw_i = pw as i32;
    let ph_i = ph as i32;
    for y in 0..th {
        let gy = ty + y;
        let ly = (gy - py0) as i32;
        for x in 0..tw {
            let gx = tx + x;
            let lx = (gx - px0) as i32;
            let mut result = P6_BIAS;
            for s in 0..9 {
                let sx = clamp_i(lx + SPATIAL_DX[s], 0, pw_i - 1) as usize;
                let sy = clamp_i(ly + SPATIAL_DY[s], 0, ph_i - 1) as usize;
                let base = (sy * pw + sx) * 8;
                for o in 0..4 {
                    let row = &P6_W[o][s];
                    result[o] += feat[base] * row[0]
                        + feat[base + 1] * row[1]
                        + feat[base + 2] * row[2]
                        + feat[base + 3] * row[3]
                        + feat[base + 4] * row[4]
                        + feat[base + 5] * row[5]
                        + feat[base + 6] * row[6]
                        + feat[base + 7] * row[7];
                }
            }
            for iy in 0..2 {
                for ix in 0..2 {
                    let dx2 = gx * 2 + ix;
                    let dy2 = gy * 2 + iy;
                    let fx = (dx2 as f32 + 0.5) * inv_out_w * w as f32 - 0.5;
                    let fy = (dy2 as f32 + 0.5) * inv_out_h * h as f32 - 0.5;
                    let rgb = bilinear_channel(raw, w, h, chan, fx, fy);
                    let u = RGB2UV[0][0] * rgb[0]
                        + RGB2UV[0][1] * rgb[1]
                        + RGB2UV[0][2] * rgb[2];
                    let v = RGB2UV[1][0] * rgb[0]
                        + RGB2UV[1][1] * rgb[1]
                        + RGB2UV[1][2] * rgb[2];
                    let y_luma = result[ix * 2 + iy];
                    let r = YUV2RGB[0][0] * y_luma + YUV2RGB[0][1] * u + YUV2RGB[0][2] * v;
                    let g = YUV2RGB[1][0] * y_luma + YUV2RGB[1][1] * u + YUV2RGB[1][2] * v;
                    let b = YUV2RGB[2][0] * y_luma + YUV2RGB[2][1] * u + YUV2RGB[2][2] * v;
                    let to_u8 = |v: f32| {
                        if v <= 0.0 {
                            0
                        } else if v >= 1.0 {
                            255
                        } else {
                            (v * 255.0 + 0.5) as u8
                        }
                    };
                    let ob = ((y * 2 + iy) * out_tw + (x * 2 + ix)) * 4;
                    tile_out[ob + chan[0]] = to_u8(r);
                    tile_out[ob + chan[1]] = to_u8(g);
                    tile_out[ob + chan[2]] = to_u8(b);
                    tile_out[ob + 3] = 255;
                }
            }
        }
    }
}

/// Largest tile box in bytes: `(2*TILE) x (2*TILE)` RGBA.
const MAX_TILE_BOX: usize = TILE * 2 * (TILE * 2) * 4;

/// Copy one tile box into the full-size output.
fn blit_tile(
    out: &mut [u8],
    out_w: usize,
    tx: usize,
    ty: usize,
    tw: usize,
    th: usize,
    tile: &[u8],
) {
    let out_tw = tw * 2;
    for r in 0..th * 2 {
        let dst = ((ty * 2 + r) * out_w + tx * 2) * 4;
        let src = r * out_tw * 4;
        out[dst..dst + out_tw * 4].copy_from_slice(&tile[src..src + out_tw * 4]);
    }
}

/// Largest patch buffer in floats: `(TILE + 2*HALO)^2 * 8ch`.
fn max_patch_buf() -> Vec<f32> {
    vec![0.0f32; (TILE + 2 * HALO as usize).pow(2) * 8]
}

/// Upscale `tiles` into `out`. Scratch is reused across the caller's tiles.
#[allow(clippy::too_many_arguments)]
fn process_tiles(
    raw: &[u8],
    w: usize,
    h: usize,
    luma: &[f32],
    tiles: &[(usize, usize, usize, usize)],
    out: &mut [u8],
    out_w: usize,
    chan: [usize; 3],
    feat: &mut [f32],
    buf_a: &mut [f32],
    buf_b: &mut [f32],
    tile_buf: &mut [u8],
) {
    for &(tx, ty, tw, th) in tiles {
        let box_len = (tw * 2) * (th * 2) * 4;
        process_tile(
            raw,
            w,
            h,
            luma,
            tx,
            ty,
            tw,
            th,
            chan,
            feat,
            buf_a,
            buf_b,
            &mut tile_buf[..box_len],
        );
        blit_tile(out, out_w, tx, ty, tw, th, &tile_buf[..box_len]);
    }
}
