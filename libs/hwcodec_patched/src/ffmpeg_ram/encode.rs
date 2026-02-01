use crate::{
    common::{
        DataFormat::{self, *},
        Quality, RateControl, TEST_TIMEOUT_MS,
    },
    ffmpeg::{init_av_log, AVPixelFormat},
    ffmpeg_ram::{
        ffmpeg_linesize_offset_length, ffmpeg_ram_encode, ffmpeg_ram_free_encoder,
        ffmpeg_ram_new_encoder, ffmpeg_ram_set_bitrate, CodecInfo, AV_NUM_DATA_POINTERS,
    },
};
use log::trace;
use std::{
    ffi::{c_void, CString},
    fmt::Display,
    os::raw::c_int,
    slice,
};

use super::Priority;
#[cfg(any(windows, target_os = "linux"))]
use crate::common::Driver;

#[derive(Debug, Clone, PartialEq)]
pub struct EncodeContext {
    pub name: String,
    pub mc_name: Option<String>,
    pub width: i32,
    pub height: i32,
    pub pixfmt: AVPixelFormat,
    pub align: i32,
    pub fps: i32,
    pub gop: i32,
    pub rc: RateControl,
    pub quality: Quality,
    pub kbs: i32,
    pub q: i32,
    pub thread_count: i32,
}

pub struct EncodeFrame {
    pub data: Vec<u8>,
    pub pts: i64,
    pub key: i32,
}

impl Display for EncodeFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "encode len:{}, pts:{}", self.data.len(), self.pts)
    }
}

pub struct Encoder {
    codec: *mut c_void,
    frames: *mut Vec<EncodeFrame>,
    pub ctx: EncodeContext,
    pub linesize: Vec<i32>,
    pub offset: Vec<i32>,
    pub length: i32,
}

impl Encoder {
    pub fn new(ctx: EncodeContext) -> Result<Self, ()> {
        init_av_log();
        if ctx.width % 2 == 1 || ctx.height % 2 == 1 {
            return Err(());
        }
        unsafe {
            let mut linesize = Vec::<i32>::new();
            linesize.resize(AV_NUM_DATA_POINTERS as _, 0);
            let mut offset = Vec::<i32>::new();
            offset.resize(AV_NUM_DATA_POINTERS as _, 0);
            let mut length = Vec::<i32>::new();
            length.resize(1, 0);
            let gpu = std::env::var("RUSTDESK_HWCODEC_NVENC_GPU")
                .unwrap_or("-1".to_owned())
                .parse()
                .unwrap_or(-1);
            let mc_name = ctx.mc_name.clone().unwrap_or_default();
            let codec = ffmpeg_ram_new_encoder(
                CString::new(ctx.name.as_str()).map_err(|_| ())?.as_ptr(),
                CString::new(mc_name.as_str()).map_err(|_| ())?.as_ptr(),
                ctx.width,
                ctx.height,
                ctx.pixfmt as c_int,
                ctx.align,
                ctx.fps,
                ctx.gop,
                ctx.rc as _,
                ctx.quality as _,
                ctx.kbs,
                ctx.q,
                ctx.thread_count,
                gpu,
                linesize.as_mut_ptr(),
                offset.as_mut_ptr(),
                length.as_mut_ptr(),
                Some(Encoder::callback),
            );

            if codec.is_null() {
                return Err(());
            }

            Ok(Encoder {
                codec,
                frames: Box::into_raw(Box::new(Vec::<EncodeFrame>::new())),
                ctx,
                linesize,
                offset,
                length: length[0],
            })
        }
    }

    pub fn encode(&mut self, data: &[u8], ms: i64) -> Result<&mut Vec<EncodeFrame>, i32> {
        unsafe {
            (&mut *self.frames).clear();
            let result = ffmpeg_ram_encode(
                self.codec,
                (*data).as_ptr(),
                data.len() as _,
                self.frames as *const _ as *const c_void,
                ms,
            );
            if result != 0 {
                return Err(result);
            }
            Ok(&mut *self.frames)
        }
    }

    extern "C" fn callback(data: *const u8, size: c_int, pts: i64, key: i32, obj: *const c_void) {
        unsafe {
            let frames = &mut *(obj as *mut Vec<EncodeFrame>);
            frames.push(EncodeFrame {
                data: slice::from_raw_parts(data, size as _).to_vec(),
                pts,
                key,
            });
        }
    }

    pub fn set_bitrate(&mut self, kbs: i32) -> Result<(), ()> {
        let ret = unsafe { ffmpeg_ram_set_bitrate(self.codec, kbs) };
        if ret == 0 {
            Ok(())
        } else {
            Err(())
        }
    }

    pub fn format_from_name(name: String) -> Result<DataFormat, ()> {
        if name.contains("h264") {
            return Ok(H264);
        } else if name.contains("hevc") {
            return Ok(H265);
        } else if name.contains("vp8") {
            return Ok(VP8);
        } else if name.contains("vp9") {
            return Ok(VP9);
        } else if name.contains("av1") {
            return Ok(AV1);
        }
        Err(())
    }

    pub fn available_encoders(ctx: EncodeContext, _sdk: Option<String>) -> Vec<CodecInfo> {
        use log::debug;

        if !(cfg!(windows) || cfg!(target_os = "linux") || cfg!(target_os = "macos")) {
            return vec![];
        }
        let mut codecs: Vec<CodecInfo> = vec![];
        #[cfg(any(windows, target_os = "linux"))]
        {
            let contains = |_vendor: Driver, _format: DataFormat| {
                #[cfg(all(windows, feature = "vram"))]
                {
                    if let Some(_sdk) = _sdk.as_ref() {
                        if !_sdk.is_empty() {
                            if let Ok(available) =
                                crate::vram::Available::deserialize(_sdk.as_str())
                            {
                                return available.contains(true, _vendor, _format);
                            }
                        }
                    }
                }
                true
            };
            let (_nv, amf, _intel) = crate::common::supported_gpu(true);
            debug!(
                "GPU support detected - NV: {}, AMF: {}, Intel: {}",
                _nv, amf, _intel
            );

            #[cfg(windows)]
            if _intel && contains(Driver::MFX, H264) {
                codecs.push(CodecInfo {
                    name: "h264_qsv".to_owned(),
                    format: H264,
                    priority: Priority::Best as _,
                    ..Default::default()
                });
            }
            #[cfg(windows)]
            if _intel && contains(Driver::MFX, H265) {
                codecs.push(CodecInfo {
                    name: "hevc_qsv".to_owned(),
                    format: H265,
                    priority: Priority::Best as _,
                    ..Default::default()
                });
            }
            if _nv && contains(Driver::NV, H264) {
                codecs.push(CodecInfo {
                    name: "h264_nvenc".to_owned(),
                    format: H264,
                    priority: Priority::Best as _,
                    ..Default::default()
                });
            }
            if _nv && contains(Driver::NV, H265) {
                codecs.push(CodecInfo {
                    name: "hevc_nvenc".to_owned(),
                    format: H265,
                    priority: Priority::Best as _,
                    ..Default::default()
                });
            }
            if amf && contains(Driver::AMF, H264) {
                codecs.push(CodecInfo {
                    name: "h264_amf".to_owned(),
                    format: H264,
                    priority: Priority::Best as _,
                    ..Default::default()
                });
            }
            if amf {
                // sdk not use h265
                codecs.push(CodecInfo {
                    name: "hevc_amf".to_owned(),
                    format: H265,
                    priority: Priority::Best as _,
                    ..Default::default()
                });
            }
            #[cfg(target_os = "linux")]
            {
                codecs.push(CodecInfo {
                    name: "h264_vaapi".to_owned(),
                    format: H264,
                    priority: Priority::Good as _,
                    ..Default::default()
                });
                // remove because poor quality on one of my computer
                // codecs.push(CodecInfo {
                //     name: "hevc_vaapi".to_owned(),
                //     format: H265,
                //     priority: Priority::Good as _,
                //     ..Default::default()
                // });
            }
        }

        #[cfg(target_os = "macos")]
        {
            let (_h264, h265, _, _) = crate::common::get_video_toolbox_codec_support();
            // h264 encode failed too often, not AV_CODEC_CAP_HARDWARE
            // if h264 {
            //     codecs.push(CodecInfo {
            //         name: "h264_videotoolbox".to_owned(),
            //         format: H264,
            //         priority: Priority::Best as _,
            //         ..Default::default()
            //     });
            // }
            if h265 {
                codecs.push(CodecInfo {
                    name: "hevc_videotoolbox".to_owned(),
                    format: H265,
                    priority: Priority::Best as _,
                    ..Default::default()
                });
            }
        }

        // qsv doesn't support yuv420p
        codecs.retain(|c| {
            let ctx = ctx.clone();
            if ctx.pixfmt == AVPixelFormat::AV_PIX_FMT_YUV420P && c.name.contains("qsv") {
                return false;
            }
            return true;
        });

        let mut res = vec![];

        if let Ok(yuv) = Encoder::dummy_yuv(ctx.clone()) {
            for codec in codecs {
                // Skip if this format already exists in results
                if res
                    .iter()
                    .any(|existing: &CodecInfo| existing.format == codec.format)
                {
                    continue;
                }

                debug!("Testing encoder: {}", codec.name);

                let c = EncodeContext {
                    name: codec.name.clone(),
                    mc_name: codec.mc_name.clone(),
                    ..ctx
                };

                match Encoder::new(c) {
                    Ok(mut encoder) => {
                        debug!("Encoder {} created successfully", codec.name);
                        let mut passed = false;
                        let mut last_err: Option<i32> = None;

                        let max_attempts = if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
                            3
                        } else {
                            1
                        };
                        for attempt in 0..max_attempts {
                            let pts = (attempt as i64) * 33; // 33ms is an approximation for 30 FPS (1000 / 30)
                            let start = std::time::Instant::now();
                            match encoder.encode(&yuv, pts) {
                                Ok(frames) => {
                                    let elapsed = start.elapsed().as_millis();

                                    if frames.len() == 1 {
                                        if frames[0].key == 1 && elapsed < TEST_TIMEOUT_MS as _ {
                                            debug!(
                                                "Encoder {} test passed on attempt {}",
                                                codec.name, attempt + 1
                                            );
                                            res.push(codec.clone());
                                            passed = true;
                                            break;
                                        } else {
                                            debug!(
                                                "Encoder {} test failed on attempt {} - key: {}, timeout: {}ms",
                                                codec.name,
                                                attempt + 1,
                                                frames[0].key,
                                                elapsed
                                            );
                                        }
                                    } else {
                                        debug!(
                                            "Encoder {} test failed on attempt {} - wrong frame count: {}",
                                            codec.name,
                                            attempt + 1,
                                            frames.len()
                                        );
                                    }
                                }
                                Err(err) => {
                                    last_err = Some(err);
                                    debug!(
                                        "Encoder {} test attempt {} returned error: {}",
                                        codec.name,
                                        attempt + 1,
                                        err
                                    );
                                }
                            }
                        }

                        if !passed {
                            debug!(
                                "Encoder {} test failed after retries{}",
                                codec.name,
                                last_err
                                    .map(|e| format!(" (last err: {})", e))
                                    .unwrap_or_default()
                            );
                        }
                    }
                    Err(_) => {
                        debug!("Failed to create encoder {}", codec.name);
                    }
                }
            }
        } else {
            debug!("Failed to generate dummy YUV data");
        }

        res
    }

    fn dummy_yuv(ctx: EncodeContext) -> Result<Vec<u8>, ()> {
        let mut yuv = vec![];
        if let Ok((_, _, len)) = ffmpeg_linesize_offset_length(
            ctx.pixfmt,
            ctx.width as _,
            ctx.height as _,
            ctx.align as _,
        ) {
            yuv.resize(len as _, 0);
            return Ok(yuv);
        }

        Err(())
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        unsafe {
            ffmpeg_ram_free_encoder(self.codec);
            self.codec = std::ptr::null_mut();
            let _ = Box::from_raw(self.frames);
            trace!("Encoder dropped");
        }
    }
}
