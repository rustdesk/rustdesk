//! SOS 自建视频链路：采集 → 色度转换 → 编码 → 发送。
//!
//! 替代上游 `video_service`，目的是绕开 `libs/scrap` 的两个硬限制：
//!
//! 1. `codec_thread_num()` 把线程数钳到 `min(cpu, available_memory/2)`，
//!    内存受限环境（VM）只剩 1 个编码线程；
//! 2. `VpxEncoder::encode_to_message()` **每帧都 flush**，让 VP9 放弃帧间优化。
//!
//! 上下游对比（1080p 高细节、cpu_used=7、1388kbps）：
//! - 上游：1 线程 + 每帧 flush → 约 10 fps
//! - 本模块：1 线程不 flush → 约 35 fps；4 线程 → 约 100 fps
//!
//! 保持不变的部分（复用上游公开 API）：
//! - `sos_qos`：帧率 / 画质 / 码率预算的状态机（自建，见模块文档）
//! - 光标：Windows 下 `is_cursor_embedded()` 恒为 false，光标由
//!   `input_service::new_cursor()` 独立发送，本模块不参与
//! - `RustDesk` 的 `VideoFrame`/`SwitchDisplay` 协议格式

#![cfg(all(windows, sos_vpx))]

use crate::sos_vpx::{CodecPrefer, SosCodec, SosVpxConfig, SosVpxEncoder};
use hbb_common::message_proto::{
    message, DisplayInfo, EncodedVideoFrame, EncodedVideoFrames, Message, VideoFrame,
};
use hbb_common::ResultType;
use scrap::{Display, TraitCapturer};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// 与 `sos_connection` 的 `(Instant, Arc<Message>)` 通道类型一致
pub type Sender = tokio::sync::mpsc::UnboundedSender<(tokio::time::Instant, Arc<Message>)>;
/// 视频帧通道：有界，用于背压（容量见 [`VIDEO_CHANNEL_CAPACITY`]）。
pub type VideoSender = tokio::sync::mpsc::Sender<(tokio::time::Instant, Arc<Message>)>;

// ════════════════════════════════════════════════════════════════════════
// 调优常量
//
// 所有可调数值集中在这里，按作用域分组。命名约定：
//
// | 形式                                        | 含义                       |
// |---------------------------------------------|----------------------------|
// | `*_INTERVAL`                                | 周期性动作的间隔           |
// | `*_DELAY`                                   | 单次等待（重试、让环境恢复）|
// | `*_BACKOFF`                                 | 失败后的退避，可翻倍到 `MAX_*` |
// | `MAX_*` / `MIN_*`                           | 上界 / 下界                |
// | `*_MS` `*_FPS` `*_KBPS` `*_PCT` `*_FRAMES`  | 数值自带单位               |
//
// 取值来源：能实测的都来自实测（对应的测试文件名写在该常量的注释里），
// 其余是没有调优价值的硬性下限（例如 QoS 同步周期）。
// ════════════════════════════════════════════════════════════════════════

// ── 数据通道 ──

/// 视频帧通道容量。
///
/// 有界通道本身就是背压机制：写满时捕获线程在 `blocking_send` 上阻塞，
/// 自动降帧，而不是把帧堆在内存里。2 帧足够吸收网络抖动。
///
/// 上游是「取帧即认为发送完成」（`video_ack_required = false` 时）再加一个
/// 可选的 `VideoReceived` 确认；这里用通道容量代替，语义更简单，
/// 也不依赖控制端行为。
///
/// 公开给 `sos_connection` 用来创建通道。
pub const VIDEO_CHANNEL_CAPACITY: usize = 2;

// ── 采集循环节拍 ──

/// 检查桌面切换（consent.exe）的间隔。
/// `is_process_consent_running` 需要枚举进程，代价较高，不能每帧都调。
const UAC_CHECK_INTERVAL: Duration = Duration::from_millis(1000);

/// 检查显示器尺寸/数量变化的间隔
const DISPLAY_CHECK_INTERVAL: Duration = Duration::from_millis(1000);

/// 把 QoS 的聚合结果（帧率 / 画质比 / 码率）同步给编码器的间隔。
///
/// 编码器的 `tune()` 只在数值真正变化时才下发，所以 1 秒轮询几乎没有成本；
/// 更长的间隔会让码率对窗口拖动这类突发负载反应过慢。
const QOS_SYNC_INTERVAL: Duration = Duration::from_secs(1);

/// `[VIDEO:PERF]` 分阶段耗时的统计与输出周期。
///
/// 5 秒窗口既能摊平单帧抖动，又能让日志保持可读：30fps 下每秒一条太多，
/// 每分钟 12 条正好能看清趋势。
const PERF_LOG_INTERVAL: Duration = Duration::from_secs(5);

/// 采集帧率下限。
///
/// QoS 的帧率下限是 [`crate::sos_qos::MIN_FPS`]（同为 8）。这里再兜一层，
/// 是因为采集循环不应当假设 QoS 一定正确 —— `spf` 万一下发异常值，
/// 至少不会把采集拖成零间隔忙循环。
const MIN_CAPTURE_FPS: u32 = 8;

/// 目标码率变化超过该百分比才重新配置编码器。
///
/// `tune()` 会重置码控的缓冲状态，为几个 kbps 的风吹草动重配反而让码率更不稳。
const BITRATE_CHANGE_PCT: u64 = 8;

/// 单次统计窗口内至少要有这么多帧编码样本，才判定编码器是否超预算。
/// 画面长时间静止时编码样本很少，均值没有统计意义。
const MIN_ENC_SAMPLES: u64 = 10;

// ── 关键帧 ──

/// 强制关键帧的最小间隔。
///
/// 关键帧约为 P 帧的 **9.2 倍**大（`tests/vpx_keyframe.rs` 实测）。
/// 发送队列溢出时会连续丢帧，每丢一帧就请求一个关键帧，于是拥塞时
/// 反而向已经堵住的链路里灌入成倍的突发流量 —— 实测 `sendq=9 drop=41`
/// 的同时 `raw rtt` 飙到 1439ms，正是这个正反馈。
///
/// 1.5 秒上限（最坏 0.67 个/秒）足以在对端花屏后很快恢复，
/// 额外码率开销不到 15%。
const MIN_KEYFRAME_INTERVAL: Duration = Duration::from_millis(1500);

/// 周期性关键帧间隔（帧数）。
///
/// 30fps 下等于 **2 秒**。这是一条自愈基线：即使既没收到控制端的
/// 刷新请求、也没检测到丢帧，对端最多卡 2 秒也会自动重同步。
///
/// # 为什么从 150（5 秒）改到 60
///
/// 关键帧越密，对端解码出错 / 发送队列丢帧后的恢复越快，而且每次恢复
/// 都落在一个「新鲜」的参考点上。代价是码率，必须配套
/// [`crate::sos_vpx`] 的 `RC_BUFFER_MS`（已同时改为 1000ms），
/// 否则关键帧会被压到量化器上限 —— 那样只是「更频繁地糊一次」。
///
/// 实测（`tests/vpx_keyframe_budget.rs`，1600x900 桌面 @2326kbps，buf=1000）：
///
/// | 间隔 | 关键帧字节 | 关键帧量化器 | 实际码率 | 目标 2326 |
/// |---|---|---|---|---|
/// | 30 帧(1s) | 96,910 | 44.8 | 3112 | **+34%** |
/// | **60 帧(2s)** | **70,320** | **54.0** | **2383** | **+2%** |
/// | 150 帧(5s) | — | — | 1006 | −57%（欠发） |
///
/// 30 帧虽然关键帧最清晰，但关键帧独占 25% 码率、总码率超发 34%，
/// 在有限带宽上是拿整帧画质换恢复速度，不划算。60 帧是拐点。
///
/// 注意它数的是**送进编码器的帧数**，不是墙钟时间：画面静止时采集循环
/// 不调用 `encode()`，这个计数就不推进。所以真正的恢复保证来自
/// [`request_keyframe`] 那条路径 —— 它在静止画面上也能生效
/// （见 [`should_reemit_retained`]）。
const KEYFRAME_INTERVAL_FRAMES: usize = 60;

/// 静止画面下是否要把**手上保留的那一帧**重编一次（纯函数，便于单测）。
///
/// 采集器在画面没变时返回「无新帧」，采集循环因此不会调用 `encode()`。
/// 这会让两件事**永远**得不到处理：
///
/// 1. `stale_pending`：上一帧被编码器为守码率丢掉了（`rc_dropframe_thresh`），
///    而画面恰好从此静止 —— 那次变化再也不会发给对端，控制端停在旧画面上。
/// 2. `kf_pending`：对端请求关键帧（它解码出错），或我们自己丢弃了已编码的
///    帧导致参考链断裂。请求只是置位，等下一帧生效；没有下一帧就是永久花屏。
///
/// 关键帧那条额外受 [`MIN_KEYFRAME_INTERVAL`] 节流：关键帧约 9.2 倍于 P 帧，
/// 拥塞时请求会连发，不节流就变成对已堵链路的突发。
fn should_reemit_retained(
    have_yuv: bool,
    stale_pending: bool,
    kf_pending: bool,
    since_last_keyframe: Duration,
) -> bool {
    if !have_yuv {
        // 手上没有可用的帧（刚重建编码器 / 还没成功转换过一帧）
        return false;
    }
    stale_pending || (kf_pending && since_last_keyframe >= MIN_KEYFRAME_INTERVAL)
}

// ── 采集后端 ──

/// 最多支持的显示器数量（位掩码宽度）。超过的部分退回显示器 0 的槽位。
const MAX_DISPLAY_SLOTS: usize = 64;

/// 采集后端优先级。只在「确认本档不可用」时下调，下调后定期回到最高档重试。
const TIER_DXGI: usize = 0;
const TIER_GDI: usize = 1;

/// 降级后重新探测最高档的间隔。
/// 驱动更新、会话重连、显示模式切换都可能让 DXGI 重新可用。
const RE_PROBE_INTERVAL: Duration = Duration::from_secs(30);

/// 重探测失败后的退避上限。
///
/// 每次重探测都要把捕获器和编码器整个重建（约 150ms），画面会卡一下，
/// 码率控制状态与 pts 时间轴也会清零。在不支持 DXGI 的机器上（本 VM 恒定
/// 返回 `E_UNEXPECTED 0x8000FFFF`），固定 30s 重试等于**每 30 秒主动卡一次
/// 而且永远徒劳** —— 实测日志里每 30s 就出现一次 `re-probing` +
/// `dxgi unavailable` + 编码器重建。所以失败就翻倍退避，成功才恢复 30s。
const MAX_RE_PROBE_INTERVAL: Duration = Duration::from_secs(600);

/// 同一个后端连续瞬时失败的次数上限，超过则降档。
/// 用不了永久失败判断的场景（比如某些系统上 `DuplicateOutput` 恒定返回 E_ACCESSDENIED）。
const MAX_TRANSIENT_FAILURES: u32 = 5;

// ── 重建与恢复 ──

/// 重建捕获器前的短暂停顿，给远端一点时间回到可捕获状态（比如刚退出 UAC）。
const REBUILD_SETTLE_DELAY: Duration = Duration::from_millis(50);

/// 重建失败后的初始退避，之后每次翻倍直到 [`MAX_REBUILD_BACKOFF`]。
const INITIAL_REBUILD_BACKOFF: Duration = Duration::from_secs(1);

/// 捕获器/编码器重建失败后的最大退避。
///
/// 失败最常见的原因是桌面暂时不可捕获（锁屏、UAC 安全桌面、RDP 断开、
/// 显示器正在重枚举）。实测这种状态可以持续 130 秒以上，期间旧实现
/// 每秒重试一次并打一条 ERROR。5 秒一次的空转对恢复速度没有实质影响
/// （用户能看到画面时已经过了几秒），但能让日志保持可读。
const MAX_REBUILD_BACKOFF: Duration = Duration::from_secs(5);

/// 瞬时采集错误后、重建前的等待。
const REBUILD_RETRY_DELAY: Duration = Duration::from_millis(100);

/// 降档前的等待，避免立刻用新档位去撞同一个尚未消散的错误状态。
const DEMOTE_SETTLE_DELAY: Duration = Duration::from_millis(200);

// ── 编码成本自适应 ──

/// 连续多少个统计窗口「编码超预算」才降级。
/// 3 个窗口 = 15s：拖窗/播视频的瞬时高峰不会误判，真的编不动时也能及时自救。
const ENC_OVERLOAD_WINDOWS: u32 = 3;

/// 该显示器的采集帧间隔（受 QoS 控制，但不低于 [`MIN_CAPTURE_FPS`]）
fn capture_spf(display: &str) -> Duration {
    let qos_spf = crate::sos_qos::spf(display);
    let floor = Duration::from_millis(1000 / MIN_CAPTURE_FPS as u64);
    qos_spf.min(floor)
}

// ── 链路延迟（由 `sos_connection` 的 TestDelay 测量更新） ──
//
// ── 链路延迟（由 `sos_connection` 的 TestDelay 测量更新） ──
//
// 档位初始为保守（见 [`crate::sos_vpx::LinkTier::new`]）：还没测到任何样本时
// 守码率，而不是因为初值是 0 被误判成「链路极好」而立刻超发。

/// 链路档位的迟滞状态（见 [`crate::sos_vpx::LinkTier`]）。
///
/// 必须用**状态机**而不是「本次样本是否充裕」：档位一翻就是码率 ×2 或 ÷2、
/// 丢帧开关同时切换，实测被单个迟到回显在编码器重建瞬间把 5208kbps 砍到
/// 2604kbps 并丢了 3 帧（见 `LinkTier` 的文档）。
static LINK_TIER: Mutex<crate::sos_vpx::LinkTier> = Mutex::new(crate::sos_vpx::LinkTier::new());

/// 加锁访问链路档位。锁内只做纯计算，不碰 IO、不嵌套加锁。
fn with_link_tier<R>(f: impl FnOnce(&mut crate::sos_vpx::LinkTier) -> R) -> R {
    match LINK_TIER.lock() {
        Ok(mut guard) => f(&mut guard),
        // 中毒只可能来自本模块内部的 panic，状态本身仍是普通数据
        Err(poisoned) => f(&mut poisoned.into_inner()),
    }
}

/// 由 `sos_connection` 每次 TestDelay 往返后调用**一次**（已经过 min-filter）。
///
/// 延迟与排队必须一起喂：拆成两个 setter 会让一次探测被记成两个样本，
/// 迟滞的「连续 N 次」就等价于 N/2 秒，去抖强度会悄悄减半。
///
/// `queue = raw - floor`：floor 是 min-filter 的 RTT 下界，
/// raw 是本次实际往返时间，差值就是视频帧在本机出口 / 路由缓冲里造成的等待。
pub fn note_link_sample(rtt: u32, queue: u32) {
    let changed = with_link_tier(|t| t.sample(Some(rtt), Some(queue)));
    if changed {
        let generous = link_is_generous();
        log::info!(
            "[VIDEO] link tier -> {} (rtt={}ms queue={}ms)",
            if generous { "generous" } else { "constrained" },
            rtt,
            queue
        );
    }
}

/// 链路是否充裕（RTT 低 **且** 没有排队），**已过滤抖动**
///
/// 单样本判据见 [`crate::sos_vpx::link_is_generous`]；这里返回的是
/// [`crate::sos_vpx::LinkTier`] 迟滞之后的档位，读写两侧看到的永远是稳定值。
fn link_is_generous() -> bool {
    with_link_tier(|t| t.generous())
}

/// 当前应使用的丢帧水位
///
/// 实测 1080p 高复杂度内容 @1675kbps（`tests/vpx_feed_rate.rs`）：
///
/// | dropframe_thresh | 出帧 | 丢帧率 | 实际码率 |
/// |---|---|---|---|
/// | 20（守码率） | 10/60 | 83% | 1853 kbps |
/// | 0（守帧数） | 60/60 | 0% | 3294 kbps |
///
/// 也就是说这是一个纯粹的带宽取舍：要么流畅度极差，要么码率翻倍。
/// 链路充裕时选后者；串行链路超发会养出 bufferbloat，必须守码率。
fn dropframe_thresh_for_link() -> u32 {
    if link_is_generous() {
        // 链路充裕：不再为了省带宽而丢帧
        0
    } else {
        // 链路未知或已出现排队：守码率，容忍丢帧
        crate::sos_vpx::RC_DROPFRAME_THRESH
    }
}

/// 当前的码率预算档位（与丢帧水位共用同一个链路判据）
fn link_budget() -> crate::sos_vpx::LinkBudget {
    if link_is_generous() {
        crate::sos_vpx::LinkBudget::Generous
    } else {
        crate::sos_vpx::LinkBudget::Constrained
    }
}

// ── 订阅者（多连接 × 多显示器） ──
//
// 一个连接同时只看一块屏（与 RustDesk 协议一致），但**可以有多块屏
// 被同时采集**，也可以有多个控制端同时观看同一块屏。所以订阅表是
// `(conn_id, 显示器索引)` 的列表，而不是单个槽位。

#[derive(Clone)]
struct Subscriber {
    conn_id: i32,
    /// 该订阅者要看的显示器索引（`Display::all()` 的顺序）
    display: usize,
    tx: Sender,
    tx_video: VideoSender,
}

static SUBSCRIBERS: Mutex<Vec<Subscriber>> = Mutex::new(Vec::new());

/// 控制端登录成功后订阅指定显示器。
///
/// 同一 `conn_id` 重复订阅会替换旧条目（重连、切换显示器都走这条路径）。
/// 返回该显示器的管线是否为新启动。
pub fn subscribe(conn_id: i32, display: usize, tx: Sender, tx_video: VideoSender) -> bool {
    let first_subscriber = {
        let mut list = SUBSCRIBERS.lock().unwrap();
        let was_empty = list.is_empty();
        list.retain(|s| s.conn_id != conn_id);
        list.push(Subscriber {
            conn_id,
            display,
            tx,
            tx_video,
        });
        log::info!(
            "[VIDEO] subscriber #{} attached on display #{} ({} total)",
            conn_id,
            display,
            list.len()
        );
        was_empty
    };
    // 全新会话：上一个控制端测出的链路结论不能继承过来。
    // 它可能走的是完全不同的路径，而 Generous 档位意味着码率 ×2 且关掉
    // 丢帧保护，正好是「错判代价最大」的方向。迟滞会在 5 个探测样本
    // （约 5s）后重新给出结论。多控制端场景**不**重置：那时链路仍在使用中。
    if first_subscriber {
        with_link_tier(|t| t.reset());
        log::info!("[VIDEO] link tier reset for new session (constrained until measured)");
    }
    // 新控制端接入：上一轮因本机编码能力不足而做的降级不该继承过来。
    // 只影响**该显示器**的编码档位，其他屏的结论照旧。
    if reset_enc_level(display) {
        request_refresh_display(display);
    }
    ensure_pipeline(display)
}

/// 连接断开时取消订阅。返回该显示器是否已无人观看（管线会自行退出）。
pub fn unsubscribe(conn_id: i32) {
    let mut list = SUBSCRIBERS.lock().unwrap();
    let before = list.len();
    list.retain(|s| s.conn_id != conn_id);
    log::info!(
        "[VIDEO] subscriber #{} detached ({} -> {} total)",
        conn_id,
        before,
        list.len()
    );
}

/// 切换该连接观看的显示器。返回 `true` 表示需要启动新管线。
pub fn set_subscriber_display(conn_id: i32, display: usize) -> bool {
    let changed = {
        let mut list = SUBSCRIBERS.lock().unwrap();
        match list.iter_mut().find(|s| s.conn_id == conn_id) {
            Some(s) if s.display != display => {
                log::info!(
                    "[VIDEO] subscriber #{} switches display #{} -> #{}",
                    conn_id,
                    s.display,
                    display
                );
                s.display = display;
                true
            }
            _ => false,
        }
    };
    if changed {
        ensure_pipeline(display);
    }
    changed
}

/// 该显示器当前的订阅者快照
fn subscribers_for(display: usize) -> Vec<Subscriber> {
    SUBSCRIBERS
        .lock()
        .unwrap()
        .iter()
        .filter(|s| s.display == display)
        .cloned()
        .collect()
}

/// 向单个订阅者发送普通消息（非视频帧）
fn send_msg(sub: &Subscriber, msg: Message) {
    let _ = sub.tx.send((tokio::time::Instant::now(), Arc::new(msg)));
}

/// 向单个订阅者发送视频帧（背压点：通道满时阻塞）
fn send_video(sub: &Subscriber, msg: &Arc<Message>) -> bool {
    match sub
        .tx_video
        .blocking_send((tokio::time::Instant::now(), msg.clone()))
    {
        Ok(()) => true,
        Err(_) => {
            // 接收端已关闭（连接断开），交给主循环回收订阅
            false
        }
    }
}

/// 把一帧发给该显示器的**所有**订阅者。
///
/// 返回仍然存活的订阅数。多个控制端同时观看同一块屏时共用同一个编码器
/// 输出 —— 这正是我们需要的：一次采集、一次编码、N 份发送。
fn broadcast_video(subs: &[Subscriber], msg: &Arc<Message>) -> usize {
    let mut alive = 0;
    for s in subs {
        if send_video(s, msg) {
            alive += 1;
        }
    }
    alive
}

// ── 每显示器的控制标志 ──
//
// 刷新、关键帧、采集档位、编码档位都必须是**按显示器独立**的：
// 两块屏可能一块走 DXGI、一块走 GDI（不同 GPU / 不同会话），
// 一块屏的编码器编不动不代表另一块也编不动。
// 用位掩码而不是 HashMap：读写都在每帧路径上，且显示器数量天然有上限。

/// 请求重建捕获器与编码器
static REFRESH_MASK: AtomicU64 = AtomicU64::new(0);
/// 请求下一帧编为关键帧
static KEYFRAME_MASK: AtomicU64 = AtomicU64::new(0);

#[inline]
fn slot_bit(display: usize) -> u64 {
    1u64 << (display.min(MAX_DISPLAY_SLOTS - 1) as u64)
}

/// 请求重建**所有**显示器的捕获器与编码器（UAC 切换、编码格式变化等全局事件）。
pub fn request_refresh() {
    REFRESH_MASK.fetch_or(u64::MAX, Ordering::SeqCst);
}

/// 请求重建**指定**显示器的捕获器与编码器。
pub fn request_refresh_display(display: usize) {
    REFRESH_MASK.fetch_or(slot_bit(display), Ordering::SeqCst);
}

/// 取出并清除该显示器的重建请求
fn take_refresh(display: usize) -> bool {
    REFRESH_MASK.fetch_and(!slot_bit(display), Ordering::SeqCst) & slot_bit(display) != 0
}

/// 请求下一帧编为关键帧（不重建编码器）。
///
/// 用于两类场景：
/// - 控制端发来 `Misc::refresh_video`（它自己解码出错，等一个关键帧重同步）；
/// - 本端发送队列溢出丢掉了一个已编码的帧（参考链断了）。
///
/// `kf_mode = VPX_KF_DISABLED` 意味着**除了首帧再也不会自动出关键帧**，
/// 所以没有这条路径的话，一次丢帧就是永久花屏。
///
/// 请求对所有屏生效：`Misc::refresh_video` 不携带显示器索引，
/// 而多发一个关键帧的代价（约 9 倍帧大小）远小于对端一直花屏。
pub fn request_keyframe() {
    KEYFRAME_MASK.fetch_or(u64::MAX, Ordering::SeqCst);
}

/// 取出并清除该显示器的关键帧请求（保留逻辑：窗口内不丢，留给下一帧生效）
fn has_keyframe_request(display: usize) -> bool {
    KEYFRAME_MASK.load(Ordering::SeqCst) & slot_bit(display) != 0
}

/// 消费该显示器的关键帧请求
fn take_keyframe(display: usize) {
    KEYFRAME_MASK.fetch_and(!slot_bit(display), Ordering::SeqCst);
}

/// 每个显示器当前允许的最低档，运行时可上调回 [`TIER_DXGI`]。
///
/// 必须按屏独立：一块屏的 DXGI 不可用（虚拟显示器、RDP 输出）不代表
/// 另一块主屏也不可用。
static MIN_TIER: [AtomicUsize; MAX_DISPLAY_SLOTS] =
    [const { AtomicUsize::new(TIER_DXGI) }; MAX_DISPLAY_SLOTS];

fn min_tier(display: usize) -> usize {
    MIN_TIER[display.min(MAX_DISPLAY_SLOTS - 1)].load(Ordering::Relaxed)
}

fn set_min_tier(display: usize, tier: usize) {
    MIN_TIER[display.min(MAX_DISPLAY_SLOTS - 1)].store(tier, Ordering::Relaxed);
}

/// 判断重建失败是否属于「桌面暂时不可捕获」这类**可自愈**的环境状态。
///
/// 这类错误不该以 ERROR 级别刷屏，也不该触发降档 —— 换个采集后端同样拿不到显示器。
///
/// 触发场景（实测）：会话锁屏、切到 UAC 安全桌面、RDP 断开、
/// 显示器正在重枚举。此时 `Display::all()` 同时失去 GDI（`EnumDisplayDevices`
/// 拿不到输出）与 DXGI 两条路径，返回空表。恢复后自行可用。
///
/// `SOS SHMEM not registered` 也归入此类：它只可能在「main 已判定安全桌面、
/// 但首帧尚未到达」的竞态窗口里出现，属于过渡态而非故障。正常路径下
/// 状态由 main 统一推进，这条分支基本不会走到，但绝不能让它刷 ERROR。
fn is_no_display_error(e: &anyhow::Error) -> bool {
    let msg = e.to_string();
    msg.contains("no display") || msg.contains("SHMEM not registered")
}

// ── 控制端编码能力 ──

/// (vp8 可用, vp9 可用, 偏好)
static PEER_ABILITY: Mutex<(i32, i32, i32)> = Mutex::new((0, 0, 0));

/// 记录控制端声明的解码能力与**编码偏好**。
///
/// 编码格式真的发生变化时会请求重建编码器 —— `vpx_codec_ctx_t` 绑定具体 codec，
/// 无法原地切换。这也是之前切 VP8 无效的另一个原因：就算选对了格式，
/// 也不会重建，最后还是继续推 VP9 的码流。
pub fn set_peer_codec_ability(vp8: bool, vp9: bool, prefer: CodecPrefer) {
    let new_state = (vp8 as i32, vp9 as i32, prefer.to_i32());
    let prev_prefer = {
        let guard = PEER_ABILITY.lock().unwrap();
        if *guard == new_state {
            // 控制端的每个 `Misc::Option` 都带 supported_decoding，
            // 值没变时必须直接返回，否则每次都会重建一次编码器。
            return;
        }
        CodecPrefer::from_i32(guard.2)
    };
    let before = current_codec();
    *PEER_ABILITY.lock().unwrap() = new_state;
    let after = current_codec();
    log::info!(
        "[VIDEO] peer codec: vp8={} vp9={} prefer={:?} -> encoder {:?}",
        vp8,
        vp9,
        prefer,
        after
    );
    // 偏好在 `Auto` 与显式值之间切换时，按**旧偏好**测出来的降级结论不再适用。
    //
    // 典型死档：某次负载高峰把某屏推到 `enc_level = 2`（VP8 试验）之后，
    // 用户显式选了 VP9。`next_enc_level(2)` 返回 `None`，既换不回 VP9，
    // 也不会再降 cpu-used，永久停在既非用户选择、也不自适应的状态。
    let prefer_changed = prev_prefer != prefer;
    if prefer_changed {
        reset_all_enc_levels();
    }
    if before != after {
        log::info!(
            "[VIDEO] codec changed {:?} -> {:?}, rebuilding encoder",
            before,
            after
        );
        request_refresh();
    } else if prefer_changed {
        // 格式没变，但降级档位被清掉了：必须重建一次才能把 cpu-used 复位，
        // 否则新偏好会沿用上一个偏好时的编码配置。
        log::info!(
            "[VIDEO] codec preference {:?} -> {:?}, rebuilding at default level",
            prev_prefer,
            prefer
        );
        request_refresh();
    }
}

fn peer_ability() -> (bool, bool, CodecPrefer) {
    let (vp8, vp9, prefer) = *PEER_ABILITY.lock().unwrap();
    let prefer = CodecPrefer::from_i32(prefer);
    if vp8 == 0 && vp9 == 0 {
        // 未声明时按都支持处理（RustDesk 客户端两者都支持）
        (true, true, prefer)
    } else {
        (vp8 > 0, vp9 > 0, prefer)
    }
}

// ── 编码成本自适应 ──

/// 编码成本降级级别：0 = 默认配置；1 = 用最快的 `cpu-used`；2 = 改用 VP8。
///
/// 为什么需要它：控制端的**显式偏好**（`prefer=Vp9`，即画质选「最佳」）
/// 优先级高于本机启发式，这在强 CPU 上完全正确，但在低功耗 4 核机器上
/// 会选出编不动的配置。实测（真实机器，1600x900，DXGI 可用）：
///
/// ```text
/// SosVpxEncoder: Vp9 1600x900 threads=2 cpu_used=7 bitrate=1163kbps
/// [VIDEO:PERF] loop=19.6fps it=51.1ms cap=5.0ms conv=2.0ms enc=43.5ms spf=33ms skip=0
/// ```
///
/// 采集只要 5ms、转换 2ms，编码却要 43.5ms —— 超过 30fps 的 33ms 预算，
/// 循环被钉在 19.6fps。此时 VP9 相对 VP8 的画质优势完全被掉帧抵消，
/// 用户看到的是「更清晰但更卡」，而远程桌面里卡顿的代价远大于编码格式。
///
/// 判据只采用**实测编码耗时**，不含任何机器型号/核数硬编码：
/// 耗时超过整帧预算即证明这一档配置在本机不可行，逐级退到能实时编码为止。
///
/// 按显示器独立：两块屏可以有不同的分辨率和不同的 GPU，必须分别判定。
static ENC_LEVEL: [AtomicU8; MAX_DISPLAY_SLOTS] = [const { AtomicU8::new(0) }; MAX_DISPLAY_SLOTS];
/// 当前编码成本降级级别
pub fn enc_level(display: usize) -> u8 {
    ENC_LEVEL[display.min(MAX_DISPLAY_SLOTS - 1)].load(Ordering::Relaxed)
}

/// 换到 VP8 **之前**那一档的实测每帧编码耗时（毫秒，`f32` 位模式）。
///
/// 换 VP8 是否值得只能靠对比：实测表明 VP8 并不总比 VP9 快（同一内容
/// 3 线程下反而慢 40%，换一种内容又可能快数倍，见
/// [`crate::sos_vpx::choose_codec`] 的表格）。所以升级到 VP8 时把当时的
/// 耗时记下来，下一个窗口用 [`crate::sos_vpx::vp8_trial_worth_keeping`]
/// 判断它到底有没有帮上忙。
static ENC_BASELINE_MS: [AtomicU32; MAX_DISPLAY_SLOTS] =
    [const { AtomicU32::new(0) }; MAX_DISPLAY_SLOTS];

/// 该屏已实测「换 VP8 不划算」，不再尝试。
/// 新控制端接入时由 [`reset_enc_level`] 清除（对端能力/负载可能变了）。
static VP8_REJECTED: [AtomicBool; MAX_DISPLAY_SLOTS] =
    [const { AtomicBool::new(false) }; MAX_DISPLAY_SLOTS];

fn enc_baseline_ms(display: usize) -> f64 {
    let bits = ENC_BASELINE_MS[display.min(MAX_DISPLAY_SLOTS - 1)].load(Ordering::Relaxed);
    f32::from_bits(bits) as f64
}

fn set_enc_baseline_ms(display: usize, ms: f64) {
    let v = if ms.is_finite() && ms > 0.0 {
        ms as f32
    } else {
        0.0
    };
    ENC_BASELINE_MS[display.min(MAX_DISPLAY_SLOTS - 1)].store(v.to_bits(), Ordering::Relaxed);
}

fn vp8_rejected(display: usize) -> bool {
    VP8_REJECTED[display.min(MAX_DISPLAY_SLOTS - 1)].load(Ordering::Relaxed)
}

/// 新控制端接入时复位该显示器的编码降级结论。返回是否真的变了。
///
/// 连「VP8 被否决」一起清掉：新对端可能有不同的解码能力，负载也可能变了，
/// 旧的实测结论不一定还成立。
pub fn reset_enc_level(display: usize) -> bool {
    let slot = display.min(MAX_DISPLAY_SLOTS - 1);
    let was_rejected = VP8_REJECTED[slot].swap(false, Ordering::Relaxed);
    ENC_BASELINE_MS[slot].store(0, Ordering::Relaxed);
    let changed = ENC_LEVEL[slot].swap(0, Ordering::Relaxed) != 0;
    if changed {
        log::info!("[VIDEO] encoder level reset for display #{}", display);
    }
    changed || was_rejected
}

/// 清掉所有显示器的实测降级结论。
///
/// 控制端**显式**改变编码偏好时调用：降级阶梯是针对某个具体格式标定的，
/// 换了格式（或从 `Auto` 变成显式）之后旧结论不再成立。
fn reset_all_enc_levels() {
    for d in 0..MAX_DISPLAY_SLOTS {
        reset_enc_level(d);
    }
}

/// 降一级编码成本。返回 true 表示配置确实变了，调用方需要重建编码器。
///
/// `enc_avg_ms` 是**当前档位**的实测每帧编码耗时，换到 VP8 时用它做基线。
fn step_down_encoder(display: usize, enc_avg_ms: f64) -> bool {
    let cur = enc_level(display);
    let (peer_vp8, _, prefer) = peer_ability();
    // 已实测「VP8 不划算」就不再试，直接停在当前档位。
    //
    // 显式偏好下也**不**做 VP8 试验：用户已经指定了格式，实测降级只应影响
    // cpu-used（第 1 级），不该悄悄把格式换掉。
    let allow_vp8 = peer_vp8 && prefer == CodecPrefer::Auto && !vp8_rejected(display);
    let Some(next) = crate::sos_vpx::next_enc_level(cur, allow_vp8) else {
        return false;
    };
    if next == 2 {
        // 记下升级前的耗时，下一个窗口用它判断 VP8 是否真的更快。
        set_enc_baseline_ms(display, enc_avg_ms);
    }
    ENC_LEVEL[display.min(MAX_DISPLAY_SLOTS - 1)].store(next, Ordering::Relaxed);
    log::warn!(
        "[VIDEO] display #{} encoder cannot keep up with the frame budget, level {} ({})",
        display,
        next,
        if next == 1 {
            "fastest cpu-used".to_string()
        } else {
            format!("VP8 trial, baseline {:.1}ms", enc_avg_ms)
        }
    );
    true
}

/// 已在 VP8 上仍然超预算：用实测对比决定是保留还是回退。
///
/// 返回 true 表示已回退到默认档位，调用方需要重建编码器。
fn revert_vp8_if_useless(display: usize, enc_avg_ms: f64) -> bool {
    let baseline = enc_baseline_ms(display);
    if crate::sos_vpx::vp8_trial_worth_keeping(baseline, enc_avg_ms) {
        // VP8 确实更快（或者基线数据无效）：保留。
        return false;
    }
    let slot = display.min(MAX_DISPLAY_SLOTS - 1);
    VP8_REJECTED[slot].store(true, Ordering::Relaxed);
    ENC_BASELINE_MS[slot].store(0, Ordering::Relaxed);
    ENC_LEVEL[slot].store(0, Ordering::Relaxed);
    log::warn!(
        "[VIDEO] display #{} VP8 is not faster here ({:.1}ms vs baseline {:.1}ms), \
         reverting to the default codec and keeping the quality",
        display,
        enc_avg_ms,
        baseline
    );
    true
}

// ── 安全桌面状态 ──
static UAC_ACTIVE: AtomicBool = AtomicBool::new(false);
/// 由 `main` 的桌面切换监听线程设置，`sos_video` 的采集循环每秒兜底复核一次。
///
/// 语义是「当前处于安全桌面（锁屏 / UAC）」，此时画面来自 SYSTEM 子进程
/// 写入的共享内存，而不是本地采集。
pub fn set_uac_active(active: bool) {
    if UAC_ACTIVE.swap(active, Ordering::SeqCst) != active {
        log::info!("[VIDEO] secure desktop active = {}", active);
        request_refresh();
    }
}

pub fn is_uac_active() -> bool {
    UAC_ACTIVE.load(Ordering::SeqCst)
}

// ── 启动 ──

/// 已在运行的采集管线（显示器索引集合）
static RUNNING: Mutex<Vec<usize>> = Mutex::new(Vec::new());

/// 启动后台捕获线程。只需在进程启动时调用一次。
///
/// 真正的采集线程是**按显示器**派生的：有多少块屏被观看就有多少条管线，
/// 各自持有独立的采集器与编码器，互不影响。这里只做一次性的进程级初始化，
/// 再为已订阅的显示器补齐管线。
pub fn start() {
    raise_timer_resolution();
    let wanted: Vec<usize> = SUBSCRIBERS
        .lock()
        .unwrap()
        .iter()
        .map(|s| s.display)
        .collect();
    for d in wanted {
        ensure_pipeline(d);
    }
}

/// 确保某块屏的采集管线在运行。已运行则什么都不做。
///
/// 返回是否新启动了一条线程。
pub fn ensure_pipeline(display: usize) -> bool {
    {
        let mut running = RUNNING.lock().unwrap();
        if running.contains(&display) {
            return false;
        }
        running.push(display);
    }
    // 新管线第一次循环就会创建采集器与编码器，所以之前累积的重建请求
    // 已经没有必要。不清掉的话会看到「创建 → 立刻再重建一次」：
    // 多付一次 DXGI 探测（~170ms）+ 一次编码器分配，画面起步卡一下。
    // 实测日志里 `capturer=dxgi(sos)` 连着打两条就是这个问题。
    take_refresh(display);
    let spawned = std::thread::Builder::new()
        .name(format!("sos-video-{}", display))
        .spawn(move || {
            capture_loop(display);
            // 线程退出（该屏无人观看）：放开槽位，下次订阅可以重新拉起
            RUNNING.lock().unwrap().retain(|d| *d != display);
            log::info!("[VIDEO] capture thread for display #{} exited", display);
        });
    match spawned {
        Ok(_) => true,
        Err(e) => {
            log::error!(
                "[VIDEO] failed to spawn capture thread for display #{}: {}",
                display,
                e
            );
            RUNNING.lock().unwrap().retain(|d| *d != display);
            false
        }
    }
}

/// 把当前线程所在进程的定时器精度提到 1ms。
///
/// Windows 默认调度粒度约 15.6ms，`std::thread::sleep` 请求 5ms 也会睡满一个 tick。
/// 视频要跑到 30fps（33ms/帧）就必须有亚 tick 级的睡眠精度，否则帧间隔会被
/// 统一放大到 15.6ms 的整数倍，实际帧率远低于目标值。
///
/// 这是进程级设置，只需在进程启动时调用一次，进程退出时自动失效。
fn raise_timer_resolution() {
    unsafe {
        // 返回值为 0 表示成功；失败不影响功能，只是精度仍然受限
        let r = windows::Win32::Media::timeBeginPeriod(1);
        if r != 0 {
            log::warn!("[VIDEO] timeBeginPeriod(1) failed: {}", r);
        }
    }
}

/// 单块显示器的采集循环。
///
/// 该屏的订阅者全部离开后自行退出（见循环开头的空闲判定）。
fn capture_loop(display: usize) {
    log::info!("[VIDEO] capture thread started for display #{}", display);
    // 这些缓冲放在循环外，避免与 `state.capturer` 同时可变借用 `state`
    let mut yuv: Vec<u8> = Vec::new();
    let mut mid_data: Vec<u8> = Vec::new();

    let mut state: Option<CaptureState> = None;
    let mut last_uac_check = Instant::now();
    let mut last_display_check = Instant::now();
    let mut last_qos_update = Instant::now();
    // 当前下发给编码器的目标码率（kbps）。用它做变更判定，而不是画质比 ——
    // 帧率变化同样会改变码率（见 `SosVpxConfig::bitrate_kbps_for`）。
    let mut cur_kbps: u32 = 0;
    // 当前下发给编码器的丢帧水位。0 = 不丢帧（链路充裕时）。
    let mut cur_drop_thresh: u32 = crate::sos_vpx::RC_DROPFRAME_THRESH;
    // 编码器的 pts 基准点：pts = 从此刻起的真实毫秒数。
    // 编码器重建时重置，避免时间戳跳变过大。
    let mut pts_origin = Instant::now();
    // 同一后端连续瞬时失败次数；成功出帧即清零
    let mut rebuild_streak: u32 = 0;
    let mut last_reprobe = Instant::now();
    // 上次强制关键帧的时刻（节流用，见 [`MIN_KEYFRAME_INTERVAL`]）
    let mut last_keyframe_at = Instant::now();
    // 重探测退避：失败一次翻倍，成功后恢复 [`RE_PROBE_INTERVAL`]
    let mut reprobe_interval = RE_PROBE_INTERVAL;

    // 分阶段耗时诊断：区分采集 / 颜色转换 / 编码 的瓶颈
    let mut perf_last = Instant::now();
    let (mut n_cap, mut n_conv, mut n_enc, mut n_frame) = (0u32, 0u64, 0u64, 0u32);
    // 桌面无变化而被跳过的次数（WouldBlock），用于区分「采集慢」和「画面没动」
    let mut n_skip: u64 = 0;
    // 编码器为守住码率而主动丢掉的帧（帧率下采样），与上面两种都不同
    let mut n_enc_drop: u64 = 0;
    // `yuv` 里是否已经有一份可用的、与**当前**编码器同尺寸的帧。
    // 重建编码器/分辨率变化时必须清零，否则会把旧尺寸的缓冲喂进去。
    let mut have_yuv = false;
    // 手上那份内容还没成功送达对端（被编码器丢掉，或关键帧请求待生效）。
    let mut stale_pending = false;
    // 静止画面下「重编上一帧」的次数，供诊断确认这条路真的被走到
    let mut n_stale: u64 = 0;
    // 阻塞在视频通道上的次数与耗时（网络背压）
    let mut n_send: u64 = 0;
    let mut send_us: u64 = 0;
    // 迭代次数与整轮实际耗时：用于发现「各阶段之和远小于实际帧间隔」的隐藏开销
    let mut n_iter: u64 = 0;
    let mut iter_us: u64 = 0;
    let mut last_iter_at: Option<Instant> = None;
    // WouldBlock（等新帧）耗时：它也占整帧预算，但之前完全没计入
    let mut wait_us: u64 = 0;
    // 周期性检查（UAC / 桌面切换 / 显示器尺寸）耗时
    let mut chk_us: u64 = 0;
    let mut n_chk: u64 = 0;
    // 连续多少个统计窗口编码超预算（见 [`ENC_OVERLOAD_WINDOWS`]）
    let mut enc_overload: u32 = 0;
    // 重建失败时的退避（见 [`MAX_REBUILD_BACKOFF`]）与“桌面不可捕获”状态的日志闸门
    let mut rebuild_backoff = INITIAL_REBUILD_BACKOFF;
    let mut no_display_logged = false;
    // 绝对时间轴：每轮把 deadline 推 spf，sleep 超时会自动在下轮减掉
    let mut next_frame_at = Instant::now();
    let (mut cap_us, mut conv_us, mut enc_us) = (0u64, 0u64, 0u64);

    loop {
        // 统计整轮实际耗时（包括 continue 路径），用于核对 spf 与实际帧率是否一致
        if let Some(prev) = last_iter_at {
            iter_us += prev.elapsed().as_micros() as u64;
            n_iter += 1;
        }
        last_iter_at = Some(Instant::now());

        // 1. 该屏无订阅者：释放资源并退出线程
        //
        // 不在这里空转等待：这块屏可能再也不会被人看，而观看它的控制端
        // 再次订阅时 `ensure_pipeline()` 会重新拉起一条线程。
        let subs = subscribers_for(display);
        if subs.is_empty() {
            if state.take().is_some() {
                log::info!(
                    "[VIDEO] no subscriber for display #{}, releasing capturer/encoder",
                    display
                );
                // 只释放采集/编码资源。QoS 状态刻意保留 —— 控制端短暂重连时
                // 不该丢掉已经测出来的帧率和画质。
            }
            return;
        }

        // 2. 轮询桌面切换（只用于触发重建，不改安全桌面状态）
        if last_uac_check.elapsed() >= UAC_CHECK_INTERVAL {
            last_uac_check = Instant::now();
            let chk = Instant::now();
            // 安全桌面状态**只由 main 的 watcher 线程维护**，这里绝不能
            // 自己 set_uac_active()。原因：main 那个线程同时负责
            // SetEvent(CAPTURE_START) → 等 SYSTEM 子进程首帧 → 注册 SHMEM，
            // 是一个有先后依赖的序列；video 线程先把它翻成 true 的话，
            // `CaptureState::new` 会走 `acquire_uac_capturer()` 并硬失败。
            //
            // 现场日志就是这种死循环：连接建立后 20 秒里每 3 秒一条
            // `[VIDEO] display #0 failed to create capturer/encoder:
            //  SOS SHMEM not registered`，而真正的首次
            // `set_uac_active(true)` 直到客户端断开后才发生 ——
            // 控制端全程黑屏。
            if librustdesk::platform::windows::desktop_changed() && !is_uac_active() {
                request_refresh();
            }
            chk_us += chk.elapsed().as_micros() as u64;
            n_chk += 1;
        }

        // 3. 无状态或收到刷新请求：重建捕获器 + 编码器
        if state.is_none() || take_refresh(display) {
            state = None;
            // 旧缓冲与新编码器尺寸不一定一致，必须作废
            have_yuv = false;
            stale_pending = false;
            // 重建时给远方一点时间回到可捕获状态（比如刚退出 UAC）
            std::thread::sleep(REBUILD_SETTLE_DELAY);
            match CaptureState::new(display) {
                Ok(s) => {
                    cur_kbps = s.initial_bitrate;
                    cur_drop_thresh = s.initial_drop_thresh;
                    // 新编码器从 0 开始计时，pts 不能带着旧时间轴
                    pts_origin = Instant::now();
                    // 新编码器的首帧必然是关键帧，节流窗口从现在重新起算，
                    // 否则重建后紧接着又会为一个待处理请求再发一个大关键帧。
                    last_keyframe_at = Instant::now();
                    rebuild_backoff = INITIAL_REBUILD_BACKOFF;
                    if no_display_logged {
                        no_display_logged = false;
                        log::info!("[VIDEO] display #{} capturable again", display);
                    }
                    log::info!(
                        "[VIDEO] display #{} capturing {}x{} via {}, {:?} threads={} bitrate={}kbps",
                        display,
                        s.width,
                        s.height,
                        s.capturer_name,
                        s.encoder.codec(),
                        s.threads,
                        s.encoder.bitrate_kbps()
                    );
                    // 通知**该屏的每个**观看者当前显示参数
                    if let Some(msg) = make_switch_display_msg(
                        s.display_index,
                        s.width,
                        s.height,
                        &s.display_name,
                        s.display_origin,
                    ) {
                        for sub in subscribers_for(display) {
                            send_msg(&sub, msg.clone());
                        }
                    }
                    // 把真实设备名登记给 `sos_connection::Session`，让 QoS 的
                    // 按屏聚合与这里用同一个 key（连接可能建立在锁屏期间，
                    // 那时枚举不到显示器，名字为空）。
                    set_display_name(display, &s.display_name);
                    state = Some(s);
                }
                Err(e) => {
                    // 重建失败最常见的原因是**桌面暂时不可捕获**：会话锁屏、
                    // 切到 UAC 安全桌面、RDP 断开或显示器正在重枚举。此时
                    // `Display::all()` 返回空表（GDI 的 `EnumDisplayDevices`
                    // 与 DXGI 都拿不到输出），一会儿之后会自己恢复。
                    //
                    // 实测日志：这段状态持续了 **130 秒**，旧实现每秒打一条
                    // ERROR（"no display available"），并把 `gap_ms[max]`
                    // 抬到 31.5 秒。所以这里必须：
                    // 1. 只在**状态变化**时记一条，不刷屏；
                    // 2. 指数退避（1s → 5s），避免每秒空转一次完整枚举；
                    // 3. 恢复时补一条恢复日志，便于把日志段和用户操作对上。
                    let no_display = is_no_display_error(&e);
                    if no_display {
                        rebuild_backoff = (rebuild_backoff * 2).min(MAX_REBUILD_BACKOFF);
                        if !no_display_logged {
                            no_display_logged = true;
                            log::warn!(
                                "[VIDEO] display #{} not capturable yet ({}), retrying in background",
                                display,
                                e
                            );
                        }
                    } else {
                        rebuild_backoff = MAX_REBUILD_BACKOFF;
                        log::error!(
                            "[VIDEO] display #{} failed to create capturer/encoder: {}",
                            display,
                            e
                        );
                    }
                    // 退避期间分片睡眠：一旦收到刷新请求（例如安全桌面的
                    // SYSTEM 首帧就绪、`set_uac_active(true)`）立刻重建，
                    // 不必等满 5 秒的退避窗口 —— 那 5 秒控制端是完全黑屏。
                    let deadline = Instant::now() + rebuild_backoff;
                    while Instant::now() < deadline {
                        if take_refresh(display) {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(100));
                    }
                    continue;
                }
            }
        }
        let Some(st) = state.as_mut() else { continue };

        // 3.1 曾降档时定期试探更高档（环境可能已恢复）
        //
        // UAC 模式下必须整个跳过：此时采集器是 SYSTEM 子进程送来的共享内存帧，
        // 与显示后端无关。
        //
        // RDP 会话下也要跳过：Windows 在远程会话里根本不提供 Desktop Duplication，
        // 探测必然失败，唯一效果是每 30/60/120…s 白跑一次 ~170ms 的 DXGI 初始化，
        // 还会在日志里刷「probe failed」。会话结束进程就退出了，
        // 所以也没有「等它恢复」的意义。
        //
        // 探测必须**非破坏性**。旧实现把 `MIN_TIER` 复位后直接走重建路径，
        // 即使探测失败也会换掉一个工作正常的采集器 + 编码器：重新分配 8MB
        // 位图与整套编码器缓冲，丢失变化检测基线（`have_prev`）和参考帧，
        // 画面停顿约 250ms（DXGI 尝试本身就要 ~170ms）并额外产生一个关键帧。
        // 在 DXGI 不可用的机器上这会在 30/60/120/240s… 反复发生。
        // 现在把探测丢到独立线程，只有真的建成功了才请求重建。
        if is_uac_active() || is_remote_session() {
            reprobe_interval = RE_PROBE_INTERVAL;
        } else if min_tier(display) > TIER_DXGI {
            if last_reprobe.elapsed() >= reprobe_interval {
                last_reprobe = Instant::now();
                // 先退避一格；探测成功会由下面的 `else` 分支复位
                reprobe_interval = (reprobe_interval * 2).min(MAX_RE_PROBE_INTERVAL);
                log::info!(
                    "[VIDEO] display #{} probing dxgi again (retry in {}s if it fails)",
                    display,
                    reprobe_interval.as_secs()
                );
                let name = st.display_name.clone();
                std::thread::spawn(move || match crate::sos_dxgi::create(&name) {
                    Ok(_) => {
                        // 探针采集器在此就地释放；真正的重建交给主循环
                        log::info!(
                            "[VIDEO] display #{} dxgi probe succeeded, switching capturer",
                            display
                        );
                        set_min_tier(display, TIER_DXGI);
                        request_refresh_display(display);
                    }
                    Err(e) => {
                        log::info!("[VIDEO] display #{} dxgi probe failed: {}", display, e)
                    }
                });
            }
        } else if reprobe_interval != RE_PROBE_INTERVAL {
            // 已跑在最高档并稳住：恢复默认探测间隔
            reprobe_interval = RE_PROBE_INTERVAL;
            log::info!(
                "[VIDEO] display #{} fastest capturer tier restored, backoff reset",
                display
            );
        }

        // 4. 每秒把 QoS 状态同步给编码器：码率随画质比与帧率变化
        //
        // 读的是**本屏**的聚合视图：多连接看同一块屏时 fps 取最小、ratio 取最大；
        // 另一块屏的拥塞不会影响这一块。
        if last_qos_update.elapsed() >= QOS_SYNC_INTERVAL {
            last_qos_update = Instant::now();
            let qos = crate::sos_qos::display_snapshot(&st.display_name);
            // `fps` 已经夹在 `MIN_FPS..=target_fps` 之间，这里再兜一层，
            // 保证帧率绝不会低于采集侧的下限（否则码率会被缩得过小）。
            let effective_fps = qos.fps.max(MIN_CAPTURE_FPS);
            // 链路充裕时不按帧率缩放码率，并放宽总预算（
            // 详见 [`SosVpxConfig::bitrate_kbps_for_link`] 与
            // [`SosVpxConfig::GOOD_LINK_BITRATE_GAIN`]）。
            let kbps = SosVpxConfig::bitrate_kbps_for_link(
                st.width,
                st.height,
                qos.ratio,
                effective_fps,
                link_budget(),
            );
            // 丢帧水位随链路状况切换：链路充裕时不丢帧保流畅，
            // 出现排队时守码率。见 [`GOOD_LINK_RTT_MS`]。
            let thresh = dropframe_thresh_for_link();
            let kbps_changed = (kbps as i64 - cur_kbps as i64).unsigned_abs() * 100
                > cur_kbps as u64 * BITRATE_CHANGE_PCT;
            if kbps_changed || thresh != cur_drop_thresh {
                if let Err(e) = st.encoder.tune(kbps, thresh) {
                    log::warn!(
                        "[VIDEO] display #{} tune({}, {}) failed: {}",
                        display,
                        kbps,
                        thresh,
                        e
                    );
                } else {
                    log::info!(
                        "[VIDEO] display #{} ratio {:.2} fps {}/{} viewers={} -> bitrate {}kbps dropframe {} (rtt={:?}ms queue={:?}ms budget={})",
                        display,
                        qos.ratio,
                        qos.fps,
                        qos.target_fps,
                        qos.viewers,
                        kbps,
                        thresh,
                        qos.worst_rtt_ms,
                        qos.worst_queue_ms,
                        match link_budget() {
                            crate::sos_vpx::LinkBudget::Generous => "generous",
                            crate::sos_vpx::LinkBudget::Constrained => "constrained",
                        }
                    );
                    crate::sos_qos::with(|q| q.store_bitrate(&st.display_name, kbps));
                    cur_kbps = kbps;
                    cur_drop_thresh = thresh;
                }
            }
        }

        // 5. 显示器尺寸/数量变化检测（分辨率切换、显示器增减）
        if last_display_check.elapsed() >= DISPLAY_CHECK_INTERVAL {
            last_display_check = Instant::now();
            let chk = Instant::now();
            let mut detected: Option<Option<(usize, usize)>> = None;
            if !is_uac_active() {
                // 按**当前采集的这块屏**的名字查询，而不是主显示器的 SM_CXSCREEN。
                // 多显示器下两者不是一回事：改副屏分辨率时旧实现检测不到变化，
                // 编码器尺寸会一直停在旧值。
                detected = Some(display_size_by_name(&st.display_name));
            }
            chk_us += chk.elapsed().as_micros() as u64;
            n_chk += 1;
            if let Some(size) = detected {
                if size.is_none() {
                    // 查询失败不重建（`size_changed` 对 None 返回 false）：
                    // 把瞬时失败当成"分辨率变了"会陷入重建死循环。
                    log::warn!(
                        "[VIDEO] display size query failed for '{}'",
                        st.display_name
                    );
                } else if crate::sos_display::size_changed((st.width, st.height), size) {
                    let (w, h) = size.unwrap_or_default();
                    log::info!(
                        "[VIDEO] display #{} size changed {}x{} -> {}x{}, rebuild",
                        display,
                        st.width,
                        st.height,
                        w,
                        h
                    );
                    request_refresh_display(display);
                    continue;
                }
            }
        }

        // 6. 按 QoS 的帧间隔采集（带下限保护）
        //
        // 帧率取**本屏**所有观看者的最小值，因此一条慢链路只会拖慢
        // 它自己看的那块屏，其他屏不受影响。
        let spf = capture_spf(&st.display_name);
        let yuvfmt = st.encoder.yuvfmt();
        let to_io =
            |e: anyhow::Error| std::io::Error::new(std::io::ErrorKind::Other, e.to_string());
        let cap_start = Instant::now();
        let captured = st.capturer.frame(spf);
        // 只统计真实出帧的耗时。WouldBlock 里包含等待超时，
        // 会把 cap 指数抬到 spf 附近，完全失去诊断价值。
        let captured = match captured {
            Ok(f) => {
                cap_us += cap_start.elapsed().as_micros() as u64;
                n_cap += 1;
                Ok(f)
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::WouldBlock {
                    // 等新帧的时间也占整帧预算，单独计量
                    wait_us += cap_start.elapsed().as_micros() as u64;
                    n_skip += 1;
                }
                Err(e)
            }
        };
        let encoded: std::io::Result<Vec<(Vec<u8>, bool)>> = match captured {
            Ok(frame) => {
                let conv_start = Instant::now();
                let converted = frame.to(yuvfmt, &mut yuv, &mut mid_data);
                conv_us += conv_start.elapsed().as_micros() as u64;
                match converted {
                    Ok(input) => match input.yuv() {
                        Ok(data) => {
                            n_conv += 1;
                            // 转换成功，`yuv` 里就是当前帧，静止时可以直接重编
                            have_yuv = true;
                            let enc_start = Instant::now();
                            // 有控制端或丢帧请求时，强制出一个关键帧帮助对端重同步。
                            // 必须节流（见 [MIN_KEYFRAME_INTERVAL]）：拥塞时丢帧会
                            // 连发请求，每个关键帧都是 9 倍大的突发。
                            // 这里用 load 而不是 swap：请求在节流窗口内不会被丢弃，
                            // 而是留到窗口结束后第一帧生效（控制端的刷新请求因此
                            // 最多延迟 1.5 秒，而不是无声消失）。
                            if has_keyframe_request(display)
                                && last_keyframe_at.elapsed() >= MIN_KEYFRAME_INTERVAL
                            {
                                take_keyframe(display);
                                st.encoder.force_keyframe();
                                last_keyframe_at = Instant::now();
                                log::debug!("[VIDEO] display #{} forced keyframe", display);
                            }
                            let out = st
                                .encoder
                                .encode(data, frame_ms(&pts_origin))
                                .map_err(to_io);
                            enc_us += enc_start.elapsed().as_micros() as u64;
                            if out.is_ok() {
                                n_enc += 1;
                            }
                            out
                        }
                        Err(e) => Err(to_io(e)),
                    },
                    Err(e) => Err(to_io(e)),
                }
            }
            // 画面静止（`WouldBlock` / `TimedOut`）时也**可能**欠对端一份内容。两种情形：
            //
            // 1. `stale_pending`：上一帧被编码器为守码率丢掉了。此刻画面恰好
            //    静止，采集器会一直报「没变」，那次变化就永远补不上。
            // 2. 控制端请求关键帧（解码出错，或发送队列丢弃了已编码帧）。
            //    `request_keyframe()` 只是置位，靠**下一帧**生效；画面不再
            //    变化时它就永远挂着，对端停在花屏/冻结画面上直到重连。
            //
            // 两种情况都只需要把**手上最后一帧**（`yuv` 仍保留着）重编一次，
            // 不必等画面再次变化。这也是唯一与采集器无关的做法：`sos_gdi`
            // 有 `request_force_emit()` 可以强制出帧，DXGI 没有（静止时它
            // 只会一直报无新帧）。
            //
            // 注意两种「无新帧」错误都要覆盖：`sos_dxgi` 把
            // `DXGI_ERROR_WAIT_TIMEOUT` 映射成 `WouldBlock`，而 `scrap` 把它
            // 映射成 `TimedOut`（见 [`classify_frame_error`]）。
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                let kf_pending = has_keyframe_request(display);
                // 只读一次时钟：判据与下面的日志必须用同一个值
                let since_kf = last_keyframe_at.elapsed();
                let kf_due = kf_pending && since_kf >= MIN_KEYFRAME_INTERVAL;
                if !should_reemit_retained(have_yuv, stale_pending, kf_pending, since_kf) {
                    Err(e)
                } else {
                    let enc_start = Instant::now();
                    if kf_due {
                        take_keyframe(display);
                        st.encoder.force_keyframe();
                        last_keyframe_at = Instant::now();
                    }
                    n_stale += 1;
                    // 关键帧请求被节流（1.5s），这里每 1.5s 最多一条 info；
                    // 补发被编码器再次丢掉时会按帧率重试，那种重复只记 debug。
                    if kf_due {
                        log::info!(
                            "[VIDEO] display #{} static screen, re-emitting keyframe from retained frame (stale={})",
                            display,
                            stale_pending
                        );
                    } else {
                        log::debug!(
                            "[VIDEO] display #{} static screen, re-encoding retained frame to deliver dropped content",
                            display
                        );
                    }
                    let out = st
                        .encoder
                        .encode(&yuv, frame_ms(&pts_origin))
                        .map_err(to_io);
                    enc_us += enc_start.elapsed().as_micros() as u64;
                    if out.is_ok() {
                        n_enc += 1;
                    }
                    out
                }
            }
            Err(e) => Err(e),
        };

        match encoded {
            Ok(frames) => {
                // 编码器可能为了守住码率而丢掉这一帧（`rc_dropframe_thresh`）。
                //
                // 注意这里**不能** `continue`：步骤 7 的帧间距补足必须照常执行。
                // 否则丢帧后立刻又去采集，这一帧注定还是会被丢，
                // 白白多烧一次采集 + 色彩转换。
                n_enc_drop += st.encoder.take_dropped_frames() as u64;
                // 空帧就什么都不发，但**不** `continue` —— 继续走到步骤 7 的帧间距补足。
                if frames.is_empty() {
                    // 这一帧被编码器丢了。若之后画面恰巧静止，GDI 的变化检测
                    // 会把后续帧全判为「没变」，那次变化就永远补不上。
                    crate::sos_gdi::request_force_emit();
                    // 无论下一轮采集到什么，都要重编手上这一帧把变化补发出去
                    // （见上面的「无新帧」分支）。
                    stale_pending = true;
                } else if let Some(msg) = st.make_video_msg(frames) {
                    let msg = Arc::new(msg);
                    let send_start = Instant::now();
                    // 多个控制端看同一块屏时共用同一次编码输出：
                    // 采集与编码只做一次，发送做 N 次。
                    let alive = broadcast_video(&subs, &msg);
                    // 视频通道容量只有 2 帧，网络跟不上时 blocking_send 会阻塞，
                    // 这段时间既不是采集也不是编码，必须单独计量。
                    send_us += send_start.elapsed().as_micros() as u64;
                    n_send += 1;
                    // 这一份内容已经送达（至少已入队），不必再补
                    stale_pending = false;
                    if alive == 0 {
                        log::info!(
                            "[VIDEO] all frame channels for display #{} closed, releasing state",
                            display
                        );
                        state = None;
                        continue;
                    }
                }
            }
            Err(e) => {
                match classify_frame_error(&e) {
                    // 桌面静止 / 等待超时：完全正常，不重建也不降档。
                    //
                    // 注意**不能** `continue`：步骤 7 的帧间距补足必须照常执行。
                    // 否则「画面没变」会退化成零间隔忙循环 —— 而 GDI 的
                    // `frame()` 并不等待，每一轮都要付一次全屏 BitBlt（27-39ms）。
                    FrameFailure::NoFrame => {}
                    FrameFailure::Demote => {
                        log::warn!(
                            "[VIDEO] display #{} capturer unsupported ({:?}), demoting",
                            display,
                            e
                        );
                        rebuild_streak = 0;
                        demote_capturer(display, "unsupported");
                        std::thread::sleep(DEMOTE_SETTLE_DELAY);
                        continue;
                    }
                    FrameFailure::Rebuild => {
                        rebuild_streak += 1;
                        if rebuild_streak > MAX_TRANSIENT_FAILURES {
                            // 连续重建无效，说明本档在此环境不可用
                            log::warn!(
                                "[VIDEO] display #{} {} consecutive failures ({:?}), demoting",
                                display,
                                rebuild_streak,
                                e
                            );
                            rebuild_streak = 0;
                            demote_capturer(display, "repeated failures");
                        } else {
                            log::info!(
                                "[VIDEO] display #{} transient capture error ({:?}), rebuilding",
                                display,
                                e
                            );
                            request_refresh_display(display);
                        }
                        std::thread::sleep(REBUILD_RETRY_DELAY);
                        continue;
                    }
                }
            }
        }

        // 7. 补足帧间距。
        //
        // 不能用 `sleep(spf - elapsed)`：Windows 默认定时器精度约 15.6ms，
        // 任何短于这个量级的 sleep 都会被向上取整，误差逐帧累积。
        // 实测 spf=34ms 时实际每轮耗 85ms（帧率只有 29fps 的 40%）。
        // 改成绝对时间轴：每轮只把 deadline 推 spf，超时部分下轮自动减掉。
        rebuild_streak = 0;
        next_frame_at += spf;
        let now = Instant::now();
        if next_frame_at > now {
            std::thread::sleep(next_frame_at - now);
        } else if now.duration_since(next_frame_at) > spf {
            // 已经落后超过一帧，重置时间轴，避免为了“补帧”而背靠背发送
            next_frame_at = now;
        }
        n_frame += 1;

        // 8. 周期性输出分阶段耗时（定位瓶颈在采集/转换/编码）
        if perf_last.elapsed() >= PERF_LOG_INTERVAL {
            let secs = perf_last.elapsed().as_secs_f64().max(0.001);
            let per = |us: u64, n: u64| {
                if n == 0 {
                    0.0
                } else {
                    us as f64 / n as f64 / 1000.0
                }
            };
            log::info!(
                "[VIDEO:PERF] display #{} loop={:.1}fps it={:.1}ms wait={:.1}ms chk={:.1}ms cap={:.1}ms conv={:.1}ms enc={:.1}ms send={:.1}ms spf={}ms qos_fps={} viewers={} skip={} stale={} edrop={}",
                display,
                n_frame as f64 / secs,
                per(iter_us, n_iter),
                per(wait_us, n_skip),
                per(chk_us, n_chk),
                per(cap_us, n_cap as u64),
                per(conv_us, n_conv),
                per(enc_us, n_enc),
                per(send_us, n_send),
                spf.as_millis(),
                crate::sos_qos::fps(&st.display_name),
                subs.len(),
                n_skip,
                n_stale,
                n_enc_drop
            );
            // 编码成本自适应：编码耗时超过整帧预算就说明这一档配置在本机不可行。
            //
            // 只看“耗时/预算”这个比值，不看机器型号也不看核数 —— 同一份
            // 代码在强 CPU 上永远不会触发，在低功耗机器上会在 15s 内自救。
            // 要求连续 [`ENC_OVERLOAD_WINDOWS`] 个窗口都超才会降级：
            // 拖动窗口 / 播视频的瞬时高峰是正常的，不该永久降画质。
            // `n_enc` 太小（画面静止，几乎没编过帧）时统计无意义，跳过。
            if n_enc >= MIN_ENC_SAMPLES {
                let enc_avg_ms = per(enc_us, n_enc);
                let cap_avg_ms = per(cap_us, n_cap as u64);
                let conv_avg_ms = per(conv_us, n_conv);
                let budget_ms = spf.as_secs_f64() * 1000.0;
                // 判据同时考虑采集/转换耗时：采集本身慢的时候降编码档位毫无意义。
                if crate::sos_vpx::encoder_behind_budget(
                    enc_avg_ms,
                    cap_avg_ms,
                    conv_avg_ms,
                    budget_ms,
                ) {
                    enc_overload += 1;
                    // 超得越狠越不必等：严重超指向「这一档根本不可行」，
                    // 轻微超支才需要连续多个窗口排除抖动。见
                    // [`crate::sos_vpx::ENCODER_SEVERE_RATIO`]。
                    let severe =
                        crate::sos_vpx::overload_is_severe(enc_avg_ms, conv_avg_ms, budget_ms);
                    let need = if severe { 1 } else { ENC_OVERLOAD_WINDOWS };
                    log::info!(
                        "[VIDEO] display #{} encoder overloaded {}/{} (enc {:.1} + conv {:.1} > budget {:.1}ms x {:.0}%, cap {:.1}ms, severe={})",
                        display,
                        enc_overload,
                        need,
                        enc_avg_ms,
                        conv_avg_ms,
                        budget_ms,
                        crate::sos_vpx::ENCODER_BUDGET_RATIO * 100.0,
                        cap_avg_ms,
                        severe
                    );
                    if enc_overload >= need {
                        enc_overload = 0;
                        // 已在 VP8 上还超预算 → 先看它到底有没有帮上忙，
                        // 没帮上就回退，而不是一路卡在更慢更糊的编码器上。
                        let changed = if enc_level(display) >= 2 {
                            revert_vp8_if_useless(display, enc_avg_ms)
                        } else {
                            step_down_encoder(display, enc_avg_ms)
                        };
                        if changed {
                            request_refresh_display(display);
                        }
                    }
                } else {
                    enc_overload = 0;
                }
            }
            // 丢帧归因：libvpx 只在**内容压不住目标码率**时才丢帧。
            // 桌面内容（哪怕 120fps 喂帧）实测 0 丢帧，所以这个比例只在
            // 视频播放 / 快速滚动 / 大面积动画时才会抬头。
            if n_enc_drop > 0 {
                log::info!(
                    "[VIDEO] display #{} {}/{} frames dropped ({:.0}%), target={}kbps qos_fps={}",
                    display,
                    n_enc_drop,
                    n_enc,
                    n_enc_drop as f64 * 100.0 / (n_enc.max(1)) as f64,
                    crate::sos_qos::bitrate_kbps(&st.display_name),
                    crate::sos_qos::fps(&st.display_name)
                );
            }
            perf_last = Instant::now();
            n_cap = 0;
            n_conv = 0;
            n_enc = 0;
            n_frame = 0;
            n_skip = 0;
            n_enc_drop = 0;
            n_stale = 0;
            n_send = 0;
            send_us = 0;
            n_iter = 0;
            iter_us = 0;
            wait_us = 0;
            chk_us = 0;
            n_chk = 0;
            cap_us = 0;
            conv_us = 0;
            enc_us = 0;
        }
    }
}

/// 帧时间戳（毫秒）。
///
/// 必须用**真实流逝时间**，不能用 `frame_index * 33`。
///
/// libvpx 的 CBR 是「按 pts 流逝的时间发多少比特」来记账的。
/// 若 pts 固定按 33ms 递增，而实际编码速率偏离 30fps，码率就会被等比放大：
/// 实测在 83fps 灌帧时，固定 33ms 的 pts 让编码器以为时间只走了 36%，
/// 于是产出 340% 的目标码率（7377kbps 对目标 1675kbps）。
///
/// 生产环境同样受影响：桌面静止后帧变稀疏（3-5fps）时 pts 只走 1/10，
/// 编码器会误以为时间凝固而几乎不分配比特，画面恢复运动时再猛冲。
///
/// 用单调时钟取真实毫秒即可让码率控制与帧率解耦。
fn frame_ms(start: &Instant) -> i64 {
    start.elapsed().as_millis() as i64
}

/// 按显示器设备名查询当前尺寸（与 [`crate::sos_display::size_by_name`] 同义）
fn display_size_by_name(name: &str) -> Option<(usize, usize)> {
    crate::sos_display::size_by_name(name)
}

// ── 显示器选择 ──
//
// 显示器是**每个连接独立选择**的：订阅表里记录每个连接看哪块屏
// （`Subscriber.display`），QoS 里按显示器名聚合并行采集所需的帧率与码率。
// 所以这里不再有「当前显示器」这样的全局单值 —— 多个连接可以看不同的屏，
// 每块屏都有独立的采集管线。

/// 枚举所有显示器，返回 `(索引, 宽, 高, 设备名, 原点x, 原点y)`。
///
/// `Display::all()` 开销较大（`CreateDXGIFactory1` + 枚举输出），
/// 只能在重建/切换显示器时调用，**不能**放进每秒的检查里。
fn list_displays() -> ResultType<Vec<(usize, usize, usize, String, i32, i32)>> {
    let list =
        crate::sos_display::list().map_err(|e| anyhow::anyhow!("enumerate displays: {}", e))?;
    Ok(list
        .into_iter()
        .map(|d| (d.index, d.width, d.height, d.name, d.origin.0, d.origin.1))
        .collect())
}

/// 校验显示器索引是否可用，并返回该屏的 `(宽, 高, 设备名)`。
///
/// 切换是低频操作，允许付一次枚举开销来做校验。
pub fn display_info(idx: usize) -> Option<(usize, usize, String)> {
    match list_displays() {
        Ok(list) => match list.get(idx) {
            Some((_, w, h, name, _, _)) => Some((*w, *h, name.clone())),
            None => {
                log::warn!(
                    "[VIDEO] display #{} unavailable: only {} display(s) present",
                    idx,
                    list.len()
                );
                None
            }
        },
        Err(e) => {
            log::warn!("[VIDEO] enumerate displays failed: {}", e);
            None
        }
    }
}

// ── 显示器真实设备名缓存 ──
//
// 为什么需要它：连接可能建立在**锁屏期间**（`Display::all()` 返回空表），
// 此时 `display_info()` 拿不到名字，`Session` 只能记下空串；解锁后采集线程
// 才拿到真实名（`\\.\DISPLAY1`）。两边不一致会让 QoS 按屏聚合彻底失配：
// 采集线程用真实名查 `fps()`/`bitrate_kbps()` 查不到任何观看者，于是帧率
// 恒为默认值、码率上报为 0。
//
// 这里由采集线程写入真实名，`Session` 侧读取，保证两边用同一个 key。
// 读失败/未设置时由调用方回退到自己缓存的名字。

static DISPLAY_NAMES: [Mutex<String>; MAX_DISPLAY_SLOTS] =
    [const { Mutex::new(String::new()) }; MAX_DISPLAY_SLOTS];

/// 记录该屏的真实设备名（由采集线程在捕获器建成后调用）。
pub fn set_display_name(display: usize, name: &str) {
    if name.is_empty() {
        return;
    }
    let slot = display.min(MAX_DISPLAY_SLOTS - 1);
    if let Ok(mut g) = DISPLAY_NAMES[slot].lock() {
        if g.as_str() != name {
            g.clear();
            g.push_str(name);
            log::info!("[VIDEO] display #{} name cached as '{}'", display, name);
        }
    }
}

/// 该屏当前的真实设备名；从未成功采集过时返回 `None`。
///
/// 不在这里回退到 `display_info()`：那会调 `Display::all()`（数十毫秒），
/// 而本函数在每秒的 QoS 同步路径上被调用。
pub fn cached_display_name(display: usize) -> Option<String> {
    let slot = display.min(MAX_DISPLAY_SLOTS - 1);
    match DISPLAY_NAMES[slot].lock() {
        Ok(g) if !g.is_empty() => Some(g.clone()),
        _ => None,
    }
}

fn make_switch_display_msg(
    index: usize,
    width: usize,
    height: usize,
    display_name: &str,
    origin: (i32, i32),
) -> Option<Message> {
    let mut info = DisplayInfo::new();
    // 多显示器下原点不是 (0,0)。控制端靠它把鼠标坐标映射回正确的屏幕。
    info.x = origin.0;
    info.y = origin.1;
    info.width = width as i32;
    info.height = height as i32;
    info.name = display_name.to_string();
    info.online = true;
    // Windows 上光标由 mouse_cursor 服务单独发送，与上游 is_cursor_embedded() 一致
    info.cursor_embedded = false;
    let mut msg = librustdesk::video_service::make_display_changed_msg(
        // 这个索引就是 `SwitchDisplay.display`，控制端靠它把画面归到某块屏。
        // 之前硬编码 0，切到副屏后控制端仍以为是 0 号屏。
        index,
        Some(info),
        librustdesk::video_service::VideoSource::Monitor,
    )?;
    // make_display_changed_msg 可能附带 SwitchDisplay，这里补上光标标志
    if let Some(message::Union::Misc(ref mut misc)) = msg.union {
        if let Some(hbb_common::message_proto::misc::Union::SwitchDisplay(ref mut sd)) = misc.union
        {
            sd.cursor_embedded = false;
        }
    }
    Some(msg)
}

// ── 采集后端选择 ──

/// 当前进程是否运行在**远程桌面会话**里（`SM_REMOTESESSION`）。
///
/// 这个状态解释了一个最常见的困惑：同一台机器，"本地登录时 `cap=5ms`、
/// 通过 RDP 连进去后 `cap=50-80ms`"。原因是 **RDP 会话里 Windows 不提供
/// Desktop Duplication**（虚拟显示输出没有真实 GPU 输出），
/// `DuplicateOutput` 恒定返回 `E_UNEXPECTED 0x8000FFFF`，
/// 于是只能退回 GDI 的 BitBlt —— 单次全屏拷贝 50-80ms 是物理下限。
///
/// 这不是缺陷，也没有软件层的绕过办法：RDP 客户端的画面本身就是
/// RDP 协议自己编码的，我们拿到的是二次捕获。要高速率必须在
/// **物理控制台会话**里运行（无人值守远程协助的常态）。
#[cfg(windows)]
fn is_remote_session() -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_REMOTESESSION};
    unsafe { GetSystemMetrics(SM_REMOTESESSION) != 0 }
}

/// 按「性能优先、逐级降级」创建采集器。
///
/// 通用性策略（不含任何针对特定机器的硬编码）：
/// - 每次创建都从该显示器的 [`min_tier`] 档位开始尝试，成功即用；
/// - 只有明确「不支持」才降档并记住，后续直接跳过该档；
/// - 降档不是永久的：调用方会定期把档位复位后重建，
///   因为驱动更新、会话重连、显示器增减都可能让高档重新可用；
/// - 最低档是自建 GDI，它没有任何前置依赖（不需要 GPU / D3D / 窗口站），
///   是唯一能在任何 Windows 环境工作的保底路径。
///
/// 档位**按显示器独立**：混合显卡笔记本上，接在 iGPU 的输出可能支持
/// desktop duplication，而接在 dGPU 的输出不支持（或反之）。
fn create_capturer(
    slot: usize,
    display: &Display,
) -> ResultType<(Box<dyn TraitCapturer>, &'static str)> {
    if min_tier(slot) <= TIER_DXGI {
        let name = display.name();
        // 用自建 DXGI 而非 `scrap::Capturer`：后者的 Unmap/ReleaseFrame 顺序错误，
        // 会在 fastlane 模式下把 surface 永久钉在 mapped 状态，
        // 之后每次都返回 DXGI_ERROR_INVALID_CALL 且重建无法恢复。
        match crate::sos_dxgi::create(&name) {
            Ok(c) => {
                log::info!("[VIDEO] display #{} capturer=dxgi(sos)", slot);
                return Ok((Box::new(c), "dxgi(sos)"));
            }
            Err(e) => {
                // RDP 会话下这是**必然**的，且不会再恢复（会话不换就一直是它），
                // 所以直接把原因写清楚，省得反复排查「为什么这里只有 GDI」。
                if is_remote_session() {
                    log::warn!(
                        "[VIDEO] display #{} dxgi unavailable in a remote desktop session ({}): \
                         Windows does not expose Desktop Duplication there, falling back to GDI. \
                         Run on the physical console session for ~5ms capture instead of 50-80ms",
                        slot,
                        e
                    );
                    // 会话期间不会再成功，就别每 30/60/120s 空转探测了。
                    set_min_tier(slot, TIER_GDI);
                    return crate::sos_gdi::create(display)
                        .map(|c| (c as Box<dyn TraitCapturer>, "gdi(sos)"))
                        .map_err(|e| anyhow::anyhow!("SOS GDI capturer: {}", e));
                }
                log::info!("[VIDEO] display #{} dxgi unavailable: {}", slot, e);
            }
        }
        set_min_tier(slot, TIER_GDI);
    }

    let capturer =
        crate::sos_gdi::create(display).map_err(|e| anyhow::anyhow!("SOS GDI capturer: {}", e))?;
    log::info!("[VIDEO] display #{} capturer=gdi(sos)", slot);
    Ok((capturer, "gdi(sos)"))
}

/// 采集失败的性质，决定恢复动作
enum FrameFailure {
    /// 桌面无变化 / 等待超时，属正常情况
    NoFrame,
    /// 环境暂时异常（会话切换、锁屏、UAC、桌面被独占），原地重建即可
    Rebuild,
    /// 本档采集方式在此环境不受支持，应降档
    Demote,
}

/// 把采集错误细分类，决定恢复动作。
///
/// 自建 DXGI 已经把 HRESULT 映射成语义正确的 `ErrorKind`，这里据此决策。
fn classify_frame_error(e: &std::io::Error) -> FrameFailure {
    use std::io::ErrorKind::*;
    match e.kind() {
        // 无新帧：桌面静止或等待超时，完全正常
        WouldBlock | TimedOut => FrameFailure::NoFrame,
        // 桌面被独占（全屏独占程序 / 安全桌面），稍后自然恢复
        Interrupted => FrameFailure::NoFrame,
        // 本机确实没有 desktop duplication
        ConnectionRefused => FrameFailure::Demote,
        // 非法调用：说明本实现的调用序列与驱动状态不同步，重建通常无效
        InvalidData => FrameFailure::Demote,
        // ACCESS_LOST / SESSION_DISCONNECTED / E_ACCESSDENIED 等重建即可恢复
        _ => FrameFailure::Rebuild,
    }
}

/// 下调一档采集后端并请求重建
fn demote_capturer(slot: usize, reason: &str) {
    if min_tier(slot) < TIER_GDI {
        set_min_tier(slot, TIER_GDI);
        log::warn!("[VIDEO] display #{} capturer demoted ({})", slot, reason);
    }
    request_refresh_display(slot);
}

// ── 捕获状态 ──

struct CaptureState {
    capturer: Box<dyn TraitCapturer>,
    capturer_name: &'static str,
    encoder: SosVpxEncoder,
    width: usize,
    height: usize,
    display_name: String,
    /// 显示器索引（`scrap::Display::all()` 的顺序）。UAC 帧没有显示器，记为 0。
    display_index: usize,
    /// 显示器在虚拟桌面中的原点（多显示器下非 (0,0)）
    display_origin: (i32, i32),
    threads: usize,
    /// 初始码率（kbps）。重建后用于同步 `cur_kbps`，
    /// 避免把一个已经下发过的值再下发一次。
    initial_bitrate: u32,
    /// 创建时使用的丢帧水位，用于同步 `cur_drop_thresh`。
    initial_drop_thresh: u32,
}

impl CaptureState {
    /// 为指定显示器创建采集器与编码器。
    ///
    /// `slot` 是管线标识（也是 `SwitchDisplay.display` 的取值）；
    /// 显示器被拔掉时回退到 #0，而不是让这条管线永久失败。
    fn new(slot: usize) -> ResultType<Self> {
        let uac = is_uac_active();

        // UAC 安全桌面：使用 SYSTEM 子进程通过共享内存送来的帧
        let (capturer, width, height, display_name, name, idx, origin): (
            Box<dyn TraitCapturer>,
            usize,
            usize,
            String,
            &'static str,
            usize,
            (i32, i32),
        ) = if uac {
            let (shmem, w, h) = crate::sos_shmem::acquire_uac_capturer()?;
            (
                shmem,
                w,
                h,
                "UAC".to_string(),
                "shmem(GDI/SYSTEM)",
                0,
                (0, 0),
            )
        } else {
            let list = list_displays()?;
            if list.is_empty() {
                anyhow::bail!("no display available");
            }
            // 只回退到 #0，**不**跨屏借用：两块屏尺寸可能完全不同，
            // 把 #1 的帧当成 #0 发给控制端会让坐标系彻底错位。
            let target = crate::sos_display::resolve_and_log(slot as i32, list.len())
                .ok_or_else(|| anyhow::anyhow!("no display available"))?;
            let (idx, w, h, dname, ox, oy) = list[target].clone();
            let mut displays = Display::all()?;
            if idx >= displays.len() {
                anyhow::bail!("display #{} vanished while enumerating", idx);
            }
            let display = displays.remove(idx);
            let (capturer, tag) = create_capturer(slot, &display)?;
            (capturer, w, h, dname, tag, idx, (ox, oy))
        };

        let codec = current_codec_for(slot);
        let threads = SosVpxConfig::recommended_threads();
        // `on_connection_open` / `on_connection_close` 由 `sos_connection` 管理，
        // 这里只读**本屏**当前画质比，避免重建时把 QoS 状态重置。
        let ratio = crate::sos_qos::ratio(&display_name);
        let bitrate = SosVpxConfig::bitrate_kbps_for_link(
            width,
            height,
            ratio,
            SosVpxConfig::NOMINAL_FPS as u32,
            link_budget(),
        );
        // 立刻把码率登记到 QoS：控制端显示的 `target_bitrate` 读的就是这个值。
        // 只在画质比变化时才 store 会让它在画质稳定时一直为 0，
        // 客户端因此看到 `qos_bitrate=0kbps`。
        crate::sos_qos::with(|q| q.store_bitrate(&display_name, bitrate));
        let cfg = SosVpxConfig {
            width,
            height,
            codec,
            threads,
            // 降级第 1 级：VP9 用最快的 cpu-used。远程桌面里流畅度优先于
            // 一点点压缩率，何况编不动的时候画质优势根本体现不出来。
            cpu_used: if codec == SosCodec::Vp9 && enc_level(slot) >= 1 {
                SosVpxConfig::VP9_MAX_CPU_USED
            } else {
                SosVpxConfig::default_cpu_used(codec, width, height)
            },
            bitrate_kbps: bitrate,
            keyframe_interval: Some(KEYFRAME_INTERVAL_FRAMES),
        };
        let encoder = SosVpxEncoder::new(cfg)?;
        // 新编码器默认是「守码率」水位；链路充裕时立即切成不丢帧，
        // 否则重建后的头一秒会白白丢一堆帧。
        let drop_thresh = dropframe_thresh_for_link();
        let mut encoder = encoder;
        if drop_thresh != crate::sos_vpx::RC_DROPFRAME_THRESH {
            encoder.tune(bitrate, drop_thresh).ok();
        }
        Ok(Self {
            capturer,
            capturer_name: name,
            encoder,
            width,
            height,
            display_name,
            display_index: idx,
            display_origin: origin,
            threads,
            initial_bitrate: bitrate,
            initial_drop_thresh: drop_thresh,
        })
    }

    fn make_video_msg(&self, frames: Vec<(Vec<u8>, bool)>) -> Option<Message> {
        if frames.is_empty() {
            return None;
        }
        let evf: Vec<EncodedVideoFrame> = frames
            .into_iter()
            .map(|(data, key)| EncodedVideoFrame {
                data: hbb_common::bytes::Bytes::from(data),
                key,
                pts: 0,
                ..Default::default()
            })
            .collect();
        let mut vf = VideoFrame::new();
        let vpxs = EncodedVideoFrames {
            frames: evf.into(),
            ..Default::default()
        };
        match self.encoder.codec() {
            SosCodec::Vp8 => vf.set_vp8s(vpxs),
            SosCodec::Vp9 => vf.set_vp9s(vpxs),
        }
        // 控制端用这个索引把画面归到某块屏（与 `SwitchDisplay.display` 对应）
        vf.display = self.display_index as i32;
        let mut msg = Message::new();
        msg.set_video_frame(vf);
        Some(msg)
    }
}

/// 依据控制端能力/偏好与当前环境选择编码格式。
///
/// 决策顺序见 [`crate::sos_vpx::choose_codec`]：能力约束 > 显式偏好 > `Auto`。
///
/// `Auto` **不做任何基于线程数/分辨率的猜测**：实测表明 VP8 相对 VP9 的
/// 速度关系会随内容反转（同一内容 3 线程下慢 40%，另一种内容下快数倍），
/// 任何静态启发式都无法成立，而画质方向是确定的（VP8 同码率量化器更高）。
///
/// 唯一的例外是**实测出来的**编码成本降级（见 [`ENC_LEVEL`]）：默认配置
/// 在此机器上编不进帧预算时，才逐级降档，且 VP8 那一档要经过实测对比
/// 决定去留（见 [`revert_vp8_if_useless`]）。
///
/// 编码格式是**全局**的（对端能力与偏好对所有屏相同），
/// 只有降级结论按屏独立 —— 所以这里取**任一屏**都还没降级时的默认格式，
/// 实际的每屏降级由 [`current_codec_for`] 处理。
pub fn current_codec() -> SosCodec {
    current_codec_for_impl(None)
}

/// 指定显示器的编码格式（考虑该屏的降级级别）
pub fn current_codec_for(slot: usize) -> SosCodec {
    current_codec_for_impl(Some(slot))
}

fn current_codec_for_impl(slot: Option<usize>) -> SosCodec {
    let (peer_vp8, peer_vp9, prefer) = peer_ability();
    // 该屏的实测降级档位（仅在该屏真的在采集时有意义）
    let level = slot.map(enc_level).unwrap_or(0);
    // 规则本身在 [`codec_with_degradation`] 里，那里有详细的优先级说明，
    // 也有对应的回归测试（`tests/codec_prefer.rs`）。
    crate::sos_vpx::codec_with_degradation(peer_vp8, peer_vp9, prefer, level)
}

/// 供诊断日志使用：当前链路的编码格式与线程数
pub fn describe() -> String {
    format!(
        "codec={:?} threads={}",
        current_codec(),
        SosVpxConfig::recommended_threads()
    )
}

/// 当前链路实际使用的编码格式名（避免日志读取 `scrap` 的全局状态而误报）
pub fn codec_name() -> &'static str {
    match current_codec() {
        SosCodec::Vp8 => "VP8",
        SosCodec::Vp9 => "VP9",
    }
}

#[cfg(test)]
mod tests {
    use super::{should_reemit_retained, MIN_KEYFRAME_INTERVAL};
    use std::time::Duration;

    /// 一个刚过节流窗口的时长
    fn past_throttle() -> Duration {
        MIN_KEYFRAME_INTERVAL + Duration::from_millis(1)
    }

    /// 正常情形：有新帧就直接编，不需要重编保留帧
    #[test]
    fn nothing_pending_needs_no_reemit() {
        assert!(!should_reemit_retained(true, false, false, past_throttle()));
    }

    /// 手上没有可用帧时绝不重编（否则会把旧尺寸/空缓冲喂给编码器）
    #[test]
    fn no_retained_frame_never_reemits() {
        assert!(!should_reemit_retained(false, true, true, past_throttle()));
    }

    /// 编码器丢了一帧且画面此后静止：必须重编把变化补发出去，不受节流限制
    /// （重编的是 P 帧，体积与普通帧相同，节流没有必要）
    #[test]
    fn dropped_frame_is_reemitted_immediately() {
        assert!(should_reemit_retained(true, true, false, Duration::ZERO));
    }

    /// 关键帧请求在节流窗口内要等，否则拥塞时会对已堵链路连发 9 倍大的突发
    #[test]
    fn keyframe_request_respects_throttle() {
        assert!(!should_reemit_retained(
            true,
            false,
            true,
            MIN_KEYFRAME_INTERVAL - Duration::from_millis(1)
        ));
        assert!(should_reemit_retained(true, false, true, past_throttle()));
    }

    /// 关键帧请求恰好在窗口边界上就应生效（不是严格大于）
    #[test]
    fn keyframe_request_fires_at_boundary() {
        assert!(should_reemit_retained(
            true,
            false,
            true,
            MIN_KEYFRAME_INTERVAL
        ));
    }
}
