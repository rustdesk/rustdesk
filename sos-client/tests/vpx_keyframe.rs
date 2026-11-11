//! 验证「强制关键帧」真的能重新同步解码端。
//!
//! 背景：本编码器用 `kf_mode = VPX_KF_DISABLED`，除了首帧**再也不会**自动
//! 产出关键帧。一旦链路上丢了一个编码帧（发送队列溢出）或控制端解码失败，
//! 就必须靠 `force_keyframe()` 重新建立一个参考点，否则对端永久花屏。
//!
//! 运行：cargo test --release --test vpx_keyframe -- --nocapture

#![cfg(all(windows, sos_vpx))]

#[path = "../src/sos_vpx.rs"]
mod sos_vpx;

use sos_vpx::{SosCodec, SosVpxConfig, SosVpxEncoder};
use std::time::{Duration, Instant};

fn make_desktop(w: usize, h: usize, seed: usize) -> Vec<u8> {
    let (cw, ch) = (w / 2, h / 2);
    let mut buf = vec![0u8; w * h + 2 * cw * ch];
    for y in 0..h {
        let base = 200u32.saturating_sub((y * 30 / h.max(1)) as u32);
        for x in 0..w {
            buf[y * w + x] = base as u8;
        }
    }
    let shift = seed % 40;
    for block in 0..12usize {
        let bx = (block * 137 + shift) % w.saturating_sub(120).max(1);
        let by = (block * 71) % h.saturating_sub(60).max(1);
        for x in bx..(bx + 110).min(w) {
            buf[by.min(h - 1) * w + x] = 90;
        }
    }
    let uv = w * h;
    for i in 0..(cw * ch) {
        buf[uv + i] = 128;
        buf[uv + cw * ch + i] = 128;
    }
    buf
}

fn new_encoder() -> SosVpxEncoder {
    SosVpxEncoder::new(SosVpxConfig {
        width: 1920,
        height: 1080,
        codec: SosCodec::Vp9,
        threads: 4,
        cpu_used: 8,
        bitrate_kbps: 1675,
        keyframe_interval: None,
    })
    .expect("encoder")
}

/// 生产配置（`keyframe_interval: Some(n)`）：除了首帧，还会周期性出关键帧。
///
/// 这是**自愈基线**：即使丢帧检测和客户端刷新请求都失效，对端最多卡
/// n 帧也会自动重同步。之前配成 `None`（`VPX_KF_DISABLED`），
/// 除首帧外永不产生关键帧，一旦参考链断裂就只能靠重连恢复。
#[test]
fn periodic_keyframes_are_emitted() {
    let (w, h) = (1920usize, 1080usize);
    let interval = 30usize;
    let mut enc = SosVpxEncoder::new(SosVpxConfig {
        width: 1920,
        height: 1080,
        codec: SosCodec::Vp9,
        threads: 4,
        cpu_used: 8,
        bitrate_kbps: 1675,
        keyframe_interval: Some(interval),
    })
    .expect("encoder");

    let origin = Instant::now();
    let spf = Duration::from_secs_f64(1.0 / 30.0);
    let mut keys = Vec::new();
    for i in 0..(interval * 3) {
        let deadline = origin + spf * i as u32;
        let now = Instant::now();
        if deadline > now {
            std::thread::sleep(deadline - now);
        }
        let yuv = make_desktop(w, h, i);
        let pts = origin.elapsed().as_millis() as i64;
        let out = enc.encode(&yuv, pts).expect("encode");
        if out.iter().any(|(_, k)| *k) {
            keys.push(i);
        }
    }
    println!("interval={interval}，关键帧出现在: {:?}", keys);
    assert_eq!(keys.first(), Some(&0), "首帧必须是关键帧");
    assert!(
        keys.len() >= 3,
        "45 帧内至少应有 3 个关键帧，实际只有 {:?}",
        keys
    );
    // 间隔不应超过配置值太多（留一倍余量，libvpx 可能因缓冲选择稍早出）
    for pair in keys.windows(2) {
        let gap = pair[1] - pair[0];
        assert!(
            gap <= interval * 2,
            "关键帧间隔 {gap} 远超配置的 {interval}：{:?}",
            keys
        );
    }
}

/// 关掉自动关键帧后，只有首帧是关键帧；调用 `force_keyframe()` 才会再来一个。
#[test]
fn force_keyframe_produces_a_keyframe() {
    let (w, h) = (1920usize, 1080usize);
    let mut enc = new_encoder();
    let origin = Instant::now();
    let spf = Duration::from_secs_f64(1.0 / 30.0);

    let mut keys: Vec<usize> = Vec::new();
    for i in 0..20usize {
        let deadline = origin + spf * i as u32;
        let now = Instant::now();
        if deadline > now {
            std::thread::sleep(deadline - now);
        }
        let yuv = make_desktop(w, h, i);
        let pts = origin.elapsed().as_millis() as i64;
        let out = enc.encode(&yuv, pts).expect("encode");
        if out.iter().any(|(_, k)| *k) {
            keys.push(i);
        }
    }
    println!("未强制时出现关键帧的帧序号: {:?}", keys);
    assert_eq!(keys, vec![0], "kf_mode=DISABLED 时只应有首帧是关键帧");

    // 第 21 帧强制关键帧
    enc.force_keyframe();
    let mut got = Vec::new();
    for i in 20..24usize {
        let yuv = make_desktop(w, h, i);
        let pts = origin.elapsed().as_millis() as i64;
        let out = enc.encode(&yuv, pts).expect("encode");
        if out.iter().any(|(_, k)| *k) {
            got.push(i);
        }
    }
    println!("force_keyframe() 之后出现关键帧的帧序号: {:?}", got);
    assert!(
        got.contains(&20),
        "force_keyframe() 之后的第一帧必须是关键帧，实际 {:?}",
        got
    );
    // 只应生效一次
    assert_eq!(got, vec![20], "强制关键帧只应影响下一帧");
}

/// 强制出的关键帧必须真的是一帧独立编码的帧（体积远大于 P 帧），
/// 且之后立刻回到普通 P 帧 —— 否则等于每帧都在重传关键帧。
#[test]
fn forced_keyframe_is_a_real_intra_frame() {
    let (w, h) = (1920usize, 1080usize);
    let mut enc = new_encoder();
    let origin = Instant::now();
    // 先编几帧让码率控制和参考链进入稳定状态
    let mut last_p = 0usize;
    for i in 0..12usize {
        let yuv = make_desktop(w, h, i);
        let out = enc
            .encode(&yuv, origin.elapsed().as_millis() as i64)
            .expect("encode");
        if let Some((data, key)) = out.first() {
            // 第 0 帧是首帧，天然是关键帧
            if i > 0 {
                assert!(!*key, "第 {} 帧不应是关键帧", i);
            }
            last_p = data.len();
        }
    }

    enc.force_keyframe();
    let yuv = make_desktop(w, h, 12);
    let out = enc
        .encode(&yuv, origin.elapsed().as_millis() as i64)
        .expect("encode");
    let (kf_data, key) = out.first().expect("有关键帧输出");
    assert!(*key, "强制出来的帧必须是关键帧");
    println!(
        "P 帧 {} 字节 → 关键帧 {} 字节（{:.1}x）",
        last_p,
        kf_data.len(),
        kf_data.len() as f64 / last_p.max(1) as f64
    );
    assert!(
        kf_data.len() > last_p,
        "关键帧({} 字节) 应大于普通 P 帧({} 字节)，否则根本没走 intra 编码",
        kf_data.len(),
        last_p
    );

    // 下一帧必须回到 P 帧
    let yuv = make_desktop(w, h, 13);
    let out = enc
        .encode(&yuv, origin.elapsed().as_millis() as i64)
        .expect("encode");
    if let Some((_, key)) = out.first() {
        assert!(!*key, "强制关键帧后应立刻回到 P 帧");
    }
}
