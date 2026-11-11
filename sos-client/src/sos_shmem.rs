//! SOS 共享内存模块
//!
//! SHMEM 创建/打开、布局常量、`SosShmemCapturer` 读帧。
//!
//! 使用 Windows 原生 `CreateFileMappingW(INVALID_HANDLE_VALUE)` +
//! `MapViewOfFile`，以系统分页文件为后端，零磁盘写入。

use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Memory::{
    CreateFileMappingW, MapViewOfFile, OpenFileMappingW, UnmapViewOfFile, VirtualQuery,
    FILE_MAP_ALL_ACCESS, MEMORY_BASIC_INFORMATION, MEMORY_MAPPED_VIEW_ADDRESS, PAGE_READWRITE,
};

// ── SHMEM 帧数据区布局常量（写端 run_capture_shmem 与 读端 SosShmemCapturer 共享） ──

pub const SHMEM_NAME: &str = "sos";
pub const SHMEM_ADDR_CAPTURE_FRAME_INFO: usize = 120;
pub const SHMEM_ADDR_CAPTURE_WOULDBLOCK: usize = 144;
pub const SHMEM_ADDR_CAPTURE_FRAME_COUNTER: usize = 148;
pub const SHMEM_ADDR_CAPTURE_FRAME: usize = 192;

/// 是否使用 `Global\` 命名空间（跨会话可见）。
///
/// 主进程创建成功后由后续的 `open_existing`（SYSTEM 子进程）复用同一名字。
/// 若创建时因缺少 `SeCreateGlobalPrivilege` 失败，会自动退回会话内命名空间，
/// 此时子进程必须留在同一会话 —— 由 `sos_system` 侧读 [`uses_global_namespace`] 决定。
static USE_GLOBAL_NAMESPACE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(true);

/// 当前共享内存是否在 `Global\` 命名空间里（即能否跨会话打开）。
pub fn uses_global_namespace() -> bool {
    USE_GLOBAL_NAMESPACE.load(std::sync::atomic::Ordering::Relaxed)
}

/// 由 `--service` 子进程根据命令行开关设定，保证两侧命名空间一致。
///
/// 主进程创建 SHMEM 时可能因缺少 `SeCreateGlobalPrivilege` 退回会话内名字；
/// 此时若子进程仍按 `Global\` 去打开就会全部失败，所以主进程用开关把
/// 实际策略传给子进程。
pub fn set_global_namespace(on: bool) {
    USE_GLOBAL_NAMESPACE.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// 按当前命名空间策略返回内核对象名（`Global\` 前缀的去留）。
pub fn scoped_name(name: &str) -> String {
    if uses_global_namespace() {
        name.to_string()
    } else {
        name.strip_prefix(r"Global\").unwrap_or(name).to_string()
    }
}

/// 启用 `SeCreateGlobalPrivilege`。
///
/// MSDN: <https://learn.microsoft.com/en-us/windows/win32/termserv/kernel-object-namespaces>
/// "The creation of a file-mapping object in the global namespace from a
///  session other than session zero is a privileged operation."
///
/// 管理员令牌里**持有**该特权但默认**未启用**，必须显式开一次。
/// SYSTEM 令牌里默认已启用，所以子进程侧只打开、不创建。
///
/// 返回是否成功启用（失败不影响同会话路径）。
#[cfg(windows)]
fn enable_create_global_privilege() -> bool {
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Luid {
        low: u32,
        high: i32,
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct LuidAndAttributes {
        luid: Luid,
        attributes: u32,
    }
    #[repr(C)]
    struct TokenPrivileges {
        count: u32,
        privileges: [LuidAndAttributes; 1],
    }

    extern "system" {
        fn GetCurrentProcess() -> isize;
        fn OpenProcessToken(process: isize, access: u32, token: *mut isize) -> i32;
        fn LookupPrivilegeValueW(system: *const u16, name: *const u16, luid: *mut Luid) -> i32;
        #[allow(clippy::too_many_arguments)]
        fn AdjustTokenPrivileges(
            token: isize,
            disable_all: i32,
            new_state: *const TokenPrivileges,
            buffer_len: u32,
            prev_state: *mut TokenPrivileges,
            ret_len: *mut u32,
        ) -> i32;
        fn GetLastError() -> u32;
    }
    const TOKEN_ADJUST_PRIVILEGES: u32 = 0x0020;
    const TOKEN_QUERY: u32 = 0x0008;
    const SE_PRIVILEGE_ENABLED: u32 = 0x0002;
    /// `AdjustTokenPrivileges` 即使「部分特权没授予」也会返回成功，
    /// 必须再查 `GetLastError()` —— 见 MSDN 的说明。
    const ERROR_NOT_ALL_ASSIGNED: u32 = 1300;

    unsafe {
        let mut token: isize = 0;
        if OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &mut token,
        ) == 0
        {
            log::warn!("[SHMEM] OpenProcessToken failed: {}", GetLastError());
            return false;
        }

        let name: Vec<u16> = "SeCreateGlobalPrivilege\0".encode_utf16().collect();
        let mut luid = Luid { low: 0, high: 0 };
        if LookupPrivilegeValueW(std::ptr::null(), name.as_ptr(), &mut luid) == 0 {
            log::warn!("[SHMEM] LookupPrivilegeValue failed: {}", GetLastError());
            let _ = CloseHandle(HANDLE(token as _));
            return false;
        }

        let tp = TokenPrivileges {
            count: 1,
            privileges: [LuidAndAttributes {
                luid,
                attributes: SE_PRIVILEGE_ENABLED,
            }],
        };
        let ok =
            AdjustTokenPrivileges(token, 0, &tp, 0, std::ptr::null_mut(), std::ptr::null_mut());
        let err = GetLastError();
        let _ = CloseHandle(HANDLE(token as _));

        if ok == 0 || err == ERROR_NOT_ALL_ASSIGNED {
            log::warn!(
                "[SHMEM] SeCreateGlobalPrivilege not enabled (ok={} err={}), \
                 falling back to session-local names",
                ok,
                err
            );
            return false;
        }
        log::info!("[SHMEM] SeCreateGlobalPrivilege enabled");
        true
    }
}

#[cfg(not(windows))]
fn enable_create_global_privilege() -> bool {
    false
}

// ── SOS 自有共享内存封装 ──

/// SOS 自有的命名共享内存封装。
///
/// 使用 Windows `CreateFileMappingW(INVALID_HANDLE_VALUE)` + `MapViewOfFile`
/// 创建分页文件后端的命名共享内存，**不留任何磁盘文件痕迹**。
/// Drop 时自动 `UnmapViewOfFile` + `CloseHandle` 清理。
pub struct SosShmem {
    mapping: HANDLE,
    view: MEMORY_MAPPED_VIEW_ADDRESS,
    len: usize,
}

unsafe impl Send for SosShmem {}
unsafe impl Sync for SosShmem {}

impl Drop for SosShmem {
    fn drop(&mut self) {
        unsafe {
            let _ = UnmapViewOfFile(self.view);
            let _ = CloseHandle(self.mapping);
        }
    }
}

impl SosShmem {
    /// Windows 内核对象名（`CreateFileMappingW` 的命名空间）
    ///
    /// # 为什么必须带 `Global\` 前缀
    ///
    /// 默认命名空间是**按会话隔离**的。SYSTEM 子进程现在可能运行在
    /// **控制台会话**（session 1），而主进程在用户的 RDP 会话（session 2）——
    /// 主进程创建的 `RustDeskSOS_sos` 在另一个会话里根本打不开，
    /// 子进程会一直报 `SosShmem open failed`。
    ///
    /// `Global\` 让对象在整个系统范围内可见，才能跨会话共享。
    /// 创建方需要 `SeCreateGlobalPrivilege`（见 [`enable_create_global_privilege`]）。
    fn os_id(name: &str) -> Vec<u16> {
        if USE_GLOBAL_NAMESPACE.load(std::sync::atomic::Ordering::Relaxed) {
            format!("Global\\RustDeskSOS_{}\0", name)
                .encode_utf16()
                .collect()
        } else {
            format!("RustDeskSOS_{}\0", name).encode_utf16().collect()
        }
    }

    pub fn create(name: &str, size: usize) -> hbb_common::ResultType<Self> {
        let high = ((size as u64 >> 32) & 0xFFFF_FFFF) as u32;
        let low = (size as u64 & 0xFFFF_FFFF) as u32;

        // 跨会话共享要求 `Global\` 命名空间 + `SeCreateGlobalPrivilege`。
        let mut wname = Self::os_id(name);
        if USE_GLOBAL_NAMESPACE.load(std::sync::atomic::Ordering::Relaxed) {
            enable_create_global_privilege();
        }

        let mut mapping = unsafe {
            CreateFileMappingW(
                HANDLE(-1isize as _), // INVALID_HANDLE_VALUE → 分页文件后端
                None,
                PAGE_READWRITE,
                high,
                low,
                PCWSTR(wname.as_ptr()),
            )
        };

        // 全局名创建失败（无特权 / 已被别的会话占用）→ 退回会话内命名空间。
        // 此时 SYSTEM 子进程必须留在同一会话，否则它打不开这块内存。
        if mapping.is_err() && USE_GLOBAL_NAMESPACE.load(std::sync::atomic::Ordering::Relaxed) {
            let err = unsafe { windows::Win32::Foundation::GetLastError() };
            log::warn!(
                "[SHMEM] Global\\ create failed (win32 error {}), \
                 falling back to session-local namespace",
                err.0
            );
            USE_GLOBAL_NAMESPACE.store(false, std::sync::atomic::Ordering::Relaxed);
            wname = Self::os_id(name);
            mapping = unsafe {
                CreateFileMappingW(
                    HANDLE(-1isize as _),
                    None,
                    PAGE_READWRITE,
                    high,
                    low,
                    PCWSTR(wname.as_ptr()),
                )
            };
        }
        let mapping = mapping?;

        let view = unsafe { MapViewOfFile(mapping, FILE_MAP_ALL_ACCESS, 0, 0, size) };

        if view.Value.is_null() {
            let err = unsafe { windows::Win32::Foundation::GetLastError() };
            unsafe {
                let _ = CloseHandle(mapping);
            };
            return Err(anyhow::anyhow!(
                "SosShmem create '{}' MapViewOfFile failed: win32 error {}",
                name,
                err.0
            ));
        }

        log::info!(
            "SosShmem created: name={} global={} size={} mapping=0x{:x} ptr=0x{:x}",
            name,
            uses_global_namespace(),
            size,
            mapping.0 as usize,
            view.Value as usize
        );
        Ok(SosShmem {
            mapping,
            view,
            len: size,
        })
    }

    pub fn open_existing(name: &str) -> hbb_common::ResultType<Self> {
        let wname = Self::os_id(name);

        let mapping =
            unsafe { OpenFileMappingW(FILE_MAP_ALL_ACCESS.0, false, PCWSTR(wname.as_ptr())) }?;

        // 映射整个文件映射视图（传递 0 表示映射整个文件）
        let view = unsafe { MapViewOfFile(mapping, FILE_MAP_ALL_ACCESS, 0, 0, 0) };

        if view.Value.is_null() {
            let err = unsafe { windows::Win32::Foundation::GetLastError() };
            unsafe {
                let _ = CloseHandle(mapping);
            };
            return Err(anyhow::anyhow!(
                "SosShmem open '{}' MapViewOfFile failed: win32 error {}",
                name,
                err.0
            ));
        }

        // 查询映射区域的实际大小
        let mut info = MEMORY_BASIC_INFORMATION::default();
        let query_size = unsafe {
            VirtualQuery(
                Some(view.Value),
                &mut info,
                std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
            )
        };
        let region_size = if query_size > 0 { info.RegionSize } else { 0 };

        log::info!(
            "SosShmem opened: name={} mapping=0x{:x} ptr=0x{:x} len={}",
            name,
            mapping.0 as usize,
            view.Value as usize,
            region_size
        );
        Ok(SosShmem {
            mapping,
            view,
            len: region_size,
        })
    }

    pub fn as_ptr(&self) -> *const u8 {
        self.view.Value as *const u8
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn write(&self, addr: usize, data: &[u8]) {
        unsafe {
            debug_assert!(addr + data.len() <= self.len);
            let dst = (self.view.Value as *mut u8).add(addr);
            std::ptr::copy_nonoverlapping(data.as_ptr(), dst, data.len());
        }
    }
}

// ── SHMEM 读取器状态 ──

/// 存放 SOS SHMEM 及尺寸信息（持有所有权，防止悬空指针）
static SHMEM_STATE: std::sync::Mutex<Option<(SosShmem, usize, usize)>> =
    std::sync::Mutex::new(None);
// (shmem, width, height)

/// 建立 SHMEM 映射并记录尺寸，供 SOS 自建视频链路按需取用。
pub fn register_shmem(shmem: SosShmem, width: usize, height: usize) {
    let mut state = SHMEM_STATE.lock().unwrap();
    if state.is_none() {
        *state = Some((shmem, width, height));
        log::info!("SOS SHMEM registered: {}x{}", width, height);
    }
}

/// 为 SOS 自建视频链路提供一个基于 SHMEM 的捕获器（UAC 安全桌面用）。
///
/// 返回 `(捕获器, 宽, 高)`。
///
/// # 宽高必须以 SHMEM 里的帧头为准
///
/// `register_shmem` 传进来的尺寸是**主进程启动时**枚举到的显示器尺寸。
/// 锁屏 / RDP 断开时 `Display::all()` 返回空表，`main` 那边会退化成
/// 1280x720 之类的默认值，而 SYSTEM 子进程实际写的是 1920x1080 的帧。
/// 编码器按错的尺寸建出来，后续 I420 转换会直接错位（花屏或直接失败）。
///
/// 调用时机保证帧头已经写好（`main` 是等到 `SosCaptureReady` 事件才注册的），
/// 所以这里优先读真实帧尺寸。
pub fn acquire_uac_capturer(
) -> hbb_common::ResultType<(Box<dyn scrap::TraitCapturer>, usize, usize)> {
    let state = SHMEM_STATE.lock().unwrap();
    let (shmem, w, h) = state
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("SOS SHMEM not registered"))?;

    let cap = SosShmemCapturer::new(shmem.as_ptr(), shmem.len(), *w, *h);
    let (len, fw, fh) = cap.read_frame_info();
    let (w, h) = if len > 0 && fw > 0 && fh > 0 {
        if fw != *w || fh != *h {
            log::info!(
                "[SHMEM] real frame size {}x{} overrides registered {}x{}",
                fw,
                fh,
                w,
                h
            );
        }
        (fw, fh)
    } else {
        (*w, *h)
    };
    Ok((Box::new(cap), w, h))
}

/// 创建 SOS 自有 SHMEM，返回内存对象。
///
/// **即使枚举不到显示器也必须成功**：锁屏 / UAC 安全桌面 / RDP 断开时
/// `Display::all()` 返回空表，而程序可能是开机自启的 —— 在这种状态下
/// 直接 `bail!` 会让进程起不来，用户解锁后也没有可连的受控端。
/// 拿不到真实尺寸时按 1080p 预留缓冲，真实尺寸在首次成功采集时由
/// 帧信息里的宽高决定（`SosShmemCapturer::read_frame_info`）。
pub fn create_shmem() -> hbb_common::ResultType<SosShmem> {
    let displays = scrap::Display::all().unwrap_or_default();

    const ALIGN: usize = 64;
    const ADDR_FRAME: usize = 192;
    let mut max_pixel = 0;
    for d in &displays {
        let w = d.width() as usize;
        let h = d.height() as usize;
        let pixel = ((w + ALIGN - 1) / ALIGN * ALIGN) * ((h + ALIGN - 1) / ALIGN * ALIGN);
        if pixel > max_pixel {
            max_pixel = pixel;
        }
    }
    if max_pixel == 0 {
        // 无显示器：`Display::all()` 依赖 DXGI/枚举输出，锁屏与安全桌面上会
        // 返回空表；但 `GetSystemMetrics` 走的是当前会话的显示模式，
        // 这种状态下通常仍能给出真实分辨率，所以优先用它。
        //
        // 尺寸必须尽量准：写入侧（`sos_capture.rs`）对超限的帧是
        // `error! + break`（不会越界写，但会退出捕获循环）。所以宁可多留一点。
        let (w, h) = crate::sos_display::primary_size().unwrap_or((2560, 1440));
        max_pixel = ((w + ALIGN - 1) / ALIGN * ALIGN) * ((h + ALIGN - 1) / ALIGN * ALIGN);
        log::warn!(
            "[SHMEM] no display available, reserving {}x{} frame buffer",
            w,
            h
        );
    }
    let shmem_size = (ADDR_FRAME + max_pixel * 4 + ALIGN - 1) / ALIGN * ALIGN;

    let shmem = SosShmem::create(SHMEM_NAME, shmem_size)?;
    log::info!("SOS SHMEM created: size={}", shmem_size);
    Ok(shmem)
}

// ── SOS SHMEM 读取器 ──

/// SOS 自有的共享内存捕获器，实现 `scrap::TraitCapturer`。
///
/// 直接按固定偏移量从 SHMEM 中读取 FrameInfo 和帧数据，
/// 返回 `Frame::PixelBuffer(BGRA)`。
pub struct SosShmemCapturer {
    shmem_ptr: *const u8,
    shmem_len: usize,
    _width: usize,
    _height: usize,
}

unsafe impl Send for SosShmemCapturer {}
unsafe impl Sync for SosShmemCapturer {}

impl SosShmemCapturer {
    pub fn new(ptr: *const u8, len: usize, width: usize, height: usize) -> Self {
        Self {
            shmem_ptr: ptr,
            shmem_len: len,
            _width: width,
            _height: height,
        }
    }

    fn read_counter(&self) -> i32 {
        unsafe {
            let ptr = self.shmem_ptr.add(SHMEM_ADDR_CAPTURE_FRAME_COUNTER);
            *(ptr as *const i32)
        }
    }

    fn read_counter_echo(&self) -> i32 {
        unsafe {
            let ptr = self.shmem_ptr.add(SHMEM_ADDR_CAPTURE_FRAME_COUNTER + 4);
            *(ptr as *const i32)
        }
    }

    fn update_counter_echo(&self) {
        unsafe {
            let wptr = self.shmem_ptr.add(SHMEM_ADDR_CAPTURE_FRAME_COUNTER);
            let rptr = wptr.add(4);
            std::ptr::copy_nonoverlapping(wptr, rptr as *mut _, 4);
        }
    }

    fn is_wouldblock(&self) -> bool {
        unsafe {
            let ptr = self.shmem_ptr.add(SHMEM_ADDR_CAPTURE_WOULDBLOCK);
            *(ptr as *const i32) == 1
        }
    }

    fn read_frame_info(&self) -> (usize, usize, usize) {
        unsafe {
            let ptr = self.shmem_ptr.add(SHMEM_ADDR_CAPTURE_FRAME_INFO);
            let fi = ptr as *const (usize, usize, usize);
            *fi
        }
    }

    fn read_frame_data(&self, len: usize) -> &[u8] {
        unsafe {
            let ptr = self.shmem_ptr.add(SHMEM_ADDR_CAPTURE_FRAME);
            std::slice::from_raw_parts(ptr, len)
        }
    }
}

impl scrap::TraitCapturer for SosShmemCapturer {
    fn frame<'a>(&'a mut self, _timeout: std::time::Duration) -> std::io::Result<scrap::Frame<'a>> {
        use scrap::Frame;

        let wcnt = self.read_counter();
        let ecnt = self.read_counter_echo();
        let has_new = wcnt != ecnt;
        let wb = self.is_wouldblock();

        if has_new {
            let (len, w, h) = self.read_frame_info();
            log::debug!(
                "[SosShmemCapturer::frame] NEW wcnt={} ecnt={} len={} w={} h={}",
                wcnt,
                ecnt,
                len,
                w,
                h
            );
            if len > 0 && len <= self.shmem_len.saturating_sub(SHMEM_ADDR_CAPTURE_FRAME) {
                self.update_counter_echo();
                let data = self.read_frame_data(len);
                let pb = scrap::PixelBuffer::with_BGRA(data, w, h);
                return Ok(Frame::PixelBuffer(pb));
            }
        }

        static ERR_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = ERR_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if n % 200 == 0 {
            log::debug!(
                "[SosShmemCapturer::frame] NOFRAME n={} wcnt={} ecnt={} wb={} has_new={}",
                n,
                wcnt,
                ecnt,
                wb,
                has_new
            );
        }

        // 没有新帧时统一返回 WouldBlock，让上游主循环重试而非重启服务
        Err(std::io::Error::new(
            std::io::ErrorKind::WouldBlock,
            "shmem wouldblock",
        ))
    }

    fn is_gdi(&self) -> bool {
        true
    }
    fn set_gdi(&mut self) -> bool {
        true
    }
}
