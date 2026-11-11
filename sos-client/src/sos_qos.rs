//! 自建 QoS：**多连接 × 多显示器**的帧率 / 画质比 / 码率状态机。
//!
//! # 为什么不用上游 `VIDEO_QOS`
//!
//! 上游 `src/server/video_qos.rs` 有两个层面的问题：
//!
//! 1. **决策逻辑不适合我们的链路**：
//!    - `adjust_ratio()` 每 3 秒按延迟把 ratio 乘 1.15 / 0.95，并用
//!      `MAX_BR_MULTIPLE = 1.0` 封顶。而 SOS 用
//!      [`crate::sos_vpx::LinkBudget`] 表达「链路充裕时放宽 2 倍」，
//!      两套机制叠加后码率不可预测 —— 实测同一台机器在 30 秒内出现过
//!      `ratio 0.67 → 1.50 → 0.20 → 2.00` 的跳变。
//!    - `user_network_delay()` 在平均延迟 < 50ms 时每 3 个样本把 fps **+5**，
//!      实测会在 20↔30fps 之间来回跳。
//!    - 依赖注册表（`Config::get_option("enable-abr")`），无法单测。
//!
//! 2. **它假设每个显示器都有一份独立的 `DisplayData`，但 fps / ratio
//!    却是全局标量**。多显示器同时采集时，两块屏共用同一个 fps 与 ratio，
//!    一块屏拥塞会拖累另一块。SOS 每块屏有独立的采集器和编码器，
//!    QoS 必须与之一一对应。
//!
//! # 本模块的模型
//!
//! ```text
//! conns:    i32(连接) → Conn    { display, ratio?, custom_fps?, want_fps, 链路样本 }
//! displays: String    → bitrate { 最近下发给该屏编码器的码率 }
//! ```
//!
//! 一个连接**同时只看一块屏**（与 RustDesk 协议一致）。显示器是聚合单位：
//!
//! | 量 | 聚合规则 | 理由 |
//! |---|---|---|
//! | `ratio` | 该屏所有连接取**最大** | 有人要求「最佳画质」就不该被别人拉低 |
//! | `target_fps` | 该屏所有连接 `custom_fps` 取**最小** | 编码器只有一个，必须满足最慢的控制端 |
//! | `fps` | 该屏所有连接自适应帧率取**最小** | 任一链路拥塞就该降，否则那块屏会持续积压 |
//!
//! 连接的帧率是**独立**自适应的（每个连接有自己的 `want_fps` 与
//! `bad_samples`）。这样一条慢链路只会把共享的 fps 拉低，快链路恢复后
//! 也不会被慢链路的坏样本污染。
//!
//! # 单一决策依据
//!
//! | 量 | 依据 |
//! |---|---|---|
//! | `ratio` | **只由控制端设定**，QoS 不做二次调整 |
//! | `fps` | **只由排队延迟**驱动 |
//! | 码率 | `ratio × LinkBudget`，见 [`crate::sos_vpx::SosVpxConfig::bitrate_kbps_for_link`] |
//!
//! 用排队延迟而不是 RTT：RTT 高但没有排队（卫星 / 跨洋）说明瓶颈是传播时延，
//! 降帧不改善任何东西，只会让画面更卡。
//!
//! 降与升都按**比例**（×3/4、×5/4+1）而不是固定步长，30fps→8fps 与
//! 8fps→30fps 各约 5 个样本。
//!
//! # 样本节奏
//!
//! [`SosQos::on_test_delay`] 由 `sos_connection` 的 1 秒定时器调用，因此「每个
//! 样本」≈「每秒」。模块内部不读时钟，行为完全由输入序列决定，可精确单测。

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

// ── 帧率 ──

/// 帧率下限。
///
/// 上游 `MIN_FPS = 1`，配合 `response_delayed` 会把帧率压到 `MIN_FPS + 1 = 2`。
/// 2fps 的远程桌面基本不可用，而 SOS 的采集循环无论如何都不会慢于 8fps
/// （见 `sos_video::MIN_CAPTURE_FPS`），所以下限直接取 8。
pub const MIN_FPS: u32 = 8;

/// 控制端未指定时的目标帧率
pub const DEFAULT_FPS: u32 = 30;

/// 协议允许的最大帧率
pub const MAX_FPS: u32 = 120;

// ── 画质比（码率倍数基准）──
//
// 数值与上游 `libs/scrap/src/common/codec.rs` 的 `BR_*` 保持一致，
// 因为控制端的「画质」三档就是按这套倍数设计的。

/// 低画质
pub const BR_LOW: f32 = 0.5;
/// 平衡画质（控制端默认）
pub const BR_BALANCED: f32 = 0.67;
/// 最佳画质
pub const BR_BEST: f32 = 1.5;
/// 允许的画质比下限（自定义画质可能给出极小值）
pub const BR_MIN: f32 = 0.1;
/// 允许的画质比上限（防止自定义值把码率放到荒谬的量级）
pub const BR_MAX: f32 = 40.0;

// ── 排队延迟阈值 ──

/// 排队延迟超过这个值视为链路已饱和（与 `sos_vpx::GOOD_LINK_QUEUE_MS` 一致）。
pub const QUEUE_BAD_MS: u32 = 40;

/// 连续多少个样本判定饱和后才降帧。
///
/// 取 2 而不是 1：`queue = raw - min-filter 下界`，单次 `raw` 尖峰可能来自
/// 调度抖动而非真实排队。连续两次才降，误判代价也只有 25% 帧率、1 秒后即恢复。
pub const BAD_SAMPLES_TO_REDUCE: u32 = 2;

// ── 协议解析（纯函数，便于单测）──

/// 把协议里的 `ImageQuality` 值解析成画质比。
///
/// 协议定义（`libs/hbb_common/protos/message.proto`）：
/// - `NotSet = 0`：返回 `None`（不改变当前设置）
/// - `Low = 2` / `Balanced = 3` / `Best = 4`：对应三档固定倍数
/// - 其他正值：自定义倍数，编码在 `q >> 8 & 0xFFF`（千分之二为单位）
pub fn ratio_of_image_quality(q: i32) -> Option<f32> {
    match q {
        // NotSet：控制端没表态，保持当前值
        0 => None,
        2 => Some(BR_LOW),
        3 => Some(BR_BALANCED),
        4 => Some(BR_BEST),
        // 自定义：与上游 `user_image_quality` 的解码方式一致
        v if v > 0 => {
            let b = ((v >> 8) & 0xFFF) as f32 * 2.0 / 100.0;
            if b > 0.0 {
                Some(b.clamp(BR_MIN, BR_MAX))
            } else {
                None
            }
        }
        // 负值非法
        _ => None,
    }
}

/// 帧率下降一档（拥塞时每样本调用一次）
///
/// 按比例降 25%，下限 [`MIN_FPS`]。从 30fps 到下限约需 5 个样本：
/// `30 → 22 → 16 → 12 → 9 → 8`。
pub fn fps_step_down(cur: u32, target: u32) -> u32 {
    let floor = MIN_FPS.min(target.max(MIN_FPS));
    (cur * 3 / 4).max(floor)
}

/// 帧率恢复一档（链路良好时每样本调用一次）
///
/// 按比例升 25% 再 +1，上限 `target`。从下限 8fps 回到 30fps 约需 5 个样本：
/// `8 → 11 → 15 → 20 → 26 → 30`。
///
/// `+1` 是必要的：纯比例在低帧率区（8 → 10 → 12）太慢，
/// 而纯 +1 在高帧率区（26 → 27）又太慢，两者结合在两端都合理。
pub fn fps_step_up(cur: u32, target: u32) -> u32 {
    let cap = target.max(MIN_FPS);
    let next = cur * 5 / 4 + 1;
    next.min(cap).max(cur)
}

// ── 每连接的设置与链路状态 ──

#[derive(Debug, Clone, Default)]
struct Conn {
    /// 该连接正在观看的显示器（`Display::name()`）
    display: String,
    /// 控制端指定的帧率上限（`Misc::Option.custom_fps`）
    custom_fps: Option<u32>,
    /// 控制端指定的画质比（`Misc::Option.image_quality`）
    ratio: Option<f32>,
    /// 该连接自己的自适应帧率
    want_fps: u32,
    /// 该连接连续判定饱和的样本数
    bad_samples: u32,
    /// 最近一次上报的 RTT（诊断用）
    rtt_ms: Option<u32>,
    /// 最近一次上报的排队延迟（诊断用）
    queue_ms: Option<u32>,
}

impl Conn {
    fn new(display: &str) -> Self {
        Self {
            display: display.to_string(),
            want_fps: DEFAULT_FPS,
            ..Default::default()
        }
    }

    /// 该连接允许的帧率上限
    fn target_fps(&self) -> u32 {
        self.custom_fps
            .unwrap_or(DEFAULT_FPS)
            .clamp(MIN_FPS, MAX_FPS)
    }

    /// 该连接当前需要的帧率（自适应值再受自己的上限约束）
    fn fps(&self) -> u32 {
        self.want_fps
            .min(self.target_fps())
            .clamp(MIN_FPS, MAX_FPS)
    }

    /// 该连接要求的画质比（未设置时取默认档）
    fn ratio(&self) -> f32 {
        self.ratio.unwrap_or(BR_BALANCED).clamp(BR_MIN, BR_MAX)
    }
}

/// 单个显示器的 QoS 视图，供采集线程一次性读取。
#[derive(Debug, Clone)]
pub struct DisplaySnapshot {
    /// 该屏当前应使用的帧率
    pub fps: u32,
    /// 该屏所有连接允许的最低帧率上限
    pub target_fps: u32,
    /// 该屏当前画质比（所有连接取最大）
    pub ratio: f32,
    /// 该屏最近一次下发给编码器的码率
    pub bitrate_kbps: u32,
    /// 观看该屏的连接数
    pub viewers: usize,
    /// 该屏所有连接里最差的排队延迟
    pub worst_queue_ms: Option<u32>,
    /// 该屏所有连接里最大的 RTT
    pub worst_rtt_ms: Option<u32>,
}

impl Default for DisplaySnapshot {
    fn default() -> Self {
        Self {
            fps: DEFAULT_FPS,
            target_fps: DEFAULT_FPS,
            ratio: BR_BALANCED,
            bitrate_kbps: 0,
            viewers: 0,
            worst_queue_ms: None,
            worst_rtt_ms: None,
        }
    }
}

// ── 状态机本体 ──

/// QoS 状态机（可独立构造，便于单测）
#[derive(Debug, Default)]
pub struct SosQos {
    conns: HashMap<i32, Conn>,
    /// 显示器名 → 最近一次下发给该屏编码器的码率（kbps）
    bitrates: HashMap<String, u32>,
}

impl SosQos {
    /// 新建一个空状态机
    pub fn new() -> Self {
        Self::default()
    }

    // ── 连接生命周期 ──

    /// 新的控制端接入，初始观看 `display`。
    ///
    /// 重复调用同一个 `id` 只更新显示器，不重置已有设置
    /// （重连时保留画质选择是符合预期的）。
    pub fn on_connection_open(&mut self, id: i32, display: &str) {
        match self.conns.get_mut(&id) {
            Some(c) => c.display = display.to_string(),
            None => {
                self.conns.insert(id, Conn::new(display));
            }
        }
        self.bitrates.entry(display.to_string()).or_insert(0);
    }

    /// 控制端断开。
    ///
    /// 只移除该连接；其他连接的帧率与画质不受影响 —— 这点在多用户下很重要：
    /// `min` 聚合在移除最慢的那个之后会自然放宽，无需额外逻辑。
    pub fn on_connection_close(&mut self, id: i32) {
        self.conns.remove(&id);
    }

    /// 切换该连接观看的显示器。返回 `true` 表示显示器确实变了。
    pub fn set_connection_display(&mut self, id: i32, display: &str) -> bool {
        let Some(c) = self.conns.get_mut(&id) else {
            log::warn!("[QOS] switch display from unknown conn #{}", id);
            return false;
        };
        if c.display == display {
            return false;
        }
        log::info!(
            "[QOS] conn #{} switches display '{}' -> '{}'",
            id,
            c.display,
            display
        );
        c.display = display.to_string();
        self.bitrates.entry(display.to_string()).or_insert(0);
        true
    }

    /// 控制端指定帧率上限（越界忽略）
    pub fn set_custom_fps(&mut self, id: i32, fps: u32) {
        if fps < MIN_FPS || fps > MAX_FPS {
            log::debug!("[QOS] ignore custom_fps={} from conn #{}", fps, id);
            return;
        }
        match self.conns.get_mut(&id) {
            Some(c) => {
                c.custom_fps = Some(fps);
                // 上限调低时自适应值也要跟着收，不必等下一个样本
                c.want_fps = c.want_fps.min(fps).max(MIN_FPS);
            }
            None => log::warn!("[QOS] custom_fps from unknown conn #{}", id),
        }
    }

    /// 控制端切换画质（传入协议里的 `ImageQuality` 原始值）
    pub fn set_image_quality(&mut self, id: i32, image_quality: i32) {
        let Some(r) = ratio_of_image_quality(image_quality) else {
            return;
        };
        match self.conns.get_mut(&id) {
            Some(c) => c.ratio = Some(r),
            None => log::warn!("[QOS] image_quality from unknown conn #{}", id),
        }
    }

    // ── 链路样本 ──

    /// 上报一次延迟样本（`sos_connection` 的 1 秒定时器调用，约 1 Hz）。
    ///
    /// - `rtt_ms`：min-filter 之后的 RTT 下界
    /// - `queue_ms`：`raw - rtt`，本次往返里前面的视频帧造成的额外等待
    ///
    /// 只更新该连接自己的 `want_fps`；显示器级别的帧率由 [`Self::fps`]
    /// 在所有观看者之间取最小值得到。
    pub fn on_test_delay(&mut self, id: i32, rtt_ms: u32, queue_ms: u32) {
        let Some(c) = self.conns.get_mut(&id) else {
            return;
        };
        c.rtt_ms = Some(rtt_ms);
        c.queue_ms = Some(queue_ms);
        let target = c.target_fps();
        let before = c.fps();
        if queue_ms >= QUEUE_BAD_MS {
            c.bad_samples += 1;
            if c.bad_samples >= BAD_SAMPLES_TO_REDUCE {
                c.bad_samples = 0;
                c.want_fps = fps_step_down(before, target);
            }
        } else {
            c.bad_samples = 0;
            c.want_fps = fps_step_up(before, target);
        }
        let after = c.fps();
        if after != before {
            log::info!(
                "[QOS] conn #{} '{}' queue={}ms rtt={}ms -> fps {} -> {}",
                id,
                c.display,
                queue_ms,
                rtt_ms,
                before,
                after
            );
        }
    }

    // ── 显示器聚合视图 ──

    /// 观看该显示器的连接迭代器
    fn viewers_of<'a>(&'a self, display: &'a str) -> impl Iterator<Item = &'a Conn> + 'a {
        self.conns.values().filter(move |c| c.display == display)
    }

    /// 该显示器的帧率上限：所有观看者的 `custom_fps` 取最小。
    ///
    /// 编码器只有一个，必须满足要求最低的那个控制端。
    pub fn target_fps(&self, display: &str) -> u32 {
        self.viewers_of(display)
            .map(Conn::target_fps)
            .min()
            .unwrap_or(DEFAULT_FPS)
    }

    /// 该显示器当前应使用的帧率：所有观看者自适应帧率取最小。
    ///
    /// 任一链路拥塞就该降 —— 否则那块屏的编码输出会持续积压。
    pub fn fps(&self, display: &str) -> u32 {
        self.viewers_of(display)
            .map(Conn::fps)
            .min()
            .unwrap_or(DEFAULT_FPS)
            .clamp(MIN_FPS, MAX_FPS)
    }

    /// 该显示器的帧间隔
    pub fn spf(&self, display: &str) -> Duration {
        Duration::from_secs_f32(1.0 / self.fps(display).max(1) as f32)
    }

    /// 该显示器当前画质比：所有观看者取最大。
    ///
    /// 有人要求「最佳画质」就不该被其他人的「低画质」拉低。
    pub fn ratio(&self, display: &str) -> f32 {
        self.viewers_of(display)
            .map(Conn::ratio)
            .fold(None::<f32>, |acc, r| Some(acc.map_or(r, |a: f32| a.max(r))))
            .unwrap_or(BR_BALANCED)
            .clamp(BR_MIN, BR_MAX)
    }

    /// 该显示器最近一次下发的码率
    pub fn bitrate_kbps(&self, display: &str) -> u32 {
        self.bitrates.get(display).copied().unwrap_or(0)
    }

    /// 记录本次下发给该显示器编码器的码率，供 `TestDelay.target_bitrate` 上报。
    pub fn store_bitrate(&mut self, display: &str, kbps: u32) {
        self.bitrates.insert(display.to_string(), kbps);
    }

    /// 该显示器的完整视图，供采集线程一次性取走
    pub fn display_snapshot(&self, display: &str) -> DisplaySnapshot {
        let viewers: Vec<&Conn> = self.viewers_of(display).collect();
        DisplaySnapshot {
            fps: self.fps(display),
            target_fps: self.target_fps(display),
            ratio: self.ratio(display),
            bitrate_kbps: self.bitrate_kbps(display),
            viewers: viewers.len(),
            worst_queue_ms: viewers.iter().filter_map(|c| c.queue_ms).max(),
            worst_rtt_ms: viewers.iter().filter_map(|c| c.rtt_ms).max(),
        }
    }
}

// ── 全局单例 ──

static QOS: OnceLock<Mutex<SosQos>> = OnceLock::new();

fn cell() -> &'static Mutex<SosQos> {
    QOS.get_or_init(|| Mutex::new(SosQos::new()))
}

/// 加锁访问 QoS 状态。
///
/// 锁只在这一个闭包内持有，且闭包内**绝不做 IO、不加其他锁** ——
/// 采集线程与连接循环都会调用它，持锁跨 `.await` 或嵌套加锁会直接死锁。
pub fn with<R>(f: impl FnOnce(&mut SosQos) -> R) -> R {
    let mut guard = cell().lock().unwrap();
    f(&mut guard)
}

/// 该显示器的帧间隔
pub fn spf(display: &str) -> Duration {
    with(|q| q.spf(display))
}

/// 该显示器的帧率
pub fn fps(display: &str) -> u32 {
    with(|q| q.fps(display))
}

/// 该显示器的画质比
pub fn ratio(display: &str) -> f32 {
    with(|q| q.ratio(display))
}

/// 该显示器最近一次下发的码率
pub fn bitrate_kbps(display: &str) -> u32 {
    with(|q| q.bitrate_kbps(display))
}

/// 该显示器的完整视图
pub fn display_snapshot(display: &str) -> DisplaySnapshot {
    with(|q| q.display_snapshot(display))
}
