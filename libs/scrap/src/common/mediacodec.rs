use hbb_common::{anyhow::Error, bail, log, ResultType};
use ndk::media::media_codec::{MediaCodec, MediaCodecDirection, MediaFormat};
use std::ops::Deref;
use std::{
    io::Write,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use crate::ImageFormat;
use crate::{
    codec::{EncoderApi, EncoderCfg},
    I420ToABGR, I420ToARGB, ImageRgb,
};

#[cfg(target_os = "android")]
use crate::android::RelaxedAtomic;
#[cfg(target_os = "android")]
use crate::android::mediacodec::{
    VideoDecoderEnqueuer, VideoDecoderDequeuer, FrameImage,
};

#[cfg(target_os = "android")]
use once_cell::sync::Lazy;
#[cfg(target_os = "android")]
use parking_lot::Mutex;
#[cfg(target_os = "android")]
use crate::CodecFormat;

/// MediaCodec mime type name
/// 在部分机型的下 vp9 硬解效果更好， 硬件解码受制于设计模式，即 GPU->CPU-GPU 的渲染方式
pub const H264_MIME_TYPE: &str = "video/avc";
pub const H265_MIME_TYPE: &str = "video/hevc";
pub const VP8_MIME_TYPE: &str = "video/x-vnd.on2.vp8";
pub const VP9_MIME_TYPE: &str = "video/x-vnd.on2.vp9";

// TODO MediaCodecEncoder
pub static H264_DECODER_SUPPORT: AtomicBool = AtomicBool::new(false);
pub static H265_DECODER_SUPPORT: AtomicBool = AtomicBool::new(false);
pub static VP8_DECODER_SUPPORT: AtomicBool = AtomicBool::new(false);
pub static VP9_DECODER_SUPPORT: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "android")]
pub static DECODER_ENQUEUER: Lazy<Mutex<Option<crate::android::mediacodec::VideoDecoderEnqueuer>>> =
    Lazy::new(|| Mutex::new(None));
#[cfg(target_os = "android")]
pub static DECODER_DEQUEUER: Lazy<Mutex<Option<crate::android::mediacodec::VideoDecoderDequeuer>>> =
    Lazy::new(|| Mutex::new(None));
#[cfg(target_os = "android")]
pub static DECODER: Lazy<Mutex<Option<crate::CodecFormat>>> = Lazy::new(|| Mutex::new(None));


pub struct MediaCodecDecoder {
    decoder: MediaCodec,
    name: String,
}

impl Deref for MediaCodecDecoder {
    type Target = MediaCodec;

    fn deref(&self) -> &Self::Target {
        &self.decoder
    }
}

#[derive(Default)]
pub struct MediaCodecDecoders {
    pub h264: Option<MediaCodecDecoder>,
    pub h265: Option<MediaCodecDecoder>,
}

impl MediaCodecDecoder {
    pub fn new_decoders() -> MediaCodecDecoders {
        let h264 = create_media_codec(H264_MIME_TYPE, MediaCodecDirection::Decoder);
        let h265 = create_media_codec(H265_MIME_TYPE, MediaCodecDirection::Decoder);
        MediaCodecDecoders { h264, h265 }
    }

    // rgb [in/out] fmt and stride must be set in ImageRgb
    pub fn decode(&mut self, data: &[u8], rgb: &mut ImageRgb) -> ResultType<bool> {
        // take dst_stride into account please
        let dst_stride = rgb.stride();
        match self.dequeue_input_buffer(Duration::from_millis(10))? {
            Some(mut input_buffer) => {
                let mut buf = input_buffer.buffer_mut();
                if data.len() > buf.len() {
                    log::error!("Failed to decode, the input data size is bigger than input buf");
                    bail!("The input data size is bigger than input buf");
                }
                buf.write_all(&data)?;
                self.queue_input_buffer(input_buffer, 0, data.len(), 0, 0)?;
            }
            None => {
                log::debug!("Failed to dequeue_input_buffer: No available input_buffer");
            }
        };

        return match self.dequeue_output_buffer(Duration::from_millis(100))? {
            Some(output_buffer) => {
                let res_format = self.output_format();
                let w = res_format
                    .i32("width")
                    .ok_or(Error::msg("Failed to dequeue_output_buffer, width is None"))?
                    as usize;
                let h = res_format.i32("height").ok_or(Error::msg(
                    "Failed to dequeue_output_buffer, height is None",
                ))? as usize;
                let stride = res_format.i32("stride").ok_or(Error::msg(
                    "Failed to dequeue_output_buffer, stride is None",
                ))?;
                let buf = output_buffer.buffer();
                let bps = 4;
                let u = buf.len() * 2 / 3;
                let v = buf.len() * 5 / 6;
                rgb.raw.resize(h * w * bps, 0);
                let y_ptr = buf.as_ptr();
                let u_ptr = buf[u..].as_ptr();
                let v_ptr = buf[v..].as_ptr();
                unsafe {
                    match rgb.fmt() {
                        ImageFormat::ARGB => {
                            I420ToARGB(
                                y_ptr,
                                stride,
                                u_ptr,
                                stride / 2,
                                v_ptr,
                                stride / 2,
                                rgb.raw.as_mut_ptr(),
                                (w * bps) as _,
                                w as _,
                                h as _,
                            );
                        }
                        ImageFormat::ARGB => {
                            I420ToABGR(
                                y_ptr,
                                stride,
                                u_ptr,
                                stride / 2,
                                v_ptr,
                                stride / 2,
                                rgb.raw.as_mut_ptr(),
                                (w * bps) as _,
                                w as _,
                                h as _,
                            );
                        }
                        _ => {
                            bail!("Unsupported image format");
                        }
                    }
                }
                self.release_output_buffer(output_buffer, false)?;
                Ok(true)
            }
            None => {
                log::debug!("Failed to dequeue_output: No available dequeue_output");
                Ok(false)
            }
        };
    }
}

fn create_media_codec(name: &str, direction: MediaCodecDirection) -> Option<MediaCodecDecoder> {
    let codec = MediaCodec::from_decoder_type(name)?;
    let media_format = crate::android::mediacodec::configure_media_format(name);

    if let Err(e) = codec.configure(&media_format, None, direction) {
        log::error!("Failed to init decoder:{:?}", e);
        return None;
    };
    log::error!("decoder init success");
    if let Err(e) = codec.start() {
        log::error!("Failed to start decoder:{:?}", e);
        return None;
    };
    log::debug!("Init decoder successed!: {:?}", name);
    return Some(MediaCodecDecoder {
        decoder: codec,
        name: name.to_owned(),
    });
}

pub fn check_mediacodec() {
    std::thread::spawn(move || {
        // check decoders
        if let Some(h264) = create_media_codec(H264_MIME_TYPE, MediaCodecDirection::Decoder) {
            H264_DECODER_SUPPORT.swap(true, Ordering::SeqCst);
            let _ = h264.stop();
        }
        if let Some(h265) = create_media_codec(H265_MIME_TYPE, MediaCodecDirection::Decoder) {
            H265_DECODER_SUPPORT.swap(true, Ordering::SeqCst);
            let _ = h265.stop();
        }
        if let Some(vp8) = create_media_codec(VP8_MIME_TYPE, MediaCodecDirection::Decoder) {
            VP8_DECODER_SUPPORT.swap(true, Ordering::SeqCst);
            let _ = vp8.stop();
        }
        if let Some(vp9) = create_media_codec(VP9_MIME_TYPE, MediaCodecDirection::Decoder) {
            VP9_DECODER_SUPPORT.swap(true, Ordering::SeqCst);
            let _ = vp9.stop();
        }
        // TODO encoders
    });
}


pub struct XMediaCodecDecoder {
}

impl Default for XMediaCodecDecoder {
    fn default() -> Self {
        XMediaCodecDecoder::decode_init(&CodecFormat::H265).unwrap();
        Self {  }
    }
}

impl XMediaCodecDecoder {
    fn set_use(codec_format: &CodecFormat) {
        *DECODER.lock() = Some(codec_format.clone());
    }

    fn is_use(&self, codec_format: &CodecFormat) -> bool {
        match &*DECODER.lock() {
            Some(current) => current == codec_format,
            None => false,
        }
    }

    fn decode_init(codec_format: &CodecFormat) -> ResultType<bool> {
        let (enqueuer, dequeuer) = crate::android::mediacodec::video_decoder_split(
            codec_format,
            MediaCodecDirection::Decoder,
        )
        .unwrap();
        *DECODER_ENQUEUER.lock() = Some(enqueuer);
        *DECODER_DEQUEUER.lock() = Some(dequeuer);
        XMediaCodecDecoder::set_use(codec_format);
        Ok(true)
    }

    pub fn decode(&mut self, codec_format: &CodecFormat, data: &[u8], rgb: &mut ImageRgb, key: &bool , pts: &i64) -> ResultType<bool> {
        let dst_stride = rgb.stride();
        let pts_u64 = if *pts >= 0 { *pts as u64 } else { 0 };
        let flag = if *key { 1 } else { 0 };

        if !self.is_use(codec_format) {
            XMediaCodecDecoder::decode_init(codec_format).unwrap();
        }

        push_nal(Duration::from_millis(pts_u64), flag, data);

        if let Some(mut frame_image) = get_frame() {
            frame_image.i420_to_argb(&mut rgb.raw);
            return Ok(true);
        }
        Ok(false)
    }

}


fn push_nal(timestamp: Duration, flag: u32, nal: &[u8]) {
    if let Some(decoder) = &*DECODER_ENQUEUER.lock() {
        if !matches!(decoder.push_frame_nal(timestamp, flag, nal).map_err(|e| log::error!("{e}")).ok(), Some(true)) {
            log::debug!("push_frame_nal fail");
        }
    }
}

fn get_frame() -> Option<FrameImage> {
    if let Some(decoder) = &mut *DECODER_DEQUEUER.lock() {
        decoder.dequeue_frame()
    } else {
        None
    }
}
