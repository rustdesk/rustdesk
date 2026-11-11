//! 显示器几何信息查询（索引解析、按屏取尺寸）。
//!
//! 独立成模块的原因：这几处逻辑容易被"看起来等价"的 API 骗到，
//! 且都需要单独的单元测试。
//!
//! # 坐标系：为什么不能用 `EnumDisplaySettingsW`
//!
//! 同一个显示器在不同 API 下会给出**不同**的尺寸：
//!
//! ```text
//! Display::all()[0].width()          = 2304     ← GetMonitorInfo / rcMonitor
//! platform::current_resolution(name) = 2880     ← EnumDisplaySettingsW / dmPelsWidth
//! ```
//!
//! 比值 1.25 就是 DPI 缩放（125%）。`rcMonitor` 与进程的 DPI 感知模式一致
//! （未声明 DPI 感知时被系统缩放），而 `DEVMODE` 永远是物理像素。
//!
//! **采集器用的就是 `Display::width()`**（DXGI 的 `DesktopCoordinates` 同样来自
//! `rcMonitor`），所以尺寸变化检测必须用同一个坐标源。用 `DEVMODE` 会得到
//! 恒定不等的两个值 —— 每秒判一次"分辨率变了"，编码器无限重建。

/// 一台显示器的几何信息
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayEntry {
    /// `scrap::Display::all()` 中的顺序，也是 `SwitchDisplay.display` 的取值
    pub index: usize,
    /// 宽（与 `scrap::Display::width()` 同坐标系）
    pub width: usize,
    /// 高（与 `scrap::Display::height()` 同坐标系）
    pub height: usize,
    /// 设备名，例如 `\\.\DISPLAY1`。空字符串表示未知。
    pub name: String,
    /// 虚拟桌面中的原点（多显示器下非 (0,0)）
    pub origin: (i32, i32),
}

/// 把控制端发来的索引解析为实际可用的索引。
///
/// - 负数一律视为 0（协议字段是 `int32`，异常值不能让程序崩）；
/// - 越界回退到 0 而不是永久失败 —— 显示器可能被拔掉；
/// - 没有任何显示器时返回 `None`。
pub fn resolve_index(requested: i32, available: usize) -> Option<usize> {
    if available == 0 {
        return None;
    }
    let want = if requested < 0 { 0 } else { requested as usize };
    Some(if want < available { want } else { 0 })
}

/// 检测到的尺寸是否真的与当前不同。
///
/// 查询失败（显示器正在切模式、驱动瞬时不可用）时返回 `false`：
/// 把它当成"变了"会导致每次查询失败都重建一次编码器，形成死循环。
pub fn size_changed(current: (usize, usize), detected: Option<(usize, usize)>) -> bool {
    match detected {
        Some((w, h)) => w != current.0 || h != current.1,
        None => false,
    }
}

/// 枚举所有显示器。
///
/// **开销较大**（`Display::all()` 会 `CreateDXGIFactory1` + 枚举输出，
/// 虚拟机上可达数十毫秒），只能在重建捕获器 / 切换显示器时调用，
/// 绝不能放进每秒的检查里。
pub fn list() -> Result<Vec<DisplayEntry>, Box<dyn std::error::Error>> {
    let displays = scrap::Display::all()?;
    Ok(displays
        .iter()
        .enumerate()
        .map(|(index, d)| DisplayEntry {
            index,
            width: d.width() as usize,
            height: d.height() as usize,
            name: d.name(),
            origin: d.origin(),
        })
        .collect())
}

/// 枚举不到任何显示器时的占位尺寸。
///
/// 只在 `primary_size()` 也失败（`GetSystemMetrics` 返回 0）时使用。
/// 必须有值：上游控制端对**空 `displays` 列表是直接报错中止会话**
/// （`handle_peer_info` → "No displays"），因此锁屏 / UAC 安全桌面 /
/// RDP 断开时必须至少上报一个显示器，否则用户根本无法连接。
pub const FALLBACK_SIZE: (usize, usize) = (1024, 768);

/// 主显示器的尺寸（`Display::all()` 拿不到名字时的兜底）
pub fn primary_size() -> Option<(usize, usize)> {
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
        let (w, h) = unsafe { (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)) };
        if w > 0 && h > 0 {
            return Some((w as usize, h as usize));
        }
        return None;
    }
    #[cfg(not(windows))]
    {
        let displays = scrap::Display::all().ok()?;
        let d = displays.first()?;
        Some((d.width() as usize, d.height() as usize))
    }
}

/// 按显示器**设备名**查询它当前的尺寸。
///
/// 用 `EnumDisplayMonitors` + `GetMonitorInfo`，与 DXGI 的
/// `DXGI_OUTPUT_DESC.DesktopCoordinates` 是**同一个数据源**，
/// 因此结果与 `Display::width()/height()` 完全一致（见模块文档的坐标系说明）。
///
/// 相比 `GetSystemMetrics(SM_CXSCREEN/SM_CYSCREEN)`：那两个值只反映**主显示器**。
/// 多显示器下若采集的是副屏，改副屏分辨率后旧实现检测不到变化，
/// 编码器尺寸会一直停在旧值，画面错乱。
///
/// 名字为空或查询失败时回退到主显示器 —— UAC 共享内存帧没有显示器名字。
pub fn size_by_name(name: &str) -> Option<(usize, usize)> {
    if name.is_empty() {
        return primary_size();
    }
    #[cfg(windows)]
    {
        match monitor_rects() {
            Some(list) => match list.iter().find(|(n, _, _)| n.eq_ignore_ascii_case(name)) {
                Some((_, w, h)) => Some((*w, *h)),
                None => {
                    log::debug!("[DISPLAY] '{}' not found among {} monitor(s)", name, list.len());
                    primary_size()
                }
            },
            None => primary_size(),
        }
    }
    #[cfg(not(windows))]
    {
        primary_size()
    }
}

/// 枚举所有显示器的 `(设备名, 宽, 高)`。
///
/// 单次开销约几十微秒（纯内核查询，不创建 DXGI 工厂），可以每秒调用。
#[cfg(windows)]
fn monitor_rects() -> Option<Vec<(String, usize, usize)>> {
    use windows::core::BOOL;
    use windows::Win32::Foundation::{LPARAM, RECT};
    use windows::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFOEXW,
    };

    /// `EnumDisplayMonitors` 的回调：把每块显示器塞进 `LPARAM` 指向的 Vec
    unsafe extern "system" fn collect(
        hmon: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        data: LPARAM,
    ) -> BOOL {
        let out = &mut *(data.0 as *mut Vec<(String, usize, usize)>);
        let mut mi: MONITORINFOEXW = std::mem::zeroed();
        mi.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(hmon, &mut mi.monitorInfo).as_bool() {
            let r = mi.monitorInfo.rcMonitor;
            let w = (r.right - r.left).max(0) as usize;
            let h = (r.bottom - r.top).max(0) as usize;
            let len = mi
                .szDevice
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(mi.szDevice.len());
            out.push((String::from_utf16_lossy(&mi.szDevice[..len]), w, h));
        }
        // 继续枚举下一块显示器（TRUE = 1）
        BOOL(1)
    }

    let mut found: Vec<(String, usize, usize)> = Vec::new();
    unsafe {
        // 回调失败（返回 FALSE）时 Windows 会停止枚举，此时 found 里是已收集到的部分
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(collect),
            LPARAM(&mut found as *mut _ as isize),
        );
    }
    if found.is_empty() {
        None
    } else {
        Some(found)
    }
}

/// 供 `sos_connection` 使用：把 `display` 索引映射到可用的实际索引并记录日志
pub fn resolve_and_log(requested: i32, available: usize) -> Option<usize> {
    let idx = resolve_index(requested, available)?;
    if idx as i32 != requested {
        log::warn!(
            "[DISPLAY] requested display #{} unavailable ({} display(s)), using #{}",
            requested,
            available,
            idx
        );
    }
    Some(idx)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 没有显示器时索引解析必须返回 `None` —— 调用方正是靠它决定要不要上报
    /// 占位显示器。返回 `Some(0)` 会让调用方误以为存在一块屏。
    #[test]
    fn resolve_index_with_no_displays_is_none() {
        assert_eq!(resolve_index(0, 0), None);
        assert_eq!(resolve_index(-1, 0), None);
    }

    /// 占位尺寸必须始终可用：它是「上游对空 displays 直接中止会话」的最后兜底。
    #[test]
    fn fallback_size_is_usable() {
        let (w, h) = FALLBACK_SIZE;
        assert!(w > 0 && h > 0, "占位尺寸不能为 0，否则控制端会当成无显示器");
    }

    /// 尺寸查询失败不能被当成「分辨率变了」，否则会陷入重建死循环。
    #[test]
    fn failed_size_query_is_not_a_change() {
        assert!(!size_changed((1920, 1080), None));
        assert!(size_changed((1920, 1080), Some((1600, 900))));
        assert!(!size_changed((1920, 1080), Some((1920, 1080))));
    }
}
