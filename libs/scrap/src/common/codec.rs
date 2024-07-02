use std::{
    collections::HashMap,
    ffi::c_void,
    ops::{Deref, DerefMut},
    sync::{Arc, Mutex},
};

#[cfg(feature = "hwcodec")]
use crate::hwcodec::*;
#[cfg(feature = "mediacodec")]
use crate::mediacodec::{MediaCodecDecoder, H264_DECODER_SUPPORT, H265_DECODER_SUPPORT};
#[cfg(feature = "vram")]
use crate::vram::*;
use crate::{
    aom::{self, AomDecoder, AomEncoder, AomEncoderConfig},
    common::GoogleImage,
    vpxcodec::{self, VpxDecoder, VpxDecoderConfig, VpxEncoder, VpxEncoderConfig, VpxVideoCodecId},
    CodecFormat, EncodeInput, EncodeYuvFormat, ImageRgb,
};

use hbb_common::{
    anyhow::anyhow,
    bail,
    config::{option2bool, Config, PeerConfig},
    lazy_static, log,
    message_proto::{
        supported_decoding::PreferCodec, video_frame, Chroma, CodecAbility, EncodedVideoFrames,
        SupportedDecoding, SupportedEncoding, VideoFrame,
    },
    sysinfo::System,
    tokio::time::Instant,
    ResultType,
};

lazy_static::lazy_static! {
    static ref PEER_DECODINGS: Arc<Mutex<HashMap<i32, SupportedDecoding>>> = Default::default();
    static ref ENCODE_CODEC_FORMAT: Arc<Mutex<CodecFormat>> = Arc::new(Mutex::new(CodecFormat::VP9));
    static ref THREAD_LOG_TIME: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));
    static ref USABLE_ENCODING: Arc<Mutex<Option<SupportedEncoding>>> = Arc::new(Mutex::new(None));
}

pub const ENCODE_NEED_SWITCH: &'static str = "ENCODE_NEED_SWITCH";

#[derive(Debug, Clone)]
pub enum EncoderCfg {
    VPX(VpxEncoderConfig),
    AOM(AomEncoderConfig),
    #[cfg(feature = "hwcodec")]
    HWRAM(HwRamEncoderConfig),
    #[cfg(feature = "vram")]
    VRAM(VRamEncoderConfig),
}

pub trait EncoderApi {
    fn new(cfg: EncoderCfg, i444: bool) -> ResultType<Self>
    where
        Self: Sized;

    fn encode_to_message(&mut self, frame: EncodeInput, ms: i64) -> ResultType<VideoFrame>;

    fn yuvfmt(&self) -> EncodeYuvFormat;

    #[cfg(feature = "vram")]
    fn input_texture(&self) -> bool;

    fn set_quality(&mut self, quality: Quality) -> ResultType<()>;

    fn bitrate(&self) -> u32;

    fn support_abr(&self) -> bool;

    fn support_changing_quality(&self) -> bool;

    fn latency_free(&self) -> bool;

    fn is_hardware(&self) -> bool;

    fn disable(&self);
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
    vp8: Option<VpxDecoder>,
    vp9: Option<VpxDecoder>,
    av1: Option<AomDecoder>,
    #[cfg(feature = "hwcodec")]
    h264_ram: Option<HwRamDecoder>,
    #[cfg(feature = "hwcodec")]
    h265_ram: Option<HwRamDecoder>,
    #[cfg(feature = "vram")]
    h264_vram: Option<VRamDecoder>,
    #[cfg(feature = "vram")]
    h265_vram: Option<VRamDecoder>,
    #[cfg(feature = "mediacodec")]
    h264_media_codec: MediaCodecDecoder,
    #[cfg(feature = "mediacodec")]
    h265_media_codec: MediaCodecDecoder,
    format: CodecFormat,
    valid: bool,
    #[cfg(feature = "hwcodec")]
    i420: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum EncodingUpdate {
    Update(i32, SupportedDecoding),
    Remove(i32),
    NewOnlyVP9(i32),
    Check,
}

impl Encoder {
    pub fn new(config: EncoderCfg, i444: bool) -> ResultType<Encoder> {
        log::info!("new encoder: {config:?}, i444: {i444}");
        match config {
            EncoderCfg::VPX(_) => Ok(Encoder {
                codec: Box::new(VpxEncoder::new(config, i444)?),
            }),
            EncoderCfg::AOM(_) => Ok(Encoder {
                codec: Box::new(AomEncoder::new(config, i444)?),
            }),

            #[cfg(feature = "hwcodec")]
            EncoderCfg::HWRAM(_) => match HwRamEncoder::new(config, i444) {
                Ok(hw) => Ok(Encoder {
                    codec: Box::new(hw),
                }),
                Err(e) => {
                    log::error!("new hw encoder failed: {e:?}, clear config");
                    HwCodecConfig::clear(false, true);
                    *ENCODE_CODEC_FORMAT.lock().unwrap() = CodecFormat::VP9;
                    Err(e)
                }
            },
            #[cfg(feature = "vram")]
            EncoderCfg::VRAM(_) => match VRamEncoder::new(config, i444) {
                Ok(tex) => Ok(Encoder {
                    codec: Box::new(tex),
                }),
                Err(e) => {
                    log::error!("new vram encoder failed: {e:?}, clear config");
                    HwCodecConfig::clear(true, true);
                    *ENCODE_CODEC_FORMAT.lock().unwrap() = CodecFormat::VP9;
                    Err(e)
                }
            },
        }
    }

    pub fn update(update: EncodingUpdate) {
        log::info!("update:{:?}", update);
        let mut decodings = PEER_DECODINGS.lock().unwrap();
        match update {
            EncodingUpdate::Update(id, decoding) => {
                decodings.insert(id, decoding);
            }
            EncodingUpdate::Remove(id) => {
                decodings.remove(&id);
            }
            EncodingUpdate::NewOnlyVP9(id) => {
                decodings.insert(
                    id,
                    SupportedDecoding {
                        ability_vp9: 1,
                        ..Default::default()
                    },
                );
            }
            EncodingUpdate::Check => {}
        }

        let vp8_useable = decodings.len() > 0 && decodings.iter().all(|(_, s)| s.ability_vp8 > 0);
        let av1_useable = decodings.len() > 0
            && decodings.iter().all(|(_, s)| s.ability_av1 > 0)
            && !disable_av1();
        let _all_support_h264_decoding =
            decodings.len() > 0 && decodings.iter().all(|(_, s)| s.ability_h264 > 0);
        let _all_support_h265_decoding =
            decodings.len() > 0 && decodings.iter().all(|(_, s)| s.ability_h265 > 0);
        #[allow(unused_mut)]
        let mut h264vram_encoding = false;
        #[allow(unused_mut)]
        let mut h265vram_encoding = false;
        #[cfg(feature = "vram")]
        if enable_vram_option(true) {
            if _all_support_h264_decoding {
                if VRamEncoder::available(CodecFormat::H264).len() > 0 {
                    h264vram_encoding = true;
                }
            }
            if _all_support_h265_decoding {
                if VRamEncoder::available(CodecFormat::H265).len() > 0 {
                    h265vram_encoding = true;
                }
            }
        }
        #[allow(unused_mut)]
        let mut h264hw_encoding: Option<String> = None;
        #[allow(unused_mut)]
        let mut h265hw_encoding: Option<String> = None;
        #[cfg(feature = "hwcodec")]
        if enable_hwcodec_option() {
            if _all_support_h264_decoding {
                h264hw_encoding =
                    HwRamEncoder::try_get(CodecFormat::H264).map_or(None, |c| Some(c.name));
            }
            if _all_support_h265_decoding {
                h265hw_encoding =
                    HwRamEncoder::try_get(CodecFormat::H265).map_or(None, |c| Some(c.name));
            }
        }
        let h264_useable =
            _all_support_h264_decoding && (h264vram_encoding || h264hw_encoding.is_some());
        let h265_useable =
            _all_support_h265_decoding && (h265vram_encoding || h265hw_encoding.is_some());
        let mut format = ENCODE_CODEC_FORMAT.lock().unwrap();
        let preferences: Vec<_> = decodings
            .iter()
            .filter(|(_, s)| {
                s.prefer == PreferCodec::VP9.into()
                    || s.prefer == PreferCodec::VP8.into() && vp8_useable
                    || s.prefer == PreferCodec::AV1.into() && av1_useable
                    || s.prefer == PreferCodec::H264.into() && h264_useable
                    || s.prefer == PreferCodec::H265.into() && h265_useable
            })
            .map(|(_, s)| s.prefer)
            .collect();
        *USABLE_ENCODING.lock().unwrap() = Some(SupportedEncoding {
            vp8: vp8_useable,
            av1: av1_useable,
            h264: h264_useable,
            h265: h265_useable,
            ..Default::default()
        });
        // find the most frequent preference
        let mut counts = Vec::new();
        for pref in &preferences {
            match counts.iter_mut().find(|(p, _)| p == pref) {
                Some((_, count)) => *count += 1,
                None => counts.push((pref.clone(), 1)),
            }
        }
        let max_count = counts.iter().map(|(_, count)| *count).max().unwrap_or(0);
        let (most_frequent, _) = counts
            .into_iter()
            .find(|(_, count)| *count == max_count)
            .unwrap_or((PreferCodec::Auto.into(), 0));
        let preference = most_frequent.enum_value_or(PreferCodec::Auto);

        // auto: h265 > h264 > vp9/vp8
        let mut auto_codec = CodecFormat::VP9;
        if h264_useable {
            auto_codec = CodecFormat::H264;
        }
        if h265_useable {
            auto_codec = CodecFormat::H265;
        }
        if auto_codec == CodecFormat::VP9 {
            let mut system = System::new();
            system.refresh_memory();
            if vp8_useable && system.total_memory() <= 4 * 1024 * 1024 * 1024 {
                // 4 Gb
                auto_codec = CodecFormat::VP8
            }
        }

        *format = match preference {
            PreferCodec::VP8 => CodecFormat::VP8,
            PreferCodec::VP9 => CodecFormat::VP9,
            PreferCodec::AV1 => CodecFormat::AV1,
            PreferCodec::H264 => {
                if h264vram_encoding || h264hw_encoding.is_some() {
                    CodecFormat::H264
                } else {
                    auto_codec
                }
            }
            PreferCodec::H265 => {
                if h265vram_encoding || h265hw_encoding.is_some() {
                    CodecFormat::H265
                } else {
                    auto_codec
                }
            }
            PreferCodec::Auto => auto_codec,
        };
        if decodings.len() > 0 {
            log::info!(
                "usable: vp8={vp8_useable}, av1={av1_useable}, h264={h264_useable}, h265={h265_useable}",
            );
            log::info!(
                "connection count: {}, used preference: {:?}, encoder: {:?}",
                decodings.len(),
                preference,
                *format
            )
        }
    }

    #[inline]
    pub fn negotiated_codec() -> CodecFormat {
        ENCODE_CODEC_FORMAT.lock().unwrap().clone()
    }

    pub fn supported_encoding() -> SupportedEncoding {
        #[allow(unused_mut)]
        let mut encoding = SupportedEncoding {
            vp8: true,
            av1: !disable_av1(),
            i444: Some(CodecAbility {
                vp9: true,
                av1: true,
                ..Default::default()
            })
            .into(),
            ..Default::default()
        };
        #[cfg(feature = "hwcodec")]
        if enable_hwcodec_option() {
            encoding.h264 |= HwRamEncoder::try_get(CodecFormat::H264).is_some();
            encoding.h265 |= HwRamEncoder::try_get(CodecFormat::H265).is_some();
        }
        #[cfg(feature = "vram")]
        if enable_vram_option(true) {
            encoding.h264 |= VRamEncoder::available(CodecFormat::H264).len() > 0;
            encoding.h265 |= VRamEncoder::available(CodecFormat::H265).len() > 0;
        }
        encoding
    }

    pub fn usable_encoding() -> Option<SupportedEncoding> {
        USABLE_ENCODING.lock().unwrap().clone()
    }

    pub fn set_fallback(config: &EncoderCfg) {
        let format = match config {
            EncoderCfg::VPX(vpx) => match vpx.codec {
                VpxVideoCodecId::VP8 => CodecFormat::VP8,
                VpxVideoCodecId::VP9 => CodecFormat::VP9,
            },
            EncoderCfg::AOM(_) => CodecFormat::AV1,
            #[cfg(feature = "hwcodec")]
            EncoderCfg::HWRAM(hw) => {
                let name = hw.name.to_lowercase();
                if name.contains("vp8") {
                    CodecFormat::VP8
                } else if name.contains("vp9") {
                    CodecFormat::VP9
                } else if name.contains("av1") {
                    CodecFormat::AV1
                } else if name.contains("h264") {
                    CodecFormat::H264
                } else {
                    CodecFormat::H265
                }
            }
            #[cfg(feature = "vram")]
            EncoderCfg::VRAM(vram) => match vram.feature.data_format {
                hwcodec::common::DataFormat::H264 => CodecFormat::H264,
                hwcodec::common::DataFormat::H265 => CodecFormat::H265,
                _ => {
                    log::error!(
                        "should not reach here, vram not support {:?}",
                        vram.feature.data_format
                    );
                    return;
                }
            },
        };
        let current = ENCODE_CODEC_FORMAT.lock().unwrap().clone();
        if current != format {
            log::info!("codec fallback: {:?} -> {:?}", current, format);
            *ENCODE_CODEC_FORMAT.lock().unwrap() = format;
        }
    }

    pub fn use_i444(config: &EncoderCfg) -> bool {
        let decodings = PEER_DECODINGS.lock().unwrap().clone();
        let prefer_i444 = decodings
            .iter()
            .all(|d| d.1.prefer_chroma == Chroma::I444.into());
        let i444_useable = match config {
            EncoderCfg::VPX(vpx) => match vpx.codec {
                VpxVideoCodecId::VP8 => false,
                VpxVideoCodecId::VP9 => decodings.iter().all(|d| d.1.i444.vp9),
            },
            EncoderCfg::AOM(_) => decodings.iter().all(|d| d.1.i444.av1),
            #[cfg(feature = "hwcodec")]
            EncoderCfg::HWRAM(_) => false,
            #[cfg(feature = "vram")]
            EncoderCfg::VRAM(_) => false,
        };
        prefer_i444 && i444_useable && !decodings.is_empty()
    }
}

impl Decoder {
    pub fn supported_decodings(
        id_for_perfer: Option<&str>,
        _use_texture_render: bool,
        _luid: Option<i64>,
        mark_unsupported: &Vec<CodecFormat>,
    ) -> SupportedDecoding {
        let (prefer, prefer_chroma) = Self::preference(id_for_perfer);

        #[allow(unused_mut)]
        let mut decoding = SupportedDecoding {
            ability_vp8: 1,
            ability_vp9: 1,
            ability_av1: if disable_av1() { 0 } else { 1 },
            i444: Some(CodecAbility {
                vp9: true,
                av1: true,
                ..Default::default()
            })
            .into(),
            prefer: prefer.into(),
            prefer_chroma: prefer_chroma.into(),
            ..Default::default()
        };
        #[cfg(feature = "hwcodec")]
        {
            decoding.ability_h264 |= if HwRamDecoder::try_get(CodecFormat::H264).is_some() {
                1
            } else {
                0
            };
            decoding.ability_h265 |= if HwRamDecoder::try_get(CodecFormat::H265).is_some() {
                1
            } else {
                0
            };
        }
        #[cfg(feature = "vram")]
        if enable_vram_option(false) && _use_texture_render {
            decoding.ability_h264 |= if VRamDecoder::available(CodecFormat::H264, _luid).len() > 0 {
                1
            } else {
                0
            };
            decoding.ability_h265 |= if VRamDecoder::available(CodecFormat::H265, _luid).len() > 0 {
                1
            } else {
                0
            };
        }
        #[cfg(feature = "mediacodec")]
        if enable_hwcodec_option() {
            decoding.ability_h264 =
                if H264_DECODER_SUPPORT.load(std::sync::atomic::Ordering::SeqCst) {
                    1
                } else {
                    0
                };
            decoding.ability_h265 =
                if H265_DECODER_SUPPORT.load(std::sync::atomic::Ordering::SeqCst) {
                    1
                } else {
                    0
                };
        }
        for unsupported in mark_unsupported {
            match unsupported {
                CodecFormat::VP8 => decoding.ability_vp8 = 0,
                CodecFormat::VP9 => decoding.ability_vp9 = 0,
                CodecFormat::AV1 => decoding.ability_av1 = 0,
                CodecFormat::H264 => decoding.ability_h264 = 0,
                CodecFormat::H265 => decoding.ability_h265 = 0,
                _ => {}
            }
        }
        decoding
    }

    pub fn new(format: CodecFormat, _luid: Option<i64>) -> Decoder {
        log::info!("try create new decoder, format: {format:?}, _luid: {_luid:?}");
        let (mut vp8, mut vp9, mut av1) = (None, None, None);
        #[cfg(feature = "hwcodec")]
        let (mut h264_ram, mut h265_ram) = (None, None);
        #[cfg(feature = "vram")]
        let (mut h264_vram, mut h265_vram) = (None, None);
        #[cfg(feature = "mediacodec")]
        let (mut h264_media_codec, mut h265_media_codec) = (None, None);
        let mut valid = false;

        match format {
            CodecFormat::VP8 => {
                match VpxDecoder::new(VpxDecoderConfig {
                    codec: VpxVideoCodecId::VP8,
                }) {
                    Ok(v) => vp8 = Some(v),
                    Err(e) => log::error!("create VP8 decoder failed: {}", e),
                }
                valid = vp8.is_some();
            }
            CodecFormat::VP9 => {
                match VpxDecoder::new(VpxDecoderConfig {
                    codec: VpxVideoCodecId::VP9,
                }) {
                    Ok(v) => vp9 = Some(v),
                    Err(e) => log::error!("create VP9 decoder failed: {}", e),
                }
                valid = vp9.is_some();
            }
            CodecFormat::AV1 => {
                match AomDecoder::new() {
                    Ok(v) => av1 = Some(v),
                    Err(e) => log::error!("create AV1 decoder failed: {}", e),
                }
                valid = av1.is_some();
            }
            CodecFormat::H264 => {
                #[cfg(feature = "vram")]
                if !valid && enable_vram_option(false) && _luid.clone().unwrap_or_default() != 0 {
                    match VRamDecoder::new(format, _luid) {
                        Ok(v) => h264_vram = Some(v),
                        Err(e) => log::error!("create H264 vram decoder failed: {}", e),
                    }
                    valid = h264_vram.is_some();
                }
                #[cfg(feature = "hwcodec")]
                if !valid {
                    match HwRamDecoder::new(format) {
                        Ok(v) => h264_ram = Some(v),
                        Err(e) => log::error!("create H264 ram decoder failed: {}", e),
                    }
                    valid = h264_ram.is_some();
                }
                #[cfg(feature = "mediacodec")]
                if !valid && enable_hwcodec_option() {
                    h264_media_codec = MediaCodecDecoder::new(format);
                    if h264_media_codec.is_none() {
                        log::error!("create H264 media codec decoder failed");
                    }
                    valid = h264_media_codec.is_some();
                }
            }
            CodecFormat::H265 => {
                #[cfg(feature = "vram")]
                if !valid && enable_vram_option(false) && _luid.clone().unwrap_or_default() != 0 {
                    match VRamDecoder::new(format, _luid) {
                        Ok(v) => h265_vram = Some(v),
                        Err(e) => log::error!("create H265 vram decoder failed: {}", e),
                    }
                    valid = h265_vram.is_some();
                }
                #[cfg(feature = "hwcodec")]
                if !valid {
                    match HwRamDecoder::new(format) {
                        Ok(v) => h265_ram = Some(v),
                        Err(e) => log::error!("create H265 ram decoder failed: {}", e),
                    }
                    valid = h265_ram.is_some();
                }
                #[cfg(feature = "mediacodec")]
                if !valid && enable_hwcodec_option() {
                    h265_media_codec = MediaCodecDecoder::new(format);
                    if h265_media_codec.is_none() {
                        log::error!("create H265 media codec decoder failed");
                    }
                    valid = h265_media_codec.is_some();
                }
            }
            CodecFormat::Unknown => {
                log::error!("unknown codec format, cannot create decoder");
            }
        }
        if !valid {
            log::error!("failed to create {format:?} decoder");
        } else {
            log::info!("create {format:?} decoder success");
        }
        Decoder {
            vp8,
            vp9,
            av1,
            #[cfg(feature = "hwcodec")]
            h264_ram,
            #[cfg(feature = "hwcodec")]
            h265_ram,
            #[cfg(feature = "vram")]
            h264_vram,
            #[cfg(feature = "vram")]
            h265_vram,
            #[cfg(feature = "mediacodec")]
            h264_media_codec,
            #[cfg(feature = "mediacodec")]
            h265_media_codec,
            format,
            valid,
            #[cfg(feature = "hwcodec")]
            i420: vec![],
        }
    }

    pub fn format(&self) -> CodecFormat {
        self.format
    }

    pub fn valid(&self) -> bool {
        self.valid
    }

    // rgb [in/out] fmt and stride must be set in ImageRgb
    pub fn handle_video_frame(
        &mut self,
        frame: &video_frame::Union,
        rgb: &mut ImageRgb,
        _texture: &mut *mut c_void,
        _pixelbuffer: &mut bool,
        chroma: &mut Option<Chroma>,
    ) -> ResultType<bool> {
        match frame {
            video_frame::Union::Vp8s(vp8s) => {
                if let Some(vp8) = &mut self.vp8 {
                    Decoder::handle_vpxs_video_frame(vp8, vp8s, rgb, chroma)
                } else {
                    bail!("vp8 decoder not available");
                }
            }
            video_frame::Union::Vp9s(vp9s) => {
                if let Some(vp9) = &mut self.vp9 {
                    Decoder::handle_vpxs_video_frame(vp9, vp9s, rgb, chroma)
                } else {
                    bail!("vp9 decoder not available");
                }
            }
            video_frame::Union::Av1s(av1s) => {
                if let Some(av1) = &mut self.av1 {
                    Decoder::handle_av1s_video_frame(av1, av1s, rgb, chroma)
                } else {
                    bail!("av1 decoder not available");
                }
            }
            #[cfg(any(feature = "hwcodec", feature = "vram"))]
            video_frame::Union::H264s(h264s) => {
                *chroma = Some(Chroma::I420);
                #[cfg(feature = "vram")]
                if let Some(decoder) = &mut self.h264_vram {
                    *_pixelbuffer = false;
                    return Decoder::handle_vram_video_frame(decoder, h264s, _texture);
                }
                #[cfg(feature = "hwcodec")]
                if let Some(decoder) = &mut self.h264_ram {
                    return Decoder::handle_hwram_video_frame(decoder, h264s, rgb, &mut self.i420);
                }
                Err(anyhow!("don't support h264!"))
            }
            #[cfg(any(feature = "hwcodec", feature = "vram"))]
            video_frame::Union::H265s(h265s) => {
                *chroma = Some(Chroma::I420);
                #[cfg(feature = "vram")]
                if let Some(decoder) = &mut self.h265_vram {
                    *_pixelbuffer = false;
                    return Decoder::handle_vram_video_frame(decoder, h265s, _texture);
                }
                #[cfg(feature = "hwcodec")]
                if let Some(decoder) = &mut self.h265_ram {
                    return Decoder::handle_hwram_video_frame(decoder, h265s, rgb, &mut self.i420);
                }
                Err(anyhow!("don't support h265!"))
            }
            #[cfg(feature = "mediacodec")]
            video_frame::Union::H264s(h264s) => {
                *chroma = Some(Chroma::I420);
                if let Some(decoder) = &mut self.h264_media_codec {
                    Decoder::handle_mediacodec_video_frame(decoder, h264s, rgb)
                } else {
                    Err(anyhow!("don't support h264!"))
                }
            }
            #[cfg(feature = "mediacodec")]
            video_frame::Union::H265s(h265s) => {
                *chroma = Some(Chroma::I420);
                if let Some(decoder) = &mut self.h265_media_codec {
                    Decoder::handle_mediacodec_video_frame(decoder, h265s, rgb)
                } else {
                    Err(anyhow!("don't support h265!"))
                }
            }
            _ => Err(anyhow!("unsupported video frame type!")),
        }
    }

    // rgb [in/out] fmt and stride must be set in ImageRgb
    fn handle_vpxs_video_frame(
        decoder: &mut VpxDecoder,
        vpxs: &EncodedVideoFrames,
        rgb: &mut ImageRgb,
        chroma: &mut Option<Chroma>,
    ) -> ResultType<bool> {
        let mut last_frame = vpxcodec::Image::new();
        for vpx in vpxs.frames.iter() {
            for frame in decoder.decode(&vpx.data)? {
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
            *chroma = Some(last_frame.chroma());
            last_frame.to(rgb);
            Ok(true)
        }
    }

    // rgb [in/out] fmt and stride must be set in ImageRgb
    fn handle_av1s_video_frame(
        decoder: &mut AomDecoder,
        av1s: &EncodedVideoFrames,
        rgb: &mut ImageRgb,
        chroma: &mut Option<Chroma>,
    ) -> ResultType<bool> {
        let mut last_frame = aom::Image::new();
        for av1 in av1s.frames.iter() {
            for frame in decoder.decode(&av1.data)? {
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
            *chroma = Some(last_frame.chroma());
            last_frame.to(rgb);
            Ok(true)
        }
    }

    // rgb [in/out] fmt and stride must be set in ImageRgb
    #[cfg(feature = "hwcodec")]
    fn handle_hwram_video_frame(
        decoder: &mut HwRamDecoder,
        frames: &EncodedVideoFrames,
        rgb: &mut ImageRgb,
        i420: &mut Vec<u8>,
    ) -> ResultType<bool> {
        let mut ret = false;
        for h264 in frames.frames.iter() {
            for image in decoder.decode(&h264.data)? {
                // TODO: just process the last frame
                if image.to_fmt(rgb, i420).is_ok() {
                    ret = true;
                }
            }
        }
        return Ok(ret);
    }

    #[cfg(feature = "vram")]
    fn handle_vram_video_frame(
        decoder: &mut VRamDecoder,
        frames: &EncodedVideoFrames,
        texture: &mut *mut c_void,
    ) -> ResultType<bool> {
        let mut ret = false;
        for h26x in frames.frames.iter() {
            for image in decoder.decode(&h26x.data)? {
                *texture = image.frame.texture;
                ret = true;
            }
        }
        return Ok(ret);
    }

    // rgb [in/out] fmt and stride must be set in ImageRgb
    #[cfg(feature = "mediacodec")]
    fn handle_mediacodec_video_frame(
        decoder: &mut MediaCodecDecoder,
        frames: &EncodedVideoFrames,
        rgb: &mut ImageRgb,
    ) -> ResultType<bool> {
        let mut ret = false;
        for h264 in frames.frames.iter() {
            return decoder.decode(&h264.data, rgb);
        }
        return Ok(false);
    }

    fn preference(id: Option<&str>) -> (PreferCodec, Chroma) {
        let id = id.unwrap_or_default();
        if id.is_empty() {
            return (PreferCodec::Auto, Chroma::I420);
        }
        let options = PeerConfig::load(id).options;
        let codec = options
            .get("codec-preference")
            .map_or("".to_owned(), |c| c.to_owned());
        let codec = if codec == "vp8" {
            PreferCodec::VP8
        } else if codec == "vp9" {
            PreferCodec::VP9
        } else if codec == "av1" {
            PreferCodec::AV1
        } else if codec == "h264" {
            PreferCodec::H264
        } else if codec == "h265" {
            PreferCodec::H265
        } else {
            PreferCodec::Auto
        };
        let chroma = if options.get("i444") == Some(&"Y".to_string()) {
            Chroma::I444
        } else {
            Chroma::I420
        };
        (codec, chroma)
    }
}

#[cfg(any(feature = "hwcodec", feature = "mediacodec"))]
pub fn enable_hwcodec_option() -> bool {
    use hbb_common::config::keys::OPTION_ENABLE_HWCODEC;

    if cfg!(windows) || cfg!(target_os = "linux") || cfg!(target_os = "android") {
        return option2bool(
            OPTION_ENABLE_HWCODEC,
            &Config::get_option(OPTION_ENABLE_HWCODEC),
        );
    }
    false
}
#[cfg(feature = "vram")]
pub fn enable_vram_option(encode: bool) -> bool {
    use hbb_common::config::keys::OPTION_ENABLE_HWCODEC;

    if cfg!(windows) {
        let enable = option2bool(
            OPTION_ENABLE_HWCODEC,
            &Config::get_option(OPTION_ENABLE_HWCODEC),
        );
        if encode {
            enable && enable_directx_capture()
        } else {
            enable
        }
    } else {
        false
    }
}

#[cfg(windows)]
pub fn enable_directx_capture() -> bool {
    use hbb_common::config::keys::OPTION_ENABLE_DIRECTX_CAPTURE as OPTION;
    option2bool(
        OPTION,
        &Config::get_option(hbb_common::config::keys::OPTION_ENABLE_DIRECTX_CAPTURE),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    Best,
    Balanced,
    Low,
    Custom(u32),
}

impl Default for Quality {
    fn default() -> Self {
        Self::Balanced
    }
}

impl Quality {
    pub fn is_custom(&self) -> bool {
        match self {
            Quality::Custom(_) => true,
            _ => false,
        }
    }
}

pub fn base_bitrate(width: u32, height: u32) -> u32 {
    #[allow(unused_mut)]
    let mut base_bitrate = ((width * height) / 1000) as u32; // same as 1.1.9
    if base_bitrate == 0 {
        base_bitrate = 1920 * 1080 / 1000;
    }
    #[cfg(target_os = "android")]
    {
        // fix when android screen shrinks
        let fix = crate::Display::fix_quality() as u32;
        log::debug!("Android screen, fix quality:{}", fix);
        base_bitrate = base_bitrate * fix;
    }
    base_bitrate
}

pub fn codec_thread_num(limit: usize) -> usize {
    let max: usize = num_cpus::get();
    let mut res;
    let info;
    let mut s = System::new();
    s.refresh_memory();
    let memory = s.available_memory() / 1024 / 1024 / 1024;
    #[cfg(windows)]
    {
        res = 0;
        let percent = hbb_common::platform::windows::cpu_uage_one_minute();
        info = format!("cpu usage: {:?}", percent);
        if let Some(pecent) = percent {
            if pecent < 100.0 {
                res = ((100.0 - pecent) * (max as f64) / 200.0).round() as usize;
            }
        }
    }
    #[cfg(not(windows))]
    {
        s.refresh_cpu_usage();
        // https://man7.org/linux/man-pages/man3/getloadavg.3.html
        let avg = s.load_average();
        info = format!("cpu loadavg: {}", avg.one);
        res = (((max as f64) - avg.one) * 0.5).round() as usize;
    }
    res = std::cmp::min(res, max / 2);
    res = std::cmp::min(res, memory as usize / 2);
    //  Use common thread count
    res = match res {
        _ if res >= 64 => 64,
        _ if res >= 32 => 32,
        _ if res >= 16 => 16,
        _ if res >= 8 => 8,
        _ if res >= 4 => 4,
        _ if res >= 2 => 2,
        _ => 1,
    };
    // https://aomedia.googlesource.com/aom/+/refs/heads/main/av1/av1_cx_iface.c#677
    // https://aomedia.googlesource.com/aom/+/refs/heads/main/aom_util/aom_thread.h#26
    // https://chromium.googlesource.com/webm/libvpx/+/refs/heads/main/vp8/vp8_cx_iface.c#148
    // https://chromium.googlesource.com/webm/libvpx/+/refs/heads/main/vp9/vp9_cx_iface.c#190
    // https://github.com/FFmpeg/FFmpeg/blob/7c16bf0829802534004326c8e65fb6cdbdb634fa/libavcodec/pthread.c#L65
    // https://github.com/FFmpeg/FFmpeg/blob/7c16bf0829802534004326c8e65fb6cdbdb634fa/libavcodec/pthread_internal.h#L26
    // libaom: MAX_NUM_THREADS = 64
    // libvpx: MAX_NUM_THREADS = 64
    // ffmpeg: MAX_AUTO_THREADS = 16
    res = std::cmp::min(res, limit);
    // avoid frequent log
    let log = match THREAD_LOG_TIME.lock().unwrap().clone() {
        Some(instant) => instant.elapsed().as_secs() > 1,
        None => true,
    };
    if log {
        log::info!("cpu num: {max}, {info}, available memory: {memory}G, codec thread: {res}");
        *THREAD_LOG_TIME.lock().unwrap() = Some(Instant::now());
    }
    res
}

fn disable_av1() -> bool {
    // aom is very slow for x86 sciter version on windows x64
    // disable it for all 32 bit platforms
    std::mem::size_of::<usize>() == 4
}
