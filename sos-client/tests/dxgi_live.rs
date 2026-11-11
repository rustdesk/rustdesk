//! 验证 DXGI Desktop Duplication 在静止桌面下「不报错但无帧」的行为，
//! 以及在屏幕真实变化时能否正常出帧。
//!
//! 背景：`scrap` 的 DXGI 路径在桌面无变化时返回 `WouldBlock`，
//! 这与「DXGI 不可用」在日志上极易混淆，可能导致误降级到 GDI。
//!
//! 运行：cargo test --release --test dxgi_live -- --nocapture --test-threads=1

#![cfg(windows)]

use scrap::{TraitCapturer, TraitPixelBuffer};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, MoveWindow, PeekMessageW,
    RegisterClassW, ShowWindow, TranslateMessage, CS_HREDRAW, CS_VREDRAW, MSG, PM_REMOVE,
    SW_SHOW, WINDOW_EX_STYLE, WNDCLASSW, WS_OVERLAPPEDWINDOW,
};

unsafe extern "system" fn wndproc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    DefWindowProcW(h, m, w, l)
}

/// 起一个不断移动的小窗口，制造真实的桌面变化
fn spawn_animator(stop: Arc<AtomicBool>) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || unsafe {
        let instance = match GetModuleHandleW(None) {
            Ok(i) => i,
            Err(_) => return,
        };
        let class = w!("SosDxgiProbeWindow");
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            lpszClassName: class,
            ..Default::default()
        };
        RegisterClassW(&wc);
        let hwnd = match CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class,
            w!("probe"),
            WS_OVERLAPPEDWINDOW,
            40,
            40,
            320,
            240,
            None,
            None,
            Some(instance.into()),
            None,
        ) {
            Ok(h) => h,
            Err(_) => return,
        };
        let _ = ShowWindow(hwnd, SW_SHOW);

        let mut i: i32 = 0;
        while !stop.load(Ordering::Relaxed) {
            let _ = MoveWindow(hwnd, 40 + (i % 40) * 9, 40 + (i % 25) * 9, 320, 240, true);
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            i = i.wrapping_add(1);
            std::thread::sleep(Duration::from_millis(16));
        }
        let _ = DestroyWindow(hwnd);
    })
}

fn first_display() -> Option<scrap::Display> {
    scrap::Display::all().ok()?.into_iter().next()
}

#[test]
fn dxgi_static_vs_live() {
    let Some(d) = first_display() else {
        println!("[skip] no display");
        return;
    };
    let mut capturer = match scrap::Capturer::new(d) {
        Ok(c) => c,
        Err(e) => {
            println!("DXGI 创建失败: {e}  → 本机确实不支持 Desktop Duplication");
            return;
        }
    };
    if capturer.is_gdi() {
        println!("DXGI 创建后已内部回退 GDI → 本机不支持 Desktop Duplication");
        return;
    }
    println!("DXGI Desktop Duplication 创建成功（未回退 GDI）");

    // 阶段 1：静止桌面
    let mut ok = 0u32;
    let mut block = 0u32;
    let mut other = 0u32;
    let t = Instant::now();
    for _ in 0..5 {
        match capturer.frame(Duration::from_millis(300)) {
            Ok(f) => {
                std::hint::black_box(match &f {
                    scrap::Frame::PixelBuffer(pb) => pb.data().len(),
                    _ => 0,
                });
                ok += 1;
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => block += 1,
            Err(_) => other += 1,
        }
    }
    println!(
        "静止桌面: ok={ok} wouldblock={block} other={other}  用时 {:.0} ms",
        t.elapsed().as_secs_f64() * 1000.0
    );

    // 阶段 2：屏幕持续变化
    let stop = Arc::new(AtomicBool::new(false));
    let animator = spawn_animator(stop.clone());
    std::thread::sleep(Duration::from_millis(300));

    let mut ok = 0u32;
    let mut block = 0u32;
    let mut other = 0u32;
    let mut total = Duration::ZERO;
    let t = Instant::now();
    for _ in 0..60 {
        let f0 = Instant::now();
        match capturer.frame(Duration::from_millis(200)) {
            Ok(f) => {
                total += f0.elapsed();
                std::hint::black_box(match &f {
                    scrap::Frame::PixelBuffer(pb) => pb.data().len(),
                    _ => 0,
                });
                ok += 1;
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => block += 1,
            Err(_) => other += 1,
        }
    }
    let wall = t.elapsed().as_secs_f64();
    stop.store(true, Ordering::Relaxed);
    let _ = animator.join();

    println!(
        "屏幕变化: ok={ok} wouldblock={block} other={other}  墙钟 {:.2}s  → {:.1} fps",
        wall,
        ok as f64 / wall
    );
    if ok > 0 {
        println!(
            "出帧耗时: {:.2} ms/帧  →  {:.1} fps",
            total.as_secs_f64() * 1000.0 / ok as f64,
            1000.0 * ok as f64 / (total.as_secs_f64() * 1000.0)
        );
    }
}
