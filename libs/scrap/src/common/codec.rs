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
    log,
    message_proto::{video_frame, EncodedVideoFrames, Message, VideoCodecState},
    ResultType,
};
#[cfg(feature = "hwcodec")]
use hbb_common::{
    config::{Config2, PeerConfig},
    lazy_static,
    message_proto::video_codec_state::PerferCodec,
};

#[cfg(feature = "hwcodec")]
lazy_static::lazy_static! {
    static ref PEER_DECODER_STATES: Arc<Mutex<HashMap<i32, VideoCodecState>>> = Default::default();
}
const SCORE_VPX: i32 = 90;

#[derive(Debug, Clone)]
pub struct HwEncoderConfig {
    pub codec_name: String,
    pub width: usize,
    pub height: usize,
    pub bitrate: i32,
}

#[derive(Debug, Clone)]
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

    fn set_bitrate(&mut self, bitrate: u32) -> ResultType<()>;
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
    hw: HwDecoders,
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
        log::info!("new encoder:{:?}", config);
        match config {
            EncoderCfg::VPX(_) => Ok(Encoder {
                codec: Box::new(VpxEncoder::new(config)?),
            }),

            #[cfg(feature = "hwcodec")]
            EncoderCfg::HW(_) => match HwEncoder::new(config) {
                Ok(hw) => Ok(Encoder {
                    codec: Box::new(hw),
                }),
                Err(e) => {
                    check_config_process(true);
                    Err(e)
                }
            },
            #[cfg(not(feature = "hwcodec"))]
            _ => Err(anyhow!("unsupported encoder type")),
        }
    }

    // TODO
    pub fn update_video_encoder(id: i32, update: EncoderUpdate) {
        #[cfg(feature = "hwcodec")]
        {
            let mut states = PEER_DECODER_STATES.lock().unwrap();
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
            let name = HwEncoder::current_name();
            if states.len() > 0 {
                let best = HwEncoder::best();
                let enabled_h264 = best.h264.is_some()
                    && states.len() > 0
                    && states.iter().all(|(_, s)| s.score_h264 > 0);
                let enabled_h265 = best.h265.is_some()
                    && states.len() > 0
                    && states.iter().all(|(_, s)| s.score_h265 > 0);

                // Preference first
                let mut preference = PerferCodec::Auto;
                let preferences: Vec<_> = states
                    .iter()
                    .filter(|(_, s)| {
                        s.perfer == PerferCodec::VPX.into()
                            || s.perfer == PerferCodec::H264.into() && enabled_h264
                            || s.perfer == PerferCodec::H265.into() && enabled_h265
                    })
                    .map(|(_, s)| s.perfer)
                    .collect();
                if preferences.len() > 0 && preferences.iter().all(|&p| p == preferences[0]) {
                    preference = preferences[0].enum_value_or(PerferCodec::Auto);
                }

                match preference {
                    PerferCodec::VPX => *name.lock().unwrap() = None,
                    PerferCodec::H264 => {
                        *name.lock().unwrap() = best.h264.map_or(None, |c| Some(c.name))
                    }
                    PerferCodec::H265 => {
                        *name.lock().unwrap() = best.h265.map_or(None, |c| Some(c.name))
                    }
                    PerferCodec::Auto => {
                        // score encoder
                        let mut score_vpx = SCORE_VPX;
                        let mut score_h264 = best.h264.as_ref().map_or(0, |c| c.score);
                        let mut score_h265 = best.h265.as_ref().map_or(0, |c| c.score);

                        // score decoder
                        score_vpx += states.iter().map(|s| s.1.score_vpx).sum::<i32>();
                        if enabled_h264 {
                            score_h264 += states.iter().map(|s| s.1.score_h264).sum::<i32>();
                        }
                        if enabled_h265 {
                            score_h265 += states.iter().map(|s| s.1.score_h265).sum::<i32>();
                        }

                        if enabled_h265 && score_h265 >= score_vpx && score_h265 >= score_h264 {
                            *name.lock().unwrap() = best.h265.map_or(None, |c| Some(c.name));
                        } else if enabled_h264
                            && score_h264 >= score_vpx
                            && score_h264 >= score_h265
                        {
                            *name.lock().unwrap() = best.h264.map_or(None, |c| Some(c.name));
                        } else {
                            *name.lock().unwrap() = None;
                        }
                    }
                }

                log::info!(
                    "connection count:{}, used preference:{:?}, encoder:{:?}",
                    states.len(),
                    preference,
                    name.lock().unwrap()
                )
            } else {
                *name.lock().unwrap() = None;
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
        if check_hwcodec_config() {
            return HwEncoder::current_name().lock().unwrap().clone();
        } else {
            return None;
        }
        #[cfg(not(feature = "hwcodec"))]
        return None;
    }

    pub fn supported_encoding() -> (bool, bool) {
        #[cfg(feature = "hwcodec")]
        if check_hwcodec_config() {
            let best = HwEncoder::best();
            (
                best.h264.as_ref().map_or(false, |c| c.score > 0),
                best.h265.as_ref().map_or(false, |c| c.score > 0),
            )
        } else {
            (false, false)
        }
        #[cfg(not(feature = "hwcodec"))]
        (false, false)
    }
}

impl Decoder {
    pub fn video_codec_state(_id: &str) -> VideoCodecState {
        #[cfg(feature = "hwcodec")]
        if check_hwcodec_config() {
            let best = HwDecoder::best();
            VideoCodecState {
                score_vpx: SCORE_VPX,
                score_h264: best.h264.map_or(0, |c| c.score),
                score_h265: best.h265.map_or(0, |c| c.score),
                perfer: Self::codec_preference(_id).into(),
                ..Default::default()
            }
        } else {
            return VideoCodecState {
                score_vpx: SCORE_VPX,
                ..Default::default()
            };
        }
        #[cfg(not(feature = "hwcodec"))]
        VideoCodecState {
            score_vpx: SCORE_VPX,
            ..Default::default()
        }
    }

    pub fn new(config: DecoderCfg) -> Decoder {
        let vpx = VpxDecoder::new(config.vpx).unwrap();
        Decoder {
            vpx,
            #[cfg(feature = "hwcodec")]
            hw: HwDecoder::new_decoders(),
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
            video_frame::Union::Vp9s(vp9s) => {
                Decoder::handle_vp9s_video_frame(&mut self.vpx, vp9s, rgb)
            }
            #[cfg(feature = "hwcodec")]
            video_frame::Union::H264s(h264s) => {
                if let Some(decoder) = &mut self.hw.h264 {
                    Decoder::handle_hw_video_frame(decoder, h264s, rgb, &mut self.i420)
                } else {
                    Err(anyhow!("don't support h264!"))
                }
            }
            #[cfg(feature = "hwcodec")]
            video_frame::Union::H265s(h265s) => {
                if let Some(decoder) = &mut self.hw.h265 {
                    Decoder::handle_hw_video_frame(decoder, h265s, rgb, &mut self.i420)
                } else {
                    Err(anyhow!("don't support h265!"))
                }
            }
            _ => Err(anyhow!("unsupported video frame type!")),
        }
    }

    fn handle_vp9s_video_frame(
        decoder: &mut VpxDecoder,
        vp9s: &EncodedVideoFrames,
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
    fn handle_hw_video_frame(
        decoder: &mut HwDecoder,
        frames: &EncodedVideoFrames,
        rgb: &mut Vec<u8>,
        i420: &mut Vec<u8>,
    ) -> ResultType<bool> {
        let mut ret = false;
        for h264 in frames.frames.iter() {
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
    fn codec_preference(id: &str) -> PerferCodec {
        let codec = PeerConfig::load(id)
            .options
            .get("codec-preference")
            .map_or("".to_owned(), |c| c.to_owned());
        if codec == "vp9" {
            PerferCodec::VPX
        } else if codec == "h264" {
            PerferCodec::H264
        } else if codec == "h265" {
            PerferCodec::H265
        } else {
            PerferCodec::Auto
        }
    }
}

#[cfg(feature = "hwcodec")]
fn check_hwcodec_config() -> bool {
    if let Some(v) = Config2::get().options.get("enable-hwcodec") {
        return v != "N";
    }
    return true; // default is true
}
