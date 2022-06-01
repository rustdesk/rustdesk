use std::ops::{Deref, DerefMut};
#[cfg(feature = "hwcodec")]
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[cfg(feature = "hwcodec")]
use crate::hwcodec::*;
use crate::vpxcodec::*;

use hbb_common::{
    anyhow::anyhow,
    message_proto::{video_frame, ImageQuality, Message, VP9s, VideoCodecState},
    ResultType,
};
#[cfg(feature = "hwcodec")]
use hbb_common::{
    lazy_static, log,
    message_proto::{H264s, H265s},
};

#[cfg(feature = "hwcodec")]
lazy_static::lazy_static! {
    static ref VIDEO_CODEC_STATES: Arc<Mutex<HashMap<i32, VideoCodecState>>> = Default::default();
}

#[derive(Debug, Clone)]
pub struct HwEncoderConfig {
    pub codec_name: String,
    pub width: usize,
    pub height: usize,
    pub quallity: ImageQuality,
}

pub enum EncoderCfg {
    VPX(VpxEncoderConfig),
    HW(HwEncoderConfig),
}

pub trait EncoderApi {
    fn new(cfg: EncoderCfg) -> ResultType<Self>
    where
        Self: Sized;

    fn encode_to_message(&mut self, frame: &[u8], ms: i64) -> ResultType<Message>;

    fn use_yuv(&self) -> bool;
}

pub struct DecoderCfg {
    pub vpx: VpxDecoderConfig,
}

pub struct Encoder {
    pub codec: Box<dyn EncoderApi>,
}

impl Deref for Encoder {
    type Target = Box<dyn EncoderApi>;

    fn deref(&self) -> &Self::Target {
        &self.codec
    }
}

impl DerefMut for Encoder {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.codec
    }
}

pub struct Decoder {
    vpx: VpxDecoder,
    #[cfg(feature = "hwcodec")]
    hw: Arc<Mutex<HwDecoderInstance>>,
    #[cfg(feature = "hwcodec")]
    i420: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum EncoderUpdate {
    State(VideoCodecState),
    Remove,
    DisableHwIfNotExist,
}

impl Encoder {
    pub fn new(config: EncoderCfg) -> ResultType<Encoder> {
        match config {
            EncoderCfg::VPX(_) => Ok(Encoder {
                codec: Box::new(VpxEncoder::new(config)?),
            }),

            #[cfg(feature = "hwcodec")]
            EncoderCfg::HW(_) => Ok(Encoder {
                codec: Box::new(HwEncoder::new(config)?),
            }),
            #[cfg(not(feature = "hwcodec"))]
            _ => Err(anyhow!("unsupported encoder type")),
        }
    }

    // TODO
    pub fn update_video_encoder(id: i32, update: EncoderUpdate) {
        #[cfg(feature = "hwcodec")]
        {
            let mut states = VIDEO_CODEC_STATES.lock().unwrap();
            match update {
                EncoderUpdate::State(state) => {
                    states.insert(id, state);
                }
                EncoderUpdate::Remove => {
                    states.remove(&id);
                }
                EncoderUpdate::DisableHwIfNotExist => {
                    if !states.contains_key(&id) {
                        states.insert(id, VideoCodecState::default());
                    }
                }
            }
            let (encoder_h264, encoder_h265) = HwEncoder::best();
            let mut enabled_h264 = encoder_h264.is_some();
            let mut enabled_h265 = encoder_h265.is_some();
            let mut score_vpx = 90;
            let mut score_h264 = encoder_h264.as_ref().map_or(0, |c| c.score);
            let mut score_h265 = encoder_h265.as_ref().map_or(0, |c| c.score);

            for state in states.iter() {
                enabled_h264 = enabled_h264 && state.1.H264;
                enabled_h265 = enabled_h265 && state.1.H265;
                score_vpx += state.1.ScoreVpx;
                score_h264 += state.1.ScoreH264;
                score_h265 += state.1.ScoreH265;
            }

            let current_encoder_name = HwEncoder::current_name();
            if enabled_h265 && score_h265 >= score_vpx && score_h265 >= score_h264 {
                *current_encoder_name.lock().unwrap() = Some(encoder_h265.unwrap().name);
            } else if enabled_h264 && score_h264 >= score_vpx && score_h264 >= score_h265 {
                *current_encoder_name.lock().unwrap() = Some(encoder_h264.unwrap().name);
            } else {
                *current_encoder_name.lock().unwrap() = None;
            }
            if states.len() > 0 {
                log::info!(
                    "connection count:{}, h264:{}, h265:{}, score: vpx({}), h264({}), h265({}), set current encoder name {:?}",
                    states.len(),
                    enabled_h264,
                    enabled_h265,
                    score_vpx,
                    score_h264,
                    score_h265,
                    current_encoder_name.lock().unwrap()
                    )
            }
        }
        #[cfg(not(feature = "hwcodec"))]
        {
            let _ = id;
            let _ = update;
        }
    }
    #[inline]
    pub fn current_hw_encoder_name() -> Option<String> {
        #[cfg(feature = "hwcodec")]
        return HwEncoder::current_name().lock().unwrap().clone();
        #[cfg(not(feature = "hwcodec"))]
        return None;
    }
}

impl Decoder {
    // TODO
    pub fn video_codec_state() -> VideoCodecState {
        let mut state = VideoCodecState::default();
        state.ScoreVpx = 90;

        #[cfg(feature = "hwcodec")]
        {
            let hw = HwDecoder::instance();
            state.H264 = hw.lock().unwrap().h264.is_some();
            state.ScoreH264 = hw.lock().unwrap().h264.as_ref().map_or(0, |d| d.info.score);
            state.H265 = hw.lock().unwrap().h265.is_some();
            state.ScoreH265 = hw.lock().unwrap().h265.as_ref().map_or(0, |d| d.info.score);
        }

        state
    }

    pub fn new(config: DecoderCfg) -> Decoder {
        let vpx = VpxDecoder::new(config.vpx).unwrap();
        Decoder {
            vpx,
            #[cfg(feature = "hwcodec")]
            hw: HwDecoder::instance(),
            #[cfg(feature = "hwcodec")]
            i420: vec![],
        }
    }

    pub fn handle_video_frame(
        &mut self,
        frame: &video_frame::Union,
        rgb: &mut Vec<u8>,
    ) -> ResultType<bool> {
        match frame {
            video_frame::Union::vp9s(vp9s) => {
                Decoder::handle_vp9s_video_frame(&mut self.vpx, vp9s, rgb)
            }
            #[cfg(feature = "hwcodec")]
            video_frame::Union::h264s(h264s) => {
                if let Some(decoder) = &mut self.hw.lock().unwrap().h264 {
                    Decoder::handle_h264s_video_frame(decoder, h264s, rgb, &mut self.i420)
                } else {
                    Err(anyhow!("don't support h264!"))
                }
            }
            #[cfg(feature = "hwcodec")]
            video_frame::Union::h265s(h265s) => {
                if let Some(decoder) = &mut self.hw.lock().unwrap().h265 {
                    Decoder::handle_h265s_video_frame(decoder, h265s, rgb, &mut self.i420)
                } else {
                    Err(anyhow!("don't support h265!"))
                }
            }
            _ => Err(anyhow!("unsupported video frame type!")),
        }
    }

    fn handle_vp9s_video_frame(
        decoder: &mut VpxDecoder,
        vp9s: &VP9s,
        rgb: &mut Vec<u8>,
    ) -> ResultType<bool> {
        let mut last_frame = Image::new();
        for vp9 in vp9s.frames.iter() {
            for frame in decoder.decode(&vp9.data)? {
                drop(last_frame);
                last_frame = frame;
            }
        }
        for frame in decoder.flush()? {
            drop(last_frame);
            last_frame = frame;
        }
        if last_frame.is_null() {
            Ok(false)
        } else {
            last_frame.rgb(1, true, rgb);
            Ok(true)
        }
    }

    #[cfg(feature = "hwcodec")]
    fn handle_h264s_video_frame(
        decoder: &mut HwDecoder,
        h264s: &H264s,
        rgb: &mut Vec<u8>,
        i420: &mut Vec<u8>,
    ) -> ResultType<bool> {
        let mut ret = false;
        for h264 in h264s.h264s.iter() {
            for image in decoder.decode(&h264.data)? {
                // TODO: just process the last frame
                if image.bgra(rgb, i420).is_ok() {
                    ret = true;
                }
            }
        }
        return Ok(ret);
    }

    #[cfg(feature = "hwcodec")]
    fn handle_h265s_video_frame(
        decoder: &mut HwDecoder,
        h265s: &H265s,
        rgb: &mut Vec<u8>,
        i420: &mut Vec<u8>,
    ) -> ResultType<bool> {
        let mut ret = false;
        for h265 in h265s.h265s.iter() {
            for image in decoder.decode(&h265.data)? {
                // TODO: just process the last frame
                if image.bgra(rgb, i420).is_ok() {
                    ret = true;
                }
            }
        }
        return Ok(ret);
    }
}
