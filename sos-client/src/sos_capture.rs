//! GDI 帧捕获模块
//!
//! 安全桌面（锁屏 / UAC）时，SYSTEM 子进程通过 GDI 抓屏写入 SHMEM。

#[cfg(windows)]
extern "C" {
    fn SosSwitchToInputDesktop() -> i32;
}

use crate::sos_shmem::{
    SosShmem, SHMEM_ADDR_CAPTURE_FRAME, SHMEM_ADDR_CAPTURE_FRAME_COUNTER,
    SHMEM_ADDR_CAPTURE_FRAME_INFO, SHMEM_ADDR_CAPTURE_WOULDBLOCK,
};
use scrap::{Capturer, Frame, TraitCapturer, TraitPixelBuffer};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// 采集器创建失败后的重试间隔。
///
/// 只有「建 DC / 分配 DIB 就失败」才走这里（真正的稀缺资源）；
/// 「建得起来但抄不动像素」属于会话级暂时状态，走 [`FRAME_RETRY_INTERVAL`]。
const CAPTURER_RETRY_INTERVAL: Duration = Duration::from_secs(1);

/// 采集器已存在、但单帧失败时的重试间隔。
///
/// 现场日志：不可采状态持续了 **21 秒**，而旧实现每次失败都重建采集器
/// （2 个 DC + 8MB DIB + 18 次 BitBlt + 6 行 WARN），重试频率被压在 1Hz。
/// 保持采集器存活后单帧重试只需要一次 BitBlt，可以跑到 25Hz，
/// 一旦会话恢复可采就 **40ms 内**出首帧。
const FRAME_RETRY_INTERVAL: Duration = Duration::from_millis(40);

/// 连续多久采不到帧就重建一次采集器（刷新 DC 与 DIB）。
const CAPTURER_REBUILD_AFTER: Duration = Duration::from_secs(10);

/// 硬错误（非 `WouldBlock`）日志的最小间隔。
///
/// 现场日志里 `BitBlt` 受阻时每帧一条 ERROR，30 秒 900 行。
const HARD_ERROR_LOG_INTERVAL: Duration = Duration::from_secs(5);

/// 会话连接状态的刷新间隔。
///
/// `WTSQuerySessionInformationW` 是本地调用（微秒级），但没必要每帧问一次 ——
/// 会话状态是秒级变化的。
const SESSION_CHECK_INTERVAL: Duration = Duration::from_secs(2);

/// 状态图尺寸。
///
/// 必须能装进 SHMEM 预留的帧缓冲（`create_shmem` 按最大显示器尺寸或
/// 2560x1440 预留）。1280x720 在所有场景下都安全。
const STATUS_IMAGE_SIZE: (usize, usize) = (1280, 720);

/// 连续失败多久之后开始给控制端发「原因说明图」。
///
/// 取 5 秒：正常的桌面切换 / 显示器重枚举会在 1-2 秒内恢复，
/// 超过 5 秒还没画面基本就是硬状态（会话断开、无显示器）。
const STATUS_AFTER_FAILING: Duration = Duration::from_secs(5);

/// 生成「当前无法采集」的说明文字。
///
/// 现场的证据链：用户在 **RDP 会话内**启动 SOS → 关闭 RDP → 从控制端接入。
/// 此时会话变成 `WTSDisconnected`，Windows **不再为该会话合成桌面**，
/// 于是 `BitBlt` 在两个桌面上都返回 `E_ACCESSDENIED`（0x80070005），
/// `DXGI` 也没有输出 —— 和显示器休眠、和锁屏权限都无关。
///
/// 用户重新接入 RDP 的那一刻（画面恢复合成），同一个 `CreateDCW(L"DISPLAY")`
/// + 同一个 `CAPTUREBLT` 立刻就成功了 —— 现场日志 412.366 那一行即是。
///
/// 这是操作系统的硬性限制：**断开的会话没有 framebuffer**，任何抓屏 API
/// 都拿不到像素。能做的只有把原因说清楚，而不是给控制端一片黑。
fn status_lines(session: crate::sos_uac::SessionState) -> Vec<String> {
    let mut v = vec![
        "屏幕内容当前不可用".to_string(),
        "Screen content unavailable".to_string(),
        String::new(),
        format!("会话状态：{}", session.label()),
        crate::sos_uac::desktop_state(),
        String::new(),
    ];
    if !session.may_have_surface() {
        v.push("Windows 不会为「已断开」的会话合成桌面，".to_string());
        v.push("BitBlt / PrintWindow / DXGI 均无法获取像素。".to_string());
        v.push(String::new());
        v.push("请在受控端重新接入远程桌面（或直接登录本机），".to_string());
        v.push("画面会在 1-2 秒内自动恢复，无需重启 SOS。".to_string());
    } else {
        v.push("当前会话没有可用的显示表面（可能无显示器或正在切换）。".to_string());
        v.push(String::new());
        v.push("接入显示器 / 恢复会话后画面会自动出现。".to_string());
    }
    v
}

/// 在 SHMEM 容量允许的范围内挑一个装得下状态图的尺寸。
///
/// 帧缓冲是按**真实显示器**尺寸预留的，小屏机器（1024x768）装不下
/// 1280x720 的状态图，所以必须按容量收缩，否则会越界写。
fn fit_status_size(avail_pixels: usize) -> Option<(usize, usize)> {
    if avail_pixels < 320 * 180 {
        return None;
    }
    let (mut w, mut h) = STATUS_IMAGE_SIZE;
    while w * h > avail_pixels && w > 320 {
        w = w * 3 / 4 / 2 * 2;
        h = h * 3 / 4 / 2 * 2;
    }
    if w * h > avail_pixels {
        return None;
    }
    Some((w, h))
}

/// 往 SHMEM 写一帧并推进计数器。
///
/// 抽成函数是因为「实时帧」与「状态图」两条路径都要做同一套动作，
/// 复制粘贴容易漏掉 `WOULDBLOCK` 标志或计数器，导致主进程读不到新帧。
unsafe fn write_frame(
    shmem: &SosShmem,
    addr_frame: usize,
    data: &[u8],
    w: usize,
    h: usize,
) {
    shmem.write(addr_frame, data);
    shmem.write(SHMEM_ADDR_CAPTURE_WOULDBLOCK, &1i32.to_ne_bytes());
    let info = [
        &data.len().to_ne_bytes()[..],
        &w.to_ne_bytes()[..],
        &h.to_ne_bytes()[..],
    ]
    .concat();
    shmem.write(SHMEM_ADDR_CAPTURE_FRAME_INFO, &info);
    let ptr = shmem.as_ptr().add(SHMEM_ADDR_CAPTURE_FRAME_COUNTER);
    let old = *(ptr as *const i32);
    std::ptr::write_volatile(ptr as *mut i32, if old == i32::MAX { 0 } else { old + 1 });
}

/// 通知主进程「首帧就绪」。
unsafe fn signal_ready(ready_event: isize) {
    if ready_event != 0 {
        extern "system" {
            fn SetEvent(h: isize) -> i32;
        }
        SetEvent(ready_event);
    }
}

/// 建立采集器。返回 `(采集器, 宽, 高)`。
///
/// 返回 `Err` 仅表示「连 DC / DIB 都建不出来」（真正的资源问题）。
/// 「建得起来但抄不动像素」不在这里判定 —— 那是会话级、会自愈的状态，
/// 交给 `frame()` 高频重试。
///
/// # 为什么不再回退到默认桌面
///
/// 上一版在安全桌面失败后切回 `Default` 再试。现场日志证明这是无效的：
///
/// ```text
/// self-test BitBlt|CAPTUREBLT failed 3/3 (拒绝访问 0x80070005)   ← Winlogon
/// SosSwitchToDefaultDesktop returned true
/// self-test BitBlt|CAPTUREBLT failed 3/3 (拒绝访问 0x80070005)   ← Default，完全一样
/// ```
///
/// 两种桌面失败得一模一样，说明不是「桌面权限」而是**整个会话当前没有
/// 可复制的显示表面**。回退只会把重试和日志量翻倍。
fn build_capturer() -> Result<(Box<dyn TraitCapturer>, usize, usize), String> {
    // 屏幕 DC 绑定在**创建时该线程所在的桌面**上，所以必须先切桌面再建。
    if librustdesk::platform::windows::desktop_changed() {
        log::info!("[CAP] Switching capture thread to input desktop...");
        let switched = unsafe { SosSwitchToInputDesktop() } != 0;
        log::info!(
            "[CAP] SosSwitchToInputDesktop returned {} ({})",
            switched,
            crate::sos_uac::desktop_state()
        );
    }
    let (c, w, h) = build_on_current_desktop()?;
    log::info!("[CAP] Capturer built {}x{} on current desktop", w, h);
    Ok((c, w, h))
}

/// 在当前线程所在桌面上建采集器：先试 scrap 枚举（多屏正确），
/// 再退到不依赖枚举的 `sos_gdi::primary()`（内置 BitBlt 自检）。
fn build_on_current_desktop() -> Result<(Box<dyn TraitCapturer>, usize, usize), String> {
    match scrap::Display::all() {
        Ok(mut displays) if !displays.is_empty() => {
            let display = displays.remove(0);
            let dw = display.width() as usize;
            let dh = display.height() as usize;
            match Capturer::new(display) {
                Ok(mut c) => {
                    if c.set_gdi() {
                        log::info!("[CAP] Capturer ready (scrap GDI) {}x{}", dw, dh);
                        return Ok((Box::new(c), dw, dh));
                    }
                    log::warn!("[CAP] scrap set_gdi() failed, falling back to primary");
                }
                Err(e) => log::warn!("[CAP] scrap Capturer::new failed ({:?}), falling back", e),
            }
        }
        // 安全桌面：`EnumDisplayDevicesW` / `EnumDisplaySettingsExW` 拿不到设备
        Ok(_) => {}
        Err(e) => log::warn!("[CAP] Display::all() failed: {}", e),
    }

    let (cap, w, h) =
        crate::sos_gdi::primary().map_err(|e| format!("gdi primary failed: {}", e))?;
    log::info!("[CAP] Capturer ready (GDI primary) {}x{}", w, h);
    Ok((cap, w, h))
}

/// GDI 捕获写 SHMEM（由 SYSTEM 子进程捕获线程调用）。
/// 通过 `active` 标志实现按需启停：仅安全桌面（锁屏 / UAC）时主进程发信号启动。
pub fn run(shmem_name: &str, active: Arc<AtomicBool>) {
    const ADDR_FRAME: usize = SHMEM_ADDR_CAPTURE_FRAME;

    log::info!("[CAP] Opening SHMEM: {}", shmem_name);

    let shmem = match SosShmem::open_existing(shmem_name) {
        Ok(m) => m,
        Err(e) => {
            log::error!("[CAP] Failed to open SHMEM: {}", e);
            return;
        }
    };
    log::info!("[CAP] SHMEM opened size={}", shmem.len());
    if shmem.len() < ADDR_FRAME {
        log::error!("[CAP] SHMEM too small: {} < {}", shmem.len(), ADDR_FRAME);
        return;
    }

    // 打开命名自动重置事件：写完首帧后 SetEvent，主进程 WaitForSingleObject 被唤醒
    let ready_event = {
        let name_wide: Vec<u16> = crate::sos_shmem::scoped_name(
            crate::sos_constants::CAPTURE_READY_EVENT,
        )
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        unsafe {
            extern "system" {
                fn OpenEventW(access: u32, inherit: i32, name: *const u16) -> isize;
            }
            const EVENT_MODIFY_STATE: u32 = 0x0002;
            OpenEventW(EVENT_MODIFY_STATE, 0, name_wide.as_ptr())
        }
    };

    // 采集器懒创建。锁屏 / UAC 刚生效时显示器可能枚举不到，也可能是
    // 线程还没切到输入桌面 —— 这些都会在 1s 内恢复。旧实现在这里直接
    // `return`，捕获线程永久退出，之后无论如何都采不到画面
    // （现象：锁屏一次以后就再也看不到登录界面）。
    let mut cap: Option<(Box<dyn TraitCapturer>, usize, usize)> = None;
    let mut next_build_at = std::time::Instant::now();

    let mut first = true;
    let mut paused_logged = false;
    let mut cap_err_count = 0u32;
    let mut frame_count = 0u64;
    let mut last_heartbeat = std::time::Instant::now();
    // 连续创建失败次数（只用于日志闸门，见下面的懒创建分支）
    let mut build_failures = 0u32;
    // 硬错误（非 WouldBlock）日志闸门：首几次全记，之后每 5s 一条
    let mut hard_err_count = 0u32;
    let mut last_hard_err_log = std::time::Instant::now();

    // ── 会话状态（「RDP 断开」vs「锁屏」）──
    //
    // 上游 `src/platform/windows.cc::is_session_locked()` 用
    // `WTSQuerySessionInformationW(WTSSessionInfoEx)` 的 `SessionFlags`
    // 判断**锁屏**；这里要判断的是更严重的一种：**会话已断开**
    // （`WTSConnectState`），因为它意味着**根本没有 framebuffer** ——
    // 任何抓屏 API 都不可能成功。
    //
    // 上游自己也在 `display_service.rs` 里承认了这个场景无法解决：
    // "The virtual display cannot be added when there's no session
    //  (eg. when exiting from RDP). Because `desktop_changed()` always
    //  returns true at that time."
    let mut session = crate::sos_uac::session_state();
    let mut last_session_check = std::time::Instant::now();
    // 连续失败从什么时候开始（成功出一帧就清空）
    let mut failing_since: Option<std::time::Instant> = None;
    // 下次允许重建采集器的时间点（失败自愈用，防止每帧都重建）
    let mut rebuild_at: Option<std::time::Instant> = None;
    // 是否已经给控制端发过「原因说明图」（避免反复重发）
    let mut status_sent = false;
    log::info!(
        "[CAP] initial session state: {} ({})",
        session.label(),
        crate::sos_uac::desktop_state()
    );

    // 本线程**永不返回**：任何错误都只退避重试。线程一旦退出，之后无论
    // 锁屏多少次都采不到画面（旧实现就是这样永久黑屏的）。

    loop {
        // 等待主进程的捕获启动信号（安全桌面激活时）
        if !active.load(Ordering::Relaxed) {
            if !paused_logged {
                paused_logged = true;
                // 释放采集器：它的 GDI DC 绑定在安全桌面上，留着只是占句柄
                cap = None;
                status_sent = false;
                failing_since = None;
                // 回到**当前输入桌面**（通常是 Default）。`SetThreadDesktop`
                // 是线程级且不可逆的，如果一直挂在旧的 Winlogon 桌面上，
                // 下次 `CreateDCW(L"DISPLAY")` 可能落到已失效的桌面上。
                // 只在真的不同时才切，暂停期间也只做一次。
                if librustdesk::platform::windows::desktop_changed() {
                    let switched = unsafe { SosSwitchToInputDesktop() } != 0;
                    log::info!(
                        "[CAP] Paused; returned capture thread to input desktop ({})",
                        switched
                    );
                }
                log::info!("[CAP] Capture paused, waiting for secure-desktop signal...");
            }
            // 重置状态，确保下次启动时重新输出 "First frame written"
            first = true;
            frame_count = 0;
            cap_err_count = 0;
            last_heartbeat = std::time::Instant::now();
            // park 而非 sleep：控制线程收到 START 信号时会 unpark，实现零延迟唤醒
            // timeout 500ms 为安全兜底（极端情况 unpark 丢失时不会永久阻塞）
            std::thread::park_timeout(Duration::from_millis(500));
            continue;
        }
        paused_logged = false;

        // ── 刷新会话状态 ──
        if last_session_check.elapsed() >= SESSION_CHECK_INTERVAL {
            last_session_check = std::time::Instant::now();
            let cur = crate::sos_uac::session_state();
            if cur != session {
                log::warn!(
                    "[CAP] session state {} -> {} ({})",
                    session.label(),
                    cur.label(),
                    crate::sos_uac::desktop_state()
                );
                session = cur;
                // 状态变化（尤其「重新接入」）→ 立刻重试实时采集，不等退避
                next_build_at = std::time::Instant::now();
                status_sent = false;
                failing_since = None;
            }
        }

        // ── 持续失败 → 给控制端一张「原因说明图」──
        //
        // 这是本轮的核心：会话断开时实时采集**永远不可能成功**，
        // 换任何方法都只会拿到 E_ACCESSDENIED。与其让控制端对着黑屏猜，
        // 不如把状态画出来。图是 GDI 直接画进 DIB 的，不依赖显示表面。
        if !status_sent {
            let failing_too_long = failing_since
                .map(|t| t.elapsed() >= STATUS_AFTER_FAILING)
                .unwrap_or(false);
            if !session.may_have_surface() || failing_too_long {
                let avail = shmem.len().saturating_sub(ADDR_FRAME) / 4;
                match fit_status_size(avail) {
                    Some((sw, sh)) => {
                        let lines = status_lines(session);
                        let refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
                        match crate::sos_gdi::status_image_bgra(sw, sh, &refs) {
                            Ok(data) => {
                                log::warn!(
                                    "[CAP] no live frames (session={}), sending status image {}x{} to viewer",
                                    session.label(),
                                    sw,
                                    sh
                                );
                                unsafe { write_frame(&shmem, ADDR_FRAME, &data, sw, sh) };
                                status_sent = true;
                                // 这也算「首帧」：主进程要据此切到 SHMEM 采集，
                                // 否则控制端连这张说明图都收不到。
                                unsafe { signal_ready(ready_event) };
                                if first {
                                    first = false;
                                    log::info!(
                                        "[CAP] First frame = status image, len={}",
                                        data.len()
                                    );
                                }
                            }
                            Err(e) => log::warn!("[CAP] status image render failed: {}", e),
                        }
                    }
                    None => log::warn!(
                        "[CAP] SHMEM too small for status image ({} px available)",
                        avail
                    ),
                }
            }
        }

        // 懒创建 / 失败重建。失败只等待重试，**绝不退出线程**。
        if cap.is_none() {
            if std::time::Instant::now() < next_build_at {
                std::thread::sleep(CAPTURER_RETRY_INTERVAL);
                continue;
            }
            match build_capturer() {
                Ok(c) => {
                    cap = Some(c);
                    if build_failures > 0 {
                        log::info!(
                            "[CAP] Capturer available again after {} failed attempts",
                            build_failures
                        );
                        build_failures = 0;
                    }
                    // 新采集器 → 首帧要重新 SetEvent 通知主进程
                    first = true;
                    log::info!("[CAP] Capture started, writing frames to SHMEM...");
                }
                Err(why) => {
                    next_build_at = std::time::Instant::now() + CAPTURER_RETRY_INTERVAL;
                    failing_since.get_or_insert_with(std::time::Instant::now);
                    // 只在**首次**失败时记一条。会话被锁/被断开时这里会持续
                    // 失败很久（每一轮都是完整的一次显示器枚举），
                    // 每秒一条 WARN 会把真正有用的日志淹掉。
                    build_failures += 1;
                    if build_failures == 1 {
                        log::warn!(
                            "[CAP] Capturer unavailable ({}), retrying every {:?} ({})",
                            why,
                            CAPTURER_RETRY_INTERVAL,
                            crate::sos_uac::desktop_state()
                        );
                    }
                }
            }
            continue;
        }

        let Some(entry) = cap.as_mut() else { continue };
        let (dw, dh) = (entry.1, entry.2);
        let capturer = &mut entry.0;

        match capturer.frame(Duration::from_millis(33)) {
            Ok(frame) => {
                if let Frame::PixelBuffer(pixels) = frame {
                    let data = pixels.data();
                    let len = data.len();
                    if len > shmem.len().saturating_sub(ADDR_FRAME) {
                        // 绝不能 `break` —— 那会让捕获线程永久退出。
                        // 触发条件是分辨率超过了 SHMEM 预留（例如启动时锁屏，
                        // 缓冲区按当时的显示模式预留，解锁后用户把分辨率调大）。
                        // 丢弃这一帧并重建采集器，线程继续活着，分辨率回落即恢复。
                        log::error!(
                            "[CAP] Frame too large for SHMEM: {} > {}, dropping",
                            len,
                            shmem.len().saturating_sub(ADDR_FRAME)
                        );
                        cap = None;
                        next_build_at = std::time::Instant::now() + CAPTURER_RETRY_INTERVAL;
                        continue;
                    }
                    unsafe { write_frame(&shmem, ADDR_FRAME, data, dw, dh) };
                    if first {
                        log::info!("[CAP] First frame written, len={}", len);
                        first = false;
                        // 通知主进程：首帧就绪，可以切到 SHMEM 采集
                        unsafe { signal_ready(ready_event) };
                    }
                    frame_count += 1;
                    cap_err_count = 0;
                    hard_err_count = 0;
                    // 真的采到了 → 清掉失败计时，并允许下一次硬状态重新发说明图
                    failing_since = None;
                    status_sent = false;
                    if last_heartbeat.elapsed().as_secs() >= 5 {
                        log::info!("[CAP] Heartbeat: {} frames written (GDI)", frame_count);
                        last_heartbeat = std::time::Instant::now();
                    }
                }
            }
            Err(e) => {
                if librustdesk::platform::windows::desktop_changed() {
                    log::info!("[CAP] Desktop changed, rebuilding capturer...");
                    // 整体重建而不是 set_gdi()：`scrap::Display` 里的 DXGI
                    // 对象属于旧桌面，只换一个 DC 不够；重建时
                    // `build_capturer()` 会先 `SosSwitchToInputDesktop()`。
                    cap = None;
                    next_build_at = std::time::Instant::now();
                    cap_err_count = 0;
                    std::thread::sleep(Duration::from_millis(100));
                    continue;
                }
                cap_err_count += 1;
                if e.kind() != std::io::ErrorKind::WouldBlock {
                    // 【日志闸门】硬错误必须限流。
                    //
                    // 现场日志里 `BitBlt` 返回 E_ACCESSDENIED 时这里是每帧一条
                    // ERROR，30 秒刷了 900 行，把真正有用的信息全埋了。
                    // 现在：前 3 次全记，之后每 5 秒一条，且带累计次数。
                    hard_err_count += 1;
                    failing_since.get_or_insert_with(std::time::Instant::now);
                    let due = last_hard_err_log.elapsed() >= HARD_ERROR_LOG_INTERVAL;
                    if hard_err_count <= 3 || due {
                        last_hard_err_log = std::time::Instant::now();
                        log::warn!(
                            "[CAP] Capture error #{} ({:?}): {:?} (session={} {})",
                            hard_err_count,
                            e.kind(),
                            e,
                            session.label(),
                            crate::sos_uac::desktop_state()
                        );
                    }
                    // 【自愈】持续硬失败时定期重建采集器，让 `sos_gdi` 重新
                    // 探测一遍屏幕 DC 来源与采集方式（会话恢复后可能已不同）。
                    // 用**时间**而不是次数做闸门：帧率会变，次数不可靠。
                    if rebuild_at
                        .map(|t| std::time::Instant::now() >= t)
                        .unwrap_or(true)
                    {
                        rebuild_at = Some(std::time::Instant::now() + CAPTURER_REBUILD_AFTER);
                        log::warn!(
                            "[CAP] still no frames after {} errors, rebuilding capturer",
                            hard_err_count
                        );
                        cap = None;
                        next_build_at = std::time::Instant::now();
                    }
                } else {
                    // WouldBlock：画面没变化，这是正常状态（静止桌面）。
                    // 但如果连**首帧**都没出过，说明问题不是「没变化」，
                    // 记账以便触发状态图。
                    if first {
                        failing_since.get_or_insert_with(std::time::Instant::now);
                        if cap_err_count % 100 == 0 {
                            log::info!(
                                "[CAP] still waiting for first frame ({}, session={})",
                                cap_err_count,
                                session.label()
                            );
                        }
                    }
                }
                // 保持 25Hz 的重试节奏：会话一恢复就能在一帧之内出画面
                std::thread::sleep(FRAME_RETRY_INTERVAL);
            }
        }
    }
}
