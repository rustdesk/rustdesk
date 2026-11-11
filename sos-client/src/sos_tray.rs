//! Windows 系统托盘模块
//!
//! 提供任务栏托盘图标和右键菜单交互。
//! 使用 Win32 API（windows crate）实现，无额外 UI 框架依赖。

#![allow(non_snake_case)]

use tokio::sync::mpsc;

/// 托盘命令枚举
#[derive(Debug, Clone)]
pub enum TrayCommand {
    /// 退出程序
    Exit,
    /// 左键双击——显示设备信息
    ShowInfo,
}

#[cfg(windows)]
mod win32_impl {
    use super::TrayCommand;
    use hbb_common::ResultType;
    use tokio::sync::mpsc;
    use windows::Win32::Foundation::*;
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::System::DataExchange::*;
    use windows::Win32::System::LibraryLoader::*;
    use windows::Win32::System::Memory::*;
    use windows::Win32::System::Ole::CF_UNICODETEXT;
    use windows::Win32::UI::Shell::*;
    use windows::Win32::UI::WindowsAndMessaging::*;

    const WM_TRAY_ICON: u32 = WM_USER + 1;
    pub(super) const WM_TRAY_UPDATE_TIP: u32 = WM_USER + 3;
    /// 双击托盘图标：弹出 ID/密码气泡
    pub(super) const WM_TRAY_SHOW_INFO: u32 = WM_USER + 4;
    const TRAY_ICON_ID: u32 = 1;

    /// 托盘窗口句柄，用于 PostMessage 驱动 tooltip 刷新
    pub(super) static TRAY_HWND: std::sync::atomic::AtomicIsize =
        std::sync::atomic::AtomicIsize::new(0);

    const MENU_SHOW_ID: u16 = 100;
    const MENU_SHOW_PASSWORD: u16 = 101;
    const MENU_EXIT: u16 = 6;

    struct TrayState {
        tx: mpsc::UnboundedSender<TrayCommand>,
    }

    /// 获取设备 ID（从注册表实时读取，支持 UUID_MISMATCH 后变更）
    fn get_device_id() -> String {
        crate::sos_config::RegistryConfig::get_id()
    }

    /// 更新托盘提示文字（只刷新文字，不碰图标）
    pub fn run(tx: mpsc::UnboundedSender<TrayCommand>) -> ResultType<()> {
        // 注册窗口类
        let class_name: Vec<u16> = "RustDeskSOS_Tray\0".encode_utf16().collect();
        let hinstance = unsafe { GetModuleHandleW(None)?.into() };

        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(tray_window_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: HICON::default(),
            hCursor: HCURSOR::default(),
            hbrBackground: HBRUSH(5 as *mut std::ffi::c_void), // COLOR_WINDOW+1
            lpszMenuName: windows::core::PCWSTR::null(),
            lpszClassName: windows::core::PCWSTR(class_name.as_ptr()),
        };

        unsafe {
            RegisterClassW(&wc);
        } // returns atom (class id), ignore on success

        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                windows::core::PCWSTR(class_name.as_ptr()),
                windows::core::w!("RustDeskSOS"),
                WS_OVERLAPPEDWINDOW,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                Some(HWND::default()),
                Some(HMENU::default()),
                Some(hinstance),
                None,
            )
        }?;

        // 添加托盘图标（提示文字显示 ID 和密码）
        let tip_text = build_tip_text();
        let tip: Vec<u16> = format!("{}\0", tip_text).encode_utf16().collect();
        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ICON_ID,
            uFlags: NIF_MESSAGE | NIF_TIP | NIF_ICON,
            uCallbackMessage: WM_TRAY_ICON,
            szTip: {
                let mut arr = [0u16; 128];
                let len = tip.len().min(127);
                arr[..len].copy_from_slice(&tip[..len]);
                arr
            },
            ..Default::default()
        };

        // 设置托盘图标（从 exe 嵌入的资源加载）
        nid.hIcon = unsafe {
            let hinst = GetModuleHandleW(None)
                .map(|h| HINSTANCE(h.0))
                .unwrap_or_default();
            LoadIconW(Some(hinst), windows::core::PCWSTR(1 as *const u16)).unwrap_or_default()
        };
        nid.uFlags |= NIF_ICON;
        unsafe {
            let _ = Shell_NotifyIconW(NIM_ADD, &nid);
        }

        // 创建状态对象并保存到窗口
        let state = Box::into_raw(Box::new(TrayState { tx }));
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);
        }

        // 对外公开 HWND，供密码变更时 PostMessage 唤醒刷新 tooltip
        TRAY_HWND.store(hwnd.0 as isize, std::sync::atomic::Ordering::Release);

        // 消息循环
        let mut msg = MSG::default();
        loop {
            let result = unsafe { GetMessageW(&mut msg, Some(HWND::default()), 0, 0) };
            if result.0 == 0 || result.0 == -1 {
                break;
            }
            unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        // 清理
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
        }
        unsafe {
            let _ = Box::from_raw(state);
        }
        Ok(())
    }

    unsafe extern "system" fn tray_window_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if msg == WM_TRAY_ICON {
            let state = get_state(hwnd);
            match lparam.0 as u32 {
                WM_RBUTTONUP => {
                    // 右键菜单前先刷新提示文字
                    update_tip(hwnd);
                    // 复用刚刷新的 ID 构造菜单，避免重复读注册表
                    let device_id = get_device_id();
                    show_context_menu(hwnd, &device_id);
                }
                WM_LBUTTONDBLCLK => {
                    let _ = state.tx.send(TrayCommand::ShowInfo);
                }
                _ => {}
            }
            return LRESULT(0);
        }

        if msg == WM_TRAY_UPDATE_TIP {
            update_tip(hwnd);
            return LRESULT(0);
        }

        if msg == WM_TRAY_SHOW_INFO {
            show_info_balloon(hwnd);
            return LRESULT(0);
        }

        if msg == WM_COMMAND {
            let state = get_state(hwnd);
            let cmd = (wparam.0 & 0xFFFF) as u16;
            match cmd {
                MENU_SHOW_ID => {
                    let _ = open_clipboard_and_set_text(&get_device_id());
                }
                MENU_SHOW_PASSWORD => {
                    // 从内存获取当前密码再复制
                    let pwd = crate::sos_config::get_current_password();
                    if !pwd.is_empty() {
                        let _ = open_clipboard_and_set_text(&pwd);
                    }
                }
                MENU_EXIT => {
                    let _ = state.tx.send(TrayCommand::Exit);
                }
                _ => {}
            }
            return LRESULT(0);
        }

        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }

    unsafe fn get_state(hwnd: HWND) -> &'static mut TrayState {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TrayState;
        &mut *ptr
    }

    /// 更新托盘提示文字（只刷新文字，不碰图标）
    unsafe fn update_tip(hwnd: HWND) {
        let tip_text = build_tip_text();
        let tip: Vec<u16> = format!("{}\0", tip_text).encode_utf16().collect();
        let nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ICON_ID,
            uFlags: NIF_TIP,
            szTip: {
                let mut arr = [0u16; 128];
                let len = tip.len().min(127);
                arr[..len].copy_from_slice(&tip[..len]);
                arr
            },
            ..Default::default()
        };
        let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
    }

    fn build_tip_text() -> String {
        let device_id = get_device_id();
        let pwd = crate::sos_config::get_current_password();
        if pwd.is_empty() {
            format!("RustDesk SOS\nID: {}", device_id)
        } else {
            format!("RustDesk SOS\nID: {}\n密码: {}", device_id, pwd)
        }
    }

    /// 弹出气泡通知显示设备 ID 与当前密码（双击托盘图标触发）
    unsafe fn show_info_balloon(hwnd: HWND) {
        let device_id = get_device_id();
        let pwd = crate::sos_config::get_current_password();
        let body = if pwd.is_empty() {
            format!("ID: {}", device_id)
        } else {
            format!("ID: {}\n密码: {}", device_id, pwd)
        };
        let title: Vec<u16> = "RustDesk SOS\0".encode_utf16().collect();
        let body: Vec<u16> = format!("{}\0", body).encode_utf16().collect();

        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ICON_ID,
            uFlags: NIF_INFO,
            dwInfoFlags: NIIF_INFO,
            ..Default::default()
        };
        let n = body.len().min(nid.szInfo.len() - 1);
        nid.szInfo[..n].copy_from_slice(&body[..n]);
        let t = title.len().min(nid.szInfoTitle.len() - 1);
        nid.szInfoTitle[..t].copy_from_slice(&title[..t]);
        if !Shell_NotifyIconW(NIM_MODIFY, &nid).as_bool() {
            log::warn!("Failed to show tray balloon");
        }
    }

    unsafe fn show_context_menu(hwnd: HWND, device_id: &str) {
        if let Ok(menu) = CreatePopupMenu() {
            let id_text = format!("ID: {}\0", device_id);
            let id_wide: Vec<u16> = id_text.encode_utf16().collect();
            let _ = AppendMenuW(
                menu,
                MF_STRING,
                MENU_SHOW_ID as usize,
                windows::core::PCWSTR(id_wide.as_ptr()),
            );

            let pwd = crate::sos_config::get_current_password();
            let pwd_display = if pwd.is_empty() {
                "密码: (未设置)\0".to_string()
            } else {
                format!("密码: {}\0", pwd)
            };
            let pwd_wide: Vec<u16> = pwd_display.encode_utf16().collect();
            let _ = AppendMenuW(
                menu,
                MF_STRING,
                MENU_SHOW_PASSWORD as usize,
                windows::core::PCWSTR(pwd_wide.as_ptr()),
            );

            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
            let _ = AppendMenuW(
                menu,
                MF_STRING,
                MENU_EXIT as usize,
                windows::core::w!("退出"),
            );

            let mut point = std::mem::zeroed();
            let _ = GetCursorPos(&mut point);

            let _ = SetForegroundWindow(hwnd);
            let _ = TrackPopupMenu(menu, TPM_RIGHTBUTTON, point.x, point.y, Some(0), hwnd, None);
            let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM::default(), LPARAM::default());
            let _ = DestroyMenu(menu);
        }
    }

    fn open_clipboard_and_set_text(text: &str) -> ResultType<()> {
        let wide: Vec<u16> = format!("{}\0", text).encode_utf16().collect();
        unsafe {
            OpenClipboard(Some(HWND::default()))?;
            EmptyClipboard()?;
            let hmem = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2)?;
            let ptr = GlobalLock(hmem) as *mut u16;
            if ptr.is_null() {
                CloseClipboard()?;
                return Err(anyhow::anyhow!("GlobalLock failed"));
            }
            std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
            GlobalUnlock(hmem)?;
            SetClipboardData(CF_UNICODETEXT.0 as u32, Some(HANDLE(hmem.0)))?;
            CloseClipboard()?;
        }
        Ok(())
    }
}

/// 密码变更时通知托盘刷新 tooltip（不依赖定时器）
/// 通过 PostMessage 发送 WM_TRAY_UPDATE_TIP，在托盘线程的消息循环中异步处理
#[cfg(windows)]
pub fn notify_password_changed() {
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::PostMessageW;
    let raw = win32_impl::TRAY_HWND.load(std::sync::atomic::Ordering::Acquire);
    if raw != 0 {
        unsafe {
            let _ = PostMessageW(
                Some(HWND(raw as *mut _)),
                win32_impl::WM_TRAY_UPDATE_TIP,
                WPARAM::default(),
                LPARAM::default(),
            );
        }
    }
}

#[cfg(not(windows))]
pub fn notify_password_changed() {}

/// 弹出显示设备 ID 与密码的气泡（主循环收到 `TrayCommand::ShowInfo` 时调用）
#[cfg(windows)]
pub fn show_info() {
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::PostMessageW;
    let raw = win32_impl::TRAY_HWND.load(std::sync::atomic::Ordering::Acquire);
    if raw != 0 {
        unsafe {
            let _ = PostMessageW(
                Some(HWND(raw as *mut _)),
                win32_impl::WM_TRAY_SHOW_INFO,
                WPARAM::default(),
                LPARAM::default(),
            );
        }
    }
}

#[cfg(not(windows))]
pub fn show_info() {}

/// 公开的托盘入口（非 Windows 平台提供空实现）
#[cfg(not(windows))]
pub fn run(_tx: mpsc::UnboundedSender<TrayCommand>) -> hbb_common::ResultType<()> {
    log::warn!("System tray is only supported on Windows");
    // 阻塞以保持兼容接口
    std::thread::park();
    Ok(())
}

/// 公开的托盘入口
#[cfg(windows)]
pub fn run(tx: mpsc::UnboundedSender<TrayCommand>) -> hbb_common::ResultType<()> {
    win32_impl::run(tx)
}
