#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(improper_ctypes)]
#![allow(dead_code)]
#![allow(unused_imports)]

impl Default for vpx_codec_enc_cfg {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

impl Default for vpx_codec_ctx {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

impl Default for vpx_image_t {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

include!(concat!(env!("OUT_DIR"), "/vpx_ffi.rs"));
