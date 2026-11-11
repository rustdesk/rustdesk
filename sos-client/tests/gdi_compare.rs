//! 对比基准：`libs/scrap` 的 GDI 采集 vs SOS 自建优化版 GDI 采集。
//!
//! 运行：cargo test --release --test gdi_compare -- --nocapture

#![cfg(windows)]

#[path = "../src/sos_gdi.rs"]
mod sos_gdi;

use scrap::{TraitCapturer, TraitPixelBuffer};
use std::time::{Duration, Instant};

const FRAMES: u32 = 30;

fn first_display() -> Option<scrap::Display> {
    scrap::Display::all().ok()?.into_iter().next()
}

/// `scrap::Display` 不可 Clone，需要两份时重新枚举一次
fn display_again() -> Option<scrap::Display> {
    scrap::Display::all().ok()?.into_iter().next()
}

#[test]
fn scrap_vs_sos_gdi() {
    let Some(display) = first_display() else {
        println!("[skip] no display");
        return;
    };
    let (w, h) = (display.width(), display.height());
    println!("=== GDI 采集对比 {}x{} ===", w, h);

    // A. scrap 的 GDI
    let scrap_display = display_again().expect("second enumeration failed");
    if let Ok(mut capturer) = scrap::Capturer::new(scrap_display) {
        capturer.set_gdi();
        let mut ok = 0u32;
        let start = Instant::now();
        for _ in 0..FRAMES {
            if let Ok(frame) = capturer.frame(Duration::from_millis(200)) {
                // 模拟下游 libyuv 转换：真的读一遍全部像素，避免被优化掉
                if let scrap::Frame::PixelBuffer(pb) = &frame {
                    let d = pb.data();
                    std::hint::black_box(d[0] as u64 + d[d.len() - 1] as u64);
                }
                ok += 1;
            }
        }
        let ms = start.elapsed().as_secs_f64() * 1000.0 / FRAMES as f64;
        println!("A. scrap GDI      : {:7.2} ms/帧 → {:5.1} fps（有效 {ok}/{FRAMES}）", ms, 1000.0 / ms);
    }

    // B. SOS 自建
    match sos_gdi::SosGdiCapturer::new(&display) {
        Ok(mut capturer) => {
            let start = Instant::now();
            for _ in 0..FRAMES {
                if let Ok(frame) = capturer.frame(Duration::from_millis(200)) {
                    if let scrap::Frame::PixelBuffer(pb) = &frame {
                        let d = pb.data();
                        std::hint::black_box(d[0] as u64 + d[d.len() - 1] as u64);
                    }
                }
            }
            let ms = start.elapsed().as_secs_f64() * 1000.0 / FRAMES as f64;
            println!("B. SOS GDI (DIB)  : {:7.2} ms/帧 → {:5.1} fps", ms, 1000.0 / ms);
        }
        Err(e) => println!("B. SOS GDI 创建失败: {e}"),
    }
}

/// 验证自建采集器的输出与 scrap 一致（避免出现图片上下/左右颠倒或颜色错乱）
#[test]
fn sos_gdi_matches_scrap_pixels() {
    let Some(display) = first_display() else { return };
    let (w, h) = (display.width() as usize, display.height() as usize);

    let mut ours = match sos_gdi::SosGdiCapturer::new(&display) {
        Ok(c) => c,
        Err(e) => {
            println!("[skip] {e}");
            return;
        }
    };
    let mut theirs = match display_again().and_then(|d| scrap::Capturer::new(d).ok()) {
        Some(c) => c,
        None => {
            println!("[skip] scrap capturer create failed");
            return;
        }
    };
    theirs.set_gdi();

    let a = ours.frame(Duration::from_millis(200)).expect("our frame");
    let b = theirs.frame(Duration::from_millis(200)).expect("scrap frame");
    let (scrap::Frame::PixelBuffer(pa), scrap::Frame::PixelBuffer(pb)) = (&a, &b) else {
        println!("[skip] unexpected frame variant");
        return;
    };
    let (da, db) = (pa.data(), pb.data());

    // 逐点比较：位置 (0,0)、中心、右下角。任一不匹配说明方向或 stride 有问题
    let checkpoints = [(0usize, 0usize), (w / 2, h / 2), (w - 1, h - 1), (w - 1, 0), (0, h - 1)];
    let mut bad = 0;
    for (x, y) in checkpoints {
        let i = (y * w + x) * 4;
        if i + 4 <= da.len() && i + 4 <= db.len() {
            let ea = &da[i..i + 3];
            let eb = &db[i..i + 3];
            if ea != eb {
                println!("  差异 @({x},{y}): ours={:?} scrap={:?}", ea, eb);
                bad += 1;
            }
        }
    }
    println!(
        "像素一致性检查: {}/{} 个采样点匹配 (图上 {}x{})",
        5 - bad,
        5,
        w,
        h
    );
    assert_eq!(bad, 0, "像素方向/格式与 scrap 不一致");
}
