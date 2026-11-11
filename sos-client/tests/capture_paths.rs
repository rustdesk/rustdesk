//! 屏幕采集路径横评：GDI / Magnification API / DXGI。
//!
//! 目的：确认在本机（DXGI 不可用的虚拟机）还有哪些可用且更优的采集路径，
//! 以及各路径的通用性边界。
//!
//! 运行：cargo test --release --test capture_paths -- --nocapture --test-threads=1

#![cfg(windows)]

#[path = "../src/sos_gdi.rs"]
mod sos_gdi;

use scrap::{TraitCapturer, TraitPixelBuffer};
use std::time::{Duration, Instant};

const FRAMES: u32 = 30;

fn first_display() -> Option<scrap::Display> {
    scrap::Display::all().ok()?.into_iter().next()
}

/// 读一遍全部像素，避免优化器把采集当成死代码
fn touch(frame: &scrap::Frame) {
    if let scrap::Frame::PixelBuffer(pb) = frame {
        let d = pb.data();
        if !d.is_empty() {
            std::hint::black_box(d[0] as u64 + d[d.len() / 2] as u64 + d[d.len() - 1] as u64);
        }
    }
}

fn bench(label: &str, capturer: &mut dyn TraitCapturer) {
    let mut ok = 0u32;
    let mut total = Duration::ZERO;
    for _ in 0..FRAMES {
        let t = Instant::now();
        match capturer.frame(Duration::from_millis(500)) {
            Ok(frame) => {
                total += t.elapsed();
                touch(&frame);
                ok += 1;
            }
            Err(e) => {
                println!("  {label}: frame error {e}");
                break;
            }
        }
    }
    if ok == 0 {
        println!("{label:<24}: 不可用");
        return;
    }
    let ms = total.as_secs_f64() * 1000.0 / ok as f64;
    println!("{label:<24}: {ms:>7.2} ms/帧  →  {:>5.1} fps  ({ok}/{FRAMES} 有效)", 1000.0 / ms);
}

#[test]
fn capture_paths() {
    let Some(display) = first_display() else {
        println!("[skip] no display");
        return;
    };
    let (w, h) = (display.width(), display.height());
    let origin = display.origin();
    println!("=== 采集路径横评 {w}x{h} origin={origin:?} ===");

    // 1. GDI：自建优化版（BitBlt 直写自顶向下 DIB，无额外拷贝）
    if let Ok(mut c) = sos_gdi::create(&display) {
        bench("GDI (sos, DIB直写)", c.as_mut());
    }

    // 2. Magnification API：DWM 合成面 → 回调，RDP / 安全桌面也能用
    if scrap::CapturerMag::is_supported() {
        match scrap::CapturerMag::new(origin, w, h) {
            Ok(mut c) => bench("Magnification API", &mut c),
            Err(e) => println!("{:<24}: 创建失败 {}", "Magnification API", e),
        }
    } else {
        println!("{:<24}: 不可用（Magnification.dll）", "Magnification API");
    }

    // 3. DXGI Desktop Duplication：最快，但需要可用 GPU / 输出适配器
    let Some(d2) = first_display() else {
        return;
    };
    match scrap::Capturer::new(d2) {
        Ok(mut c) => {
            if c.is_gdi() {
                println!("{:<24}: DXGI 不可用，已内部回退 GDI", "DXGI");
            } else {
                bench("DXGI Desktop Dup", &mut c);
            }
        }
        Err(e) => println!("{:<24}: 创建失败 {}", "DXGI", e),
    }
}
