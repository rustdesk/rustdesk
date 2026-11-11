//! SOS 自建 libvpx 编码器。
//!
//! 存在的原因：`libs/scrap` 的 `VpxEncoder` 把线程数写死为 `codec_thread_num(64)`，
//! 该函数最后一步是 `min(res, available_memory/2)`。在虚拟机 / 受限会话里
//! 可用内存常被报成 1G，导致只剩 **1 个编码线程**，1080p 高细节画面只有 8fps。
//!
//! 这里直接链接 vcpkg 提供的同一个 `vpx.lib`（`build.rs` 生成绑定），
//! 由 sos-client 自己掌控线程数、CPU 等级、码率与分辨率，完全不改动 `libs/`。

use hbb_common::{bail, ResultType};
use std::os::raw::{c_int, c_uint};

#[allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    dead_code
)]
#[allow(unused_imports)]
#[allow(clippy::all)]
mod ffi {
    include!(concat!(env!("OUT_DIR"), "/vpx_ffi.rs"));
}
pub use ffi::*;

/// 编码格式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SosCodec {
    Vp8,
    Vp9,
}

/// 控制端的编码偏好，对应协议里的 `SupportedDecoding.prefer`。
///
/// H264 / H265 / AV1 无法软件编码，映射为 [`CodecPrefer::Auto`]。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CodecPrefer {
    #[default]
    Auto,
    Vp8,
    Vp9,
}

impl CodecPrefer {
    /// 用于存入静态变量的紧凑表示
    pub fn to_i32(self) -> i32 {
        match self {
            Self::Auto => 0,
            Self::Vp8 => 1,
            Self::Vp9 => 2,
        }
    }

    pub fn from_i32(v: i32) -> Self {
        match v {
            1 => Self::Vp8,
            2 => Self::Vp9,
            _ => Self::Auto,
        }
    }
}

/// 选择编码格式（纯函数，便于单测）
///
/// 决策顺序与上游 `scrap::codec::Encoder::update()` 一致：
///
/// 1. 对端**只能**解一种 → 必须选它，否则对端黑屏；
/// 2. 对端**显式**偏好 VP8/VP9 → 直接采用；
/// 3. `Auto` → **VP9**。
///
/// 第 2 步曾经是严重缺陷：原实现只看 `ability_vp8` / `ability_vp9`，
/// **完全忽略 `prefer`**。而 RustDesk 客户端切换编码时只改 `prefer`，
/// 两个 ability 恒为 true，于是用户的切换永远不生效 —— 日志里
/// `codec` 一直停在 VP9，无论控制端怎么选。
///
/// # 为什么 `Auto` 不再按线程数/分辨率猜
///
/// 旧实现是 `threads <= 2 && pixels > 720p → VP8`，依据是
/// 「线程少时 VP8 软编码快约一倍」。**实测不支持这个说法**。
///
/// 同一份桌面内容、同一目标码率、关闭编码器丢帧后测得的每帧阻塞耗时
/// （`tests/codec_cost.rs`，两次独立运行的范围）：
///
/// | 线程 | VP8 相对速度 | VP8 量化器 |
/// |---|---|---|
/// | 1 | 1.05x ~ 1.33x（更快） | 1600x900 持平 / 1080p 高 31% |
/// | 2 | **0.67x ~ 0.79x** | 高 14% ~ 22% |
/// | 3 | **0.56x ~ 0.62x** | 高 19% ~ 21% |
/// | 4 | **0.47x ~ 0.59x** | 高 19% ~ 26% |
/// | 6 | **0.44x ~ 0.58x** | 高 24% ~ 38% |
///
/// （「相对速度」= VP9 耗时 / VP8 耗时，所以 < 1 表示 VP8 **更慢**。
/// 量化器是 0-63 刻度，越大越糊。）
///
/// 关键结论：在 **2 线程及以上**，VP8 在速度与画质**两个维度同时更差** ——
/// 它比 VP9 慢 27%~127%，同时量化器还高 14%~38%。原因是 VP9 realtime 有
/// 真正的帧内并行工具（row-mt + tile columns），VP8 的并行度很弱，线程越多
/// 差距越大。而 1 线程时 VP8 也只快 5%~33%，不足以补偿画质。
///
/// [`SosVpxConfig::recommended_threads`] 的下限是 **2**，所以本项目的实际
/// 工作区间里 VP8 没有任何优势 —— `Auto` 选 VP9 是速度与画质同时更优。
///
/// 另外，速度关系本身还会**随内容反转**：换成水平条纹状的合成内容
/// （`tests/vpx_enc_throughput.rs` 的旧数据）VP8 反而快数倍。既然连方向
/// 都不稳定，任何静态启发式都无法成立。
///
/// 「这台机器编不动 VP9」只能由**实测编码耗时**判定，见
/// [`next_enc_level`] / [`encoder_behind_budget`] / [`vp8_trial_worth_keeping`]。
pub fn choose_codec(peer_vp8: bool, peer_vp9: bool, prefer: CodecPrefer) -> SosCodec {
    if !peer_vp9 && peer_vp8 {
        return SosCodec::Vp8;
    }
    if !peer_vp8 && peer_vp9 {
        return SosCodec::Vp9;
    }
    match prefer {
        CodecPrefer::Vp8 => SosCodec::Vp8,
        CodecPrefer::Vp9 | CodecPrefer::Auto => SosCodec::Vp9,
    }
}

/// 在 [`choose_codec`] 之上叠加「本机实测编不动」的降级结论。
///
/// `enc_level` 来自 [`next_enc_level`] 的阶梯：`>= 2` 表示该屏已经实测
/// 换到 VP8 试验档。这个降级**只能**在控制端没有表态（`Auto`）时生效。
///
/// # 为什么显式偏好必须优先
///
/// 早期实现是无条件 `enc_level >= 2 → VP8`，优先级高于一切。后果：某次负载
/// 高峰把某屏推到试验档之后，该档位一直保留，控制端再切 VP9 都会被静默压回
/// VP8 —— 日志照样打印 `codec changed Vp8 -> Vp9`，实际却建了 VP8，
/// 于是「切换编码协议无效」。实测降级是 `Auto` 的兜底，不该推翻用户的显式选择。
pub fn codec_with_degradation(
    peer_vp8: bool,
    peer_vp9: bool,
    prefer: CodecPrefer,
    enc_level: u8,
) -> SosCodec {
    if prefer == CodecPrefer::Auto && enc_level >= 2 && peer_vp8 {
        return SosCodec::Vp8;
    }
    choose_codec(peer_vp8, peer_vp9, prefer)
}

// ── 码率控制缓冲（单位：毫秒）──
//
// libvpx 的 `rc_buf_sz` / `rc_buf_initial_sz` / `rc_buf_optimal_sz` 单位是
// **毫秒**，不是比特。vpx_encoder.h 原文："This value is expressed in units of
// time (milliseconds). For example, a value of 5000 indicates that the client
// will buffer (at least) 5000ms worth of encoded data."
//
// 这些值决定编码器在多大的时间窗口内「记账」：窗口越大，它越可以在复杂画面
// 上超发、在简单画面上攒预算。远程桌面要的是低延迟，所以窗口不能过长。
//
// # 为什么是 1000ms 而不是 200ms（`tests/vpx_keyframe_budget.rs` 实测）
//
// 关键帧体积约为 P 帧的 **9 倍**，而 buff 只有 200ms（30fps 下 6 帧）时
// 根本装不下这次透支 —— 编码器唯一的出路是**把关键帧压到量化器上限**：
//
// | buf | 关键帧字节 | 关键帧量化器 | P 帧量化器 | 实际码率(目标2326) |
// |---|---|---|---|---|
// | 200 | 33,775 | **62.5 / 63** | 18.2 | 2631 (+13%) |
// | 500 | 51,408 | 58.5 | 16.8 | 2528 (+9%) |
// | **1000** | **70,320** | **54.0** | **15.3** | **2383 (+2%)** |
//
// 200ms 下关键帧量化器 62.5 意味着「关键帧基本是糊的」——而关键帧正是
// 对端出错后唯一的重建参考点，糊在这里等于每次恢复都重影一次。
// 更反直觉的是**码率反而更准**：200ms 时为了补关键帧的亏空，后续 P 帧
// 被抬价（10576B vs 8905B），总码率反而超到 1.13x。
//
// 1000ms 仍然远低于会养出 bufferbloat 的量级：历史上误写成
// `target_bitrate * 5`（1675kbps → **8375ms**）时实测 `sent_kbps` 跑到
// 1.27x 并把发送队列压到 25 帧；1000ms 实测只有 1.02x。
//
// 注意 `rc_buf_sz` 是**码率记账窗口**，不是编码延迟：`g_lag_in_frames = 0`
// 保证编码器每帧立刻吐包，真正的延迟在发送队列（`MAX_VIDEO_BACKLOG`）。
const RC_BUFFER_MS: c_uint = 1000;

/// `rc_buf_initial_sz` / `rc_buf_optimal_sz` 相对 `rc_buf_sz` 的比例。
/// 取 60% / 50%：起始水位略低，避免开机头几秒被当成「有大量存款」而超发。
fn rc_buffer_derived(buf_ms: c_uint) -> (c_uint, c_uint) {
    ((buf_ms * 6 / 10).max(1), (buf_ms / 2).max(1))
}

/// 帧率下采样的触发水位（目标缓冲充盈度百分比）。
///
/// 语义见 `vpx_encoder.h`：缓冲低于这个百分比时才丢一帧，0 表示禁用。
/// 也就是说**值越小越保守**。
///
/// 实测 1080p 高复杂度内容 @1675kbps（`tests/vpx_feed_rate.rs`）：
///
/// | thresh | 出帧 | 丢帧率 | 实际码率 | 相对目标 |
/// |---|---|---|---|---|
/// | 20 | 10/60 | 83% | 1853 | 1.11x |
/// | 10 | 12/60 | 80% | 1848 | 1.10x |
/// | 5 | 10/60 | 83% | 1800 | 1.07x |
/// | **0** | **60/60** | **0%** | **3294** | **1.97x** |
///
/// 关键结论：**5-20 之间调几乎无效**（内容复杂度足以把缓冲一次打到底），
/// 只有 0 能真正保住帧数，代价是码率翻倍。所以这个值不是"调优参数"，
/// 而是**链路充裕度的开关**：
///
/// - 链路充裕（低 RTT）→ 不丢帧，优先流畅度；
/// - 链路可能拥塞（高 RTT / 中继）→ 守码率，容忍丢帧。
///
/// `sos_video` 会根据实测 RTT 在两者间切换。
pub const RC_DROPFRAME_THRESH: c_uint = 20;

/// 允许超过目标码率的上限（百分比）。
///
/// **已经实测确认：这个参数不能减少丢帧。** libvpx 的默认值是 100
/// （允许超发一倍），实测目标 1675kbps 时 `sent_kbps` 会跑到 2520-2720，
/// 在中继链路上养出 bufferbloat（RTT 34ms → 211ms），所以收窄到 10。
///
/// 曾怀疑 10 太严导致「宁可丢帧也不超发」，用 1080p 高复杂度内容实测：
///
/// | overshoot | 丢帧率 | 实际码率 |
/// |---|---|---|
/// | 10% | 83% | 1853 kbps |
/// | 30% | 83% | 1853 kbps |
/// | 60% | 83% | 1852 kbps |
///
/// 三者完全相同 —— 它约束的是**总码率**，而丢帧本身已经让总码率达标了。
/// 减少丢帧只能靠 [`RC_DROPFRAME_THRESH`] 或提高目标码率。
///
/// 对**关键帧**同样无效（`tests/vpx_keyframe_budget.rs`，间隔 60 帧、不丢帧）：
///
/// | overshoot | 关键帧字节 | 关键帧量化器 |
/// |---|---|---|
/// | 10% | 33,775 | 62.5 |
/// | 50% | 33,775 | 62.5 |
/// | 100% | 33,775 | 62.5 |
///
/// 一字不差。关键帧被压质量的原因是**记账窗口太短**，不是超发上限 ——
/// 正确的旋钮是 [`RC_BUFFER_MS`]。
const RC_OVERSHOOT_PCT: c_uint = 10;

// ── 仅诊断用的线程级覆盖 ──
//
// 码率控制的两个关键参数（缓冲时长、丢帧水位）必须能对比测试才能定值，
// 但给 `SosVpxConfig` 加字段会波及 14 处构造点，噪音太大。
// 这里用一个线程局部覆盖，只影响调用它的测试线程。
thread_local! {
    static RC_OVERRIDE: std::cell::Cell<Option<(c_uint, c_uint)>> =
        const { std::cell::Cell::new(None) };
}

/// 仅诊断用：覆盖当前线程创建的编码器的码率控制参数。
///
/// 传入 `None` 恢复生产默认值。**生产代码不应调用。**
#[doc(hidden)]
#[allow(dead_code)]
pub fn set_rc_override(buf_ms: Option<(u32, u32)>) {
    RC_OVERRIDE.with(|c| c.set(buf_ms.map(|(a, b)| (a as c_uint, b as c_uint))));
}

fn rc_params() -> (c_uint, c_uint) {
    RC_OVERRIDE
        .with(|c| c.get())
        .unwrap_or((RC_BUFFER_MS, RC_DROPFRAME_THRESH))
}

// 仅诊断用：覆盖 `rc_overshoot_pct`（`None` 用生产默认值）。
//
// 这个参数直接决定「宁可超发还是宁可丢帧」：设 10 表示只允许超发 10%，
// 压不住就只能丢帧；放宽则帧数更多但占用带宽更高。
thread_local! {
    static OVERSHOOT_OVERRIDE: std::cell::Cell<Option<c_uint>> =
        const { std::cell::Cell::new(None) };
}

/// 仅诊断用：覆盖 `rc_overshoot_pct`。**生产代码不应调用。**
#[doc(hidden)]
#[allow(dead_code)]
pub fn set_overshoot_override(pct: Option<u32>) {
    OVERSHOOT_OVERRIDE.with(|c| c.set(pct.map(|v| v as c_uint)));
}

fn overshoot_pct() -> c_uint {
    OVERSHOOT_OVERRIDE
        .with(|c| c.get())
        .unwrap_or(RC_OVERSHOOT_PCT)
}

// 仅诊断用：覆盖 `VP8E_SET_MAX_INTRA_BITRATE_PCT`（关键帧码率上限）。
//
// 语义（`vp8cx.h`）：关键帧最大体积，单位是**平均每帧码率的百分比**，
// **默认 0 = 不额外钳制**。也就是说它不是「提高关键帧预算」的旋钮 ——
// 设成正数只会收紧。真正限制关键帧的是 CBR 的记账窗口
// （[`RC_BUFFER_MS`]）与 [`RC_OVERSHOOT_PCT`]，见
// `tests/vpx_keyframe_budget.rs` 的实测。
thread_local! {
    static INTRA_BITRATE_OVERRIDE: std::cell::Cell<Option<c_uint>> =
        const { std::cell::Cell::new(None) };
}

/// 仅诊断用：覆盖 `VP8E_SET_MAX_INTRA_BITRATE_PCT`（0 = 不限）。**生产代码不应调用。**
#[doc(hidden)]
#[allow(dead_code)]
pub fn set_intra_bitrate_override(pct: Option<u32>) {
    INTRA_BITRATE_OVERRIDE.with(|c| c.set(pct.map(|v| v as c_uint)));
}

// 仅诊断用：覆盖 `rc_end_usage`（CBR / VBR / CQ / Q）。
//
// 远程桌面该用哪种码率控制模式是个需要实测的取舍，不能凭直觉定：
// CBR 严格守码率但有丢帧风险，VBR 允许码率随内容波动，
// CQ 按质量编码、码率只作上限。
thread_local! {
    static RCMODE_OVERRIDE: std::cell::Cell<Option<c_int>> = const { std::cell::Cell::new(None) };
}

// 仅诊断用：覆盖 VP9 的 `VP9E_SET_TUNE_CONTENT`（`None` = 生产默认，不调用）。
//
// libvpx 提供了 `VP9E_CONTENT_SCREEN`（palette + intra block copy），
// 对桌面文字/大面积同色区域理论上明显省码率。上游 `libs/scrap` 没设它，
// 我们也没设 —— 是否值得开启必须用本仓库的桌面内容基准实测（见
// `tests/vpx_screen_content.rs`），不能因为「有个选项」就打开。
thread_local! {
    static TUNE_CONTENT_OVERRIDE: std::cell::Cell<Option<c_int>> =
        const { std::cell::Cell::new(None) };
}

/// 仅诊断用：覆盖 `VP9E_SET_TUNE_CONTENT`（参数用 `vp9e_tune_content` 的值）。
/// **生产代码不应调用。**
#[doc(hidden)]
#[allow(dead_code)]
pub fn set_tune_content_override(v: Option<i32>) {
    TUNE_CONTENT_OVERRIDE.with(|c| c.set(v.map(|x| x as c_int)));
}

/// 仅诊断用：覆盖 `rc_end_usage`。**生产代码不应调用。**
#[doc(hidden)]
#[allow(dead_code)]
pub fn set_rcmode_override(mode: Option<i32>) {
    RCMODE_OVERRIDE.with(|c| c.set(mode));
}

fn rc_mode() -> vpx_rc_mode {
    match RCMODE_OVERRIDE.with(|c| c.get()) {
        Some(v) if v == vpx_rc_mode::VPX_VBR as c_int => vpx_rc_mode::VPX_VBR,
        Some(v) if v == vpx_rc_mode::VPX_CQ as c_int => vpx_rc_mode::VPX_CQ,
        Some(v) if v == vpx_rc_mode::VPX_Q as c_int => vpx_rc_mode::VPX_Q,
        _ => vpx_rc_mode::VPX_CBR,
    }
}

/// 编码器内存实测（`tests/vpx_mem.rs`，工作集增量）：
///
/// | 配置 | 1 线程 | 6 线程 |
/// |---|---|---|
/// | VP9 1920x1080 | 158.6 MB | 178.2 MB |
/// | VP8 1920x1080 | 72.4 MB | 72.6 MB |
/// | VP9 1280x720 | 78.8 MB | 99.1 MB |
/// | VP8 1280x720 | 32.4 MB | 32.7 MB |
///
/// 也就是说线程数对内存几乎无影响（每线程约 4 MB），
/// 开销主要是分辨率决定的**基础工作集**。
/// 因此线程数只需按 CPU 取值，不应拿内存去卡。
/// 只有当可用内存低到连基础工作集都放不下时（< [`ENCODER_MIN_FREE_BYTES`]）
/// 才收敛线程，避免换页拖死整个进程。
const ENCODER_MIN_FREE_BYTES: u64 = 512 * 1024 * 1024;

/// 当前可用物理内存（字节）。取不到时返回 `None`，由调用方退回只看 CPU 的策略。
#[cfg(windows)]
fn available_phys_bytes() -> Option<u64> {
    use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut status = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    unsafe { GlobalMemoryStatusEx(&mut status).ok()? };
    Some(status.ullAvailPhys)
}

#[cfg(not(windows))]
fn available_phys_bytes() -> Option<u64> {
    None
}

/// 物理内存总量（MB），仅用于日志诊断
pub fn total_phys_mb() -> u64 {
    #[cfg(windows)]
    {
        use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
        let mut status = MEMORYSTATUSEX {
            dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
            ..Default::default()
        };
        if unsafe { GlobalMemoryStatusEx(&mut status).is_ok() } {
            return status.ullTotalPhys / 1024 / 1024;
        }
    }
    0
}

/// 可用物理内存（MB），仅用于日志诊断
pub fn avail_phys_mb() -> u64 {
    available_phys_bytes().unwrap_or(0) / 1024 / 1024
}

impl SosCodec {
    fn iface(self) -> *const vpx_codec_iface {
        unsafe {
            match self {
                SosCodec::Vp8 => vpx_codec_vp8_cx(),
                SosCodec::Vp9 => vpx_codec_vp9_cx(),
            }
        }
    }
}

/// 链路充裕时是否放宽码率预算
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkBudget {
    /// 低 RTT 且无排队：不做帧率缩放，并放宽总预算
    Generous,
    /// 链路未知或已出现积压：按帧率缩放，守住每帧体积
    Constrained,
}

/// 自建编码器配置
#[derive(Debug, Clone, Copy)]
pub struct SosVpxConfig {
    pub width: usize,
    pub height: usize,
    pub codec: SosCodec,
    /// 编码线程数（0 = 自动，按 CPU 核数取值，不受 available_memory 限制）
    pub threads: usize,
    /// libvpx `cpu-used`：越大越快、质量越低。VP9 建议 5-8，VP8 建议 12-16
    pub cpu_used: u32,
    /// 目标码率（kbps）
    pub bitrate_kbps: u32,
    /// 关键帧间隔（帧数）；None 表示禁用自动关键帧
    pub keyframe_interval: Option<usize>,
}

impl SosVpxConfig {
    /// 按 CPU 核数推算合理的线程数：留出约 1/4 的核心给采集/网络/系统，
    /// 至少 2 个、最多 8 个。
    ///
    /// 不用内存卡线程数：实测每线程仅增加约 4 MB（见 [`ENCODER_MIN_FREE_BYTES`] 的表格），
    /// 真正吃内存的是分辨率决定的基础工作集。
    /// 只有在可用内存低于 512 MB 时才收敛到 2 个线程——那个量级下
    /// 换页带来的损失远比少几个线程严重。
    pub fn recommended_threads() -> usize {
        let cpus = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        // 原来取「核数的一半」，在 4 核机器上只给编码器 2 个线程。
        // 但自建 DXGI 采集实测只要 5ms、颜色转换 2ms、网络更轻，
        // 剩下两个核纯属闲置，而编码器正是瓶颈：
        // 实测 1600x900 VP9 cpu_used=7 → 2 线程 3.4ms / 3 线程 2.8ms / 4 线程 2.2ms。
        // 改为 3/4 核数（4 核→3、8 核→6、16 核→8），保留 25% 给其他工作。
        let cpu_based = (cpus * 3 / 4).clamp(2, 8);
        match available_phys_bytes() {
            Some(avail) if avail < ENCODER_MIN_FREE_BYTES => {
                log::warn!(
                    "[VPX] only {}MB memory available, limiting encoder to 2 threads",
                    avail / 1024 / 1024
                );
                2
            }
            _ => cpu_based,
        }
    }

    /// 依据分辨率与编码格式给出默认码率（kbps）
    pub fn default_bitrate_kbps(width: usize, height: usize) -> u32 {
        let pixels = width * height;
        // 1080p ≈ 2500kbps，720p ≈ 1200kbps，随像素线性缩放
        ((pixels as u64 * 2500 / (1920 * 1080)) as u32).max(300)
    }

    /// 满码率对应的标称帧率，是 [`bitrate_kbps_for`] 的缩放基准点。
    pub const NOMINAL_FPS: f32 = 30.0;

    /// 码率随帧率缩放的下限。
    ///
    /// 取 **0.5**（即帧率不低亁15fps 时的码率），而不是更小的值 ——
    /// 实测证明码率降得太低会**自相矛盾**：
    ///
    /// ```text
    /// ratio 1.28 fps 3 -> bitrate 318kbps
    /// → encode_out=1.8fps  edrop=8/19 (42%)
    /// ```
    ///
    /// 1080p 在 318kbps 下根本编不出可用的帧，`rc_dropframe_thresh` 只能
    /// 疯狂丢帧 —— 而“丢帧”正是我们想避免的。更糟的是它把采集帧率
    /// 实际已经低于 `MIN_CAPTURE_FPS` 的事实掩盖了：我们按 3fps 发预算，
    /// 却按 8fps 采集，预算从一开始就是错的。
    ///
    /// 宁可接受低帧率下帧体积变大（多一点排队），也不能让编码器编不出来。
    const MIN_FPS_FACTOR: f32 = 0.5;

    /// 计算编码目标码率（kbps）。
    ///
    /// 关键点：**码率必须随帧率等比缩放**，不能只看画质比。
    ///
    /// 视频帧和 `TestDelay` 探针走同一条 TCP 流，一帧的传输时间约等于
    /// `bitrate / (fps × 链路带宽)`，这段时间里探针只能排队 ——
    /// 于是**测到的 RTT 基本就等于这一帧的排队延迟**。
    ///
    /// 实测数据与这个模型完全吻合：
    ///
    /// | 码率 | 帧率 | 每帧 | 实测 rtt |
    /// |---|---|---|---|
    /// | 2500 kbps | 30 | 10 KB | 35 ms |
    /// | 2868 kbps | 2 | 179 KB | 451 ms |
    ///
    /// 一旦 QoS 因为延迟高而压低帧率、而码率保持不变，每帧就变大、
    /// 排队更久、测到的延迟更高 —— **正反馈死循环**。
    /// 实测中 QoS 把帧率压到 2fps 后码率仍是 2868kbps，RTT 停在 451ms
    /// 长达数秒，用户看到的就是画面卡死。
    ///
    /// 按 `fps / NOMINAL_FPS` 缩放后，排队延迟只与画质比有关、与帧率**解耦**，
    /// 这个循环就消失了。满帧率（30fps）下结果与原公式完全一致。
    ///
    /// 但缩放不能无限做：见 [`Self::MIN_FPS_FACTOR`]。
    pub fn bitrate_kbps_for(width: usize, height: usize, ratio: f32, fps: u32) -> u32 {
        let full = Self::default_bitrate_kbps(width, height) as f32;
        let factor = (fps as f32 / Self::NOMINAL_FPS).clamp(Self::MIN_FPS_FACTOR, 1.0);
        (full * ratio * factor).max(150.0) as u32
    }

    /// 链路充裕时额外放宽的码率倍率。
    ///
    /// **依据是量化器实测**（`tests/vpx_budget.rs`，1600x900 桌面文字内容，
    /// VP9 cpu_used=7，30fps）。量化器是 0-63 刻度，`rc_max_quantizer=63`：
    ///
    /// | 目标码率 | 量化器均值 | 占刻度 |
    /// |---|---|---|
    /// | 1163 kbps（Balanced 默认点） | **47.7** | **76%** |
    /// | 1500 kbps | 42.3 | 67% |
    /// | 2000 kbps | 35.1 | 56% |
    /// | 2604 kbps（Best） | 24.9 | 40% |
    /// | 3200 kbps | 18.5 | 29% |
    /// | 4000 kbps | 14.4 | 23% |
    ///
    /// 也就是说默认工作点（Balanced）把量化器推到刻度 76% 的位置 ——
    /// 文字边缘基本被抹平。取 2.0 倍后 Balanced 约为 2326 kbps，
    /// 量化器落在 30 附近（刻度 48%），是「能看清」与「不浪费带宽」的平衡点。
    ///
    /// 只在**链路充裕**时启用：内网/直连的瓶颈在画质而不在带宽。
    /// 拥塞链路上超发会直接变成 bufferbloat（实测 RTT 34ms → 211ms）。
    pub const GOOD_LINK_BITRATE_GAIN: f32 = 2.0;

    /// 按链路预算计算目标码率。
    ///
    /// 与 [`Self::bitrate_kbps_for`] 的关键差别：
    ///
    /// - **Constrained**：`bitrate × fps/30`。帧率降低时码率同步降低，
    ///   每帧体积恒定，这样 `TestDelay` 探测不会被大帧挡住。
    ///   这是防 bufferbloat 的必需机制。
    /// - **Generous**：**不**做帧率缩放，另加 [`Self::GOOD_LINK_BITRATE_GAIN`] 倍。
    ///
    /// 为什么不缩放在充裕链路上是对的：帧率下降时若不降码率，
    /// 每帧体积会变大 —— 但宽裕链路没有排队问题，大帧只是传得更快；
    /// 而单帧质量反而**更好**（同样的码率摊到更少的帧上）。
    /// 缩放在这里没有收益，只有画质损失，而且是双重的：
    /// 帧率已经少了，码率再砍一半。
    pub fn bitrate_kbps_for_link(
        width: usize,
        height: usize,
        ratio: f32,
        fps: u32,
        budget: LinkBudget,
    ) -> u32 {
        match budget {
            LinkBudget::Generous => {
                let full = Self::default_bitrate_kbps(width, height) as f32;
                (full * ratio * Self::GOOD_LINK_BITRATE_GAIN).max(400.0) as u32
            }
            LinkBudget::Constrained => Self::bitrate_kbps_for(width, height, ratio, fps),
        }
    }

    /// 依据分辨率与编码格式给出默认 `cpu-used`
    pub fn default_cpu_used(codec: SosCodec, width: usize, height: usize) -> u32 {
        match codec {
            SosCodec::Vp8 => 12,
            SosCodec::Vp9 => {
                if width * height >= 1920 * 1080 {
                    8 // 高分辨率优先实时性
                } else {
                    7
                }
            }
        }
    }

    /// VP9 realtime 模式下 `cpu-used` 的上限（`vpx_encoder.h`：realtime 0-9）。
    ///
    /// 供 [`crate::sos_video`] 在实测「编码跟不上帧预算」时降级使用：
    /// 这是画质损失最小的一档提速手段（实测 1600x900 约 1.17x）。
    ///
    /// 下一档是换 VP8，但**不能假定它更快**：在 2 线程及以上，VP8 实测比
    /// VP9 慢 27%~127%，且量化器还高 14%~38%（见 [`choose_codec`] 的表格）。
    /// 所以 VP8 只作为**试验**，是否保留由 [`vp8_trial_worth_keeping`] 的
    /// 实测对比决定。
    pub const VP9_MAX_CPU_USED: u32 = 9;
}

// ── 编码成本自适应（可单测部分）──
//
// 决策依据只有「实测编码耗时」与「帧预算」，不含任何机器型号 / 核数判断。
// 这样同一份代码在高性能机器上永远不触发，在编不动的机器上会自动自救。

/// 编码耗时占帧预算的比例超过此值即视为「跟不上」。
///
/// 取 0.9 而不是 1.0：留 10% 余量吸收调度抖动。
/// 用 1.0 的话，实测 30fps 下编码 30ms 会被判定为「还行」，
/// 但加上采集 5ms + 转换 2ms 就已经超预算了 —— 判据必须留出余量。
pub const ENCODER_BUDGET_RATIO: f64 = 0.9;

/// 编码是否已经成为瓶颈，且整帧已经做不完（纯函数，便于单测）。
///
/// 两个条件缺一不可：
///
/// 1. **编码必须主导**（`enc > cap`）：如果采集本身就要 50ms（GDI BitBlt），
///    那么无论把编码降到多快都不可能跑到 30fps，降级只会白白牺牲画质。
///    真实日志里就有这种误判：`cap=50.3ms enc=31.8ms` 被判为「编码超预算」
///    并降了一级 —— 但瓶颈明明在采集。
/// 2. **整帧超预算**（`enc + conv > spf`）：转换是编码的附属开销，
///    必须一起计入。单独看 `enc > 0.9 × spf` 会把「刚好跑满」判成「超了」。
///
/// `budget_ms <= 0` 或数值无效时返回 `false`：宁可不降级也不误降。
pub fn encoder_behind_budget(
    enc_avg_ms: f64,
    cap_avg_ms: f64,
    conv_avg_ms: f64,
    budget_ms: f64,
) -> bool {
    if budget_ms <= 0.0 || !enc_avg_ms.is_finite() || !budget_ms.is_finite() {
        return false;
    }
    // 采集比编码还慢 → 瓶颈不在编码器
    if enc_avg_ms <= cap_avg_ms {
        return false;
    }
    enc_avg_ms + conv_avg_ms > budget_ms * ENCODER_BUDGET_RATIO
}

/// 「严重超预算」的倍数：整帧耗时超过预算这么多倍时，**一个统计窗口**
/// 就足以定论，不必等连续多个窗口。
///
/// 作用是缩短自救时间。正常情况下要求连续
/// 3 个 5s 窗口（15s）才降一级，是为了避免把拖窗/播视频的瞬时高峰
/// 误判成「永久编不动」；但当整帧耗时已经超过预算 1.5 倍时，那不是抖动，
/// 而是这一档配置根本不可行 —— 在编不动的机器上，硬等 15s 才降一级、
/// 45s 才走完阶梯，用户会先失去耐心。
pub const ENCODER_SEVERE_RATIO: f64 = 1.5;

/// 整帧耗时是否**严重**超出预算（纯函数，便于单测）。
///
/// 只回答「超得有多狠」，是否算「编码落后」由
/// [`encoder_behind_budget`] 判定 —— 后者会先排除「瓶颈在采集」的情形。
pub fn overload_is_severe(enc_avg_ms: f64, conv_avg_ms: f64, budget_ms: f64) -> bool {
    if !(budget_ms > 0.0) || !budget_ms.is_finite() {
        return false;
    }
    if !enc_avg_ms.is_finite() || !conv_avg_ms.is_finite() {
        return false;
    }
    enc_avg_ms + conv_avg_ms > budget_ms * ENCODER_SEVERE_RATIO
}

/// 编码成本降级阶梯：0 = 默认；1 = 最快的 `cpu-used`；2 = 改用 VP8（**试验**）。
///
/// 返回 `None` 表示已到底或无法再降（对端解不了 VP8 / 已实测 VP8 无用）。
/// 顺序按「画质损失从小到大」排列，先试代价小的。
///
/// 第 2 级是**试验性**的：实测表明在 2 线程及以上 VP8 反而更慢
/// （见 [`choose_codec`] 的表格），所以「换 VP8」必须由数据决定去留 ——
/// 见 [`vp8_trial_worth_keeping`]。
pub fn next_enc_level(cur: u8, peer_vp8: bool) -> Option<u8> {
    match cur {
        0 => Some(1),
        1 if peer_vp8 => Some(2),
        _ => None,
    }
}

/// 「VP8 试验」要保留下来所需的改善幅度。
///
/// 换 VP8 是有代价的：同码率下量化器高 14%~39%，文字边缘更糊。所以只在
/// 编码耗时**明显**下降时才值得保留。取 0.85 表示「至少快 15%」：
/// 低于这个幅度，画质损失换来的流畅度提升用户根本看不出来。
pub const VP8_TRIAL_KEEP_RATIO: f64 = 0.85;

/// 换到 VP8 之后，实测耗时有没有真的下降（纯函数，便于单测）。
///
/// - `baseline_ms`：**换之前**那一档的实测每帧编码耗时；
/// - `trial_ms`：当前（VP8）的实测每帧编码耗时。
///
/// 数据无效（`baseline_ms <= 0` 或非有限值）时返回 `true`，即**保留**
/// VP8 —— 宁可不回退，也不要拿垃圾数据把正在工作的配置换掉。
pub fn vp8_trial_worth_keeping(baseline_ms: f64, trial_ms: f64) -> bool {
    if !(baseline_ms > 0.0) || !baseline_ms.is_finite() {
        return true;
    }
    if !trial_ms.is_finite() {
        return true;
    }
    trial_ms < baseline_ms * VP8_TRIAL_KEEP_RATIO
}

// ── 链路预算判据（可单测部分）──

/// RTT 低于此值才可能被判为充裕链路
pub const GOOD_LINK_RTT_MS: u32 = 50;

/// 排队延迟低于此值才可能被判为充裕链路。
///
/// 只用 RTT 作判据有一个真实漏洞：**低延迟 Wi-Fi / 小区宽带**同样只有
/// 5-30ms，会被判成好链路，但它们的实际容量远不如有线内网。
/// 排队延迟 = `raw - RTT 下界`，恒为正，直接反映本地出口是否积压，
/// 与路径长短无关 —— 比 RTT 更物理。
pub const GOOD_LINK_QUEUE_MS: u32 = 40;

/// 链路是否充裕：RTT 低 **且** 没有排队。
///
/// 两个条件都必需，且**任一未知时都判为保守**：
/// 连接初期还没测到样本，宁可少给码率也不能无节制超发
/// （实测超发会在中继链路上把 RTT 从 34ms 推到 211ms）。
pub fn link_is_generous(rtt: Option<u32>, queue: Option<u32>) -> bool {
    matches!(
        (rtt, queue),
        (Some(r), Some(q)) if r < GOOD_LINK_RTT_MS && q < GOOD_LINK_QUEUE_MS
    )
}

// ── 链路档位迟滞（可单测部分）──
//
// 「链路是否充裕」直接决定两件事：码率乘不乘 [`SosVpxConfig::GOOD_LINK_BITRATE_GAIN`]
// （2 倍），以及 `rc_dropframe_thresh` 是 0 还是 20。用**单个探测样本**直接翻转
// 档位是危险的，实测证据（生产日志）：
//
// ```text
// 211.291  peer requested refresh of display 0      ← 控制端主线程正忙
// 211.464  重建编码器 → bitrate=2604kbps            ← 1736 × 1.5，Constrained
// 213.036  ratio 1.50 ... -> 5208kbps dropframe 0    ← 两秒后又回到 Generous
// 214.174  3/133 frames dropped (2%)                 ← 这一秒编码器在丢帧
// ```
//
// 码率本该是 5208，却在重建瞬间被一个迟到回显（`queue >= 40ms`）砍到 2604，
// 并且顺手打开了丢帧。同时反方向的抖动同样有害：一个「时延漂亮」的样本就把
// 码率翻倍，会在真实拥塞的链路上养出 bufferbloat。
//
// 所以档位必须带迟滞：进档慢（连续确认），出档也不能靠单样本。

/// 进入 Generous 所需的连续好样本数。
///
/// 每个控制端 1Hz 探测一次，所以单连接时 5 个样本 = **5 秒**（多连接时
/// 样本是共享的，N 个控制端会把窗口缩短到 5/N 秒 —— 这只会让进档更快，
/// 方向仍是保守的）。进档是唯一可能超发的动作（码率 ×2、关掉丢帧保护），
/// 必须确认链路真的稳定。代价是连接建立后前 5 秒走保守预算
/// （2604kbps 而不是 5208kbps），这是刻意选择的「宁可先糊 5 秒」。
pub const LINK_GOOD_SAMPLES_TO_UPGRADE: u32 = 5;

/// 退出 Generous 所需的连续坏样本数。
///
/// 取 2，与 [`crate::sos_qos`] 的 `BAD_SAMPLES_TO_REDUCE` 对齐：真实拥塞会
/// 连续出现（把路由缓冲填满需要时间），而单个迟到回显不足以推翻档位。
/// 这正好修掉上面那段日志里的「重建瞬间被砍半」。
/// 多连接共享计数时，两个不同控制端的迟到回显也会合成一次出档 ——
/// 出档是保守方向，误判的代价只是短暂少给码率。
pub const LINK_BAD_SAMPLES_TO_DOWNGRADE: u32 = 2;

/// 链路档位的迟滞状态机。
///
/// 只有 [`Self::sample`] 会改变档位，读侧用 [`Self::generous`] 取当前值。
/// 初始为**保守**（Constrained）：还没有任何样本时不能超发。
#[derive(Debug, Clone, Copy)]
pub struct LinkTier {
    generous: bool,
    good: u32,
    bad: u32,
}

impl LinkTier {
    /// 保守初始状态（Constrained）
    pub const fn new() -> Self {
        Self {
            generous: false,
            good: 0,
            bad: 0,
        }
    }

    /// 当前是否处于 Generous 档位
    pub fn generous(&self) -> bool {
        self.generous
    }

    /// 回到保守初始状态。
    ///
    /// 生产路径在**第一个控制端接入**时调用（见 `sos_video::subscribe`）：
    /// 上一次会话测出来的「链路充裕」不能继承给一条全新的连接 —— 它可能
    /// 走的是完全不同的网络，而 Generous 档位会把码率翻倍并关掉丢帧保护。
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// 喂入**一次探测**样本，返回档位是否发生变化。
    ///
    /// 一次 `TestDelay` 往返只能调用一次 —— 迟滞的语义是「连续 N 次探测」，
    /// 拆成两个 setter 各喂一次会把 N 秒压缩成 N/2 秒。
    pub fn sample(&mut self, rtt: Option<u32>, queue: Option<u32>) -> bool {
        if link_is_generous(rtt, queue) {
            self.bad = 0;
            if self.generous {
                self.good = 0;
                return false;
            }
            self.good += 1;
            if self.good >= LINK_GOOD_SAMPLES_TO_UPGRADE {
                self.generous = true;
                self.good = 0;
                return true;
            }
            return false;
        }
        // 坏样本（含「样本未知」）：进档计数清零
        self.good = 0;
        if !self.generous {
            self.bad = 0;
            return false;
        }
        self.bad += 1;
        if self.bad >= LINK_BAD_SAMPLES_TO_DOWNGRADE {
            self.generous = false;
            self.bad = 0;
            return true;
        }
        false
    }
}

impl Default for LinkTier {
    fn default() -> Self {
        Self::new()
    }
}

/// libvpx 的结构体含有 union / 保留字段，`mem::zeroed()` 在部分版本会触发
/// `invalid_value` 检查而直接 panic。`MaybeUninit::zeroed().assume_init()` 与
/// `libs/scrap` 的做法一致，不触发该检查。
#[inline]
unsafe fn zeroed_unchecked<T>() -> T {
    std::mem::MaybeUninit::zeroed().assume_init()
}

/// 求出 libvpx 在 `scrap::STRIDE_ALIGN` 下的 I420 内存布局。
///
/// 做法与 `libs/scrap` 的 `VpxEncoder::get_yuvfmt` 一致：用假指针 wrap 一次，
/// 读取 libvpx 自己算出的 stride 与平面偏移，避免手写对齐公式出错。
fn calc_yuvfmt(width: usize, height: usize) -> scrap::EncodeYuvFormat {
    unsafe {
        let mut img: vpx_image_t = zeroed_unchecked();
        vpx_img_wrap(
            &mut img,
            vpx_img_fmt_t::VPX_IMG_FMT_I420,
            width as c_uint,
            height as c_uint,
            scrap::STRIDE_ALIGN as c_uint,
            1 as *mut u8,
        );
        scrap::EncodeYuvFormat {
            pixfmt: scrap::Pixfmt::I420,
            w: img.w as usize,
            h: img.h as usize,
            stride: vec![
                img.stride[0] as usize,
                img.stride[1] as usize,
                img.stride[2] as usize,
            ],
            u: img.planes[1] as usize - img.planes[0] as usize,
            v: img.planes[2] as usize - img.planes[0] as usize,
        }
    }
}

/// 紧凑 I420 编码器
pub struct SosVpxEncoder {
    ctx: vpx_codec_ctx_t,
    width: usize,
    height: usize,
    codec: SosCodec,
    pts: i64,
    /// 与 `scrap::convert_to_yuv` 输出完全一致的布局（64 字节对齐）
    yuvfmt: scrap::EncodeYuvFormat,
    /// 每帧所需的 YUV 缓冲区大小
    frame_len: usize,
    /// 因帧率下采样（`rc_dropframe_thresh`）被丢掉的帧数。
    ///
    /// libvpx 丢帧时不会产出帧包，因此只能靠「输入帧数 vs 输出包数」统计。
    /// 这个计数是判断「画面卡顿是网络/编码问题还是编码器主动降帧」的唯一依据。
    /// 上一帧的拷贝，用于变化检测
    dropped_frames: usize,
    /// 下一帧是否强制编为关键帧
    force_kf: bool,
}

// vpx_codec_ctx_t 内部只保存指向堆上分配的状态，安全使用由调用方串行化保证
unsafe impl Send for SosVpxEncoder {}

impl SosVpxEncoder {
    pub fn new(cfg: SosVpxConfig) -> ResultType<Self> {
        if cfg.width == 0 || cfg.height == 0 {
            bail!("invalid encoder size {}x{}", cfg.width, cfg.height);
        }
        let width = cfg.width & !1; // I420 需要偶数宽高
        let height = cfg.height & !1;
        let threads = if cfg.threads == 0 {
            SosVpxConfig::recommended_threads()
        } else {
            cfg.threads
        };

        unsafe {
            let iface = cfg.codec.iface();
            if iface.is_null() {
                bail!("libvpx encoder interface unavailable for {:?}", cfg.codec);
            }

            let mut c: vpx_codec_enc_cfg_t = zeroed_unchecked();
            let err = vpx_codec_enc_config_default(iface, &mut c, 0);
            if err != vpx_codec_err_t::VPX_CODEC_OK {
                bail!("vpx_codec_enc_config_default failed: {:?}", err);
            }

            c.g_w = width as c_uint;
            c.g_h = height as c_uint;
            c.g_timebase.num = 1;
            c.g_timebase.den = 1000;
            // —— 这是本模块存在的理由：线程数由我们自己决定 ——
            c.g_threads = threads as c_uint;
            // 关闭错误恢复模式。
            //
            // `VPX_ERROR_RESILIENT_DEFAULT` 会限制帧间预测的引用方式，
            // 以便接收端在某帧丢失后仍能继续解码——代价是**显著更高的码率**。
            // 我们的传输层是 TCP（中继也是 TCP 转发），不存在丢包乱序，
            // 这个保护纯属浪费。实测开启时 1080p@30fps 跑出 6753kbps，
            // 是目标值 1675 的 4 倍。
            c.g_error_resilient = 0;
            // 关键：必须关闭前瞻缓冲，否则编码器会攒满 g_lag_in_frames 才吐包。
            // 上游 `scrap` 没设这个值（VP9 默认 25），靠每帧 `flush()` 强行逼出数据，
            // 代价是每帧都损失帧间优化——这正是本模块要绕开的瓶颈。
            c.g_lag_in_frames = 0;
            c.rc_end_usage = rc_mode();
            c.rc_target_bitrate = cfg.bitrate_kbps as c_uint;
            // 码率控制缓冲。单位是**毫秒**，不是比特（见 vpx_encoder.h 的注释：
            // "This value is expressed in units of time (milliseconds)"）。
            //
            // 原实现写的是 `rc_target_bitrate * 5`，在 1675kbps 下等于 **8375 毫秒**，
            // 也就是允许编码器在 8 秒窗口内任意突发。实测后果：
            // `sent_kbps` 冲到 2133（目标 1675），发送队列积压到 25 帧（约 0.8 秒延迟）。
            //
            // 远程桌面要求低延迟，缓冲必须短。200ms 足以吸收拖窗造成的瞬时复杂度，
            // 又不至于让码率长期超发。
            let (buf_ms, drop_thresh) = rc_params();
            let (buf_init, buf_opt) = rc_buffer_derived(buf_ms);
            c.rc_buf_sz = buf_ms;
            c.rc_buf_initial_sz = buf_init;
            c.rc_buf_optimal_sz = buf_opt;
            c.rc_undershoot_pct = 95;
            // 超发上限：**默认值是 100，即允许超发一倍**，这是本文件里
            // 最容易被忽略的坑。语义与取值权衡见 [`RC_OVERSHOOT_PCT`]。
            c.rc_overshoot_pct = overshoot_pct();
            // 量化器上下限。
            //
            // 默认 `rc_min_quantizer = 0` 意味着编码器可以一路降到近乎无损，
            // 遇到高频细节（照片、视频、渐变噪声）时即使严重超发也「符合规则」——
            // 实测在不可压缩内容上产出 340% 的目标码率。
            // 抬到 4 给码率控制留出真正的调节余量，画质损失几乎不可见。
            c.rc_min_quantizer = 4;
            // VP9 的 Q 索引有效范围是 0-63。取满上限，让码率控制在极端内容上
            // 仍有下压空间——否则目标码率会彻底失效（实测 QP 上限设 56 时，
            // 目标 800kbps 与 3200kbps 的产出完全相同）。
            //
            // VP8 也是 0-63：实测设 127 会直接返回 `VPX_CODEC_INVALID_PARAM`。
            c.rc_max_quantizer = 63;
            // 禁止编码器自行缩放分辨率。远程桌面的像素尺寸必须与显示一致，
            // 否则控制端收到的是拉伸过的画面，文字会糊。
            c.rc_resize_allowed = 0;
            // 帧率下采样的开关：允许编码器丢帧，但尽量不丢。
            //
            // 语义（vpx_encoder.h）：这是**目标缓冲充盈度的百分比**，
            // 当缓冲低于这个百分比时编码器才会丢一帧；0 表示彻底禁用。
            // 也就是说**值越小越保守**——只有缓冲快见底时才丢。
            //
            // 之前设 0（永不丢帧）看起来「最保真」，实际适得其反：
            // 遇到压不下来又必须出帧的内容（照片、视频、渐变），
            // 编码器唯一的出路就是疯狂超发（实测 4 倍目标码率），
            // 在中继链路上养出 bufferbloat，RTT 从 34ms 涨到 211ms，
            // QoS 再把帧率一路压到 2fps —— 用户看到的是「卡死 + 延迟爆炸」。
            //
            // 反过来，丢几帧换来码率守约、延迟稳定，体验明显更好。
            // 具体水位见 [`RC_DROPFRAME_THRESH`]。
            c.rc_dropframe_thresh = drop_thresh;
            match cfg.keyframe_interval {
                Some(n) => {
                    // 必须显式设回 AUTO：默认值虽然是 AUTO，但同一个搞制器
                    // 重建时若漏了这一步就会继承上一次的 DISABLED。
                    c.kf_mode = vpx_kf_mode::VPX_KF_AUTO;
                    c.kf_min_dist = 0;
                    c.kf_max_dist = n as c_uint;
                }
                None => c.kf_mode = vpx_kf_mode::VPX_KF_DISABLED,
            }

            let mut ctx: vpx_codec_ctx_t = zeroed_unchecked();
            let err =
                vpx_codec_enc_init_ver(&mut ctx, iface, &c, 0, VPX_ENCODER_ABI_VERSION as c_int);
            if err != vpx_codec_err_t::VPX_CODEC_OK {
                bail!("vpx_codec_enc_init_ver failed: {:?}", err);
            }

            match cfg.codec {
                SosCodec::Vp9 => {
                    let _ = vpx_codec_control_(
                        &mut ctx,
                        vp8e_enc_control_id::VP8E_SET_CPUUSED as c_int,
                        cfg.cpu_used as c_int,
                    );
                    // 行级多线程 + 4 列 tile，充分利用多核
                    let _ = vpx_codec_control_(
                        &mut ctx,
                        vp8e_enc_control_id::VP9E_SET_ROW_MT as c_int,
                        1 as c_int,
                    );
                    let _ = vpx_codec_control_(
                        &mut ctx,
                        vp8e_enc_control_id::VP9E_SET_TILE_COLUMNS as c_int,
                        4 as c_int,
                    );
                    // 仅诊断路径：见 [`set_tune_content_override`]。
                    if let Some(tune) = TUNE_CONTENT_OVERRIDE.with(|c| c.get()) {
                        let _ = vpx_codec_control_(
                            &mut ctx,
                            vp8e_enc_control_id::VP9E_SET_TUNE_CONTENT as c_int,
                            tune,
                        );
                    }
                    // 仅诊断路径：见 [`set_intra_bitrate_override`]。
                    if let Some(pct) = INTRA_BITRATE_OVERRIDE.with(|c| c.get()) {
                        let _ = vpx_codec_control_(
                            &mut ctx,
                            vp8e_enc_control_id::VP8E_SET_MAX_INTRA_BITRATE_PCT as c_int,
                            pct,
                        );
                    }
                }
                SosCodec::Vp8 => {
                    let _ = vpx_codec_control_(
                        &mut ctx,
                        vp8e_enc_control_id::VP8E_SET_CPUUSED as c_int,
                        cfg.cpu_used as c_int,
                    );
                    // 仅诊断路径：见 [`set_intra_bitrate_override`]。
                    if let Some(pct) = INTRA_BITRATE_OVERRIDE.with(|c| c.get()) {
                        let _ = vpx_codec_control_(
                            &mut ctx,
                            vp8e_enc_control_id::VP8E_SET_MAX_INTRA_BITRATE_PCT as c_int,
                            pct,
                        );
                    }
                }
            }

            log::info!(
                "SosVpxEncoder: {:?} {}x{} threads={} cpu_used={} bitrate={}kbps",
                cfg.codec,
                width,
                height,
                threads,
                cfg.cpu_used,
                cfg.bitrate_kbps
            );

            // 求出 libvpx 在 STRIDE_ALIGN 下使用的内存布局，
            // 供 `scrap::convert_to_yuv` 按同一布局写入。
            let yuvfmt = calc_yuvfmt(width, height);
            // 布局为 Y/U/V 三个平面顺序排列，末端取三个平面末尾的最大值
            let half_h = yuvfmt.h / 2;
            let frame_len =
                (yuvfmt.u + half_h * yuvfmt.stride[1]).max(yuvfmt.v + half_h * yuvfmt.stride[2]);
            log::debug!(
                "SosVpxEncoder layout: stride={:?} u={} v={} frame_len={}",
                yuvfmt.stride,
                yuvfmt.u,
                yuvfmt.v,
                frame_len
            );

            Ok(Self {
                ctx,
                width,
                height,
                codec: cfg.codec,
                pts: 0,
                yuvfmt,
                frame_len,
                dropped_frames: 0,
                force_kf: false,
            })
        }
    }

    /// 与 `SosVpxEncoder::encode` 期望的输入匹配的 YUV 布局。
    /// 传入给 `scrap::convert_to_yuv` / `Frame::to` 即可直接喂给本编码器。
    pub fn yuvfmt(&self) -> scrap::EncodeYuvFormat {
        self.yuvfmt.clone()
    }

    /// 编码一帧 YUV（布局必须与 `yuvfmt()` 一致，即经 `scrap::convert_to_yuv` 转换输出）
    ///
    /// 返回 `(压缩数据, 是否关键帧)` 列表。
    pub fn encode(&mut self, i420: &[u8], ms: i64) -> ResultType<Vec<(Vec<u8>, bool)>> {
        if i420.len() < self.frame_len {
            bail!(
                "yuv frame too small: {} < {} ({}x{}, stride {:?})",
                i420.len(),
                self.frame_len,
                self.width,
                self.height,
                self.yuvfmt.stride
            );
        }
        unsafe {
            let mut img: vpx_image_t = zeroed_unchecked();
            let ptr = vpx_img_wrap(
                &mut img,
                vpx_img_fmt_t::VPX_IMG_FMT_I420,
                self.width as c_uint,
                self.height as c_uint,
                scrap::STRIDE_ALIGN as c_uint,
                i420.as_ptr() as *mut u8,
            );
            if ptr.is_null() {
                bail!("vpx_img_wrap failed");
            }
            // vpx_img_wrap 只借用输入缓冲，不会接管所有权（仅 vpx_img_free 会释放）
            img.img_data = std::ptr::null_mut();

            self.pts = ms;
            // `kf_mode = VPX_KF_DISABLED` 意味着除了首帧再也不会自动出关键帧。
            // 一旦链路上丢了一个编码帧，解码端的参考链就断了，必须靠强制
            // 关键帧重新同步。
            let flags = if std::mem::take(&mut self.force_kf) {
                VPX_EFLAG_FORCE_KF as vpx_enc_frame_flags_t
            } else {
                0
            };
            let err = vpx_codec_encode(
                &mut self.ctx,
                &img,
                self.pts as vpx_codec_pts_t,
                1,
                flags,
                VPX_DL_REALTIME as vpx_enc_deadline_t,
            );
            if err != vpx_codec_err_t::VPX_CODEC_OK {
                bail!("vpx_codec_encode failed: {:?}", err);
            }
            let out = self.collect();
            // 丢帧时 libvpx 不产出任何帧包
            if out.is_empty() {
                self.dropped_frames += 1;
            }
            Ok(out)
        }
    }

    /// 冲刷编码器内部缓冲。
    ///
    /// 注意：常规编码**不应**每帧调用（那正是上游 `scrap` 的瓶颈所在），
    /// 仅在停止推流前冲刷一次即可。
    #[allow(dead_code)]
    pub fn flush(&mut self) -> ResultType<Vec<(Vec<u8>, bool)>> {
        unsafe {
            let err = vpx_codec_encode(
                &mut self.ctx,
                std::ptr::null(),
                0,
                1,
                0,
                VPX_DL_REALTIME as vpx_enc_deadline_t,
            );
            if err != vpx_codec_err_t::VPX_CODEC_OK {
                bail!("vpx_codec_encode(flush) failed: {:?}", err);
            }
            Ok(self.collect())
        }
    }

    unsafe fn collect(&mut self) -> Vec<(Vec<u8>, bool)> {
        let mut out = Vec::new();
        let mut iter: vpx_codec_iter_t = std::ptr::null();
        loop {
            let pkt = vpx_codec_get_cx_data(&mut self.ctx, &mut iter);
            if pkt.is_null() {
                break;
            }
            if (*pkt).kind != vpx_codec_cx_pkt_kind::VPX_CODEC_CX_FRAME_PKT {
                continue;
            }
            let frame = (*pkt).data.frame;
            if frame.buf.is_null() || frame.sz == 0 {
                continue;
            }
            let key = (frame.flags as u32) & VPX_FRAME_IS_KEY != 0;
            out.push((
                std::slice::from_raw_parts(frame.buf as *const u8, frame.sz as usize).to_vec(),
                key,
            ));
        }
        out
    }

    /// 运行中调整码率（仅测试用）。
    ///
    /// 等价于 [`Self::tune`] 只改码率、丢帧水位保持当前值。
    /// 生产路径只用 `tune()`，它在同一次 `vpx_codec_enc_config_set`
    /// 里把码率与丢帧水位一起下发，避免中间出现不一致的状态。
    #[allow(dead_code)]
    pub fn set_bitrate(&mut self, kbps: u32) -> ResultType<()> {
        let thresh = unsafe { (*self.ctx.config.enc).rc_dropframe_thresh };
        self.tune(kbps, thresh)
    }

    /// 运行中同时调整码率与丢帧水位。
    ///
    /// 丢帧水位必须能运行时改：它在「链路充裕时不丢帧」和
    /// 「链路拥塞时守码率」之间切换（见 `sos_video` 的 `GOOD_LINK_RTT_MS`）。
    pub fn tune(&mut self, kbps: u32, drop_thresh: c_uint) -> ResultType<()> {
        unsafe {
            let mut c = *self.ctx.config.enc;
            c.rc_target_bitrate = kbps as c_uint;
            c.rc_dropframe_thresh = drop_thresh;
            // 缓冲时长与目标码率无关，保持固定的毫秒值（见构造函数处的说明）
            let (buf_ms, _) = rc_params();
            let (buf_init, buf_opt) = rc_buffer_derived(buf_ms);
            c.rc_buf_sz = buf_ms;
            c.rc_buf_initial_sz = buf_init;
            c.rc_buf_optimal_sz = buf_opt;
            let err = vpx_codec_enc_config_set(&mut self.ctx, &c);
            if err != vpx_codec_err_t::VPX_CODEC_OK {
                bail!("vpx_codec_enc_config_set failed: {:?}", err);
            }
        }
        Ok(())
    }

    /// 上一帧实际使用的量化器（**0-63**，越大越糊）。
    ///
    /// 这是判断「码率是否把画质压死」的**直接证据**：
    /// 码率充裕时编码器会在低量化器上工作，细节得以保留；
    /// 码率不够时它会一路推到 `rc_max_quantizer` 上限，画面变成色块。
    /// 光看「实际码率 = 目标码率」无法区分这两种情况，
    /// 因为 CBR 恰好会在压不住时放弃守住目标（实测 2.19 倍超发）。
    ///
    /// 必须用 `_64` 变体：`VP8E_GET_LAST_QUANTIZER` 返回的是编解码器
    /// **内部刻度**（VP8 0-127、VP9 0-255），与 `rc_*_quantizer` 的
    /// 0-63 不是同一个单位，直接比较会得出相反的结论。
    ///
    /// 仅用于测试/诊断，生产路径不读量化器。
    #[allow(dead_code)]
    pub fn quantizer(&mut self) -> Option<i32> {
        let mut q: c_int = 0;
        let err = unsafe {
            vpx_codec_control_(
                &mut self.ctx,
                vp8e_enc_control_id::VP8E_GET_LAST_QUANTIZER_64 as c_int,
                &mut q as *mut c_int,
            )
        };
        if err == vpx_codec_err_t::VPX_CODEC_OK {
            Some(q)
        } else {
            None
        }
    }

    pub fn codec(&self) -> SosCodec {
        self.codec
    }

    /// 取出并清零「被编码器主动丢掉」的帧数
    pub fn take_dropped_frames(&mut self) -> usize {
        std::mem::take(&mut self.dropped_frames)
    }

    /// 要求下一帧编为关键帧（用于丢失参考帧后的重同步）
    pub fn force_keyframe(&mut self) {
        self.force_kf = true;
    }

    /// 当前目标码率（kbps）
    pub fn bitrate_kbps(&self) -> u32 {
        unsafe { (*self.ctx.config.enc).rc_target_bitrate as u32 }
    }
}

impl Drop for SosVpxEncoder {
    fn drop(&mut self) {
        unsafe {
            let _ = vpx_codec_destroy(&mut self.ctx);
        }
    }
}
