#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use serde_derive::{Deserialize, Serialize};
include!(concat!(env!("OUT_DIR"), "/common_ffi.rs"));

pub(crate) const DATA_H264_720P: &[u8] = include_bytes!("res/720p.h264");
pub(crate) const DATA_H265_720P: &[u8] = include_bytes!("res/720p.h265");

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub enum Driver {
    NV,
    AMF,
    MFX,
    FFMPEG,
}

#[cfg(any(windows, target_os = "linux"))]
pub(crate) fn supported_gpu(_encode: bool) -> (bool, bool, bool) {
    #[cfg(target_os = "linux")]
    use std::ffi::c_int;
    #[cfg(target_os = "linux")]
    extern "C" {
        pub(crate) fn linux_support_nv() -> c_int;
        pub(crate) fn linux_support_amd() -> c_int;
        pub(crate) fn linux_support_intel() -> c_int;
    }

    #[allow(unused_unsafe)]
    unsafe {
        #[cfg(windows)]
        {
            #[cfg(feature = "vram")]
            return (
                _encode && crate::vram::nv::nv_encode_driver_support() == 0
                    || !_encode && crate::vram::nv::nv_decode_driver_support() == 0,
                crate::vram::amf::amf_driver_support() == 0,
                crate::vram::mfx::mfx_driver_support() == 0,
            );
            #[cfg(not(feature = "vram"))]
            return (true, true, true);
        }

        #[cfg(target_os = "linux")]
        return (
            linux_support_nv() == 0,
            linux_support_amd() == 0,
            linux_support_intel() == 0,
        );
        #[allow(unreachable_code)]
        (false, false, false)
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn get_video_toolbox_codec_support() -> (bool, bool, bool, bool) {
    use std::ffi::c_void;

    extern "C" {
        fn checkVideoToolboxSupport(
            h264_encode: *mut i32,
            h265_encode: *mut i32,
            h264_decode: *mut i32,
            h265_decode: *mut i32,
        ) -> c_void;
    }

    let mut h264_encode = 0;
    let mut h265_encode = 0;
    let mut h264_decode = 0;
    let mut h265_decode = 0;
    unsafe {
        checkVideoToolboxSupport(
            &mut h264_encode as *mut _,
            &mut h265_encode as *mut _,
            &mut h264_decode as *mut _,
            &mut h265_decode as *mut _,
        );
    }
    (
        h264_encode == 1,
        h265_encode == 1,
        h264_decode == 1,
        h265_decode == 1,
    )
}

pub fn get_gpu_signature() -> u64 {
    #[cfg(any(windows, target_os = "macos"))]
    {
        extern "C" {
            pub fn GetHwcodecGpuSignature() -> u64;
        }
        unsafe { GetHwcodecGpuSignature() }
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        0
    }
}

// called by child process
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub fn setup_parent_death_signal() {
    use std::sync::Once;

    static INIT: Once = Once::new();

    INIT.call_once(|| {
        use std::ffi::c_int;
        extern "C" {
            fn setup_parent_death_signal() -> c_int;
        }
        unsafe {
            let result = setup_parent_death_signal();
            if result == 0 {
                log::debug!("Successfully set up parent death signal");
            } else {
                log::warn!("Failed to set up parent death signal: {}", result);
            }
        }
    });
}

// called by parent process
#[cfg(windows)]
pub fn child_exit_when_parent_exit(child_process_id: u32) -> bool {
    unsafe {
        extern "C" {
             fn add_process_to_new_job(child_process_id: u32) -> i32;
        }
        let result = add_process_to_new_job(child_process_id);
        result == 0
    }
}