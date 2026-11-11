//! GDI 采集耗时拆解基准。
//!
//! 目的：查清 `libs/scrap` 的 GDI 路径为什么慢，把耗时拆成
//! BitBlt / CAPTUREBLT / GetDIBits / 全帧翻转 四部分。
//!
//! 运行：cargo test --release --test gdi_bench -- --nocapture

#![cfg(windows)]

use scrap::TraitCapturer;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject,
    GetDC, GetDIBits, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CAPTUREBLT,
    DIB_RGB_COLORS, HGDIOBJ, RGBQUAD, SRCCOPY,
};

const FRAMES: u32 = 30;

fn bit_depth_32(w: i32, h: i32, top_down: bool) -> BITMAPINFO {
    let mut bmi = BITMAPINFO::default();
    bmi.bmiHeader = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: w,
        biHeight: if top_down { -h } else { h },
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        biSizeImage: (w * h * 4) as u32,
        ..Default::default()
    };
    bmi.bmiColors = [RGBQUAD::default()];
    bmi
}

/// A: DIB section + BitBlt(SRCCOPY)，直接从指针读 —— 理论最快
unsafe fn measure_dib_bitblt(w: i32, h: i32, captureblt: bool) -> f64 {
    let screen = GetDC(None);
    let dc = CreateCompatibleDC(Some(screen));
    let bmi = bit_depth_32(w, h, true);
    let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
    let bmp = CreateDIBSection(Some(dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0)
        .expect("CreateDIBSection failed");
    SelectObject(dc, HGDIOBJ(bmp.0));
    let rop = if captureblt {
        SRCCOPY | CAPTUREBLT
    } else {
        SRCCOPY
    };

    let mut checksum = 0u64;
    let start = Instant::now();
    for _ in 0..FRAMES {
        // 必须检查返回值，否则优化器可能把整个调用当成无副作用
        if BitBlt(dc, 0, 0, w, h, Some(screen), 0, 0, rop).is_err() {
            println!("BitBlt failed");
            break;
        }
        let slice = std::slice::from_raw_parts(bits as *const u8, (w * h * 4) as usize);
        checksum = checksum.wrapping_add(slice[0] as u64 + slice[slice.len() - 1] as u64);
    }
    let ms = start.elapsed().as_secs_f64() * 1000.0 / FRAMES as f64;

    let _ = DeleteObject(HGDIOBJ(bmp.0));
    let _ = DeleteDC(dc);
    ReleaseDC(None, screen);
    std::hint::black_box(checksum);
    ms
}

/// C: 兼容位图 + BitBlt + GetDIBits（复刻 scrap 的取帧方式）
unsafe fn measure_bitblt_getdibits(w: i32, h: i32) -> f64 {
    let screen = GetDC(None);
    let dc = CreateCompatibleDC(Some(screen));
    let bmp = CreateCompatibleBitmap(screen, w, h);
    SelectObject(dc, HGDIOBJ(bmp.0));
    let mut buf = vec![0u8; (w * h * 4) as usize];
    let mut bmi = bit_depth_32(w, h, false);

    let start = Instant::now();
    for _ in 0..FRAMES {
        let _ = BitBlt(dc, 0, 0, w, h, Some(screen), 0, 0, SRCCOPY | CAPTUREBLT);
        let n = GetDIBits(
            dc,
            bmp,
            0,
            h as u32,
            Some(buf.as_mut_ptr() as *mut _),
            &mut bmi,
            DIB_RGB_COLORS,
        );
        std::hint::black_box(n);
    }
    let ms = start.elapsed().as_secs_f64() * 1000.0 / FRAMES as f64;

    let _ = DeleteObject(HGDIOBJ(bmp.0));
    let _ = DeleteDC(dc);
    ReleaseDC(None, screen);
    ms
}

/// 手写全帧翻转（模拟 scrap 的 ARGBMirror + ARGBRotate 180 的内存开销）
fn measure_flip_passes(w: usize, h: usize) -> f64 {
    let stride = w * 4;
    let mut src = vec![0u8; stride * h];
    let mut dst1 = vec![0u8; stride * h];
    let mut dst2 = vec![0u8; stride * h];
    for (i, b) in src.iter_mut().enumerate() {
        *b = (i % 251) as u8;
    }

    let start = Instant::now();
    for _ in 0..FRAMES {
        // 垂直镜像
        for y in 0..h {
            let s = y * stride;
            let d = (h - 1 - y) * stride;
            dst1[d..d + stride].copy_from_slice(&src[s..s + stride]);
        }
        // 180° 旋转
        for y in 0..h {
            let s = (h - 1 - y) * stride;
            let d = y * stride;
            for x in 0..w {
                let si = s + x * 4;
                let di = d + (w - 1 - x) * 4;
                dst2[di..di + 4].copy_from_slice(&src[si..si + 4]);
            }
        }
    }
    let ms = start.elapsed().as_secs_f64() * 1000.0 / FRAMES as f64;
    std::hint::black_box(&dst1);
    std::hint::black_box(&dst2);
    ms
}

/// D: scrap 的完整 GDI 路径
fn measure_scrap_gdi(w: usize, h: usize) -> Option<f64> {
    let mut displays = scrap::Display::all().ok()?;
    let display = displays.drain(..).next()?;
    let mut capturer = scrap::Capturer::new(display).ok()?;
    capturer.set_gdi();

    let mut ok = 0u32;
    let start = Instant::now();
    for _ in 0..FRAMES {
        match capturer.frame(Duration::from_millis(200)) {
            Ok(frame) => {
                std::hint::black_box(frame);
                ok += 1;
            }
            // scrap 在画面无变化时返回 WouldBlock，不计入
            Err(_) => {}
        }
    }
    let ms = start.elapsed().as_secs_f64() * 1000.0 / FRAMES as f64;
    println!("  (scrap 有效帧 {ok}/{FRAMES})");
    Some(ms)
}

#[test]
fn gdi_capture_breakdown() {
    let (w, h) = (1920i32, 1080i32);
    println!("=== GDI 采集耗时拆解 {}x{}，每项 {} 帧 ===", w, h, FRAMES);

    let dib = unsafe { measure_dib_bitblt(w, h, false) };
    println!("A. DIB section + BitBlt(SRCCOPY)          : {:8.2} ms/帧", dib);

    let dib_cblt = unsafe { measure_dib_bitblt(w, h, true) };
    println!(
        "B. DIB section + BitBlt(SRCCOPY|CAPTUREBLT): {:8.2} ms/帧  (CAPTUREBLT 增量 {:+.2} ms)",
        dib_cblt,
        dib_cblt - dib
    );

    let getdibits = unsafe { measure_bitblt_getdibits(w, h) };
    println!(
        "C. 兼容位图 + BitBlt + GetDIBits          : {:8.2} ms/帧  (GetDIBits 增量 {:+.2} ms)",
        getdibits,
        getdibits - dib_cblt
    );

    let flip = measure_flip_passes(w as usize, h as usize);
    println!("D. 手写镜像+旋转两遍全帧                  : {:8.2} ms/帧", flip);

    if let Some(scrap_ms) = measure_scrap_gdi(w as usize, h as usize) {
        println!("E. scrap 完整 GDI frame()                 : {:8.2} ms/帧", scrap_ms);
        println!(
            "   → 理论下限 {:.2} ms，其中 GetDIBits 约 {:.2} ms、全帧翻转约 {:.2} ms",
            dib,
            getdibits - dib_cblt,
            flip
        );
    }
}

/// 分辨率缩放：判断 BitBlt 是否受内存带宽限制
#[test]
fn gdi_bitblt_scaling() {
    println!("=== BitBlt 随分辨率的缩放 ===");
    for (w, h) in [
        (640i32, 480i32),
        (1280, 720),
        (1920, 1080),
        (2560, 1440),
        (2880, 1800),
    ] {
        let ms = unsafe { measure_dib_bitblt(w, h, false) };
        let mb = (w as f64 * h as f64 * 4.0) / 1024.0 / 1024.0;
        println!(
            "{:>5}x{:<5} ({:5.1} MB) : {:7.2} ms/帧 → {:5.1} fps 上限, 等效带宽 {:6.0} MB/s",
            w,
            h,
            mb,
            ms,
            1000.0 / ms,
            mb * 1000.0 / ms
        );
    }
}

/// 重复 BitBlt 同一区域：区分“固定开销”与“逐像素拷贝”
#[test]
fn gdi_bitblt_repeat_overhead() {
    println!("=== BitBlt 1920x1080 连续 100 次（检查是否有额外固定开销） ===");
    let (w, h) = (1920i32, 1080i32);
    unsafe {
        let screen = GetDC(None);
        let dc = CreateCompatibleDC(Some(screen));
        let bmi = bit_depth_32(w, h, true);
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let bmp = CreateDIBSection(Some(dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0).unwrap();
        SelectObject(dc, HGDIOBJ(bmp.0));
        let mut samples = Vec::new();
        for i in 0..100u32 {
            let t = Instant::now();
            let _ = BitBlt(dc, 0, 0, w, h, Some(screen), 0, 0, SRCCOPY);
            samples.push(t.elapsed().as_secs_f64() * 1000.0);
            if i == 0 {
                println!("  首次（可能含 DWM 初始化）: {:.2} ms", samples[0]);
            }
        }
        let rest = &samples[1..];
        let min = rest.iter().cloned().fold(f64::MAX, f64::min);
        let max = rest.iter().cloned().fold(0.0f64, f64::max);
        let avg = rest.iter().sum::<f64>() / rest.len() as f64;
        println!("  后续 99 次: min={:.2} avg={:.2} max={:.2} ms", min, avg, max);
        let _ = DeleteObject(HGDIOBJ(bmp.0));
        let _ = DeleteDC(dc);
        ReleaseDC(None, screen);
    }
}
