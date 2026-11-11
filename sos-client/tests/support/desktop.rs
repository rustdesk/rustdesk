//! 编码器基准用的**可比较**内容生成器。
//!
//! 为什么需要它：拿纯噪声或纯静态图衡量编码器都没有意义 ——
//! 噪声不可压缩（所有编码器都在量化器上限处空转），静态图则几乎没有
//! P 帧工作量。真实桌面介于两者之间：大面积平坦背景 + 局部细密文字 +
//! 少量运动。
//!
//! 同一份内容喂给不同编码器/不同配置，比较才有意义。

/// 桌面风格画面的 Y 平面。
///
/// `seed` 让每帧有轻微变化（文字行整体平移 1px + 一个移动的高对比方块），
/// 否则 P 帧几乎是空的，会把所有编码器都测成「极快」。
pub fn desktop_y(w: usize, h: usize, seed: usize) -> Vec<u8> {
    let mut y = vec![0u8; w * h];
    // 浅色背景 + 极轻微渐变（模拟窗口阴影）
    for row in 0..h {
        let base = 246u32.saturating_sub((row * 12 / h.max(1)) as u32);
        y[row * w..(row + 1) * w].fill(base as u8);
    }
    // 标题栏
    let title_h = (h / 18).max(8);
    for row in 0..title_h.min(h) {
        y[row * w..(row + 1) * w].fill(62);
    }
    // 小字：5x7 点阵、行距 14px、字距 7px，整体每帧水平平移
    let shift = seed % 7;
    let line_h = 14usize;
    let mut row = title_h + 8;
    let mut line = 0usize;
    while row + 7 < h {
        let text_len = 40 + (line * 17) % 60;
        for ch_i in 0..text_len {
            let x0 = 12 + ch_i * 7 + shift;
            if x0 + 5 >= w {
                break;
            }
            // 笔画式字形：左竖、右竖、上横、中横、底部短横
            for gy in 0..7usize {
                for gx in 0..5usize {
                    let on = gx == 0 || gx == 4 || gy == 0 || gy == 3 || (gy == 6 && gx < 3);
                    if on {
                        y[(row + gy) * w + x0 + gx] = 40;
                    }
                }
            }
        }
        row += line_h;
        line += 1;
    }
    // 一个移动的高对比方块，制造真实的帧间运动
    let bx = (seed * 11) % w.saturating_sub(200).max(1);
    let by = h / 2;
    for gy in 0..100usize {
        let ry = by + gy;
        if ry >= h {
            break;
        }
        let x_end = (bx + 180).min(w);
        y[ry * w + bx..ry * w + x_end].fill(20);
    }
    y
}

/// 桌面风格画面的 I420 帧（色度恒为中性 128 —— 桌面内容几乎没有色度细节）
pub fn desktop_i420(w: usize, h: usize, seed: usize) -> Vec<u8> {
    let (cw, ch) = (w / 2, h / 2);
    let mut buf = vec![0u8; w * h + 2 * cw * ch];
    buf[..w * h].copy_from_slice(&desktop_y(w, h, seed));
    buf[w * h..].fill(128);
    buf
}
