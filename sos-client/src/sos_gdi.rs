//! SOS 自建 GDI 采集器。
//!
//! 存在原因：`libs/scrap` 的 GDI 路径每帧做了大量冗余工作。实测 1920x1080：
//!
//! | 阶段 | 耗时 |
//! |---|---|
//! | BitBlt（不可避免） | 16.6 ms |
//! | GetDIBits | +0.02 ms |
//! | ARGBMirror + ARGBRotate180（两遍全帧） | +2.3 ms |
//! | 每帧 8MB 临时缓冲分配 | 未单独计量 |
//! | `would_block_if_equal`（8MB memcmp + 8MB memcpy） | 未单独计量 |
//! | **scrap 合计** | **34.6 ms/帧** |
//!
//! 本模块的做法：
//! 1. `CreateDIBSection` 建一个**自顶向下**的 32 位 DIB，`BitBlt` 直接写进这块内存，
//!    **不使用 `GetDIBits`**，也**不需要镜像/旋转**（负 `biHeight` 已让内存顺序正确）；
//! 2. 做一次 8MB 顺序扫描的变化检测，画面没变就返回 `WouldBlock`，
//!    省掉整帧的颜色转换与编码（见下方「变化检测」说明）；
//! 3. 直接返回 `Frame::PixelBuffer(BGRA)`，交给 `libyuv` 做颜色转换。
//!
//! 预期把每帧从 34.6 ms 降到约 17 ms（接近 BitBlt 的理论下限）。
//!
//! # 变化检测
//!
//! GDI 没有「画面变了没」的信号，只能自己比。实测在采集循环里
//! `BitBlt` 占 27-39 ms，而颜色转换 + 编码只占 5-11 ms。
//! 若不做变化检测，画面静止时也会每帧白白重做一次转换与编码。
//!
//! 一次 8MB 顺序扫描约 0.5-1 ms，且带早退：画面一变第一个字就命中，
//! 繁忙时几乎零成本；静止时全量扫描，代价远低于它省下的编码。
//!
//! 注意：`BitBlt` 本身的开销**省不掉** —— 不抓下来就无从比较。
//! 所以本优化只省转换与编码，不减少采集本身的 CPU 开销。

#![cfg(windows)]

use scrap::{Frame, PixelBuffer, TraitCapturer};
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;
use windows::core::PCWSTR;
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleDC, CreateDCW, CreateDIBSection, DeleteDC, DeleteObject, GetDC,
    GetDeviceCaps, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    HBITMAP, HDC, HGDIOBJ, HORZRES, RGBQUAD, SRCCOPY, VERTRES,
};

// `PrintWindow` / `GetDesktopWindow` 自己声明。
//
// `windows` crate 的各个 feature 集合对这些 user32 导出的归属时有变动，
// 直接声明可以免掉 feature 与版本差异 —— 本文件只需要这两个符号。
extern "system" {
    fn GetDesktopWindow() -> isize;
    fn PrintWindow(hwnd: isize, hdc: *mut std::ffi::c_void, flags: u32) -> i32;
}

/// `PW_RENDERFULLCONTENT`：让 `PrintWindow` 渲染 DirectComposition 内容
/// （Win8.1+）。不带它时安全桌面/合成画面往往只能拿到空白。
const PW_RENDERFULLCONTENT_FLAG: u32 = 0x0000_0002;

/// 采集方式（构造时探测选定，帧级失败时自动降级）。
///
/// 顺序即优先级：**纯 `SRCCOPY` 最先** —— 它是权限要求最低的写法；
/// `CAPTUREBLT` 能抓到分层/透明窗口（更正确），但现场日志里它是最先
/// 被拒的那个（`ERROR_ACCESS_DENIED`），所以放第二；
/// `PrintWindow(桌面窗口)` 是最后一道兜底 —— 它走窗口渲染路径，
/// 与屏幕表面的访问检查是两套机制。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CaptureMode {
    BitBlt = 0,
    BitBltCaptureBlt = 1,
    PrintWindow = 2,
}

impl CaptureMode {
    fn next(self) -> Option<Self> {
        match self {
            Self::BitBlt => Some(Self::BitBltCaptureBlt),
            Self::BitBltCaptureBlt => Some(Self::PrintWindow),
            Self::PrintWindow => None,
        }
    }
}

/// 供日志/诊断查询的当前模式（跨 capturer 实例共享，便于现场判断降级到了哪一档）
static ACTIVE_MODE: AtomicU8 = AtomicU8::new(CaptureMode::BitBlt as u8);

fn mode_name(m: CaptureMode) -> &'static str {
    match m {
        CaptureMode::BitBlt => "BitBlt",
        CaptureMode::BitBltCaptureBlt => "BitBlt|CAPTUREBLT",
        CaptureMode::PrintWindow => "PrintWindow(desktop)",
    }
}

/// 连续多少次采集失败之后才降级。
///
/// 取 30（约 1 秒，重试 33ms/次）。现场日志证明失败是**会话级、暂时性**的
/// （两种桌面、三种写法同时失败，21 秒后突然全部成功），所以频繁降档只会
/// 掩盖真正的可用档位、白白多出一堆 GDI 调用。
const DEGRADE_AFTER: u32 = 30;

pub struct SosGdiCapturer {
    /// 目标显示器 DC
    screen_dc: windows::Win32::Graphics::Gdi::HDC,
    /// `screen_dc` 是否来自 `GetDC(NULL)`（必须用 `ReleaseDC` 释放，不能用 `DeleteDC`）
    screen_from_get_dc: bool,
    /// 内存 DC（选入了 DIB section）
    mem_dc: windows::Win32::Graphics::Gdi::HDC,
    bmp: HBITMAP,
    bits: *mut u8,
    width: usize,
    height: usize,
    /// 每行字节数（32 位对齐，等于 width * 4）
    stride: usize,
    /// 显示器在虚拟桌面中的原点（多显示器时需要）
    origin: (i32, i32),
    /// 当前采集方式。帧级失败会自动降级，见 [`TraitCapturer::frame`]。
    mode: CaptureMode,
    /// 连续失败次数（用于决定降级/报错）
    consecutive_fail: u32,
    /// 上一帧的拷贝，用于变化检测
    prev: Vec<u8>,
    /// `prev` 是否已经有有效内容（首帧必须放行）
    have_prev: bool,
}

unsafe impl Send for SosGdiCapturer {}

impl SosGdiCapturer {
    /// 用显示器设备名创建（与 scrap 一致，可正确处理多显示器）
    pub fn new(display: &scrap::Display) -> Result<Self, Box<dyn std::error::Error>> {
        let width = display.width() as usize;
        let height = display.height() as usize;
        if width == 0 || height == 0 {
            return Err("invalid display size".into());
        }
        let name: Vec<u16> = display
            .name()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        unsafe {
            let screen_dc = CreateDCW(PCWSTR(name.as_ptr()), PCWSTR::null(), PCWSTR::null(), None);
            if screen_dc.is_invalid() {
                return Err("CreateDCW failed".into());
            }
            Self::from_screen_dc(screen_dc, false, width, height, display.origin())
        }
    }

    /// 不依赖显示器枚举，直接抓**当前线程所在桌面**的整屏。
    ///
    /// 安全桌面（Winlogon / UAC）上 `EnumDisplayDevicesW` 与
    /// `EnumDisplaySettingsExW` 都拿不到设备，`scrap::Display::all()` 返回空表，
    /// 于是 `SosGdiCapturer::new` 根本无从调用。但屏幕本身一直都在，可以用
    /// 两种不需要枚举的方式拿到屏幕 DC：
    ///
    /// 1. `CreateDCW(L"DISPLAY")` —— 经典写法；
    /// 2. `GetDC(NULL)` —— 当前线程所在桌面的屏幕 DC。
    ///
    /// 两种都试，且**每个候选都做一次真实的 `BitBlt` 自检**：
    /// 现场日志里 `CreateDCW(L"DISPLAY")` 能成功建出 1920x1080 的 DC，
    /// 但随后的 `BitBlt` 连续返回 `E_ACCESSDENIED`(0x80070005) 长达 30 秒 ——
    /// 只在解锁那一刻成功了一次。也就是说「能建 DC」不等于「能抄像素」，
    /// 必须实测。
    ///
    /// 返回 `(采集器, 宽, 高)`。
    pub fn primary() -> Result<(Self, usize, usize), Box<dyn std::error::Error>> {
        // "DISPLAY" 的宽字符形式。手写而不依赖 `w!` 宏，避免版本差异。
        const DISPLAY: [u16; 8] = [
            b'D' as u16,
            b'I' as u16,
            b'S' as u16,
            b'P' as u16,
            b'L' as u16,
            b'A' as u16,
            b'Y' as u16,
            0,
        ];
        // 每个候选的失败原因都记下来，最后一次性报给上层 —— 现场只需要
        // 一行日志就能知道「两种 DC 来源各自是怎么失败的」。
        let mut attempts: Vec<String> = Vec::new();
        unsafe {
            // 候选 1：CreateDCW(L"DISPLAY")
            let screen_dc = CreateDCW(
                PCWSTR(DISPLAY.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                None,
            );
            if !screen_dc.is_invalid() {
                let w = GetDeviceCaps(Some(screen_dc), HORZRES) as usize;
                let h = GetDeviceCaps(Some(screen_dc), VERTRES) as usize;
                if w > 0 && h > 0 {
                    match Self::from_screen_dc(screen_dc, false, w, h, (0, 0)) {
                        Ok(cap) => {
                            log::info!("SosGdiCapturer: source=DISPLAY {}x{}", w, h);
                            return Ok((cap, w, h));
                        }
                        // 失败时 `from_screen_dc` 已经释放了 DC
                        Err(e) => attempts.push(format!("CreateDCW(DISPLAY): {}", e)),
                    }
                } else {
                    let _ = DeleteDC(screen_dc);
                    attempts.push(format!("DISPLAY DC size {}x{}", w, h));
                }
            } else {
                attempts.push("CreateDCW(DISPLAY) failed".into());
            }
            // 候选 2：GetDC(NULL) —— 当前桌面的屏幕 DC
            let screen_dc = GetDC(None);
            if !screen_dc.is_invalid() {
                let w = GetDeviceCaps(Some(screen_dc), HORZRES) as usize;
                let h = GetDeviceCaps(Some(screen_dc), VERTRES) as usize;
                if w > 0 && h > 0 {
                    match Self::from_screen_dc(screen_dc, true, w, h, (0, 0)) {
                        Ok(cap) => {
                            log::info!("SosGdiCapturer: source=GetDC(NULL) {}x{}", w, h);
                            return Ok((cap, w, h));
                        }
                        Err(e) => attempts.push(format!("GetDC(NULL): {}", e)),
                    }
                } else {
                    let _ = ReleaseDC(None, screen_dc);
                    attempts.push(format!("GetDC(NULL) size {}x{}", w, h));
                }
            } else {
                attempts.push("GetDC(NULL) failed".into());
            }
        }
        Err(attempts.join("; ").into())
    }

    unsafe fn from_screen_dc(
        screen_dc: HDC,
        from_get_dc: bool,
        width: usize,
        height: usize,
        origin: (i32, i32),
    ) -> Result<Self, Box<dyn std::error::Error>> {
        if width == 0 || height == 0 {
            Self::release_screen_dc(screen_dc, from_get_dc);
            return Err("invalid display size".into());
        }
        let mem_dc = CreateCompatibleDC(Some(screen_dc));
        if mem_dc.is_invalid() {
            Self::release_screen_dc(screen_dc, from_get_dc);
            return Err("CreateCompatibleDC failed".into());
        }

        // 负 biHeight = 自顶向下，内存顺序与 libyuv 的 BGRA 期望一致
        let mut bmi = BITMAPINFO::default();
        bmi.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width as i32,
            biHeight: -(height as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            biSizeImage: (width * height * 4) as u32,
            ..Default::default()
        };
        bmi.bmiColors = [RGBQUAD::default()];

        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let bmp = match CreateDIBSection(Some(mem_dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0) {
            Ok(b) if !bits.is_null() => b,
            _ => {
                let _ = DeleteDC(mem_dc);
                Self::release_screen_dc(screen_dc, from_get_dc);
                return Err("CreateDIBSection failed".into());
            }
        };
        SelectObject(mem_dc, HGDIOBJ(bmp.0));

        let mut capturer = Self {
            screen_dc,
            screen_from_get_dc: from_get_dc,
            mem_dc,
            bmp,
            bits: bits as *mut u8,
            width,
            height,
            // 32 位 DIB 的行跨距恒为 width * 4（已按 4 字节对齐）
            stride: width * 4,
            origin,
            mode: CaptureMode::BitBlt,
            consecutive_fail: 0,
            prev: vec![0u8; width * height * 4],
            have_prev: false,
        };

        // ── 自检：建得出 DC 不代表抄得动像素 ──
        //
        // 但自检**不能否决采集器**。上一版把「自检全失败」当构造失败，于是
        // 上层每秒重建一次采集器（每次重建 = 2 个 DC + 8MB DIB + 18 次 BitBlt
        // + 6 行 WARN），而现场真正的状态是「会话级、暂时性不可采」——
        // 21 秒后同样的代码一次就成功了。
        //
        // 现在的做法：只用 32x32 小区域探测一次，把结果记进日志，然后**始终**
        // 把采集器交出去；真正的重试由 `frame()` 负责（每次只有一次 BitBlt，
        // 可以 25-30Hz 地试）。
        let mut probe_err = None;
        let mut mode = CaptureMode::BitBlt;
        loop {
            match capturer.probe(mode) {
                Ok(()) => break,
                Err(e) => {
                    probe_err = Some(e.to_string());
                    match mode.next() {
                        Some(n) => mode = n,
                        None => break,
                    }
                }
            }
        }
        let probe_ok = probe_err.is_none();
        if !probe_ok {
            // 探测失败无法降档，回退到最低要求的一档交给 `frame()` 重试
            mode = CaptureMode::BitBlt;
        }
        capturer.mode = mode;
        ACTIVE_MODE.store(mode as u8, Ordering::Relaxed);

        log::info!(
            "SosGdiCapturer: {}x{} origin=({},{}) stride={} mode={} probe={}{}",
            width,
            height,
            origin.0,
            origin.1,
            capturer.stride,
            mode_name(mode),
            if probe_ok { "ok" } else { "FAILED" },
            if probe_ok {
                String::new()
            } else {
                format!(" ({})", probe_err.unwrap_or_default())
            }
        );
        Ok(capturer)
    }

    /// 便宜的可用性探测：只 BLT 左上角 32x32。
    ///
    /// 失败本身不贵（直接被拒），但**成功**时会拷 8MB；用小区域可以在
    /// 几十微秒内判定「现在能不能抄」，适合高频重试。
    fn probe(&self, mode: CaptureMode) -> std::io::Result<()> {
        const P: i32 = 32;
        match mode {
            CaptureMode::BitBlt | CaptureMode::BitBltCaptureBlt => {
                let rop = if mode == CaptureMode::BitBltCaptureBlt {
                    SRCCOPY | windows::Win32::Graphics::Gdi::CAPTUREBLT
                } else {
                    SRCCOPY
                };
                unsafe {
                    BitBlt(
                        self.mem_dc,
                        0,
                        0,
                        P.min(self.width as i32),
                        P.min(self.height as i32),
                        Some(self.screen_dc),
                        self.origin.0,
                        self.origin.1,
                        rop,
                    )
                    .map_err(|e| {
                        std::io::Error::new(std::io::ErrorKind::Other, format!("BitBlt: {}", e))
                    })
                }
            }
            CaptureMode::PrintWindow => self.capture_with(mode),
        }
    }

    unsafe fn release_screen_dc(dc: HDC, from_get_dc: bool) {
        if from_get_dc {
            let _ = ReleaseDC(None, dc);
        } else {
            let _ = DeleteDC(dc);
        }
    }

    /// 用指定方式抓一帧到 DIB。
    ///
    /// `PrintWindow` 不用 `screen_dc`，它直接让桌面窗口把自己画进内存 DC ——
    /// 走的是窗口渲染路径，与屏幕表面的访问检查是**两套**，所以在
    /// 安全桌面上有独立的机会。
    fn capture_with(&self, mode: CaptureMode) -> std::io::Result<()> {
        match mode {
            CaptureMode::BitBltCaptureBlt | CaptureMode::BitBlt => {
                let rop = if mode == CaptureMode::BitBltCaptureBlt {
                    SRCCOPY | windows::Win32::Graphics::Gdi::CAPTUREBLT
                } else {
                    SRCCOPY
                };
                unsafe {
                    BitBlt(
                        self.mem_dc,
                        0,
                        0,
                        self.width as i32,
                        self.height as i32,
                        Some(self.screen_dc),
                        self.origin.0,
                        self.origin.1,
                        rop,
                    )
                    .map_err(|e| {
                        std::io::Error::new(std::io::ErrorKind::Other, format!("BitBlt: {}", e))
                    })
                }
            }
            CaptureMode::PrintWindow => unsafe {
                let hwnd = GetDesktopWindow();
                if hwnd == 0 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "GetDesktopWindow failed",
                    ));
                }
                if PrintWindow(hwnd, self.mem_dc.0, PW_RENDERFULLCONTENT_FLAG) != 0 {
                    Ok(())
                } else {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("PrintWindow: {}", std::io::Error::last_os_error()),
                    ))
                }
            },
        }
    }
}

impl TraitCapturer for SosGdiCapturer {
    fn frame<'a>(&'a mut self, _timeout: Duration) -> std::io::Result<Frame<'a>> {
        // ── 帧级自动降级 ──
        //
        // 自检通过不代表稳定：现场日志里正是「自检过一次、正式采集连续被拒」。
        // 但要避免「一次瞬时失败就永久丢掉 CAPTUREBLT」—— 层叠窗口捕获会比它差。
        // 所以先连续重试 DEGRADE_AFTER 次，仍然不行才往下换档。
        match self.capture_with(self.mode) {
            Ok(()) => {
                // 从「持续失败」恢复：这条日志是现场判断「什么时候开始有画面」的唯一依据
                if self.consecutive_fail >= DEGRADE_AFTER {
                    log::info!(
                        "SosGdiCapturer: capture recovered after {} failed frames (mode={})",
                        self.consecutive_fail,
                        mode_name(self.mode)
                    );
                }
                self.consecutive_fail = 0;
            }
            Err(e) => {
                self.consecutive_fail += 1;
                if self.consecutive_fail < DEGRADE_AFTER {
                    return Err(e);
                }
                let mut mode = self.mode;
                let mut good = None;
                while let Some(n) = mode.next() {
                    mode = n;
                    if self.capture_with(mode).is_ok() {
                        good = Some(mode);
                        break;
                    }
                }
                match good {
                    Some(m) => {
                        log::warn!(
                            "SosGdiCapturer: degraded {} -> {} after {} failures ({})",
                            mode_name(self.mode),
                            mode_name(m),
                            self.consecutive_fail,
                            e
                        );
                        self.mode = m;
                        ACTIVE_MODE.store(m as u8, Ordering::Relaxed);
                        // 换档后画面来源可能不同，变化检测基线作废
                        self.have_prev = false;
                        self.consecutive_fail = 0;
                    }
                    // 已到底档仍失败 —— 交给上层重建（可能换成默认桌面）
                    None => return Err(e),
                }
            }
        }

        let len = self.stride * self.height;
        let bits = self.bits;

        // 变化检测。`capture_with` 已经把内容写进 DIB，这里顺序扫一遍与上一帧比较；
        // 相同就返回 `WouldBlock`，由上层按正常「无新帧」路径处理。
        {
            let force = FORCE_EMIT.swap(false, std::sync::atomic::Ordering::Relaxed);
            let cur = unsafe { std::slice::from_raw_parts(bits as *const u8, len) };
            let changed = frame_changed(&mut self.prev, &mut self.have_prev, cur);
            if !changed && !force {
                return Err(std::io::Error::from(std::io::ErrorKind::WouldBlock));
            }
        }

        // DIB section 的内存在 Capturer 生命周期内有效且地址固定
        let data = unsafe { std::slice::from_raw_parts(bits as *const u8, len) };
        Ok(Frame::PixelBuffer(PixelBuffer::with_BGRA(
            data,
            self.width,
            self.height,
        )))
    }

    fn is_gdi(&self) -> bool {
        true
    }

    fn set_gdi(&mut self) -> bool {
        true
    }
}

impl Drop for SosGdiCapturer {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteObject(HGDIOBJ(self.bmp.0));
            let _ = DeleteDC(self.mem_dc);
            // `GetDC` 拿到的必须 `ReleaseDC`，`CreateDCW` 的必须 `DeleteDC`。
            // 混用会泄漏（`ReleaseDC` 对 `CreateDCW` 的 DC 直接失败）。
            Self::release_screen_dc(self.screen_dc, self.screen_from_get_dc);
        }
    }
}

/// 供 `sos_video` 使用：按显示器创建优化版 GDI 采集器
pub fn create(
    display: &scrap::Display,
) -> Result<Box<dyn TraitCapturer>, Box<dyn std::error::Error>> {
    Ok(Box::new(SosGdiCapturer::new(display)?))
}

/// 供 `sos_capture` 使用：不依赖显示器枚举，直接抓当前桌面的整屏。
///
/// 安全桌面（Winlogon / UAC）上唯一可用的采集方式 —— 那里
/// `scrap::Display::all()` 返回空表。返回 `(采集器, 宽, 高)`。
pub fn primary() -> Result<(Box<dyn TraitCapturer>, usize, usize), Box<dyn std::error::Error>> {
    let (cap, w, h) = SosGdiCapturer::primary()?;
    Ok((Box::new(cap), w, h))
}

// ── 状态图（无法采集时给控制端一个明确的理由） ──

// GDI 文字/填充相关的最小 FFI 声明。
//
// 与 `PrintWindow` 同理：这些 gdi32 导出在 `windows` crate 的 feature
// 集合里归属时有变动，本文件只用到几个，直接声明最稳。
//
// 句柄一律用 `*mut c_void` 而不是 `isize`，这样可以直接吃
// `windows` crate 的 `HDC.0` / `HGDIOBJ(...)`，避免到处 `as isize`。
type RawHandle = *mut std::ffi::c_void;

extern "system" {
    fn FillRect(hdc: RawHandle, rect: *const StatusRect, brush: RawHandle) -> i32;
    fn CreateSolidBrush(color: u32) -> RawHandle;
    fn SetBkMode(hdc: RawHandle, mode: i32) -> i32;
    fn SetTextColor(hdc: RawHandle, color: u32) -> u32;
    fn DrawTextW(
        hdc: RawHandle,
        text: *const u16,
        len: i32,
        rect: *mut StatusRect,
        format: u32,
    ) -> i32;
    #[allow(clippy::too_many_arguments)]
    fn CreateFontW(
        c_height: i32,
        c_width: i32,
        c_escapement: i32,
        c_orientation: i32,
        c_weight: i32,
        b_italic: u32,
        b_underline: u32,
        b_strike_out: u32,
        i_char_set: u32,
        i_out_precision: u32,
        i_clip_precision: u32,
        i_quality: u32,
        i_pitch_and_family: u32,
        face_name: *const u16,
    ) -> RawHandle;
}

#[repr(C)]
struct StatusRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

/// `RGB(r,g,b)` —— GDI 的字节序是 0x00BBGGRR
const fn rgb(r: u32, g: u32, b: u32) -> u32 {
    (r & 0xFF) | ((g & 0xFF) << 8) | ((b & 0xFF) << 16)
}

/// 把若干行文字渲染成一张 `width × height` 的 **BGRA** 位图数据（自顶向下）。
///
/// 纯 GDI 绘制 + `CreateDIBSection`，**不需要显示表面**，所以在会话断开、
/// 锁屏、没有显示器时都能正常产出 —— 这一点和抓屏有本质区别，也正是它能
/// 用来解释「为什么没有画面」的原因。
///
/// 只返回数据而不是采集器：调用方（`sos_capture`）直接写进 SHMEM 即可，
/// 不必替换正在重试的实时采集器，避免把状态图和恢复逻辑耦合在一起。
pub fn status_image_bgra(
    width: usize,
    height: usize,
    lines: &[&str],
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    if width == 0 || height == 0 {
        return Err("invalid size".into());
    }
    if width > 16_384 || height > 16_384 {
        return Err("size too large".into());
    }

    let text = lines.join("\r\n");
    let text_wide: Vec<u16> = text.encode_utf16().collect();
    let face: Vec<u16> = "Microsoft YaHei"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    // 字号按图高取，保证 1080p 与 720p 都可读
    let font_height = -((height as i32 / 22).clamp(14, 72));

    unsafe {
        let mem_dc = CreateCompatibleDC(None);
        if mem_dc.is_invalid() {
            return Err("CreateCompatibleDC failed".into());
        }

        let mut bmi = BITMAPINFO::default();
        bmi.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width as i32,
            biHeight: -(height as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            biSizeImage: (width * height * 4) as u32,
            ..Default::default()
        };
        bmi.bmiColors = [RGBQUAD::default()];

        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let bmp = match CreateDIBSection(Some(mem_dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0) {
            Ok(b) if !bits.is_null() => b,
            _ => {
                let _ = DeleteDC(mem_dc);
                return Err("CreateDIBSection failed".into());
            }
        };
        SelectObject(mem_dc, HGDIOBJ(bmp.0));

        // 深色背景 + 浅灰文字，和「黑屏」在视觉上明确不同
        let full = StatusRect {
            left: 0,
            top: 0,
            right: width as i32,
            bottom: height as i32,
        };
        let brush = CreateSolidBrush(rgb(0x1E, 0x1E, 0x1E));
        if !brush.is_null() {
            FillRect(mem_dc.0, &full, brush);
            let _ = DeleteObject(HGDIOBJ(brush));
        }

        SetBkMode(mem_dc.0, 1); // TRANSPARENT
        SetTextColor(mem_dc.0, rgb(0xE8, 0xE8, 0xE8));

        // FW_NORMAL=400, DEFAULT_CHARSET=1, CLEARTYPE_QUALITY=5
        let font = CreateFontW(
            font_height,
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            1,
            0,
            0,
            5,
            0,
            face.as_ptr(),
        );
        let old_font = if !font.is_null() {
            Some(SelectObject(mem_dc, HGDIOBJ(font)))
        } else {
            None
        };

        // 留出边距，居中显示
        let pad_x = (width as i32 / 12).max(8);
        let pad_y = (height as i32 / 8).max(8);
        let mut rect = StatusRect {
            left: pad_x,
            top: pad_y,
            right: width as i32 - pad_x,
            bottom: height as i32 - pad_y,
        };
        // DT_CENTER|DT_VCENTER|DT_WORDBREAK|DT_NOPREFIX
        const FLAGS: u32 = 0x0001 | 0x0004 | 0x0010 | 0x0800;
        DrawTextW(
            mem_dc.0,
            text_wide.as_ptr(),
            text_wide.len() as i32,
            &mut rect,
            FLAGS,
        );

        if let Some(old) = old_font {
            SelectObject(mem_dc, old);
        }
        if !font.is_null() {
            let _ = DeleteObject(HGDIOBJ(font));
        }

        // 拷出像素后立刻释放 GDI 资源
        let len = width * height * 4;
        let data = std::slice::from_raw_parts(bits as *const u8, len).to_vec();

        let _ = DeleteObject(HGDIOBJ(bmp.0));
        let _ = DeleteDC(mem_dc);

        log::info!(
            "SosGdiCapturer: status image {}x{} ({} lines)",
            width,
            height,
            lines.len()
        );
        Ok(data)
    }
}

/// 「下一帧无条件输出」标志。
///
/// 堵一个死角：编码器为守码率丢掉了一帧（内容确实变了），而之后画面恰巧
/// 完全静止 —— 变化检测会把后续帧全部判为「没变」，于是那次变化**永远**
/// 补不上，控制端就停在旧画面上。上层发现编码器丢帧时调用本函数。
static FORCE_EMIT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// 请求下一次 `frame()` 无视变化检测，无条件输出一帧。
pub fn request_force_emit() {
    FORCE_EMIT.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// 变化检测核心：`cur` 与 `prev` 不同则返回 `true` 并把 `prev` 更新为 `cur`。
///
/// 独立成自由函数便于单测（不需要真实显示器）。
fn frame_changed(prev: &mut Vec<u8>, have_prev: &mut bool, cur: &[u8]) -> bool {
    if prev.len() != cur.len() {
        // 分辨率切换：尺寸一变必然是新内容
        prev.resize(cur.len(), 0);
        prev.copy_from_slice(cur);
        *have_prev = true;
        return true;
    }
    if *have_prev && prev.as_slice() == cur {
        return false;
    }
    prev.copy_from_slice(cur);
    *have_prev = true;
    true
}

#[cfg(test)]
mod tests {
    use super::frame_changed;

    fn state(len: usize) -> (Vec<u8>, bool) {
        (vec![0u8; len], false)
    }

    #[test]
    fn first_frame_always_passes() {
        let (mut prev, mut have) = state(16);
        // 内容全 0 也必须放行，否则纯黑画面永远出不了首帧
        assert!(frame_changed(&mut prev, &mut have, &[0u8; 16]));
        assert!(have);
    }

    #[test]
    fn identical_frame_is_skipped() {
        let (mut prev, mut have) = state(16);
        let a = [7u8; 16];
        assert!(frame_changed(&mut prev, &mut have, &a));
        assert!(!frame_changed(&mut prev, &mut have, &a));
        assert!(!frame_changed(&mut prev, &mut have, &a));
    }

    #[test]
    fn single_byte_change_is_detected() {
        let (mut prev, mut have) = state(16);
        let mut a = [7u8; 16];
        assert!(frame_changed(&mut prev, &mut have, &a));
        // 只改最后一个字节：早退比较不能漏掉尾部
        a[15] = 8;
        assert!(frame_changed(&mut prev, &mut have, &a));
        assert!(!frame_changed(&mut prev, &mut have, &a));
        // 再改回首字节
        a[0] = 9;
        assert!(frame_changed(&mut prev, &mut have, &a));
    }

    #[test]
    fn size_change_is_always_new() {
        let (mut prev, mut have) = state(16);
        assert!(frame_changed(&mut prev, &mut have, &[1u8; 16]));
        // 分辨率切换：长度不同即使前缀相同也算变化
        assert!(frame_changed(&mut prev, &mut have, &[1u8; 8]));
        assert_eq!(prev.len(), 8);
        assert!(!frame_changed(&mut prev, &mut have, &[1u8; 8]));
    }
}
