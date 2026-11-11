//! SOS 自建 DXGI Desktop Duplication 采集器。
//!
//! 存在原因：`libs/scrap` 的 DXGI 路径有两个会导致「永不恢复」的缺陷，
//! 且都在我们无法修改的 `libs/` 里。
//!
//! ## scrap 的缺陷 1：Unmap 与 ReleaseFrame 顺序颠倒
//!
//! ```ignore
//! fn unmap(&self) {
//!     (*self.duplication.0).ReleaseFrame();      // 先释放 frame
//!     if self.fastlane {
//!         (*self.duplication.0).UnMapDesktopSurface();  // 此时已失效
//!     }
//! }
//! ```
//!
//! 当 `DXGI_OUTDUPL_DESC::DesktopImageInSystemMemory == TRUE`（fastlane）时，
//! `ReleaseFrame` 会让 `UnMapDesktopSurface` 失败，surface 永久停留在 mapped 状态。
//! 此后每次 `MapDesktopSurface` 都返回 `DXGI_ERROR_INVALID_CALL`，
//! 表现为 `ErrorKind::InvalidData` 且**重建采集器也无法恢复**（同一进程内
//! surface 状态被驱动持有），最终只能降级。
//!
//! 正确顺序：先 `Unmap` / `UnMapDesktopSurface`，再 `ReleaseFrame`。
//!
//! ## scrap 的缺陷 2：提前 return 时不释放 frame
//!
//! `load_frame` 在 `LastPresentTime == 0` 时直接 `return Err(WouldBlock)`，
//! 但没有调用 `ReleaseFrame`。下一次 `AcquireNextFrame` 随即返回
//! `DXGI_ERROR_INVALID_CALL`，从此进入「每次重建都立刻失败」的循环。
//!
//! ## 本实现
//!
//! - 用 `frame_acquired` 标记严格保证「acquire 与 release 成对」，
//!   任何提前返回路径都会先释放；
//! - 只用 `CopyResource` + `Map` 的通用路径（不依赖 fastlane），
//!   顺序固定为 Map → 拷贝 → Unmap → ReleaseFrame；
//! - 显式把 HRESULT 映射成语义正确的 `io::ErrorKind`，让上层能区分
//!   「无新帧」「可恢复」「本机不支持」；
//! - 屏幕旋转（非 IDENTITY）时直接拒绝创建，由上层降级到 GDI，
//!   避免在这里引入 CPU 旋转的额外全帧拷贝。

#![cfg(windows)]

use scrap::{Frame, PixelBuffer, TraitCapturer};
use std::time::Duration;
use windows::core::{Interface, Result as WinResult};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_UNKNOWN;
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D, D3D11_CPU_ACCESS_READ,
    D3D11_CREATE_DEVICE_FLAG, D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_READ, D3D11_SDK_VERSION,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_MODE_ROTATION_IDENTITY,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, IDXGIAdapter1, IDXGIFactory1, IDXGIOutput1, IDXGIOutputDuplication,
    IDXGIResource, DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_INVALID_CALL,
    DXGI_ERROR_NOT_CURRENTLY_AVAILABLE, DXGI_ERROR_NOT_FOUND, DXGI_ERROR_SESSION_DISCONNECTED,
    DXGI_ERROR_UNSUPPORTED, DXGI_ERROR_WAIT_TIMEOUT, DXGI_OUTDUPL_DESC, DXGI_OUTDUPL_FRAME_INFO,
};

/// 把 Win32 的 UTF-16 定长名字转成 `String`
fn utf16z_to_string(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

/// 把 DXGI/COM 的 HRESULT 翻译成 `io::ErrorKind`。
///
/// 上层靠这个区分三种情况：无新帧（继续）、可恢复（重建）、不支持（降档）。
/// `scrap` 在这里只做了粗糙的映射，导致上层无法正确决策。
fn map_hresult(e: &windows::core::Error) -> std::io::Error {
    use std::io::ErrorKind::*;
    let kind = match e.code() {
        // 超时等待：桌面没有新帧，完全正常
        DXGI_ERROR_WAIT_TIMEOUT => WouldBlock,
        // 访问丢失：分辨率切换、显示器热插拔、UAC 桌面切换 —— 重建即可恢复
        DXGI_ERROR_ACCESS_LOST => ConnectionReset,
        // 会话断开：用户注销 / 远程会话结束
        DXGI_ERROR_SESSION_DISCONNECTED => ConnectionAborted,
        // 桌面当前被独占（全屏独占程序、安全桌面）—— 稍后重试
        DXGI_ERROR_NOT_CURRENTLY_AVAILABLE => Interrupted,
        // 本机不支持 desktop duplication
        DXGI_ERROR_UNSUPPORTED => ConnectionRefused,
        // 非法调用：多半是本实现与驱动的状态不同步，重建通常无效，降档最稳
        DXGI_ERROR_INVALID_CALL => InvalidData,
        _ => Other,
    };
    std::io::Error::new(kind, format!("{} ({:#010X})", e.message(), e.code().0))
}

/// 在指定显示器上创建 DXGI 采集器
pub fn create(display_name: &str) -> Result<SosDxgiCapturer, Box<dyn std::error::Error>> {
    unsafe {
        let (adapter, output) = find_output(display_name)?;
        let (device, context) = create_device(&adapter)?;
        let duplication = output.DuplicateOutput(&device)?;

        let desc: DXGI_OUTDUPL_DESC = duplication.GetDesc();
        if desc.Rotation != DXGI_MODE_ROTATION_IDENTITY {
            // 旋转屏幕需要额外的全帧旋转，交给 GDI 路径更省事
            return Err(format!(
                "display rotation {:?} not supported by DXGI path",
                desc.Rotation
            )
            .into());
        }

        let width = desc.ModeDesc.Width as usize;
        let height = desc.ModeDesc.Height as usize;
        if width == 0 || height == 0 {
            return Err("DXGI reported zero display size".into());
        }

        Ok(SosDxgiCapturer {
            device,
            context,
            duplication,
            staging: None,
            data: Vec::new(),
            width,
            height,
            acquired: AcquireState::Idle,
        })
    }
}

/// 遍历所有适配器与输出，找到名字匹配的显示器
unsafe fn find_output(name: &str) -> WinResult<(IDXGIAdapter1, IDXGIOutput1)> {
    let factory: IDXGIFactory1 = CreateDXGIFactory1()?;
    let mut i = 0u32;
    while let Ok(adapter) = factory.EnumAdapters1(i) {
        let mut j = 0u32;
        while let Ok(output) = adapter.EnumOutputs(j) {
            if let Ok(desc) = output.GetDesc() {
                if utf16z_to_string(&desc.DeviceName) == name {
                    let output1: IDXGIOutput1 = output.cast()?;
                    return Ok((adapter, output1));
                }
            }
            j += 1;
        }
        i += 1;
    }
    Err(windows::core::Error::from_hresult(DXGI_ERROR_NOT_FOUND))
}

unsafe fn create_device(adapter: &IDXGIAdapter1) -> WinResult<(ID3D11Device, ID3D11DeviceContext)> {
    let mut device: Option<ID3D11Device> = None;
    let mut context: Option<ID3D11DeviceContext> = None;
    D3D11CreateDevice(
        adapter,
        D3D_DRIVER_TYPE_UNKNOWN,
        Default::default(),
        D3D11_CREATE_DEVICE_FLAG(0),
        None,
        D3D11_SDK_VERSION,
        Some(&mut device),
        None,
        Some(&mut context),
    )?;
    match (device, context) {
        (Some(d), Some(c)) => Ok((d, c)),
        _ => Err(windows::core::Error::from_hresult(DXGI_ERROR_UNSUPPORTED)),
    }
}

pub struct SosDxgiCapturer {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    duplication: IDXGIOutputDuplication,
    /// CPU 可读的 staging 纹理，尺寸变化时重建
    staging: Option<ID3D11Texture2D>,
    /// 已从 staging 拷贝出来的紧密排列 BGRA 数据
    data: Vec<u8>,
    width: usize,
    height: usize,
    /// 严格跟踪 frame 是否已 acquire 但未 release
    acquired: AcquireState,
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum AcquireState {
    Idle,
    /// `AcquireNextFrame` 成功，必须 `ReleaseFrame`
    Held,
}

impl SosDxgiCapturer {
    fn ensure_staging(&mut self, width: u32, height: u32) -> WinResult<()> {
        let need_new = match &self.staging {
            None => true,
            Some(t) => unsafe {
                let mut d = D3D11_TEXTURE2D_DESC::default();
                t.GetDesc(&mut d);
                d.Width != width || d.Height != height
            },
        };
        if !need_new {
            return Ok(());
        }
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
        };
        let mut tex: Option<ID3D11Texture2D> = None;
        unsafe {
            self.device.CreateTexture2D(&desc, None, Some(&mut tex))?;
        }
        self.staging = tex;
        Ok(())
    }

    /// 无论成功失败，只要 acquire 过就必须 release。
    /// `scrap` 正是漏了提前返回路径上的这一步，才导致此后永久失败。
    fn release_frame(&mut self) {
        if self.acquired == AcquireState::Held {
            unsafe {
                let _ = self.duplication.ReleaseFrame();
            }
            self.acquired = AcquireState::Idle;
        }
    }

    fn grab(&mut self, timeout_ms: u32) -> std::io::Result<()> {
        // 上一帧若还挂着（理论上不该发生），先释放，避免 InvalidCall
        self.release_frame();

        let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut resource: Option<IDXGIResource> = None;
        unsafe {
            self.duplication
                .AcquireNextFrame(timeout_ms, &mut info, &mut resource)
                .map_err(|e| map_hresult(&e))?;
        }
        // AcquireNextFrame 成功即持有 frame，之后任何路径都必须 release
        self.acquired = AcquireState::Held;

        // 只有光标移动、没有新画面时 LastPresentTime 为 0
        if info.LastPresentTime == 0 {
            self.release_frame();
            return Err(std::io::Error::from(std::io::ErrorKind::WouldBlock));
        }

        let Some(resource) = resource else {
            self.release_frame();
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "AcquireNextFrame returned no desktop resource",
            ));
        };

        let texture: ID3D11Texture2D = resource.cast().map_err(|e| map_hresult(&e))?;
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        unsafe {
            texture.GetDesc(&mut desc);
        }
        let (w, h) = (desc.Width, desc.Height);

        let result = (|| -> std::io::Result<()> {
            self.ensure_staging(w, h).map_err(|e| map_hresult(&e))?;
            let staging = self.staging.as_ref().ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::Other, "staging texture missing")
            })?;

            unsafe {
                // GPU 纹理 → CPU 可读 staging
                let src: &windows::Win32::Graphics::Direct3D11::ID3D11Resource =
                    &texture.cast().map_err(|e| map_hresult(&e))?;
                let dst: &windows::Win32::Graphics::Direct3D11::ID3D11Resource =
                    &staging.cast().map_err(|e| map_hresult(&e))?;
                self.context.CopyResource(dst, src);

                let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
                self.context
                    .Map(staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                    .map_err(|e| map_hresult(&e))?;

                // Map 成功，之后必须 Unmap，异常路径也要执行
                let copy_result = copy_rows(
                    mapped.pData,
                    mapped.RowPitch as usize,
                    w as usize,
                    h as usize,
                    &mut self.data,
                    &mut self.width,
                    &mut self.height,
                );
                self.context.Unmap(staging, 0);
                copy_result
            }
        })();

        // 关键：先 Unmap（上面已完成），再 ReleaseFrame
        self.release_frame();
        result
    }
}

/// 把带 padding 的 BGRA 行数据拷成紧密排列
///
/// # Safety
/// `src` 必须指向至少 `pitch * height` 字节的可读内存。
unsafe fn copy_rows(
    src: *const std::ffi::c_void,
    pitch: usize,
    width: usize,
    height: usize,
    out: &mut Vec<u8>,
    out_w: &mut usize,
    out_h: &mut usize,
) -> std::io::Result<()> {
    if src.is_null() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Map returned null pointer",
        ));
    }
    let row_bytes = width * 4;
    if pitch < row_bytes {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("DXGI row pitch {} < {}", pitch, row_bytes),
        ));
    }
    let need = row_bytes * height;
    out.resize(need, 0);
    let base = src as *const u8;
    if pitch == row_bytes {
        // 无 padding：一次整块拷贝
        std::ptr::copy_nonoverlapping(base, out.as_mut_ptr(), need);
    } else {
        for y in 0..height {
            std::ptr::copy_nonoverlapping(
                base.add(y * pitch),
                out.as_mut_ptr().add(y * row_bytes),
                row_bytes,
            );
        }
    }
    *out_w = width;
    *out_h = height;
    Ok(())
}

impl TraitCapturer for SosDxgiCapturer {
    fn frame<'a>(&'a mut self, timeout: Duration) -> std::io::Result<Frame<'a>> {
        self.grab(timeout.as_millis().min(u32::MAX as u128) as u32)?;
        Ok(Frame::PixelBuffer(PixelBuffer::with_BGRA(
            &self.data,
            self.width,
            self.height,
        )))
    }

    fn is_gdi(&self) -> bool {
        false
    }

    fn set_gdi(&mut self) -> bool {
        false
    }
}

impl Drop for SosDxgiCapturer {
    fn drop(&mut self) {
        // Drop 里也要保证 frame 已释放，否则同一个进程再创建 duplication 会失败
        self.release_frame();
        self.staging = None;
        let _ = &self.device;
        let _ = &self.context;
    }
}
