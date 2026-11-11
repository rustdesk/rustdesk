//! 关键帧**密度**与关键帧**码率预算**的实测。
//!
//! ## 要回答的问题
//!
//! 远程桌面希望关键帧更密（对端解码出错、发送队列丢帧后能更快重同步），
//! 但关键帧体积约为 P 帧的 **9 倍**，密度一提就会挤占总码率。所以必须量化：
//!
//! 1. 把 `KEYFRAME_INTERVAL_FRAMES` 从 150 缩到 60 / 30，总码率涨多少？
//! 2. 关键帧本身是否被 CBR 压质量？—— 用**关键帧的量化器**判断，
//!    而不是看总码率。总码率守住了但关键帧 Q 飙升，等于关键帧是糊的。
//! 3. 「给关键帧更多预算」到底该动哪个旋钮：
//!    - `rc_buf_sz`（CBR 记账窗口）—— 窗口太短时装不下一个 9 倍大的关键帧；
//!    - `rc_overshoot_pct`（允许超发上限）；
//!    - `VP8E_SET_MAX_INTRA_BITRATE_PCT`——**默认 0 = 不限**，
//!      设成正数只会收紧，本测试用来证伪它。
//!
//! ## 测量要点
//!
//! - 按 30fps **真实节流**喂帧：CBR 按 pts 流逝的时间记账，全速灌帧会让
//!   「实际码率」失去意义（见 `sos_vpx::frame_ms` 的说明）；
//! - 关掉编码器丢帧（`tune(kbps, 0)`），否则「关键帧小」可能只是「被丢了」；
//! - 内容用 `support/desktop.rs`：关键帧大小强依赖内容，噪声测不出真实比例。
//!
//! 运行：`cargo test --release --test vpx_keyframe_budget -- --nocapture`

#![cfg(all(windows, sos_vpx))]

#[path = "../src/sos_vpx.rs"]
mod sos_vpx;

#[path = "support/desktop.rs"]
mod desktop;

use std::time::{Duration, Instant};

use sos_vpx::{SosCodec, SosVpxConfig, SosVpxEncoder};

/// 每组编码多少帧（30fps 下 4 秒）
const FRAMES: usize = 120;
const WARMUP: usize = 10;
/// 现场 generous 档位 / Balanced 画质下的目标码率
const KBPS: u32 = 2326;
const W: usize = 1600;
const H: usize = 900;

#[derive(Default)]
struct Run {
    kf_bytes: Vec<usize>,
    p_bytes: Vec<usize>,
    kf_q: Vec<i32>,
    p_q: Vec<i32>,
    total_bytes: usize,
    dropped: usize,
}

impl Run {
    fn kbps(&self, frames: usize) -> f64 {
        self.total_bytes as f64 * 8.0 / (frames as f64 / 30.0) / 1000.0
    }
    fn kf_avg(&self) -> f64 {
        avg(&self.kf_bytes)
    }
    fn p_avg(&self) -> f64 {
        avg(&self.p_bytes)
    }
    fn kf_q_avg(&self) -> f64 {
        avg_i32(&self.kf_q)
    }
    fn p_q_avg(&self) -> f64 {
        avg_i32(&self.p_q)
    }
    /// 关键帧相对 P 帧的倍数
    fn ratio(&self) -> f64 {
        if self.p_avg() <= 0.0 {
            0.0
        } else {
            self.kf_avg() / self.p_avg()
        }
    }
    /// 关键帧占用的总码率比例（%）
    fn kf_share(&self) -> f64 {
        let kf_total: usize = self.kf_bytes.iter().sum();
        if self.total_bytes == 0 {
            0.0
        } else {
            kf_total as f64 * 100.0 / self.total_bytes as f64
        }
    }
}

fn avg(v: &[usize]) -> f64 {
    if v.is_empty() {
        0.0
    } else {
        v.iter().sum::<usize>() as f64 / v.len() as f64
    }
}

fn avg_i32(v: &[i32]) -> f64 {
    if v.is_empty() {
        0.0
    } else {
        v.iter().sum::<i32>() as f64 / v.len() as f64
    }
}

struct Params {
    label: String,
    kf_interval: usize,
    buf_ms: u32,
    overshoot: u32,
    intra_pct: u32,
}

fn measure(p: &Params) -> Run {
    sos_vpx::set_rc_override(Some((p.buf_ms, 0)));
    sos_vpx::set_overshoot_override(Some(p.overshoot));
    sos_vpx::set_intra_bitrate_override(if p.intra_pct == 0 {
        None
    } else {
        Some(p.intra_pct)
    });

    let cfg = SosVpxConfig {
        width: W,
        height: H,
        codec: SosCodec::Vp9,
        threads: 3,
        cpu_used: SosVpxConfig::default_cpu_used(SosCodec::Vp9, W, H),
        bitrate_kbps: KBPS,
        keyframe_interval: Some(p.kf_interval),
    };
    let mut enc = SosVpxEncoder::new(cfg).expect("encoder");
    // 关掉丢帧：否则「关键帧小」可能只是「直接丢了」
    enc.tune(KBPS, 0).expect("tune");

    let mut run = Run::default();
    let origin = Instant::now();
    let spf = Duration::from_secs_f64(1.0 / 30.0);
    for i in 0..(FRAMES + WARMUP) {
        let deadline = origin + spf * i as u32;
        let now = Instant::now();
        if deadline > now {
            std::thread::sleep(deadline - now);
        }
        let yuv = desktop::desktop_i420(W, H, i);
        let out = enc
            .encode(&yuv, origin.elapsed().as_millis() as i64)
            .expect("encode");
        run.dropped += enc.take_dropped_frames();
        if i < WARMUP {
            continue;
        }
        let q = enc.quantizer().unwrap_or(-1);
        for (data, key) in out {
            run.total_bytes += data.len();
            if key {
                run.kf_bytes.push(data.len());
                run.kf_q.push(q);
            } else {
                run.p_bytes.push(data.len());
                run.p_q.push(q);
            }
        }
    }

    sos_vpx::set_rc_override(None);
    sos_vpx::set_overshoot_override(None);
    sos_vpx::set_intra_bitrate_override(None);
    run
}

fn report(p: &Params, r: &Run) {
    println!(
        "{:<28} kbps={:>6.0} kf={:>7.0}B p={:>6.0}B 比={:>4.1}x 占比={:>4.1}% kfQ={:>4.1} pQ={:>4.1} 丢={}",
        p.label,
        r.kbps(FRAMES),
        r.kf_avg(),
        r.p_avg(),
        r.ratio(),
        r.kf_share(),
        r.kf_q_avg(),
        r.p_q_avg(),
        r.dropped
    );
}

/// 关键帧**密度**的代价：间隔从 150 缩短到 60 / 30。
#[test]
fn keyframe_interval_cost() {
    println!();
    println!("══ 关键帧密度（VP9 {W}x{H} @ {KBPS}kbps, buf=200ms, overshoot=10, 不丢帧）══");
    for interval in [150usize, 90, 60, 30] {
        let p = Params {
            label: format!("间隔 {interval} 帧 ({:.1}s)", interval as f64 / 30.0),
            kf_interval: interval,
            buf_ms: 200,
            overshoot: 10,
            intra_pct: 0,
        };
        let r = measure(&p);
        report(&p, &r);
    }
    println!();
}

/// 关键帧**预算**：动哪个旋钮能让关键帧不被压质量。
#[test]
fn keyframe_budget_knobs() {
    println!();
    println!("══ 关键帧预算（间隔固定 60 帧 = 2s）══");
    let cases = [
        ("现状 buf=200 over=10", 200u32, 10u32, 0u32),
        ("buf=500 over=10", 500, 10, 0),
        ("buf=1000 over=10", 1000, 10, 0),
        ("buf=200 over=50", 200, 50, 0),
        ("buf=200 over=100", 200, 100, 0),
        ("buf=500 over=50", 500, 50, 0),
        ("intra_pct=450 (收紧)", 200, 10, 450),
        ("intra_pct=900", 200, 10, 900),
    ];
    for (label, buf_ms, overshoot, intra_pct) in cases {
        let p = Params {
            label: label.to_string(),
            kf_interval: 60,
            buf_ms,
            overshoot,
            intra_pct,
        };
        let r = measure(&p);
        report(&p, &r);
    }
    println!();
}

/// 交叉验证：`rc_buf_sz` × 关键帧间隔。
///
/// 上面两组各自单独看会漏掉交互：buffer 决定「一次关键帧能透支多少」，
/// 间隔决定「多久透支一次」。两者必须一起选。
#[test]
fn buffer_x_interval() {
    println!();
    println!("══ rc_buf_sz × 关键帧间隔 交叉验证（目标 {KBPS}kbps, over=10, 不丢帧）══");
    for buf_ms in [200u32, 500, 1000] {
        for interval in [30usize, 60, 150] {
            let p = Params {
                label: format!("buf={buf_ms} 间隔={interval}"),
                kf_interval: interval,
                buf_ms,
                overshoot: 10,
                intra_pct: 0,
            };
            let r = measure(&p);
            report(&p, &r);
        }
    }
    println!();
}

/// **回归保护**：生产配置（`RC_BUFFER_MS=1000` + `KEYFRAME_INTERVAL_FRAMES=60`）
/// 必须同时满足「关键帧没被压糊」与「总码率没失控」。
///
/// 这条断言存在的理由：以后若有人把 buffer 调回去、或把间隔继续缩短，
/// 上面两条性质会分别以「关键帧量化器飙升」和「码率超发」的形式破掉，
/// 而日志里的 `sent_kbps` 单独看是看不出来的（总码率可能仍然正常，
/// 只是全部预算都被关键帧吃掉了）。
#[test]
fn production_config_is_healthy() {
    let p = Params {
        label: "生产配置 buf=1000 间隔=60".to_string(),
        kf_interval: 60,
        buf_ms: 1000,
        overshoot: 10,
        intra_pct: 0,
    };
    let r = measure(&p);
    report(&p, &r);

    assert!(
        !r.kf_bytes.is_empty(),
        "120 帧内应有周期性关键帧（间隔 60 帧），实际一个都没有"
    );
    // 量化器是 0-63。实测生产配置关键帧 54.0；buffer=200 时会飙到 62.5。
    // 留出余量避免抖动误报，但足以拦住「buffer 被调回去」的回归。
    assert!(
        r.kf_q_avg() < 60.0,
        "关键帧量化器 {:.1} 过高，说明关键帧被码率控制压糊了（检查 RC_BUFFER_MS）",
        r.kf_q_avg()
    );
    let ratio = r.kbps(FRAMES) / KBPS as f64;
    assert!(
        ratio < 1.25,
        "实际码率 {:.0}kbps 超目标 {KBPS}kbps 的 {:.0}%，关键帧开销失控",
        r.kbps(FRAMES),
        ratio * 100.0
    );
}
