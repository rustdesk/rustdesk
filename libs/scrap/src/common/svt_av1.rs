#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(improper_ctypes)]
#![allow(dead_code)]

include!(concat!(env!("OUT_DIR"), "/svt_av1_ffi.rs"));

use crate::codec::{base_bitrate, codec_thread_num, EncoderApi, EncoderCfg};
use crate::{EncodeInput, EncodeYuvFormat, Pixfmt, STRIDE_ALIGN};
use hbb_common::{
    anyhow::Context,
    bail,
    bytes::Bytes,
    log,
    message_proto::{EncodedVideoFrame, EncodedVideoFrames, VideoFrame},
    ResultType,
};
use std::{mem::MaybeUninit, os::raw::c_void, ptr, slice};

// SVT-AV1 rejects on-the-fly target bitrates above 100_000 kbps.
const MAX_TARGET_BITRATE_KBPS: u32 = 100_000;
// CBR budgets bits per frame from the configured frame rate, VideoQoS pushes
// its authoritative pacing rate in via set_fps.
const DEFAULT_FPS: u32 = 30;

#[derive(Clone, Copy, Debug)]
pub struct SvtAv1EncoderConfig {
    pub width: u32,
    pub height: u32,
    pub quality: f32,
    pub keyframe_interval: Option<usize>,
}

pub struct SvtAv1Encoder {
    handle: *mut EbComponentType,
    width: usize,
    height: usize,
    yuvfmt: EncodeYuvFormat,
    // current target bitrate in kbps, same unit as aom's rc_target_bitrate
    bitrate: u32,
    // bitrate change (kbps) to apply with the next picture via RATE_CHANGE_EVENT
    pending_bitrate: Option<u32>,
    // frame rate currently configured in the encoder
    fps: u32,
    // frame rate change to apply with the next picture via FRAME_RATE_CHANGE_EVENT
    pending_fps: Option<u32>,
}

// The handle is only used behind &mut self, SVT-AV1 synchronizes internally.
unsafe impl Send for SvtAv1Encoder {}

impl EncoderApi for SvtAv1Encoder {
    fn new(cfg: EncoderCfg, i444: bool) -> ResultType<Self>
    where
        Self: Sized,
    {
        match cfg {
            EncoderCfg::SVTAV1(config) => {
                if i444 {
                    // SVT-AV1 only supports 4:2:0 input, callers must fall back to aom for i444.
                    bail!("svt-av1 encoder does not support I444");
                }
                if !Self::support(config.width, config.height) {
                    bail!(
                        "svt-av1 encoder does not support resolution {}x{}",
                        config.width,
                        config.height
                    );
                }
                let mut handle: *mut EbComponentType = ptr::null_mut();
                let mut c: MaybeUninit<EbSvtAv1EncConfiguration> = MaybeUninit::zeroed();
                let res = unsafe { svt_av1_enc_init_handle(&mut handle, c.as_mut_ptr()) };
                if res != EbErrorType::EB_ErrorNone || handle.is_null() {
                    bail!("svt_av1_enc_init_handle failed: {res:?}");
                }
                // c is loaded with the library defaults now, only override what we need.
                let mut c = unsafe { c.assume_init() };
                let bitrate = Self::bitrate(config.width, config.height, config.quality)
                    .min(MAX_TARGET_BITRATE_KBPS);
                Self::apply_config(&mut c, &config, bitrate);
                let mut res = unsafe { svt_av1_enc_set_parameter(handle, &mut c) };
                if res == EbErrorType::EB_ErrorNone {
                    res = unsafe { svt_av1_enc_init(handle) };
                }
                if res != EbErrorType::EB_ErrorNone {
                    unsafe {
                        svt_av1_enc_deinit_handle(handle);
                    }
                    bail!("failed to init svt-av1 encoder: {res:?}");
                }
                Ok(Self {
                    handle,
                    width: config.width as _,
                    height: config.height as _,
                    yuvfmt: Self::get_yuvfmt(config.width, config.height),
                    bitrate,
                    pending_bitrate: None,
                    fps: DEFAULT_FPS,
                    pending_fps: None,
                })
            }
            _ => bail!("encoder type mismatch"),
        }
    }

    fn encode_to_message(&mut self, input: EncodeInput, ms: i64) -> ResultType<VideoFrame> {
        let frames = self
            .encode(ms, input.yuv()?)
            .with_context(|| "Failed to encode")?;
        if frames.len() > 0 {
            Ok(Self::create_video_frame(frames))
        } else {
            bail!("no valid frame");
        }
    }

    fn yuvfmt(&self) -> EncodeYuvFormat {
        self.yuvfmt.clone()
    }

    #[cfg(feature = "vram")]
    fn input_texture(&self) -> bool {
        false
    }

    fn set_quality(&mut self, ratio: f32) -> ResultType<()> {
        let bitrate =
            Self::bitrate(self.width as _, self.height as _, ratio).min(MAX_TARGET_BITRATE_KBPS);
        if bitrate > 0 && bitrate != self.bitrate {
            self.bitrate = bitrate;
            self.pending_bitrate = Some(bitrate);
        }
        Ok(())
    }

    fn set_fps(&mut self, fps: u32) {
        if fps == 0 {
            return;
        }
        // also cancels a queued change that a later call made moot again
        if fps == self.fps {
            self.pending_fps = None;
        } else {
            self.pending_fps = Some(fps);
        }
    }

    fn bitrate(&self) -> u32 {
        self.bitrate
    }

    fn support_changing_quality(&self) -> bool {
        true
    }

    fn latency_free(&self) -> bool {
        true
    }

    fn is_hardware(&self) -> bool {
        false
    }

    fn disable(&self) {}
}

impl SvtAv1Encoder {
    pub fn support(width: u32, height: u32) -> bool {
        width >= 64
            && height >= 64
            && width <= 16384
            && height <= 8704
            && width % 2 == 0
            && height % 2 == 0
    }

    fn apply_config(c: &mut EbSvtAv1EncConfiguration, cfg: &SvtAv1EncoderConfig, bitrate: u32) {
        c.enc_mode = Self::preset(cfg.width, cfg.height);
        c.source_width = cfg.width;
        c.source_height = cfg.height;
        // CBR budgets bits per frame from this rate, set_fps adjusts it on the
        // fly with FRAME_RATE_CHANGE_EVENT.
        c.frame_rate_numerator = DEFAULT_FPS;
        c.frame_rate_denominator = 1;
        c.encoder_bit_depth = 8;
        c.encoder_color_format = EbColorFormat::EB_YUV420;
        c.profile = EbAv1SeqProfile::MAIN_PROFILE;
        // Low delay + rtc + CBR: one packet out per picture in, svt_av1_enc_get_packet
        // blocks until the packet for the sent picture is ready, and rate/keyframe
        // changes on the fly are allowed.
        c.pred_structure = PredStructure::LOW_DELAY;
        c.rtc = true;
        c.rate_control_mode = SvtAv1RcMode::SVT_AV1_RC_MODE_CBR as _;
        c.target_bit_rate = bitrate.min(MAX_TARGET_BITRATE_KBPS) * 1000;
        // Full envelope of aom's calc_q_values so later bitrate changes are not clipped.
        c.min_qp_allowed = 5;
        c.max_qp_allowed = 45;
        let (q_min, q_max) = Self::calc_q_values(cfg.quality);
        c.qp = (q_min + q_max) / 2;
        c.look_ahead_distance = 0;
        c.recode_loop = 0; // DISALLOW_RECODE
        c.scene_change_detection = 0;
        c.screen_content_mode = 1;
        c.tune = 1; // PSNR, low delay does not support tune 0
        c.level_of_parallelism = Self::parallelism();
        c.intra_refresh_type = SvtAv1IntraRefreshType::SVT_AV1_KF_REFRESH; // closed GOP
        c.intra_period_length = match cfg.keyframe_interval {
            Some(keyframe_interval) => keyframe_interval.saturating_sub(1) as _,
            None => -1, // no periodic intra refresh, keyframes only on demand
        };
        // pic_type is left to the encoder and keyframes come from encoder
        // restarts, the force_key_frames flag must stay off for low delay CBR
        // (the library resets it with a warning).
        c.force_key_frames = false;
    }

    fn preset(width: u32, height: u32) -> i8 {
        // Mirrors aom's get_cpu_speed buckets, M9-M13 are the rtc presets.
        if width * height <= 320 * 180 {
            9
        } else if width * height <= 640 * 360 {
            10
        } else {
            11
        }
    }

    fn parallelism() -> u32 {
        // level_of_parallelism is a level from 1 to 6, not a thread count.
        match codec_thread_num(64) {
            n if n >= 32 => 6,
            n if n >= 16 => 5,
            n if n >= 8 => 4,
            n if n >= 4 => 3,
            n if n >= 2 => 2,
            _ => 1,
        }
    }

    fn encode(&mut self, ms: i64, data: &[u8]) -> ResultType<Vec<EncodedVideoFrame>> {
        let fmt = &self.yuvfmt;
        let chroma_height = (fmt.h + 1) / 2;
        let len = fmt.v + fmt.stride[2] * chroma_height;
        if data.len() < len {
            bail!("len not enough: {} < {}", data.len(), len);
        }
        let mut io = EbSvtIOFormat {
            luma: data.as_ptr() as *mut u8,
            cb: data[fmt.u..].as_ptr() as *mut u8,
            cr: data[fmt.v..].as_ptr() as *mut u8,
            y_stride: fmt.stride[0] as _,
            cr_stride: fmt.stride[2] as _,
            cb_stride: fmt.stride[1] as _,
        };
        let mut hdr: EbBufferHeaderType = unsafe { std::mem::zeroed() };
        hdr.size = std::mem::size_of::<EbBufferHeaderType>() as _;
        hdr.p_buffer = &mut io as *mut EbSvtIOFormat as *mut u8;
        hdr.n_filled_len = len as _;
        hdr.pts = ms;
        hdr.pic_type = EbAv1PictureType::EB_AV1_INVALID_PICTURE; // encoder decides

        // send_picture copies the private data list, stack lifetime is fine.
        // The pending changes are only cleared after a successful send, so a
        // failed send retries them with the next picture.
        let pending_bitrate = self.pending_bitrate;
        let pending_fps = self.pending_fps;
        let mut rate_info = SvtAv1RateInfo {
            seq_qp: 0,
            target_bit_rate: pending_bitrate.unwrap_or(0).min(MAX_TARGET_BITRATE_KBPS) * 1000,
        };
        let mut fps_info = SvtAv1FrameRateInfo {
            frame_rate_numerator: pending_fps.unwrap_or(0),
            frame_rate_denominator: 1,
        };
        let mut rate_node = EbPrivDataNode {
            node_type: PrivDataType::RATE_CHANGE_EVENT,
            data: &mut rate_info as *mut SvtAv1RateInfo as *mut c_void,
            size: std::mem::size_of::<SvtAv1RateInfo>() as _,
            next: ptr::null_mut(),
        };
        let mut fps_node = EbPrivDataNode {
            node_type: PrivDataType::FRAME_RATE_CHANGE_EVENT,
            data: &mut fps_info as *mut SvtAv1FrameRateInfo as *mut c_void,
            size: std::mem::size_of::<SvtAv1FrameRateInfo>() as _,
            next: ptr::null_mut(),
        };
        let mut list: *mut EbPrivDataNode = ptr::null_mut();
        if pending_bitrate.is_some() {
            list = &mut rate_node;
        }
        if pending_fps.is_some() {
            fps_node.next = list;
            list = &mut fps_node;
        }
        hdr.p_app_private = list as *mut c_void;
        // The input YUV planes are copied inside send_picture.
        let res = unsafe { svt_av1_enc_send_picture(self.handle, &mut hdr) };
        if res != EbErrorType::EB_ErrorNone {
            bail!("svt_av1_enc_send_picture failed: {res:?}");
        }
        self.pending_bitrate = None;
        self.pending_fps = None;
        if let Some(fps) = pending_fps {
            self.fps = fps;
        }

        // In low delay mode get_packet blocks until the packet for the picture just
        // sent is ready, and each picture produces exactly one packet. Do not call
        // it again without sending another picture, it would block forever.
        let mut frames = Vec::new();
        let mut pkt: *mut EbBufferHeaderType = ptr::null_mut();
        let res = unsafe { svt_av1_enc_get_packet(self.handle, &mut pkt, 0) };
        match res {
            EbErrorType::EB_ErrorNone if !pkt.is_null() => unsafe {
                let h = &*pkt;
                frames.push(EncodedVideoFrame {
                    data: Bytes::from(
                        slice::from_raw_parts(h.p_buffer, h.n_filled_len as usize).to_vec(),
                    ),
                    key: h.pic_type == EbAv1PictureType::EB_AV1_KEY_PICTURE,
                    pts: h.pts,
                    ..Default::default()
                });
                svt_av1_enc_release_out_buffer(&mut pkt);
            },
            EbErrorType::EB_NoErrorEmptyQueue => {}
            _ => {
                if !pkt.is_null() {
                    unsafe {
                        svt_av1_enc_release_out_buffer(&mut pkt);
                    }
                }
                bail!("svt_av1_enc_get_packet failed: {res:?}");
            }
        }
        Ok(frames)
    }

    #[inline]
    fn create_video_frame(frames: Vec<EncodedVideoFrame>) -> VideoFrame {
        let mut vf = VideoFrame::new();
        let av1s = EncodedVideoFrames {
            frames: frames.into(),
            ..Default::default()
        };
        vf.set_av1s(av1s);
        vf
    }

    fn bitrate(width: u32, height: u32, ratio: f32) -> u32 {
        let bitrate = base_bitrate(width, height) as f32;
        (bitrate * ratio) as u32
    }

    // Same mapping as AomEncoder::calc_q_values, only used to seed the start qp.
    #[inline]
    fn calc_q_values(ratio: f32) -> (u32, u32) {
        let b = (ratio * 100.0) as u32;
        let b = std::cmp::min(b, 200);
        let q_min1 = 24;
        let q_min2 = 5;
        let q_max1 = 45;
        let q_max2 = 25;

        let t = b as f32 / 200.0;

        let mut q_min: u32 = ((1.0 - t) * q_min1 as f32 + t * q_min2 as f32).round() as u32;
        let mut q_max = ((1.0 - t) * q_max1 as f32 + t * q_max2 as f32).round() as u32;

        q_min = q_min.clamp(q_min2, q_min1);
        q_max = q_max.clamp(q_max2, q_max1);

        (q_min, q_max)
    }

    fn get_yuvfmt(width: u32, height: u32) -> EncodeYuvFormat {
        let w = width as usize;
        let h = height as usize;
        let align = |x: usize| (x + STRIDE_ALIGN - 1) & !(STRIDE_ALIGN - 1);
        let stride_y = align(w);
        let stride_uv = align((w + 1) / 2);
        let u = stride_y * h;
        let v = u + stride_uv * ((h + 1) / 2);
        EncodeYuvFormat {
            pixfmt: Pixfmt::I420,
            w,
            h,
            stride: vec![stride_y, stride_uv, stride_uv],
            u,
            v,
        }
    }
}

impl Drop for SvtAv1Encoder {
    fn drop(&mut self) {
        unsafe {
            if self.handle.is_null() {
                return;
            }
            // svt_av1_enc_deinit drains the pipeline itself, but logs an error
            // when EOS was not sent first.
            let mut hdr: EbBufferHeaderType = std::mem::zeroed();
            hdr.size = std::mem::size_of::<EbBufferHeaderType>() as _;
            hdr.pic_type = EbAv1PictureType::EB_AV1_INVALID_PICTURE;
            hdr.flags = EB_BUFFERFLAG_EOS;
            let _ = svt_av1_enc_send_picture(self.handle, &mut hdr);
            let res = svt_av1_enc_deinit(self.handle);
            if res != EbErrorType::EB_ErrorNone {
                log::error!("svt_av1_enc_deinit failed: {res:?}");
            }
            let res = svt_av1_enc_deinit_handle(self.handle);
            if res != EbErrorType::EB_ErrorNone {
                log::error!("svt_av1_enc_deinit_handle failed: {res:?}");
            }
            self.handle = ptr::null_mut();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::GoogleImage;

    fn fake_i420(fmt: &EncodeYuvFormat, index: usize) -> Vec<u8> {
        let chroma_height = (fmt.h + 1) / 2;
        let len = fmt.v + fmt.stride[2] * chroma_height;
        let mut yuv = vec![128u8; len];
        // moving gradient so every frame differs
        for y in 0..fmt.h {
            let row = &mut yuv[y * fmt.stride[0]..y * fmt.stride[0] + fmt.w];
            for (x, px) in row.iter_mut().enumerate() {
                *px = ((x + y + index * 17) % 256) as u8;
            }
        }
        yuv
    }

    #[test]
    fn test_encode_and_decode_with_aom() {
        let (width, height) = (640u32, 480u32);
        let mut enc = SvtAv1Encoder::new(
            crate::codec::EncoderCfg::SVTAV1(SvtAv1EncoderConfig {
                width,
                height,
                quality: 1.0,
                keyframe_interval: None,
            }),
            false,
        )
        .unwrap();
        let fmt = enc.yuvfmt();
        let mut dec = crate::aom::AomDecoder::new().unwrap();
        for i in 0..20usize {
            if i == 5 {
                // exercises the FRAME_RATE_CHANGE_EVENT path on the next frame
                enc.set_fps(60);
            }
            if i == 10 {
                // exercises the RATE_CHANGE_EVENT path on the next frame
                enc.set_quality(0.5).unwrap();
            }
            let yuv = fake_i420(&fmt, i);
            let frames = enc.encode(i as i64 * 33, &yuv).unwrap();
            // one packet out per picture in, this is the low delay contract
            assert_eq!(frames.len(), 1, "no packet for frame {i}");
            assert_eq!(frames[0].key, i == 0, "unexpected key flag for frame {i}");
            assert!(!frames[0].data.is_empty());
            let mut decoded = 0;
            for f in &frames {
                for img in dec.decode(&f.data).unwrap() {
                    assert_eq!(img.width(), width as usize);
                    assert_eq!(img.height(), height as usize);
                    decoded += 1;
                }
            }
            for img in dec.flush().unwrap() {
                assert_eq!(img.width(), width as usize);
                decoded += 1;
            }
            assert!(decoded >= 1, "aom failed to decode svt-av1 frame {i}");
        }
    }

    #[test]
    fn test_unsupported_resolution() {
        assert!(SvtAv1Encoder::new(
            crate::codec::EncoderCfg::SVTAV1(SvtAv1EncoderConfig {
                width: 62,
                height: 62,
                quality: 1.0,
                keyframe_interval: None,
            }),
            false,
        )
        .is_err());
    }
}
