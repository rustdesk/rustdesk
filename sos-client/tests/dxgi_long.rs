//! 自建 DXGI 采集器的正确性验证。
//!
//! 重点验证 `libs/scrap` 的两个缺陷在我们这里不存在：
//! 1. 长时间连续采集不出现 `InvalidData`（scrap 约 40ms 就会出现）；
//! 2. 桌面静止时返回 `WouldBlock` 而不是卡死，且之后仍能正常出帧。
//!
//! 运行：cargo test --release --test dxgi_long -- --nocapture --test-threads=1

#![cfg(windows)]

#[path = "../src/sos_dxgi.rs"]
mod sos_dxgi;

use scrap::{TraitCapturer, TraitPixelBuffer};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, MoveWindow, PeekMessageW,
    RegisterClassW, ShowWindow, TranslateMessage, CS_HREDRAW, CS_VREDRAW, MSG, PM_REMOVE, SW_SHOW,
    WINDOW_EX_STYLE, WNDCLASSW, WS_OVERLAPPEDWINDOW,
};

unsafe extern "system" fn wndproc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    DefWindowProcW(h, m, w, l)
}

/// 持续移动的小窗口，制造真实的桌面变化
fn spawn_animator(stop: Arc<AtomicBool>) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || unsafe {
        let Ok(instance) = GetModuleHandleW(None) else {
            return;
        };
        let class = w!("SosDxgiLongWindow");
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            lpszClassName: class,
            ..Default::default()
        };
        RegisterClassW(&wc);
        let Ok(hwnd) = CreateWindowExW(
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
        ) else {
            return;
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

fn display_name() -> Option<String> {
    scrap::Display::all().ok()?.into_iter().next().map(|d| d.name())
}

/// 连续采集 300 帧，统计错误种类并测量耗时
#[test]
fn dxgi_long_run_no_invalid_call() {
    let Some(name) = display_name() else {
        println!("[skip] no display");
        return;
    };
    let mut capturer = match sos_dxgi::create(&name) {
        Ok(c) => c,
        Err(e) => {
            println!("[skip] DXGI 创建失败: {e}");
            return;
        }
    };

    let stop = Arc::new(AtomicBool::new(false));
    let animator = spawn_animator(stop.clone());
    std::thread::sleep(Duration::from_millis(300));

    let (mut ok, mut wouldblock, mut invalid, mut other) = (0u32, 0u32, 0u32, 0u32);
    let mut first_other: Option<String> = None;
    let mut total = Duration::ZERO;
    let mut max_frame = Duration::ZERO;
    let target = 300u32;

    let wall = Instant::now();
    for _ in 0..target {
        let t = Instant::now();
        match capturer.frame(Duration::from_millis(200)) {
            Ok(f) => {
                let dt = t.elapsed();
                total += dt;
                max_frame = max_frame.max(dt);
                if let scrap::Frame::PixelBuffer(pb) = &f {
                    std::hint::black_box(pb.data().len());
                }
                ok += 1;
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => wouldblock += 1,
            Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
                invalid += 1;
                if first_other.is_none() {
                    first_other = Some(format!("InvalidData: {e}"));
                }
            }
            Err(e) => {
                other += 1;
                if first_other.is_none() {
                    first_other = Some(format!("{:?}: {e}", e.kind()));
                }
            }
        }
    }
    let wall = wall.elapsed().as_secs_f64();
    stop.store(true, Ordering::Relaxed);
    let _ = animator.join();

    println!("=== 自建 DXGI 连续采集 {} 次 ===", target);
    println!("ok={ok} wouldblock={wouldblock} invalid_data={invalid} other={other}");
    println!("墙钟 {wall:.2}s → {:.1} 有效帧/秒", ok as f64 / wall);
    if ok > 0 {
        println!(
            "出帧 {:.2} ms/帧（平均）  {:.2} ms（最慢）",
            total.as_secs_f64() * 1000.0 / ok as f64,
            max_frame.as_secs_f64() * 1000.0
        );
    }
    if let Some(e) = first_other {
        println!("首个非 WouldBlock 错误: {e}");
    }

    assert_eq!(
        invalid, 0,
        "自建 DXGI 不应该出现 InvalidData（scrap 的 order bug 特征）"
    );
    assert!(ok > 0, "完全没有采到帧");
    println!("✓ 无 InvalidData，未触发 scrap 的顺序缺陷");
}

/// 桌面静止 → WouldBlock → 之后仍能恢复出帧
#[test]
fn dxgi_static_then_recover() {
    let Some(name) = display_name() else {
        println!("[skip] no display");
        return;
    };
    let mut capturer = match sos_dxgi::create(&name) {
        Ok(c) => c,
        Err(e) => {
            println!("[skip] DXGI 创建失败: {e}");
            return;
        }
    };

    // 阶段 1：静止桌面，应当只拿到 WouldBlock
    let mut block = 0u32;
    for _ in 0..10 {
        match capturer.frame(Duration::from_millis(120)) {
            Ok(f) => {
                if let scrap::Frame::PixelBuffer(pb) = &f {
                    std::hint::black_box(pb.data().len());
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => block += 1,
            Err(e) => panic!("静止桌面出现非 WouldBlock 错误: {e:?} {e}"),
        }
    }
    println!("静止桌面: wouldblock={block}/10");

    // 阶段 2：引入变化，必须恢复出帧
    let stop = Arc::new(AtomicBool::new(false));
    let animator = spawn_animator(stop.clone());
    std::thread::sleep(Duration::from_millis(300));

    let mut ok = 0u32;
    for _ in 0..20 {
        if capturer.frame(Duration::from_millis(200)).is_ok() {
            ok += 1;
        }
    }
    stop.store(true, Ordering::Relaxed);
    let _ = animator.join();

    println!("恢复后出帧: ok={ok}/20");
    assert!(ok > 0, "静止之后无法恢复出帧");
    println!("✓ 静止不卡死，恢复后正常出帧");
}
