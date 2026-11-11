//! 测量自建编码器的真实内存占用。
//!
//! 目的：`sos_vpx::recommended_threads()` 里「每个编码线程 512MB」只是估算，
//! 在 4GB 虚拟机上是决定性能与稳定性的关键参数，必须用实测值替换。
//!
//! 运行：cargo test --release --test vpx_mem -- --nocapture --test-threads=1

#![cfg(all(windows, sos_vpx))]

#[path = "../src/sos_vpx.rs"]
mod sos_vpx;

use sos_vpx::{SosCodec, SosVpxConfig, SosVpxEncoder};
use std::time::Instant;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows::Win32::System::Threading::GetCurrentProcess;

/// 当前进程工作集（字节）
fn working_set() -> u64 {
    let mut c = PROCESS_MEMORY_COUNTERS {
        cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        ..Default::default()
    };
    unsafe {
        let proc: HANDLE = GetCurrentProcess();
        if GetProcessMemoryInfo(proc, &mut c, c.cb).is_err() {
            return 0;
        }
    }
    c.WorkingSetSize as u64
}

/// 可压缩的内容：渐变 + 少量噪声，贴近桌面而非纯色
fn make_i420(w: usize, h: usize, seed: u32) -> Vec<u8> {
    let (cw, ch) = (w / 2, h / 2);
    let mut buf = vec![0u8; w * h + 2 * cw * ch];
    for y in 0..h {
        for x in 0..w {
            let v = (x * 255 / w.max(1) + y * 3 + seed as usize * 7) % 256;
            buf[y * w + x] = (v ^ ((x ^ y) & 0x1f)) as u8;
        }
    }
    let uv_off = w * h;
    for y in 0..ch {
        for x in 0..cw {
            let v = ((x * 255 / cw.max(1)) + y * 5 + seed as usize * 3) % 256;
            buf[uv_off + y * cw + x] = v as u8;
            buf[uv_off + cw * ch + y * cw + x] = (255 - v) as u8;
        }
    }
    buf
}

struct Measurement {
    peak_kb: u64,
    retained_kb: u64,
    fps: f64,
}

fn measure(codec: SosCodec, w: usize, h: usize, threads: usize) -> Option<Measurement> {
    let baseline = working_set();
    let cfg = SosVpxConfig {
        width: w,
        height: h,
        codec,
        threads,
        cpu_used: SosVpxConfig::default_cpu_used(codec, w, h),
        bitrate_kbps: SosVpxConfig::default_bitrate_kbps(w, h),
        keyframe_interval: None,
    };
    let mut enc = SosVpxEncoder::new(cfg).ok()?;
    let after_new = working_set();

    let frames = 20u32;
    let mut bytes = 0usize;
    let start = Instant::now();
    for i in 0..frames {
        let yuv = make_i420(w, h, i);
        if let Ok(out) = enc.encode(&yuv, (i * 33) as i64) {
            for (d, _k) in out {
                bytes += d.len();
            }
        }
    }
    let elapsed = start.elapsed().as_secs_f64();
    let peak = working_set().max(after_new);

    drop(enc);
    std::thread::sleep(std::time::Duration::from_millis(300));
    let after_drop = working_set();

    let _ = bytes;
    Some(Measurement {
        peak_kb: peak.saturating_sub(baseline) / 1024,
        retained_kb: after_drop.saturating_sub(baseline) / 1024,
        fps: frames as f64 / elapsed.max(0.001),
    })
}

#[test]
fn encoder_memory_by_config() {
    println!("进程初始工作集: {} MB", working_set() / 1024 / 1024);
    println!(
        "{:<12} {:>8} {:>8} {:>12} {:>12} {:>8}",
        "codec", "res", "threads", "峰值增量MB", "释放后残留MB", "fps"
    );

    for (w, h) in [(1920usize, 1080usize), (1280, 720)] {
        for codec in [SosCodec::Vp9, SosCodec::Vp8] {
            for threads in [1usize, 2, 3, 4, 6] {
                match measure(codec, w, h, threads) {
                    Some(m) => println!(
                        "{:<12} {:>8} {:>8} {:>12.1} {:>12.1} {:>8.1}",
                        format!("{codec:?}"),
                        format!("{w}x{h}"),
                        threads,
                        m.peak_kb as f64 / 1024.0,
                        m.retained_kb as f64 / 1024.0,
                        m.fps
                    ),
                    None => println!(
                        "{:<12} {:>8} {:>8} 创建失败",
                        format!("{codec:?}"),
                        format!("{w}x{h}"),
                        threads
                    ),
                }
            }
        }
    }
}
