#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(improper_ctypes)]
#![allow(dead_code)]
#![allow(unused_imports)]

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

extern "C" {
    pub fn rustdesk_vpx_enc_cfg_alloc_default(
        iface: *const vpx_codec_iface,
        usage: ::std::os::raw::c_uint,
        out: *mut *mut vpx_codec_enc_cfg_t,
    ) -> vpx_codec_err_t;

    pub fn rustdesk_vpx_enc_cfg_free(cfg: *mut vpx_codec_enc_cfg_t);

    pub fn rustdesk_vpx_enc_cfg_set_basic(
        c: *mut vpx_codec_enc_cfg_t,
        w: ::std::os::raw::c_uint,
        h: ::std::os::raw::c_uint,
        threads: ::std::os::raw::c_uint,
        bitrate: ::std::os::raw::c_uint,
        profile: ::std::os::raw::c_uint,
    );

    pub fn rustdesk_vpx_enc_cfg_set_quantizer(
        c: *mut vpx_codec_enc_cfg_t,
        q_min: ::std::os::raw::c_uint,
        q_max: ::std::os::raw::c_uint,
    );

    pub fn rustdesk_vpx_enc_cfg_set_keyframe(
        c: *mut vpx_codec_enc_cfg_t,
        min_dist: ::std::os::raw::c_uint,
        max_dist: ::std::os::raw::c_uint,
        disabled: ::std::os::raw::c_int,
    );

    pub fn rustdesk_vpx_enc_cfg_set_target_bitrate(
        c: *mut vpx_codec_enc_cfg_t,
        bitrate: ::std::os::raw::c_uint,
    );

    pub fn rustdesk_vpx_enc_cfg_get_target_bitrate(
        c: *const vpx_codec_enc_cfg_t,
    ) -> ::std::os::raw::c_uint;

    pub fn rustdesk_vpx_dec_cfg_alloc(
        threads: ::std::os::raw::c_uint,
        w: ::std::os::raw::c_uint,
        h: ::std::os::raw::c_uint,
        out: *mut *mut vpx_codec_dec_cfg_t,
    ) -> vpx_codec_err_t;

    pub fn rustdesk_vpx_dec_cfg_free(cfg: *mut vpx_codec_dec_cfg_t);
}
