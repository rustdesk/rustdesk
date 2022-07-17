use std::{io::Write, time::Duration};

use hbb_common::{bail, ResultType};
#[cfg(target_os = "android")]
use ndk::media::media_codec::{MediaCodec, MediaCodecDirection, MediaFormat};

use crate::{
    codec::{EncoderApi, EncoderCfg},
    I420ToARGB,
};

pub struct MediaCodecEncoder {
    encoder: MediaCodec,
}

impl EncoderApi for MediaCodecEncoder {
    fn new(cfg: EncoderCfg) -> ResultType<Self>
    where
        Self: Sized,
    {
        if let EncoderCfg::HW(cfg) = cfg {
            create_media_codec(&cfg.codec_name, MediaCodecDirection::Encoder)
        } else {
            bail!("encoder type mismatch")
        }
    }

    fn encode_to_message(
        &mut self,
        frame: &[u8],
        ms: i64,
    ) -> ResultType<hbb_common::message_proto::Message> {
        todo!()
    }

    fn use_yuv(&self) -> bool {
        todo!()
    }

    fn set_bitrate(&mut self, bitrate: u32) -> ResultType<()> {
        todo!()
    }
}

pub struct MediaCodecDecoder {
    decoder: MediaCodec,
    // pub info: CodecInfo,
}

pub struct MediaCodecDecoders {
    pub h264: Option<MediaCodecDecoder>,
    pub h265: Option<MediaCodecDecoder>,
}

// "video/x-vnd.on2.vp8" - VP8 video (i.e. video in .webm)
// "video/x-vnd.on2.vp9" - VP9 video (i.e. video in .webm)
// "video/avc" - H.264/AVC video
// "video/hevc" - H.265/HEVC video

impl MediaCodecDecoder {
    pub fn new_decoders() -> MediaCodecDecoders {
        // 直接生成 h264 和 h265
        // 264
        let h264 = create_media_codec("video/avc", MediaCodecDirection::Decoder)
            .map(|decoder| MediaCodecDecoder { decoder });
        let h265 = create_media_codec("video/hevc", MediaCodecDirection::Decoder)
            .map(|decoder| MediaCodecDecoder { decoder });

        MediaCodecDecoders { h264, h265 }
    }

    pub fn decode(&mut self, data: &[u8], rgb: &mut Vec<u8>) -> ResultType<bool> {
        log::debug!("start dequeue_input");

        match self
            .decoder
            .dequeue_input_buffer(Duration::from_millis(10))
            .unwrap()
        {
            Some(mut input_buffer) => {
                let mut buf = input_buffer.buffer_mut();
                log::debug!(
                    "dequeue_input success:buf ptr:{:?},len:{}",
                    buf.as_ptr(),
                    buf.len()
                );
                if data.len() > buf.len() {
                    log::error!("break! res.len()>buf.len()");
                    bail!("break! res.len()>buf.len()");
                }
                buf.write_all(&data).unwrap();
                if let Err(e) = self
                    .decoder
                    .queue_input_buffer(input_buffer, 0, data.len(), 0, 0)
                {
                    log::debug!("debug queue_input_buffer:{:?}", e);
                };
            }
            None => {
                log::debug!("dequeue_input_buffer fail :None");
            }
        };

        return match self
            .decoder
            .dequeue_output_buffer(Duration::from_millis(100))
        {
            Ok(Some(output_buffer)) => {
                log::debug!("dequeue_output success");
                // let res_format = output_buffer.format();
                let res_format = self.decoder.output_format();
                log::debug!("res_format:{:?}", res_format.str("mime"));
                log::debug!("res_color:{:?}", res_format.i32("color-format"));
                log::debug!("stride:{:?}", res_format.i32("stride"));
                let w = res_format.i32("width").unwrap() as usize;
                let h = res_format.i32("height").unwrap() as usize;
                let stride = res_format.i32("stride").unwrap(); // todo

                // let w = 1920;
                // let h = 1080;
                // let stride = 1920; // todo

                let buf = output_buffer.buffer();
                log::debug!("output_buffer ptr:{:?} len:{}", buf.as_ptr(), buf.len());
                let bps = 4;
                let u = buf.len() * 2 / 3;
                let v = buf.len() * 5 / 6;
                rgb.resize(h * w * bps, 0);
                log::debug!("start I420ToARGB,u:{},v:{},w:{},h:{}", u, v, w, h);
                let y_ptr = buf.as_ptr();
                let u_ptr = buf[u..].as_ptr();
                let v_ptr = buf[v..].as_ptr();
                log::debug!("ptr,y:{:?},u:{:?},v:{:?}", y_ptr, u_ptr, v_ptr);
                unsafe {
                    I420ToARGB(
                        y_ptr,
                        stride,
                        u_ptr,
                        stride / 2,
                        v_ptr,
                        stride / 2,
                        rgb.as_mut_ptr(),
                        (w * bps) as _,
                        w as _,
                        h as _,
                    );
                }
                log::debug!("end I420ToARGB");
                log::debug!("release_output_buffer");
                self.decoder
                    .release_output_buffer(output_buffer, false)
                    .unwrap();
                log::debug!("return true");
                Ok(true)
            }
            Ok(None) => {
                log::debug!("dequeue_output fail :None");
                Ok(false)
            }
            Err(e) => {
                log::debug!("dequeue_output fail :error:{:?}", e);
                Ok(false)
            }
        };
    }
}

fn create_media_codec(name: &str, direction: MediaCodecDirection) -> Option<MediaCodec> {
    let codec = MediaCodec::from_decoder_type(name).unwrap();
    log::debug!("start init");
    let media_format = MediaFormat::new();
    media_format.set_str("mime", name);
    media_format.set_i32("width", 0);
    media_format.set_i32("height", 0);
    media_format.set_i32("color-format", 19); // COLOR_FormatYUV420Planar
    if let Err(e) = codec.configure(&media_format, None, direction) {
        log::error!("failed to decoder.init:{:?}", e);
        return None;
    };
    log::error!("decoder init success");
    if let Err(e) = codec.start() {
        log::error!("failed to decoder.start:{:?}", e);
        return None;
    };
    log::debug!("init decoder successed!:{:?}", name);
    return Some(codec);
}
