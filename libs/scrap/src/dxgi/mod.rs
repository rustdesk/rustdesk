use std::{io, mem, ptr, slice};
pub mod gdi;
pub use gdi::CapturerGDI;

use winapi::{
    shared::dxgi::{
        CreateDXGIFactory1, IDXGIAdapter1, IDXGIFactory1, IDXGIResource, IDXGISurface,
        IID_IDXGIFactory1, IID_IDXGISurface, DXGI_MAP_READ, DXGI_OUTPUT_DESC,
        DXGI_RESOURCE_PRIORITY_MAXIMUM,
    },
    shared::dxgi1_2::IDXGIOutputDuplication,
    // shared::dxgiformat::{DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_420_OPAQUE},
    shared::dxgi1_2::{IDXGIOutput1, IID_IDXGIOutput1},
    shared::dxgitype::DXGI_MODE_ROTATION,
    shared::minwindef::{TRUE, UINT},
    shared::ntdef::LONG,
    shared::windef::HMONITOR,
    shared::winerror::{
        DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_INVALID_CALL, DXGI_ERROR_NOT_CURRENTLY_AVAILABLE,
        DXGI_ERROR_SESSION_DISCONNECTED, DXGI_ERROR_UNSUPPORTED, DXGI_ERROR_WAIT_TIMEOUT,
        E_ACCESSDENIED, E_INVALIDARG, S_OK,
    },
    um::d3d11::{
        D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D, IID_ID3D11Texture2D,
        D3D11_CPU_ACCESS_READ, D3D11_SDK_VERSION, D3D11_USAGE_STAGING,
    },
    um::d3dcommon::D3D_DRIVER_TYPE_UNKNOWN,
    um::winnt::HRESULT,
};

//TODO: Split up into files.

pub struct Capturer {
    device: *mut ID3D11Device,
    display: Display,
    context: *mut ID3D11DeviceContext,
    duplication: *mut IDXGIOutputDuplication,
    fastlane: bool,
    surface: *mut IDXGISurface,
    data: *const u8,
    len: usize,
    width: usize,
    height: usize,
    use_yuv: bool,
    yuv: Vec<u8>,
    gdi_capturer: Option<CapturerGDI>,
    gdi_buffer: Vec<u8>,
}

impl Capturer {
    pub fn new(display: Display, use_yuv: bool) -> io::Result<Capturer> {
        let mut device = ptr::null_mut();
        let mut context = ptr::null_mut();
        let mut duplication = ptr::null_mut();
        let mut desc = unsafe { mem::MaybeUninit::uninit().assume_init() };
        let mut gdi_capturer = None;

        let mut res = wrap_hresult(unsafe {
            D3D11CreateDevice(
                display.adapter as *mut _,
                D3D_DRIVER_TYPE_UNKNOWN,
                ptr::null_mut(), // No software rasterizer.
                0,               // No device flags.
                ptr::null_mut(), // Feature levels.
                0,               // Feature levels' length.
                D3D11_SDK_VERSION,
                &mut device,
                ptr::null_mut(),
                &mut context,
            )
        });

        if res.is_err() {
            gdi_capturer = display.create_gdi();
            println!("Fallback to GDI");
            if gdi_capturer.is_some() {
                res = Ok(());
            }
        } else {
            res = wrap_hresult(unsafe {
                let hres = (*display.inner).DuplicateOutput(device as *mut _, &mut duplication);
                if hres != S_OK {
                    gdi_capturer = display.create_gdi();
                    println!("Fallback to GDI");
                    if gdi_capturer.is_some() {
                        S_OK
                    } else {
                        hres
                    }
                } else {
                    hres
                }
                // NVFBC(NVIDIA Capture SDK) which xpra used already deprecated, https://developer.nvidia.com/capture-sdk

                // also try high version DXGI for better performance, e.g.
                // https://docs.microsoft.com/zh-cn/windows/win32/direct3ddxgi/dxgi-1-2-improvements
                // dxgi-1-6 may too high, only support win10 (2018)
                // https://docs.microsoft.com/zh-cn/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format
                // DXGI_FORMAT_420_OPAQUE
                // IDXGIOutputDuplication::GetFrameDirtyRects and IDXGIOutputDuplication::GetFrameMoveRects
                // can help us update screen incrementally

                /* // not supported on my PC, try in the future
                let format : Vec<DXGI_FORMAT> = vec![DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_420_OPAQUE];
                (*display.inner).DuplicateOutput1(
                    device as *mut _,
                    0 as UINT,
                    2 as UINT,
                    format.as_ptr(),
                    &mut duplication
                )
                */

                // if above not work, I think below should not work either, try later
                // https://developer.nvidia.com/capture-sdk deprecated
                // examples using directx + nvideo sdk for GPU-accelerated video encoding/decoding
                // https://github.com/NVIDIA/video-sdk-samples
            });
        }

        if let Err(err) = res {
            unsafe {
                if !device.is_null() {
                    (*device).Release();
                }
                if !context.is_null() {
                    (*context).Release();
                }
            }
            return Err(err);
        }

        if !duplication.is_null() {
            unsafe {
                (*duplication).GetDesc(&mut desc);
            }
        }

        Ok(Capturer {
            device,
            context,
            duplication,
            fastlane: desc.DesktopImageInSystemMemory == TRUE,
            surface: ptr::null_mut(),
            width: display.width() as usize,
            height: display.height() as usize,
            display,
            data: ptr::null(),
            len: 0,
            use_yuv,
            yuv: Vec::new(),
            gdi_capturer,
            gdi_buffer: Vec::new(),
        })
    }

    pub fn is_gdi(&self) -> bool {
        self.gdi_capturer.is_some()
    }

    pub fn set_gdi(&mut self) -> bool {
        self.gdi_capturer = self.display.create_gdi();
        self.is_gdi()
    }

    pub fn cancel_gdi(&mut self) {
        self.gdi_buffer = Vec::new();
        self.gdi_capturer.take();
    }

    unsafe fn load_frame(&mut self, timeout: UINT) -> io::Result<()> {
        let mut frame = ptr::null_mut();
        let mut info = mem::MaybeUninit::uninit().assume_init();
        self.data = ptr::null();

        wrap_hresult((*self.duplication).AcquireNextFrame(timeout, &mut info, &mut frame))?;

        if *info.LastPresentTime.QuadPart() == 0 {
            return Err(std::io::ErrorKind::WouldBlock.into());
        }

        if self.fastlane {
            let mut rect = mem::MaybeUninit::uninit().assume_init();
            let res = wrap_hresult((*self.duplication).MapDesktopSurface(&mut rect));

            (*frame).Release();

            if let Err(err) = res {
                Err(err)
            } else {
                self.data = rect.pBits;
                self.len = self.height * rect.Pitch as usize;
                Ok(())
            }
        } else {
            self.surface = ptr::null_mut();
            self.surface = self.ohgodwhat(frame)?;

            let mut rect = mem::MaybeUninit::uninit().assume_init();
            wrap_hresult((*self.surface).Map(&mut rect, DXGI_MAP_READ))?;

            self.data = rect.pBits;
            self.len = self.height * rect.Pitch as usize;
            Ok(())
        }
    }

    // copy from GPU memory to system memory
    unsafe fn ohgodwhat(&mut self, frame: *mut IDXGIResource) -> io::Result<*mut IDXGISurface> {
        let mut texture: *mut ID3D11Texture2D = ptr::null_mut();
        (*frame).QueryInterface(
            &IID_ID3D11Texture2D,
            &mut texture as *mut *mut _ as *mut *mut _,
        );

        let mut texture_desc = mem::MaybeUninit::uninit().assume_init();
        (*texture).GetDesc(&mut texture_desc);

        texture_desc.Usage = D3D11_USAGE_STAGING;
        texture_desc.BindFlags = 0;
        texture_desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ;
        texture_desc.MiscFlags = 0;

        let mut readable = ptr::null_mut();
        let res = wrap_hresult((*self.device).CreateTexture2D(
            &mut texture_desc,
            ptr::null(),
            &mut readable,
        ));

        if let Err(err) = res {
            (*frame).Release();
            (*texture).Release();
            (*readable).Release();
            Err(err)
        } else {
            (*readable).SetEvictionPriority(DXGI_RESOURCE_PRIORITY_MAXIMUM);

            let mut surface = ptr::null_mut();
            (*readable).QueryInterface(
                &IID_IDXGISurface,
                &mut surface as *mut *mut _ as *mut *mut _,
            );

            (*self.context).CopyResource(readable as *mut _, texture as *mut _);

            (*frame).Release();
            (*texture).Release();
            (*readable).Release();
            Ok(surface)
        }
    }

    pub fn frame<'a>(&'a mut self, timeout: UINT) -> io::Result<&'a [u8]> {
        unsafe {
            // Release last frame.
            // No error checking needed because we don't care.
            // None of the errors crash anyway.
            let result = {
                if let Some(gdi_capturer) = &self.gdi_capturer {
                    match gdi_capturer.frame(&mut self.gdi_buffer) {
                        Ok(_) => &self.gdi_buffer,
                        Err(err) => {
                            return Err(io::Error::new(io::ErrorKind::Other, err.to_string()));
                        }
                    }
                } else {
                    if self.fastlane {
                        (*self.duplication).UnMapDesktopSurface();
                    } else {
                        if !self.surface.is_null() {
                            (*self.surface).Unmap();
                            (*self.surface).Release();
                            self.surface = ptr::null_mut();
                        }
                    }

                    (*self.duplication).ReleaseFrame();
                    self.load_frame(timeout)?;
                    slice::from_raw_parts(self.data, self.len)
                }
            };
            Ok({
                if self.use_yuv {
                    crate::common::bgra_to_i420(
                        self.width as usize,
                        self.height as usize,
                        &result,
                        &mut self.yuv,
                    );
                    &self.yuv[..]
                } else {
                    result
                }
            })
        }
    }
}

impl Drop for Capturer {
    fn drop(&mut self) {
        unsafe {
            if !self.surface.is_null() {
                (*self.surface).Unmap();
                (*self.surface).Release();
            }
            if !self.duplication.is_null() {
                (*self.duplication).Release();
            }
            if !self.device.is_null() {
                (*self.device).Release();
            }
            if !self.context.is_null() {
                (*self.context).Release();
            }
        }
    }
}

pub struct Displays {
    factory: *mut IDXGIFactory1,
    adapter: *mut IDXGIAdapter1,
    /// Index of the CURRENT adapter.
    nadapter: UINT,
    /// Index of the NEXT display to fetch.
    ndisplay: UINT,
}

impl Displays {
    pub fn new() -> io::Result<Displays> {
        let mut factory = ptr::null_mut();
        wrap_hresult(unsafe { CreateDXGIFactory1(&IID_IDXGIFactory1, &mut factory) })?;

        let factory = factory as *mut IDXGIFactory1;
        let mut adapter = ptr::null_mut();
        unsafe {
            // On error, our adapter is null, so it's fine.
            (*factory).EnumAdapters1(0, &mut adapter);
        };

        Ok(Displays {
            factory,
            adapter,
            nadapter: 0,
            ndisplay: 0,
        })
    }

    // No Adapter => Some(None)
    // Non-Empty Adapter => Some(Some(OUTPUT))
    // End of Adapter => None
    fn read_and_invalidate(&mut self) -> Option<Option<Display>> {
        // If there is no adapter, there is nothing left for us to do.

        if self.adapter.is_null() {
            return Some(None);
        }

        // Otherwise, we get the next output of the current adapter.

        let output = unsafe {
            let mut output = ptr::null_mut();
            (*self.adapter).EnumOutputs(self.ndisplay, &mut output);
            output
        };

        // If the current adapter is done, we free it.
        // We return None so the caller gets the next adapter and tries again.

        if output.is_null() {
            unsafe {
                (*self.adapter).Release();
                self.adapter = ptr::null_mut();
            }
            return None;
        }

        // Advance to the next display.

        self.ndisplay += 1;

        // We get the display's details.

        let desc = unsafe {
            let mut desc = mem::MaybeUninit::uninit().assume_init();
            (*output).GetDesc(&mut desc);
            desc
        };

        // We cast it up to the version needed for desktop duplication.

        let mut inner: *mut IDXGIOutput1 = ptr::null_mut();
        unsafe {
            (*output).QueryInterface(&IID_IDXGIOutput1, &mut inner as *mut *mut _ as *mut *mut _);
            (*output).Release();
        }

        // If it's null, we have an error.
        // So we act like the adapter is done.

        if inner.is_null() {
            unsafe {
                (*self.adapter).Release();
                self.adapter = ptr::null_mut();
            }
            return None;
        }

        unsafe {
            (*self.adapter).AddRef();
        }

        Some(Some(Display {
            inner,
            adapter: self.adapter,
            desc,
        }))
    }
}

impl Iterator for Displays {
    type Item = Display;
    fn next(&mut self) -> Option<Display> {
        if let Some(res) = self.read_and_invalidate() {
            res
        } else {
            // We need to replace the adapter.

            self.ndisplay = 0;
            self.nadapter += 1;

            self.adapter = unsafe {
                let mut adapter = ptr::null_mut();
                (*self.factory).EnumAdapters1(self.nadapter, &mut adapter);
                adapter
            };

            if let Some(res) = self.read_and_invalidate() {
                res
            } else {
                // All subsequent adapters will also be empty.
                None
            }
        }
    }
}

impl Drop for Displays {
    fn drop(&mut self) {
        unsafe {
            (*self.factory).Release();
            if !self.adapter.is_null() {
                (*self.adapter).Release();
            }
        }
    }
}

pub struct Display {
    inner: *mut IDXGIOutput1,
    adapter: *mut IDXGIAdapter1,
    desc: DXGI_OUTPUT_DESC,
}

impl Display {
    pub fn width(&self) -> LONG {
        self.desc.DesktopCoordinates.right - self.desc.DesktopCoordinates.left
    }

    pub fn height(&self) -> LONG {
        self.desc.DesktopCoordinates.bottom - self.desc.DesktopCoordinates.top
    }

    pub fn attached_to_desktop(&self) -> bool {
        self.desc.AttachedToDesktop != 0
    }

    pub fn rotation(&self) -> DXGI_MODE_ROTATION {
        self.desc.Rotation
    }

    fn create_gdi(&self) -> Option<CapturerGDI> {
        if let Ok(res) = CapturerGDI::new(self.name(), self.width(), self.height()) {
            Some(res)
        } else {
            None
        }
    }

    pub fn hmonitor(&self) -> HMONITOR {
        self.desc.Monitor
    }

    pub fn name(&self) -> &[u16] {
        let s = &self.desc.DeviceName;
        let i = s.iter().position(|&x| x == 0).unwrap_or(s.len());
        &s[..i]
    }

    pub fn is_online(&self) -> bool {
        self.desc.AttachedToDesktop != 0
    }

    pub fn origin(&self) -> (LONG, LONG) {
        (
            self.desc.DesktopCoordinates.left,
            self.desc.DesktopCoordinates.top,
        )
    }
}

impl Drop for Display {
    fn drop(&mut self) {
        unsafe {
            (*self.inner).Release();
            (*self.adapter).Release();
        }
    }
}

fn wrap_hresult(x: HRESULT) -> io::Result<()> {
    use std::io::ErrorKind::*;
    Err((match x {
        S_OK => return Ok(()),
        DXGI_ERROR_ACCESS_LOST => ConnectionReset,
        DXGI_ERROR_WAIT_TIMEOUT => TimedOut,
        DXGI_ERROR_INVALID_CALL => InvalidData,
        E_ACCESSDENIED => PermissionDenied,
        DXGI_ERROR_UNSUPPORTED => ConnectionRefused,
        DXGI_ERROR_NOT_CURRENTLY_AVAILABLE => Interrupted,
        DXGI_ERROR_SESSION_DISCONNECTED => ConnectionAborted,
        E_INVALIDARG => InvalidInput,
        _ => {
            // 0x8000ffff https://www.auslogics.com/en/articles/windows-10-update-error-0x8000ffff-fixed/
            return Err(io::Error::new(Other, format!("Error code: {:#X}", x)));
        }
    })
    .into())
}
