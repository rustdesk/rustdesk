// https://github.com/astraw/vpx-encode
// https://github.com/astraw/env-libvpx-sys
// https://github.com/rust-av/vpx-rs/blob/master/src/decoder.rs

use hbb_common::anyhow::{anyhow, Context};
use hbb_common::log;
use hbb_common::message_proto::{EncodedVideoFrame, EncodedVideoFrames, Message, VideoFrame};
use hbb_common::ResultType;

use crate::codec::EncoderApi;
use crate::{GoogleImage, STRIDE_ALIGN};

use super::vpx::{vpx_codec_err_t::*, *};
use crate::{generate_call_macro, generate_call_ptr_macro, Error, Result};
use hbb_common::bytes::Bytes;
use std::os::raw::c_uint;
use std::{ptr, slice};

generate_call_macro!(call_vpx, false);
generate_call_macro!(call_vpx_allow_err, true);
generate_call_ptr_macro!(call_vpx_ptr);

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum VpxVideoCodecId {
    VP8,
    VP9,
}

impl Default for VpxVideoCodecId {
    fn default() -> VpxVideoCodecId {
        VpxVideoCodecId::VP9
    }
}

pub struct VpxEncoder {
    ctx: vpx_codec_ctx_t,
    width: usize,
    height: usize,
    id: VpxVideoCodecId,
}

pub struct VpxDecoder {
    ctx: vpx_codec_ctx_t,
}

impl EncoderApi for VpxEncoder {
    fn new(cfg: crate::codec::EncoderCfg) -> ResultType<Self>
    where
        Self: Sized,
    {
        match cfg {
            crate::codec::EncoderCfg::VPX(config) => {
                let i = match config.codec {
                    VpxVideoCodecId::VP8 => call_vpx_ptr!(vpx_codec_vp8_cx()),
                    VpxVideoCodecId::VP9 => call_vpx_ptr!(vpx_codec_vp9_cx()),
                };

                let c = match config.codec {
                    VpxVideoCodecId::VP8 => webrtc::vp8::enc_cfg(i, &config)?,
                    VpxVideoCodecId::VP9 => webrtc::vp9::enc_cfg(i, &config)?,
                };

                let mut ctx = Default::default();
                call_vpx!(vpx_codec_enc_init_ver(
                    &mut ctx,
                    i,
                    &c,
                    0,
                    VPX_ENCODER_ABI_VERSION as _
                ));
                match config.codec {
                    VpxVideoCodecId::VP8 => webrtc::vp8::set_control(&mut ctx, &c)?,
                    VpxVideoCodecId::VP9 => webrtc::vp9::set_control(&mut ctx, &c)?,
                }

                Ok(Self {
                    ctx,
                    width: config.width as _,
                    height: config.height as _,
                    id: config.codec,
                })
            }
            _ => Err(anyhow!("encoder type mismatch")),
        }
    }

    fn encode_to_message(&mut self, frame: &[u8], ms: i64) -> ResultType<Message> {
        let mut frames = Vec::new();
        for ref frame in self
            .encode(ms, frame, STRIDE_ALIGN)
            .with_context(|| "Failed to encode")?
        {
            frames.push(VpxEncoder::create_frame(frame));
        }
        for ref frame in self.flush().with_context(|| "Failed to flush")? {
            frames.push(VpxEncoder::create_frame(frame));
        }

        // to-do: flush periodically, e.g. 1 second
        if frames.len() > 0 {
            Ok(VpxEncoder::create_msg(self.id, frames))
        } else {
            Err(anyhow!("no valid frame"))
        }
    }

    fn use_yuv(&self) -> bool {
        true
    }

    fn set_bitrate(&mut self, bitrate: u32) -> ResultType<()> {
        let mut new_enc_cfg = unsafe { *self.ctx.config.enc.to_owned() };
        new_enc_cfg.rc_target_bitrate = bitrate;
        call_vpx!(vpx_codec_enc_config_set(&mut self.ctx, &new_enc_cfg));
        return Ok(());
    }
}

impl VpxEncoder {
    pub fn encode(&mut self, pts: i64, data: &[u8], stride_align: usize) -> Result<EncodeFrames> {
        if 2 * data.len() < 3 * self.width * self.height {
            return Err(Error::FailedCall("len not enough".to_string()));
        }

        let mut image = Default::default();
        call_vpx_ptr!(vpx_img_wrap(
            &mut image,
            vpx_img_fmt::VPX_IMG_FMT_I420,
            self.width as _,
            self.height as _,
            stride_align as _,
            data.as_ptr() as _,
        ));

        call_vpx!(vpx_codec_encode(
            &mut self.ctx,
            &image,
            pts as _,
            1, // Duration
            0, // Flags
            VPX_DL_REALTIME as _,
        ));

        Ok(EncodeFrames {
            ctx: &mut self.ctx,
            iter: ptr::null(),
        })
    }

    /// Notify the encoder to return any pending packets
    pub fn flush(&mut self) -> Result<EncodeFrames> {
        call_vpx!(vpx_codec_encode(
            &mut self.ctx,
            ptr::null(),
            -1, // PTS
            1,  // Duration
            0,  // Flags
            VPX_DL_REALTIME as _,
        ));

        Ok(EncodeFrames {
            ctx: &mut self.ctx,
            iter: ptr::null(),
        })
    }

    #[inline]
    pub fn create_msg(codec_id: VpxVideoCodecId, frames: Vec<EncodedVideoFrame>) -> Message {
        let mut msg_out = Message::new();
        let mut vf = VideoFrame::new();
        let vpxs = EncodedVideoFrames {
            frames: frames.into(),
            ..Default::default()
        };
        match codec_id {
            VpxVideoCodecId::VP8 => vf.set_vp8s(vpxs),
            VpxVideoCodecId::VP9 => vf.set_vp9s(vpxs),
        }
        msg_out.set_video_frame(vf);
        msg_out
    }

    #[inline]
    fn create_frame(frame: &EncodeFrame) -> EncodedVideoFrame {
        EncodedVideoFrame {
            data: Bytes::from(frame.data.to_vec()),
            key: frame.key,
            pts: frame.pts,
            ..Default::default()
        }
    }
}

impl Drop for VpxEncoder {
    fn drop(&mut self) {
        unsafe {
            let result = vpx_codec_destroy(&mut self.ctx);
            if result != VPX_CODEC_OK {
                panic!("failed to destroy vpx codec");
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct EncodeFrame<'a> {
    /// Compressed data.
    pub data: &'a [u8],
    /// Whether the frame is a keyframe.
    pub key: bool,
    /// Presentation timestamp (in timebase units).
    pub pts: i64,
}

#[derive(Clone, Copy, Debug)]
pub struct VpxEncoderConfig {
    /// The width (in pixels).
    pub width: c_uint,
    /// The height (in pixels).
    pub height: c_uint,
    /// The target bitrate (in kilobits per second).
    pub bitrate: c_uint,
    /// The codec
    pub codec: VpxVideoCodecId,
}

#[derive(Clone, Copy, Debug)]
pub struct VpxDecoderConfig {
    pub codec: VpxVideoCodecId,
    pub num_threads: u32,
}

pub struct EncodeFrames<'a> {
    ctx: &'a mut vpx_codec_ctx_t,
    iter: vpx_codec_iter_t,
}

impl<'a> Iterator for EncodeFrames<'a> {
    type Item = EncodeFrame<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            unsafe {
                let pkt = vpx_codec_get_cx_data(self.ctx, &mut self.iter);
                if pkt.is_null() {
                    return None;
                } else if (*pkt).kind == vpx_codec_cx_pkt_kind::VPX_CODEC_CX_FRAME_PKT {
                    let f = &(*pkt).data.frame;
                    return Some(Self::Item {
                        data: slice::from_raw_parts(f.buf as _, f.sz as _),
                        key: (f.flags & VPX_FRAME_IS_KEY) != 0,
                        pts: f.pts,
                    });
                } else {
                    // Ignore the packet.
                }
            }
        }
    }
}

impl VpxDecoder {
    /// Create a new decoder
    ///
    /// # Errors
    ///
    /// The function may fail if the underlying libvpx does not provide
    /// the VP9 decoder.
    pub fn new(config: VpxDecoderConfig) -> Result<Self> {
        // This is sound because `vpx_codec_ctx` is a repr(C) struct without any field that can
        // cause UB if uninitialized.
        let i = match config.codec {
            VpxVideoCodecId::VP8 => call_vpx_ptr!(vpx_codec_vp8_dx()),
            VpxVideoCodecId::VP9 => call_vpx_ptr!(vpx_codec_vp9_dx()),
        };
        let mut ctx = Default::default();
        let cfg = vpx_codec_dec_cfg_t {
            threads: if config.num_threads == 0 {
                num_cpus::get() as _
            } else {
                config.num_threads
            },
            w: 0,
            h: 0,
        };
        /*
        unsafe {
            println!("{}", vpx_codec_get_caps(i));
        }
        */
        call_vpx!(vpx_codec_dec_init_ver(
            &mut ctx,
            i,
            &cfg,
            0,
            VPX_DECODER_ABI_VERSION as _,
        ));
        Ok(Self { ctx })
    }

    /// Feed some compressed data to the encoder
    ///
    /// The `data` slice is sent to the decoder
    ///
    /// It matches a call to `vpx_codec_decode`.
    pub fn decode(&mut self, data: &[u8]) -> Result<DecodeFrames> {
        call_vpx!(vpx_codec_decode(
            &mut self.ctx,
            data.as_ptr(),
            data.len() as _,
            ptr::null_mut(),
            0,
        ));

        Ok(DecodeFrames {
            ctx: &mut self.ctx,
            iter: ptr::null(),
        })
    }

    /// Notify the decoder to return any pending frame
    pub fn flush(&mut self) -> Result<DecodeFrames> {
        call_vpx!(vpx_codec_decode(
            &mut self.ctx,
            ptr::null(),
            0,
            ptr::null_mut(),
            0
        ));
        Ok(DecodeFrames {
            ctx: &mut self.ctx,
            iter: ptr::null(),
        })
    }
}

impl Drop for VpxDecoder {
    fn drop(&mut self) {
        unsafe {
            let result = vpx_codec_destroy(&mut self.ctx);
            if result != VPX_CODEC_OK {
                panic!("failed to destroy vpx codec");
            }
        }
    }
}

pub struct DecodeFrames<'a> {
    ctx: &'a mut vpx_codec_ctx_t,
    iter: vpx_codec_iter_t,
}

impl<'a> Iterator for DecodeFrames<'a> {
    type Item = Image;
    fn next(&mut self) -> Option<Self::Item> {
        let img = unsafe { vpx_codec_get_frame(self.ctx, &mut self.iter) };
        if img.is_null() {
            return None;
        } else {
            return Some(Image(img));
        }
    }
}

// https://chromium.googlesource.com/webm/libvpx/+/bali/vpx/src/vpx_image.c
pub struct Image(*mut vpx_image_t);
impl Image {
    #[inline]
    pub fn new() -> Self {
        Self(std::ptr::null_mut())
    }

    #[inline]
    pub fn is_null(&self) -> bool {
        self.0.is_null()
    }

    #[inline]
    pub fn format(&self) -> vpx_img_fmt_t {
        // VPX_IMG_FMT_I420
        self.inner().fmt
    }

    #[inline]
    pub fn inner(&self) -> &vpx_image_t {
        unsafe { &*self.0 }
    }
}

impl GoogleImage for Image {
    #[inline]
    fn width(&self) -> usize {
        self.inner().d_w as _
    }

    #[inline]
    fn height(&self) -> usize {
        self.inner().d_h as _
    }

    #[inline]
    fn stride(&self) -> Vec<i32> {
        self.inner().stride.iter().map(|x| *x as i32).collect()
    }

    #[inline]
    fn planes(&self) -> Vec<*mut u8> {
        self.inner().planes.iter().map(|p| *p as *mut u8).collect()
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { vpx_img_free(self.0) };
        }
    }
}

unsafe impl Send for vpx_codec_ctx_t {}

mod webrtc {
    use super::*;

    const K_QP_MAX: u32 = 25; // worth adjusting
    const MODE: VideoCodecMode = VideoCodecMode::KScreensharing;
    const K_RTP_TICKS_PER_SECOND: i32 = 90000;
    const NUMBER_OF_TEMPORAL_LAYERS: u32 = 1;
    const DENOISING_ON: bool = true;
    const FRAME_DROP_ENABLED: bool = false;

    #[allow(dead_code)]
    #[derive(Debug, PartialEq, Eq)]
    enum VideoCodecMode {
        KRealtimeVideo,
        KScreensharing,
    }

    #[allow(dead_code)]
    #[derive(Debug, PartialEq, Eq)]
    enum VideoCodecComplexity {
        KComplexityLow = -1,
        KComplexityNormal = 0,
        KComplexityHigh = 1,
        KComplexityHigher = 2,
        KComplexityMax = 3,
    }

    // https://webrtc.googlesource.com/src/+/refs/heads/main/modules/video_coding/codecs/vp9/libvpx_vp9_encoder.cc
    pub mod vp9 {
        use super::*;
        const SVC: bool = false;
        // https://webrtc.googlesource.com/src/+/refs/heads/main/api/video_codecs/video_encoder.cc#35
        const KEY_FRAME_INTERVAL: u32 = 3000;
        const ADAPTIVE_QP_MODE: bool = true;

        pub fn enc_cfg(
            i: *const vpx_codec_iface_t,
            cfg: &VpxEncoderConfig,
        ) -> ResultType<vpx_codec_enc_cfg_t> {
            let mut c: vpx_codec_enc_cfg_t =
                unsafe { std::mem::MaybeUninit::zeroed().assume_init() };
            call_vpx!(vpx_codec_enc_config_default(i, &mut c, 0));

            // kProfile0
            c.g_bit_depth = vpx_bit_depth::VPX_BITS_8;
            c.g_profile = 0;
            c.g_input_bit_depth = 8;

            c.g_w = cfg.width;
            c.g_h = cfg.height;
            c.rc_target_bitrate = cfg.bitrate; // in kbit/s
            c.g_error_resilient = if SVC { VPX_ERROR_RESILIENT_DEFAULT } else { 0 };
            c.g_timebase.num = 1;
            c.g_timebase.den = K_RTP_TICKS_PER_SECOND;
            c.g_lag_in_frames = 0;
            c.rc_dropframe_thresh = if FRAME_DROP_ENABLED { 30 } else { 0 };
            c.rc_end_usage = vpx_rc_mode::VPX_CBR;
            c.g_pass = vpx_enc_pass::VPX_RC_ONE_PASS;
            c.rc_min_quantizer = if MODE == VideoCodecMode::KScreensharing {
                8
            } else {
                2
            };
            c.rc_max_quantizer = K_QP_MAX;
            c.rc_undershoot_pct = 50;
            c.rc_overshoot_pct = 50;
            c.rc_buf_initial_sz = 500;
            c.rc_buf_optimal_sz = 600;
            c.rc_buf_sz = 1000;
            // Key-frame interval is enforced manually by this wrapper.
            c.kf_mode = vpx_kf_mode::VPX_KF_DISABLED;
            // TODO(webm:1592): work-around for libvpx issue, as it can still
            // put some key-frames at will even in VPX_KF_DISABLED kf_mode.
            c.kf_max_dist = KEY_FRAME_INTERVAL;
            c.kf_min_dist = c.kf_max_dist;
            c.rc_resize_allowed = 0;
            // Determine number of threads based on the image size and #cores.
            c.g_threads = number_of_threads(c.g_w, c.g_h, num_cpus::get());

            c.temporal_layering_mode =
                vp9e_temporal_layering_mode::VP9E_TEMPORAL_LAYERING_MODE_NOLAYERING as _;
            c.ts_number_layers = 1;
            c.ts_rate_decimator[0] = 1;
            c.ts_periodicity = 1;
            c.ts_layer_id[0] = 0;

            Ok(c)
        }

        pub fn set_control(ctx: *mut vpx_codec_ctx_t, cfg: &vpx_codec_enc_cfg_t) -> ResultType<()> {
            use vp8e_enc_control_id::*;

            macro_rules! call_ctl {
                ($ctx:expr, $vpxe:expr, $arg:expr) => {{
                    call_vpx_allow_err!(vpx_codec_control_($ctx, $vpxe as i32, $arg));
                }};
            }
            call_ctl!(
                ctx,
                VP8E_SET_MAX_INTRA_BITRATE_PCT,
                max_intra_target(cfg.rc_buf_optimal_sz)
            );
            call_ctl!(ctx, VP9E_SET_AQ_MODE, if ADAPTIVE_QP_MODE { 3 } else { 0 });
            call_ctl!(ctx, VP9E_SET_FRAME_PARALLEL_DECODING, 0);
            #[cfg(not(any(target_arch = "arm", target_arch = "aarch64", target_os = "android")))]
            call_ctl!(ctx, VP9E_SET_SVC_GF_TEMPORAL_REF, 0);
            call_ctl!(
                ctx,
                VP8E_SET_CPUUSED,
                get_default_performance_flags(cfg.g_w, cfg.g_h).0
            );
            call_ctl!(ctx, VP9E_SET_TILE_COLUMNS, cfg.g_threads >> 1);
            // Turn on row-based multithreading.
            call_ctl!(ctx, VP9E_SET_ROW_MT, 1);
            let denoising = DENOISING_ON
                && allow_denoising()
                && get_default_performance_flags(cfg.g_w, cfg.g_h).1;
            call_ctl!(
                ctx,
                VP9E_SET_NOISE_SENSITIVITY,
                if denoising { 1 } else { 0 }
            );
            if MODE == VideoCodecMode::KScreensharing {
                call_ctl!(ctx, VP9E_SET_TUNE_CONTENT, 1);
            }
            // Enable encoder skip of static/low content blocks.
            call_ctl!(ctx, VP8E_SET_STATIC_THRESHOLD, 1);

            Ok(())
        }

        // return (base_layer_speed, allow_denoising)
        fn get_default_performance_flags(width: u32, height: u32) -> (u32, bool) {
            if cfg!(any(
                target_arch = "arm",
                target_arch = "aarch64",
                target_os = "android"
            )) {
                (8, true)
            } else if width * height < 352 * 288 {
                (5, true)
            } else if width * height < 1920 * 1080 {
                (7, true)
            } else {
                (9, false)
            }
        }

        fn allow_denoising() -> bool {
            // Do not enable the denoiser on ARM since optimization is pending.
            // Denoiser is on by default on other platforms.
            if cfg!(any(
                target_arch = "arm",
                target_arch = "aarch64",
                target_os = "android"
            )) {
                false
            } else {
                true
            }
        }

        fn number_of_threads(width: u32, height: u32, number_of_cores: usize) -> u32 {
            // Keep the number of encoder threads equal to the possible number of column
            // tiles, which is (1, 2, 4, 8). See comments below for VP9E_SET_TILE_COLUMNS.
            if width * height >= 1280 * 720 && number_of_cores > 4 {
                return 4;
            } else if width * height >= 640 * 360 && number_of_cores > 2 {
                return 2;
            } else {
                // Use 2 threads for low res on ARM.
                #[cfg(any(target_arch = "arm", target_arch = "aarch64", target_os = "android"))]
                if width * height >= 320 * 180 && number_of_cores > 2 {
                    return 2;
                }
                // 1 thread less than VGA.
                return 1;
            }
        }
    }

    // https://webrtc.googlesource.com/src/+/refs/heads/main/modules/video_coding/codecs/vp8/libvpx_vp8_encoder.cc
    pub mod vp8 {
        use super::*;
        // https://webrtc.googlesource.com/src/+/refs/heads/main/api/video_codecs/video_encoder.cc#23
        const DISABLE_KEY_FRAME_INTERVAL: bool = true;
        const KEY_FRAME_INTERVAL: u32 = 3000;
        const COMPLEXITY: VideoCodecComplexity = VideoCodecComplexity::KComplexityNormal;
        const K_TOKEN_PARTITIONS: vp8e_token_partitions =
            vp8e_token_partitions::VP8_ONE_TOKENPARTITION;

        pub fn enc_cfg(
            i: *const vpx_codec_iface_t,
            cfg: &VpxEncoderConfig,
        ) -> ResultType<vpx_codec_enc_cfg_t> {
            let mut c: vpx_codec_enc_cfg_t =
                unsafe { std::mem::MaybeUninit::zeroed().assume_init() };
            call_vpx!(vpx_codec_enc_config_default(i, &mut c, 0));

            c.g_w = cfg.width;
            c.g_h = cfg.height;
            c.g_timebase.num = 1;
            c.g_timebase.den = K_RTP_TICKS_PER_SECOND;
            c.g_lag_in_frames = 0;
            c.g_error_resilient = if NUMBER_OF_TEMPORAL_LAYERS > 1 {
                VPX_ERROR_RESILIENT_DEFAULT
            } else {
                0
            };
            c.rc_end_usage = vpx_rc_mode::VPX_CBR;
            c.g_pass = vpx_enc_pass::VPX_RC_ONE_PASS;
            c.rc_resize_allowed = 0;
            c.rc_min_quantizer = if MODE == VideoCodecMode::KScreensharing {
                12
            } else {
                2
            };
            c.rc_max_quantizer = K_QP_MAX;
            c.rc_undershoot_pct = 100;
            c.rc_overshoot_pct = 15;
            c.rc_buf_initial_sz = 500;
            c.rc_buf_optimal_sz = 600;
            c.rc_buf_sz = 1000;
            if !DISABLE_KEY_FRAME_INTERVAL && KEY_FRAME_INTERVAL > 0 {
                c.kf_mode = vpx_kf_mode::VPX_KF_AUTO;
                c.kf_max_dist = KEY_FRAME_INTERVAL;
            } else {
                c.kf_mode = vpx_kf_mode::VPX_KF_DISABLED;
            }
            c.g_threads = number_of_threads(c.g_w, c.g_h, num_cpus::get());
            c.rc_target_bitrate = cfg.bitrate;
            c.rc_dropframe_thresh = if FRAME_DROP_ENABLED { 30 } else { 0 };

            Ok(c)
        }

        pub fn set_control(ctx: *mut vpx_codec_ctx_t, cfg: &vpx_codec_enc_cfg_t) -> ResultType<()> {
            use vp8e_enc_control_id::*;

            macro_rules! call_ctl {
                ($ctx:expr, $vpxe:expr, $arg:expr) => {{
                    call_vpx_allow_err!(vpx_codec_control_($ctx, $vpxe as i32, $arg));
                }};
            }
            call_ctl!(
                ctx,
                VP8E_SET_STATIC_THRESHOLD,
                if MODE == VideoCodecMode::KScreensharing {
                    100
                } else {
                    1
                }
            );
            call_ctl!(
                ctx,
                VP8E_SET_CPUUSED,
                get_cpu_speed(cfg.g_w, cfg.g_h, num_cpus::get())
            );

            call_ctl!(ctx, VP8E_SET_TOKEN_PARTITIONS, K_TOKEN_PARTITIONS);
            call_ctl!(
                ctx,
                VP8E_SET_MAX_INTRA_BITRATE_PCT,
                max_intra_target(cfg.rc_buf_optimal_sz)
            );
            call_ctl!(
                ctx,
                VP8E_SET_SCREEN_CONTENT_MODE,
                if MODE == VideoCodecMode::KScreensharing {
                    2 // On with more aggressive rate control.
                } else {
                    0
                }
            );

            Ok(())
        }

        fn get_cpu_speed_default() -> i32 {
            match COMPLEXITY {
                VideoCodecComplexity::KComplexityHigh => -5,
                VideoCodecComplexity::KComplexityHigher => -4,
                VideoCodecComplexity::KComplexityMax => -3,
                _ => -6,
            }
        }

        fn get_cpu_speed(width: u32, height: u32, number_of_cores: usize) -> i32 {
            if cfg!(any(
                target_arch = "arm",
                target_arch = "aarch64",
                target_os = "android"
            )) {
                if number_of_cores <= 3 {
                    -12
                } else if width * height <= 352 * 288 {
                    -8
                } else if width * height <= 640 * 480 {
                    -10
                } else {
                    -12
                }
            } else {
                let cpu_speed_default = get_cpu_speed_default();
                if width * height < 352 * 288 {
                    if cpu_speed_default < -4 {
                        -4
                    } else {
                        cpu_speed_default
                    }
                } else {
                    cpu_speed_default
                }
            }
        }

        fn number_of_threads(width: u32, height: u32, cpus: usize) -> u32 {
            if cfg!(target_os = "android") {
                if width * height >= 320 * 180 {
                    if cpus >= 4 {
                        // 3 threads for CPUs with 4 and more cores since most of times only 4
                        // cores will be active.
                        3
                    } else if cpus == 3 || cpus == 2 {
                        2
                    } else {
                        1
                    }
                } else {
                    1
                }
            } else {
                if width * height >= 1920 * 1080 && cpus > 8 {
                    8 // 8 threads for 1080p on high perf machines.
                } else if width * height > 1280 * 960 && cpus >= 6 {
                    // 3 threads for 1080p.
                    return 3;
                } else if width * height > 640 * 480 && cpus >= 3 {
                    // Default 2 threads for qHD/HD, but allow 3 if core count is high enough,
                    // as this will allow more margin for high-core/low clock machines or if
                    // not built with highest optimization.
                    if cpus >= 6 {
                        3
                    } else {
                        2
                    }
                } else {
                    // 1 thread for VGA or less.
                    1
                }
            }
        }
    }

    fn max_intra_target(optimal_buffer_size: u32) -> u32 {
        const MAX_FRAMERATE: u32 = 60; // TODO
        let scale_par: f32 = 0.5;
        let target_pct: u32 =
            ((optimal_buffer_size as f32) * scale_par * MAX_FRAMERATE as f32 / 10.0) as u32;
        let min_intra_size: u32 = 300;
        if target_pct < min_intra_size {
            min_intra_size
        } else {
            target_pct
        }
    }
}
