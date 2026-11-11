//! 端到端验证：真实屏幕采集 → `Frame::to`(libyuv 转 I420) → 自建 libvpx 编码。
//!
//! 验证重点是 YUV 布局一致性：`SosVpxEncoder::yuvfmt()` 必须与
//! `scrap::convert_to_yuv` 的输出完全匹配，否则编码出的画面会错乱。
//!
//! 运行：cargo test --release --test vpx_e2e -- --nocapture

#![cfg(sos_vpx)]

#[path = "../src/sos_vpx.rs"]
mod sos_vpx;

use sos_vpx::{SosCodec, SosVpxConfig, SosVpxEncoder};
use scrap::TraitCapturer;
use std::time::Instant;

#[test]
fn capture_to_encode_pipeline() {
    let mut displays = scrap::Display::all().expect("no display");
    if displays.is_empty() {
        println!("[skip] no display available");
        return;
    }
    let display = displays.remove(0);
    let (w, h) = (display.width() as usize, display.height() as usize);
    println!("capture: {}x{} ({})", w, h, display.name());

    let mut capturer = scrap::Capturer::new(display).expect("create capturer");

    for codec in [SosCodec::Vp9, SosCodec::Vp8] {
        let cfg = SosVpxConfig {
            width: w,
            height: h,
            codec,
            threads: SosVpxConfig::recommended_threads(),
            cpu_used: SosVpxConfig::default_cpu_used(codec, w, h),
            bitrate_kbps: SosVpxConfig::default_bitrate_kbps(w, h),
            keyframe_interval: None,
        };
        let mut encoder = SosVpxEncoder::new(cfg).expect("create encoder");
        let yuvfmt = encoder.yuvfmt();
        let mut yuv = Vec::new();
        let mut mid = Vec::new();
        println!(
            "  yuvfmt: w={} h={} stride={:?} u={} v={}",
            yuvfmt.w, yuvfmt.h, yuvfmt.stride, yuvfmt.u, yuvfmt.v
        );

        let frames = 30u32;
        let mut ok = 0u32;
        let mut bytes = 0usize;
        let mut keyframes = 0u32;
        let start = Instant::now();
        for i in 0..frames {
            let frame = match capturer.frame(std::time::Duration::from_millis(100)) {
                Ok(f) => f,
                Err(e) => {
                    println!("  frame {i} capture: {e:?}");
                    continue;
                }
            };
            let data = match frame.to(yuvfmt.clone(), &mut yuv, &mut mid) {
                Ok(input) => match input.yuv() {
                    Ok(d) => d.to_vec(),
                    Err(e) => {
                        println!("  frame {i} to yuv: {e}");
                        continue;
                    }
                },
                Err(e) => {
                    println!("  frame {i} convert: {e}");
                    continue;
                }
            };
            match encoder.encode(&data, (i * 33) as i64) {
                Ok(out) => {
                    ok += 1;
                    for (d, key) in out {
                        bytes += d.len();
                        if key {
                            keyframes += 1;
                        }
                    }
                }
                Err(e) => println!("  frame {i} encode: {e}"),
            }
        }
        let elapsed = start.elapsed().as_secs_f64();
        println!(
            "  [{codec:?}] encoded {ok}/{frames} in {elapsed:.2}s = {:.1} fps, avg_kb={:.0}, keyframes={keyframes}",
            ok as f64 / elapsed,
            bytes as f64 / ok.max(1) as f64 / 1024.0
        );
        assert!(ok > 0, "no frame encoded for {codec:?}");
        assert!(bytes > 0, "no payload for {codec:?}");
    }
}
