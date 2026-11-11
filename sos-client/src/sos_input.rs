//! 输入事件分发模块（纯逻辑，不依赖管道）
//!
//! protobuf 反序列化 → 调上游 input_service / enigo。

use crate::sos_constants::{MSG_KEY, MSG_MOUSE, MSG_SHUTDOWN};
use std::sync::Mutex;
use std::time::Instant;

/// 事件速率追踪器
struct RateTracker {
    count: u64,
    last_log: Instant,
}

impl RateTracker {
    fn new() -> Self {
        Self {
            count: 0,
            last_log: Instant::now(),
        }
    }

    fn tick(&mut self, label: &str) {
        self.count += 1;
        let elapsed = self.last_log.elapsed().as_secs_f64();
        if elapsed >= 1.0 {
            let hz = self.count as f64 / elapsed;
            log::info!("[{}] #{} rate={:.0}Hz", label, self.count, hz);
            self.count = 0;
            self.last_log = Instant::now();
        }
    }
}

static MOUSE_RATE: std::sync::LazyLock<Mutex<RateTracker>> =
    std::sync::LazyLock::new(|| Mutex::new(RateTracker::new()));
static KEY_RATE: std::sync::LazyLock<Mutex<RateTracker>> =
    std::sync::LazyLock::new(|| Mutex::new(RateTracker::new()));

/// 分发单个输入事件。返回 false 表示 shutdown。
pub fn dispatch(msg_type: u32, payload: &[u8]) -> bool {
    match msg_type {
        MSG_SHUTDOWN => false,
        MSG_MOUSE => {
            dispatch_mouse(payload);
            MOUSE_RATE.lock().unwrap().tick("MOUSE");
            true
        }
        MSG_KEY => {
            dispatch_key(payload);
            KEY_RATE.lock().unwrap().tick("KEY");
            true
        }
        other => {
            log::debug!("[INPUT] Unknown msg_type: {}", other);
            true
        }
    }
}

fn dispatch_mouse(data: &[u8]) {
    use hbb_common::protobuf::Message;
    let t0 = std::time::Instant::now();
    match hbb_common::message_proto::MouseEvent::parse_from_bytes(data) {
        Ok(me) => {
            let parse_us = t0.elapsed().as_micros();
            // 直接调 **injector** 而不是 `handle_mouse_` wrapper。
            //
            // `handle_mouse_` = `handle_mouse_simulation_` + `handle_mouse_show_cursor_`。
            // 后者负责白板/远程光标渲染，在 SYSTEM 子进程里既没有订阅者、
            // 也不该参与画面合成 —— 纯属浪费，还会读写一些只在本进程内
            // 有意义的光标状态。
            //
            // `handle_mouse_simulation_` 内部完成：`EXITING` 检查 →
            // `try_change_desktop()`（**锁屏/安全桌面能注入的前提**）→
            // 修饰键同步 → Enigo 注入 → 相对鼠标状态维护。
            //
            // `conn = 1`：SOS 单控制端模型下恒定；多控制端时只影响
            // `LATEST_PEER_INPUT_CURSOR` 的归属判断，不影响注入本身。
            librustdesk::input_service::handle_mouse_simulation_(&me, 1);
            let handle_us = t0.elapsed().as_micros() - parse_us;
            // 每秒报告一次耗时统计
            static COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            static PARSE_TOTAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            static HANDLE_TOTAL: std::sync::atomic::AtomicU64 =
                std::sync::atomic::AtomicU64::new(0);
            let n = COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            PARSE_TOTAL.fetch_add(parse_us as u64, std::sync::atomic::Ordering::Relaxed);
            HANDLE_TOTAL.fetch_add(handle_us as u64, std::sync::atomic::Ordering::Relaxed);
            if n % 100 == 0 {
                let avg_parse = PARSE_TOTAL.load(std::sync::atomic::Ordering::Relaxed) / n;
                let avg_handle = HANDLE_TOTAL.load(std::sync::atomic::Ordering::Relaxed) / n;
                log::info!(
                    "[INPUT] #{} parse={}us handle={}us",
                    n,
                    avg_parse,
                    avg_handle
                );
            }
        }
        Err(e) => log::error!("[INPUT] Failed to parse MouseEvent: {}", e),
    }
}

/// 键盘注入。
///
/// **必须整条交给上游 `handle_key_`，不能自己走 Enigo 捷径。**
///
/// 早期实现在 `chr`（字符输入）上做了一条「快速路径」：每个事件新建一个
/// `enigo::Enigo` 直接 `key_down/key_up(Key::Layout(c))`。它比上游少做了
/// 四件事，其中第一件是致命的：
///
/// 1. **`try_change_desktop()`** —— 上游三种键盘模式（Legacy / Map /
///    Translate）的入口都先调它，把**当前线程**切到输入桌面。
///    不走这条路径，线程就停在 `Default` 桌面，而 `Winlogon`（锁屏）和
///    UAC 安全桌面只把输入送给**位于该桌面上**的线程 —— 于是锁屏界面
///    「看得见、打不出字」。
/// 2. `LockModesHandler` —— CapsLock/NumLock 与远端状态强制对齐。
/// 3. `sync_modifiers` / `release_keys` —— 修饰键不泄漏、不残留。
/// 4. `record_pressed_key` —— 记录按下状态。
///
/// 改成直接调 `handle_key_`（而不是 `handle_key`）的理由：后者在 Windows 上
/// 先转到 `portable_service::client::handle_key`，还要判一次 `RUNNING`
/// 再落到 `handle_key_`。SYSTEM 子进程本身就是注入器，这层判断没有意义。
fn dispatch_key(data: &[u8]) {
    use hbb_common::protobuf::Message;
    let ke = match hbb_common::message_proto::KeyEvent::parse_from_bytes(data) {
        Ok(k) => k,
        Err(e) => {
            log::error!("[INPUT] Failed to parse KeyEvent: {}", e);
            return;
        }
    };
    librustdesk::input_service::handle_key_(&ke);
}
