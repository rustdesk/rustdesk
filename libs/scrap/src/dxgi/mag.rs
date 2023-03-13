// logic from webrtc -- https://github.com/shiguredo/libwebrtc/blob/main/modules/desktop_capture/win/screen_capturer_win_magnifier.cc
use lazy_static;
use std::{
    ffi::CString,
    io::{Error, ErrorKind, Result},
    mem::size_of,
    sync::Mutex,
};
use winapi::{
    shared::{
        basetsd::SIZE_T,
        guiddef::{IsEqualGUID, GUID},
        minwindef::{BOOL, DWORD, FALSE, FARPROC, HINSTANCE, HMODULE, HRGN, TRUE, UINT},
        ntdef::{LONG, NULL},
        windef::{HWND, RECT},
        winerror::ERROR_CLASS_ALREADY_EXISTS,
    },
    um::{
        errhandlingapi::GetLastError,
        libloaderapi::{FreeLibrary, GetModuleHandleExA, GetProcAddress, LoadLibraryExA},
        winuser::*,
    },
};

pub const MW_FILTERMODE_EXCLUDE: u32 = 0;
pub const MW_FILTERMODE_INCLUDE: u32 = 1;
pub const GET_MODULE_HANDLE_EX_FLAG_PIN: u32 = 1;
pub const GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT: u32 = 2;
pub const GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS: u32 = 4;
pub const LOAD_LIBRARY_AS_DATAFILE: u32 = 2;
pub const LOAD_WITH_ALTERED_SEARCH_PATH: u32 = 8;
pub const LOAD_IGNORE_CODE_AUTHZ_LEVEL: u32 = 16;
pub const LOAD_LIBRARY_AS_IMAGE_RESOURCE: u32 = 32;
pub const LOAD_LIBRARY_AS_DATAFILE_EXCLUSIVE: u32 = 64;
pub const LOAD_LIBRARY_REQUIRE_SIGNED_TARGET: u32 = 128;
pub const LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR: u32 = 256;
pub const LOAD_LIBRARY_SEARCH_APPLICATION_DIR: u32 = 512;
pub const LOAD_LIBRARY_SEARCH_USER_DIRS: u32 = 1024;
pub const LOAD_LIBRARY_SEARCH_SYSTEM32: u32 = 2048;
pub const LOAD_LIBRARY_SEARCH_DEFAULT_DIRS: u32 = 4096;
pub const LOAD_LIBRARY_SAFE_CURRENT_DIRS: u32 = 8192;
pub const LOAD_LIBRARY_SEARCH_SYSTEM32_NO_FORWARDER: u32 = 16384;
pub const LOAD_LIBRARY_OS_INTEGRITY_CONTINUITY: u32 = 32768;

extern "C" {
    pub static GUID_WICPixelFormat32bppRGBA: GUID;
}

lazy_static::lazy_static! {
    static ref MAG_BUFFER: Mutex<(bool, Vec<u8>)> =  Default::default();
}

pub type REFWICPixelFormatGUID = *const GUID;
pub type WICPixelFormatGUID = GUID;

#[allow(non_snake_case)]
#[repr(C)]
#[derive(Copy, Clone)]
pub struct tagMAGIMAGEHEADER {
    pub width: UINT,
    pub height: UINT,
    pub format: WICPixelFormatGUID,
    pub stride: UINT,
    pub offset: UINT,
    pub cbSize: SIZE_T,
}
pub type MAGIMAGEHEADER = tagMAGIMAGEHEADER;
pub type PMAGIMAGEHEADER = *mut tagMAGIMAGEHEADER;

// Function types
pub type MagImageScalingCallback = ::std::option::Option<
    unsafe extern "C" fn(
        hwnd: HWND,
        srcdata: *mut ::std::os::raw::c_void,
        srcheader: MAGIMAGEHEADER,
        destdata: *mut ::std::os::raw::c_void,
        destheader: MAGIMAGEHEADER,
        unclipped: RECT,
        clipped: RECT,
        dirty: HRGN,
    ) -> BOOL,
>;

extern "C" {
    pub fn MagShowSystemCursor(fShowCursor: BOOL) -> BOOL;
}
pub type MagInitializeFunc = ::std::option::Option<unsafe extern "C" fn() -> BOOL>;
pub type MagUninitializeFunc = ::std::option::Option<unsafe extern "C" fn() -> BOOL>;
pub type MagSetWindowSourceFunc =
    ::std::option::Option<unsafe extern "C" fn(hwnd: HWND, rect: RECT) -> BOOL>;
pub type MagSetWindowFilterListFunc = ::std::option::Option<
    unsafe extern "C" fn(
        hwnd: HWND,
        dwFilterMode: DWORD,
        count: ::std::os::raw::c_int,
        pHWND: *mut HWND,
    ) -> BOOL,
>;
pub type MagSetImageScalingCallbackFunc = ::std::option::Option<
    unsafe extern "C" fn(hwnd: HWND, callback: MagImageScalingCallback) -> BOOL,
>;

#[repr(C)]
#[derive(Debug, Clone)]
struct MagInterface {
    init_succeeded: bool,
    lib_handle: HINSTANCE,
    pub mag_initialize_func: MagInitializeFunc,
    pub mag_uninitialize_func: MagUninitializeFunc,
    pub set_window_source_func: MagSetWindowSourceFunc,
    pub set_window_filter_list_func: MagSetWindowFilterListFunc,
    pub set_image_scaling_callback_func: MagSetImageScalingCallbackFunc,
}

// NOTE: MagInitialize and MagUninitialize should not be called in global init and uninit.
// If so, strange errors occur.
impl MagInterface {
    fn new() -> Result<Self> {
        let mut s = MagInterface {
            init_succeeded: false,
            lib_handle: NULL as _,
            mag_initialize_func: None,
            mag_uninitialize_func: None,
            set_window_source_func: None,
            set_window_filter_list_func: None,
            set_image_scaling_callback_func: None,
        };
        s.init_succeeded = false;
        unsafe {
            if GetSystemMetrics(SM_CMONITORS) != 1 {
                // Do not try to use the magnifier in multi-screen setup (where the API
                // crashes sometimes).
                return Err(Error::new(
                    ErrorKind::Other,
                    "Magnifier capturer cannot work on multi-screen system.",
                ));
            }

            // load lib
            let lib_file_name = "Magnification.dll";
            let lib_file_name_c = CString::new(lib_file_name).unwrap();
            s.lib_handle = LoadLibraryExA(
                lib_file_name_c.as_ptr() as _,
                NULL,
                LOAD_WITH_ALTERED_SEARCH_PATH,
            );
            if s.lib_handle.is_null() {
                return Err(Error::new(
                    ErrorKind::Other,
                    format!(
                        "Failed to LoadLibraryExA {}, error: {}",
                        lib_file_name,
                        GetLastError()
                    ),
                ));
            };

            // load functions
            s.mag_initialize_func = Some(std::mem::transmute(Self::load_func(
                s.lib_handle,
                "MagInitialize",
            )?));
            s.mag_uninitialize_func = Some(std::mem::transmute(Self::load_func(
                s.lib_handle,
                "MagUninitialize",
            )?));
            s.set_window_source_func = Some(std::mem::transmute(Self::load_func(
                s.lib_handle,
                "MagSetWindowSource",
            )?));
            s.set_window_filter_list_func = Some(std::mem::transmute(Self::load_func(
                s.lib_handle,
                "MagSetWindowFilterList",
            )?));
            s.set_image_scaling_callback_func = Some(std::mem::transmute(Self::load_func(
                s.lib_handle,
                "MagSetImageScalingCallback",
            )?));

            // MagInitialize
            if let Some(init_func) = s.mag_initialize_func {
                if FALSE == init_func() {
                    return Err(Error::new(
                        ErrorKind::Other,
                        format!("Failed to MagInitialize, error: {}", GetLastError()),
                    ));
                } else {
                    s.init_succeeded = true;
                }
            } else {
                return Err(Error::new(
                    ErrorKind::Other,
                    "Unreachable, mag_initialize_func should not be none",
                ));
            }
        }
        Ok(s)
    }

    unsafe fn load_func(lib_module: HMODULE, func_name: &str) -> Result<FARPROC> {
        let func_name_c = CString::new(func_name).unwrap();
        let func = GetProcAddress(lib_module, func_name_c.as_ptr() as _);
        if func.is_null() {
            return Err(Error::new(
                ErrorKind::Other,
                format!(
                    "Failed to GetProcAddress {}, error: {}",
                    func_name,
                    GetLastError()
                ),
            ));
        }
        Ok(func)
    }

    pub(super) fn uninit(&mut self) {
        if self.init_succeeded {
            if let Some(uninit_func) = self.mag_uninitialize_func {
                unsafe {
                    if FALSE == uninit_func() {
                        println!("Failed MagUninitialize {}", GetLastError())
                    }
                }
            }
            if !self.lib_handle.is_null() {
                unsafe {
                    if FALSE == FreeLibrary(self.lib_handle) {
                        println!("Failed FreeLibrary {}", GetLastError())
                    }
                }
                self.lib_handle = NULL as _;
            }
        }
        self.init_succeeded = false;
    }
}

impl Drop for MagInterface {
    fn drop(&mut self) {
        self.uninit();
    }
}

pub struct CapturerMag {
    mag_interface: MagInterface,
    host_window: HWND,
    magnifier_window: HWND,

    magnifier_host_class: CString,
    host_window_name: CString,
    magnifier_window_class: CString,
    magnifier_window_name: CString,

    rect: RECT,
    width: usize,
    height: usize,

    use_yuv: bool,
    data: Vec<u8>,
}

impl Drop for CapturerMag {
    fn drop(&mut self) {
        self.destroy_windows();
        self.mag_interface.uninit();
    }
}

impl CapturerMag {
    pub(crate) fn is_supported() -> bool {
        MagInterface::new().is_ok()
    }

    pub(crate) fn new(
        origin: (i32, i32),
        width: usize,
        height: usize,
        use_yuv: bool,
    ) -> Result<Self> {
        unsafe {
            let x = GetSystemMetrics(SM_XVIRTUALSCREEN);
            let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
            let w = GetSystemMetrics(SM_CXVIRTUALSCREEN);
            let h = GetSystemMetrics(SM_CYVIRTUALSCREEN);
            if !(origin.0 == x as i32
                && origin.1 == y as i32
                && width == w as usize
                && height == h as usize)
            {
                return Err(Error::new(
                    ErrorKind::Other,
                    format!(
                        "Failed Check screen rect ({}, {}, {} , {}) to ({}, {}, {}, {})",
                        origin.0,
                        origin.1,
                        origin.0 + width as i32,
                        origin.1 + height as i32,
                        x,
                        y,
                        x + w,
                        y + h
                    ),
                ));
            }
        }

        let mut s = Self {
            mag_interface: MagInterface::new()?,
            host_window: 0 as _,
            magnifier_window: 0 as _,
            magnifier_host_class: CString::new("ScreenCapturerWinMagnifierHost")?,
            host_window_name: CString::new("MagnifierHost")?,
            magnifier_window_class: CString::new("Magnifier")?,
            magnifier_window_name: CString::new("MagnifierWindow")?,
            rect: RECT {
                left: origin.0 as _,
                top: origin.1 as _,
                right: origin.0 + width as LONG,
                bottom: origin.1 + height as LONG,
            },
            width,
            height,
            use_yuv,
            data: Vec::new(),
        };

        unsafe {
            let mut instance = 0 as HMODULE;
            if 0 == GetModuleHandleExA(
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS
                    | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                DefWindowProcA as _,
                &mut instance as _,
            ) {
                return Err(Error::new(
                    ErrorKind::Other,
                    format!("Failed to GetModuleHandleExA, error: {}", GetLastError()),
                ));
            }

            // Register the host window class. See the MSDN documentation of the
            // Magnification API for more information.
            let wcex = WNDCLASSEXA {
                cbSize: size_of::<WNDCLASSEXA>() as _,
                style: 0,
                lpfnWndProc: Some(DefWindowProcA),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: instance,
                hIcon: 0 as _,
                hCursor: LoadCursorA(NULL as _, IDC_ARROW as _),
                hbrBackground: 0 as _,
                lpszClassName: s.magnifier_host_class.as_ptr() as _,
                lpszMenuName: 0 as _,
                hIconSm: 0 as _,
            };

            // Ignore the error which may happen when the class is already registered.
            if 0 == RegisterClassExA(&wcex) {
                let code = GetLastError();
                if code != ERROR_CLASS_ALREADY_EXISTS {
                    return Err(Error::new(
                        ErrorKind::Other,
                        format!("Failed to RegisterClassExA, error: {}", code),
                    ));
                }
            }

            // Create the host window.
            s.host_window = CreateWindowExA(
                WS_EX_LAYERED,
                s.magnifier_host_class.as_ptr(),
                s.host_window_name.as_ptr(),
                WS_POPUP,
                0,
                0,
                0,
                0,
                NULL as _,
                NULL as _,
                instance,
                NULL,
            );
            if s.host_window.is_null() {
                return Err(Error::new(
                    ErrorKind::Other,
                    format!(
                        "Failed to CreateWindowExA host_window, error: {}",
                        GetLastError()
                    ),
                ));
            }

            // Create the magnifier control.
            s.magnifier_window = CreateWindowExA(
                0,
                s.magnifier_window_class.as_ptr(),
                s.magnifier_window_name.as_ptr(),
                WS_CHILD | WS_VISIBLE,
                0,
                0,
                0,
                0,
                s.host_window,
                NULL as _,
                instance,
                NULL,
            );
            if s.magnifier_window.is_null() {
                return Err(Error::new(
                    ErrorKind::Other,
                    format!(
                        "Failed CreateWindowA magnifier_window, error: {}",
                        GetLastError()
                    ),
                ));
            }

            // Hide the host window.
            let _ = ShowWindow(s.host_window, SW_HIDE);

            // Set the scaling callback to receive captured image.
            if let Some(set_callback_func) = s.mag_interface.set_image_scaling_callback_func {
                if FALSE
                    == set_callback_func(
                        s.magnifier_window,
                        Some(Self::on_gag_image_scaling_callback),
                    )
                {
                    return Err(Error::new(
                        ErrorKind::Other,
                        format!(
                            "Failed to MagSetImageScalingCallback, error: {}",
                            GetLastError()
                        ),
                    ));
                }
            } else {
                return Err(Error::new(
                    ErrorKind::Other,
                    "Unreachable, set_image_scaling_callback_func should not be none",
                ));
            }
        }

        Ok(s)
    }

    pub(crate) fn set_use_yuv(&mut self, use_yuv: bool) {
        self.use_yuv = use_yuv;
    }

    pub(crate) fn exclude(&mut self, cls: &str, name: &str) -> Result<bool> {
        let name_c = CString::new(name).unwrap();
        unsafe {
            let mut hwnd = if cls.len() == 0 {
                FindWindowExA(NULL as _, NULL as _, NULL as _, name_c.as_ptr())
            } else {
                let cls_c = CString::new(cls).unwrap();
                FindWindowExA(NULL as _, NULL as _, cls_c.as_ptr(), name_c.as_ptr())
            };

            if hwnd.is_null() {
                return Ok(false);
            }

            if let Some(set_window_filter_list_func) =
                self.mag_interface.set_window_filter_list_func
            {
                if FALSE
                    == set_window_filter_list_func(
                        self.magnifier_window,
                        MW_FILTERMODE_EXCLUDE,
                        1,
                        &mut hwnd,
                    )
                {
                    return Err(Error::new(
                        ErrorKind::Other,
                        format!(
                            "Failed MagSetWindowFilterList for cls {} name {}, err: {}",
                            cls,
                            name,
                            GetLastError()
                        ),
                    ));
                }
            } else {
                return Err(Error::new(
                    ErrorKind::Other,
                    "Unreachable, MagSetWindowFilterList should not be none",
                ));
            }
        }

        Ok(true)
    }

    pub(crate) fn get_rect(&self) -> ((i32, i32), usize, usize) {
        (
            (self.rect.left as _, self.rect.top as _),
            self.width as _,
            self.height as _,
        )
    }

    fn clear_data() {
        let mut lock = MAG_BUFFER.lock().unwrap();
        lock.0 = false;
        lock.1.clear();
    }

    pub(crate) fn frame(&mut self, data: &mut Vec<u8>) -> Result<()> {
        Self::clear_data();

        unsafe {
            let x = GetSystemMetrics(SM_XVIRTUALSCREEN);
            let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
            let w = GetSystemMetrics(SM_CXVIRTUALSCREEN);
            let h = GetSystemMetrics(SM_CYVIRTUALSCREEN);
            if !(self.rect.left == x as i32
                && self.rect.top == y as i32
                && self.rect.right == (x + w) as i32
                && self.rect.bottom == (y + h) as i32)
            {
                return Err(Error::new(
                    ErrorKind::Other,
                    format!(
                        "Failed Check screen rect ({}, {}, {} , {}) to ({}, {}, {}, {})",
                        self.rect.left,
                        self.rect.top,
                        self.rect.right,
                        self.rect.bottom,
                        x,
                        y,
                        x + w,
                        y + h
                    ),
                ));
            }

            if FALSE
                == SetWindowPos(
                    self.magnifier_window,
                    HWND_TOP,
                    self.rect.left,
                    self.rect.top,
                    self.rect.right,
                    self.rect.bottom,
                    0,
                )
            {
                return Err(Error::new(
                    ErrorKind::Other,
                    format!(
                        "Failed SetWindowPos (x, y, w , h) - ({}, {}, {}, {}), error {}",
                        self.rect.left,
                        self.rect.top,
                        self.rect.right,
                        self.rect.bottom,
                        GetLastError()
                    ),
                ));
            }

            // on_gag_image_scaling_callback will be called and fill in the
            // frame before set_window_source_func_ returns.
            if let Some(set_window_source_func) = self.mag_interface.set_window_source_func {
                if FALSE == set_window_source_func(self.magnifier_window, self.rect) {
                    return Err(Error::new(
                        ErrorKind::Other,
                        format!("Failed to MagSetWindowSource, error: {}", GetLastError()),
                    ));
                }
            } else {
                return Err(Error::new(
                    ErrorKind::Other,
                    "Unreachable, set_window_source_func should not be none",
                ));
            }
        }

        let mut lock = MAG_BUFFER.lock().unwrap();
        if !lock.0 {
            return Err(Error::new(
                ErrorKind::Other,
                "No data captured by magnifier",
            ));
        }

        if self.use_yuv {
            self.data.resize(lock.1.len(), 0);
            unsafe {
                std::ptr::copy_nonoverlapping(&mut lock.1[0], &mut self.data[0], self.data.len());
            }
            crate::common::bgra_to_i420(
                self.width as usize,
                self.height as usize,
                &self.data,
                data,
            );
        } else {
            data.resize(lock.1.len(), 0);
            unsafe {
                std::ptr::copy_nonoverlapping(&mut lock.1[0], &mut data[0], data.len());
            }
        }

        Ok(())
    }

    fn destroy_windows(&mut self) {
        if !self.magnifier_window.is_null() {
            unsafe {
                if FALSE == DestroyWindow(self.magnifier_window) {
                    //
                    println!("Failed DestroyWindow magnifier window {}", GetLastError())
                }
            }
        }
        self.magnifier_window = NULL as _;

        if !self.host_window.is_null() {
            unsafe {
                if FALSE == DestroyWindow(self.host_window) {
                    //
                    println!("Failed DestroyWindow host window {}", GetLastError())
                }
            }
        }
        self.host_window = NULL as _;
    }

    unsafe extern "C" fn on_gag_image_scaling_callback(
        _hwnd: HWND,
        srcdata: *mut ::std::os::raw::c_void,
        srcheader: MAGIMAGEHEADER,
        _destdata: *mut ::std::os::raw::c_void,
        _destheader: MAGIMAGEHEADER,
        _unclipped: RECT,
        _clipped: RECT,
        _dirty: HRGN,
    ) -> BOOL {
        Self::clear_data();

        if !IsEqualGUID(&srcheader.format, &GUID_WICPixelFormat32bppRGBA) {
            // log warning?
            return FALSE;
        }
        let mut lock = MAG_BUFFER.lock().unwrap();
        lock.1.resize(srcheader.cbSize, 0);
        std::ptr::copy_nonoverlapping(srcdata as _, &mut lock.1[0], srcheader.cbSize);
        lock.0 = true;
        TRUE
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test() {
        let mut capture_mag = CapturerMag::new((0, 0), 1920, 1080, false).unwrap();
        capture_mag.exclude("", "RustDeskPrivacyWindow").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1000 * 10));
        let mut data = Vec::new();
        capture_mag.frame(&mut data).unwrap();
        println!("capture data len: {}", data.len());
    }
}
