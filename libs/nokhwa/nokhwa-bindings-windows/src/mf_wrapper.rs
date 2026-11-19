#![allow(non_snake_case)]

// We use the `dlopen` crate to load the MF DLLs dynamically at runtime.
// Because the MF DLLs are not always present on all Windows systems, we need to load them at runtime instead of linking them statically.
// This allows us to avoid linking errors on systems that do not have the MF DLLs installed.

use core::ffi::c_void;
use dlopen::symbor::Library;
use lazy_static;
use std::sync::{Arc, Mutex};
use windows::core::HRESULT;
use windows::Win32::Media::MediaFoundation::{
    IMFActivate, IMFAttributes, IMFMediaSource, IMFMediaType, IMFSample, IMFSourceReader,
};

type UINT32 = u32;
type ULONG = std::os::raw::c_ulong;
type DWORD = std::os::raw::c_ulong;

const HRESULT_ERR_NO_INTERFACE: HRESULT = HRESULT(0x80004002 as u32 as i32);

macro_rules! make_lib_wrapper {
    ($s:ident,$dll:literal,$($field:ident : $tp:ty),+) => {
        struct $s {
            _lib: Option<Library>,
            $($field: Option<$tp>),+
        }

        impl $s {
            fn new() -> Self {
                let lib_name = get_lib_name($dll);
                let lib = match Library::open(&lib_name) {
                    Ok(lib) => Some(lib),
                    Err(e) => {
                        eprintln!("Failed to load library {}, {}", &lib_name, e);
                        None
                    }
                };

                $(let $field = if let Some(lib) = &lib {
                    match unsafe { lib.symbol::<$tp>(stringify!($field)) } {
                        Ok(m) => {
                            // println!("Successfully loaded function: {}", stringify!($field));
                            Some(*m)
                        },
                        Err(e) => {
                            eprintln!("Failed to load func {}: {}", stringify!($field), e);
                            None
                        }
                    }
                } else {
                    None
                };)+

                Self {
                    _lib: lib,
                    $( $field ),+
                }
            }
        }

        impl Default for $s {
            fn default() -> Self {
                Self::new()
            }
        }
    }
}

fn get_lib_name(dll_name: &str) -> String {
    format!("{}.dll", dll_name)
}

pub type FnMFEnumDeviceSources =
    unsafe extern "system" fn(*mut c_void, *mut *mut *mut c_void, *mut UINT32) -> HRESULT;

make_lib_wrapper!(
    MFWrapper,
    "mf",
    MFEnumDeviceSources: FnMFEnumDeviceSources
);

pub type FnMFCreateMediaType = unsafe extern "system" fn(*mut *mut c_void) -> HRESULT;
pub type FnMFCreateAttributes = unsafe extern "system" fn(*mut *mut c_void, UINT32) -> HRESULT;
pub type FnMFStartup = unsafe extern "system" fn(ULONG, DWORD) -> HRESULT;
pub type FnMFShutdown = unsafe extern "system" fn() -> HRESULT;
pub type FnMFCreateSample = unsafe extern "system" fn(*mut *mut c_void) -> HRESULT;

make_lib_wrapper!(
    MFPlatWrapper,
    "mfplat",
    MFCreateMediaType: FnMFCreateMediaType,
    MFCreateAttributes: FnMFCreateAttributes,
    MFStartup: FnMFStartup,
    MFShutdown: FnMFShutdown,
    MFCreateSample: FnMFCreateSample
);

pub type FnMFCreateSourceReaderFromMediaSource =
    unsafe extern "system" fn(*mut c_void, *mut c_void, *mut *mut c_void) -> HRESULT;

make_lib_wrapper!(
    MFReadWriteWrapper,
    "mfreadwrite",
    MFCreateSourceReaderFromMediaSource: FnMFCreateSourceReaderFromMediaSource
);

lazy_static::lazy_static! {
    static ref MF_WRAPPER: Arc<Mutex<MFWrapper>> = Arc::new(Mutex::new(MFWrapper::default()));
    static ref MF_PLAT_WRAPPER: Arc<Mutex<MFPlatWrapper>> = Arc::new(Mutex::new(MFPlatWrapper::default()));
    static ref MF_READ_WRITE_WRAPPER: Arc<Mutex<MFReadWriteWrapper>> = Arc::new(Mutex::new(MFReadWriteWrapper::default()));
}

pub unsafe fn MFEnumDeviceSources<'a, P0>(
    pattributes: P0,
    pppsourceactivate: *mut *mut ::core::option::Option<IMFActivate>,
    pcsourceactivate: *mut u32,
) -> ::windows::core::Result<()>
where
    P0: ::std::convert::Into<::windows::core::InParam<'a, IMFAttributes>>,
{
    let lib = MF_WRAPPER.lock().unwrap();
    if let Some(f) = lib.MFEnumDeviceSources {
        f(
            pattributes.into().abi() as _,
            ::core::mem::transmute(pppsourceactivate),
            ::core::mem::transmute(pcsourceactivate),
        )
        .ok()
    } else {
        Err(HRESULT_ERR_NO_INTERFACE.into())
    }
}

pub unsafe fn MFCreateMediaType() -> ::windows::core::Result<IMFMediaType> {
    let lib = MF_PLAT_WRAPPER.lock().unwrap();
    if let Some(f) = lib.MFCreateMediaType {
        let mut result__ = ::core::mem::MaybeUninit::zeroed();
        f(::core::mem::transmute(result__.as_mut_ptr())).from_abi::<IMFMediaType>(result__)
    } else {
        Err(HRESULT_ERR_NO_INTERFACE.into())
    }
}

pub unsafe fn MFCreateAttributes(
    ppmfattributes: *mut ::core::option::Option<IMFAttributes>,
    cinitialsize: u32,
) -> ::windows::core::Result<()> {
    let lib = MF_PLAT_WRAPPER.lock().unwrap();
    if let Some(f) = lib.MFCreateAttributes {
        f(::core::mem::transmute(ppmfattributes), cinitialsize).ok()
    } else {
        Err(HRESULT_ERR_NO_INTERFACE.into())
    }
}

pub unsafe fn MFStartup(version: u32, dwflags: u32) -> ::windows::core::Result<()> {
    let lib = MF_PLAT_WRAPPER.lock().unwrap();
    if let Some(f) = lib.MFStartup {
        f(version, dwflags).ok()
    } else {
        Err(HRESULT_ERR_NO_INTERFACE.into())
    }
}

pub unsafe fn MFShutdown() -> ::windows::core::Result<()> {
    let lib = MF_PLAT_WRAPPER.lock().unwrap();
    if let Some(f) = lib.MFShutdown {
        f().ok()
    } else {
        Err(HRESULT_ERR_NO_INTERFACE.into())
    }
}

pub unsafe fn MFCreateSample() -> ::windows::core::Result<IMFSample> {
    let lib = MF_PLAT_WRAPPER.lock().unwrap();
    if let Some(f) = lib.MFCreateSample {
        let mut result__ = ::core::mem::MaybeUninit::zeroed();
        f(::core::mem::transmute(result__.as_mut_ptr())).from_abi::<IMFSample>(result__)
    } else {
        Err(HRESULT_ERR_NO_INTERFACE.into())
    }
}

pub unsafe fn MFCreateSourceReaderFromMediaSource<'a, P0, P1>(
    pmediasource: P0,
    pattributes: P1,
) -> ::windows::core::Result<IMFSourceReader>
where
    P0: ::std::convert::Into<::windows::core::InParam<'a, IMFMediaSource>>,
    P1: ::std::convert::Into<::windows::core::InParam<'a, IMFAttributes>>,
{
    let lib = MF_READ_WRITE_WRAPPER.lock().unwrap();
    if let Some(f) = lib.MFCreateSourceReaderFromMediaSource {
        let mut result__ = ::core::mem::MaybeUninit::zeroed();
        f(
            pmediasource.into().abi() as _,
            pattributes.into().abi() as _,
            ::core::mem::transmute(result__.as_mut_ptr()),
        )
        .from_abi::<IMFSourceReader>(result__)
    } else {
        Err(HRESULT_ERR_NO_INTERFACE.into())
    }
}

impl Drop for MFPlatWrapper {
    fn drop(&mut self) {
        unsafe {
            if let Some(f) = self.MFShutdown {
                let _ = f().ok();
            }
        }
    }
}
