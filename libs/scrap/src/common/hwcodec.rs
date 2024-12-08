use crate::{
    codec::{
        base_bitrate, codec_thread_num, enable_hwcodec_option, EncoderApi, EncoderCfg, Quality as Q,
    },
    convert::*,
    CodecFormat, EncodeInput, ImageFormat, ImageRgb, Pixfmt, HW_STRIDE_ALIGN,
};
use hbb_common::{
    anyhow::{anyhow, bail, Context},
    bytes::Bytes,
    log,
    message_proto::{EncodedVideoFrame, EncodedVideoFrames, VideoFrame},
    serde_derive::{Deserialize, Serialize},
    serde_json, ResultType,
};
use hwcodec::{
    common::{
        DataFormat, HwcodecErrno,
        Quality::{self, *},
        RateControl::{self, *},
    },
    ffmpeg::AVPixelFormat,
    ffmpeg_ram::{
        decode::{DecodeContext, DecodeFrame, Decoder},
        encode::{EncodeContext, EncodeFrame, Encoder},
        ffmpeg_linesize_offset_length, CodecInfo,
    },
};

const DEFAULT_PIXFMT: AVPixelFormat = AVPixelFormat::AV_PIX_FMT_NV12;
pub const DEFAULT_FPS: i32 = 30;
const DEFAULT_GOP: i32 = i32::MAX;
const DEFAULT_HW_QUALITY: Quality = Quality_Default;
pub const ERR_HEVC_POC: i32 = HwcodecErrno::HWCODEC_ERR_HEVC_COULD_NOT_FIND_POC as i32;

crate::generate_call_macro!(call_yuv, false);

#[cfg(not(target_os = "android"))]
lazy_static::lazy_static! {
    static ref CONFIG: std::sync::Arc<std::sync::Mutex<Option<HwCodecConfig>>> = Default::default();
    static ref CONFIG_SET_BY_IPC: std::sync::Arc<std::sync::Mutex<bool>> = Default::default();
}

#[derive(Debug, Clone)]
pub struct HwRamEncoderConfig {
    pub name: String,
    pub mc_name: Option<String>,
    pub width: usize,
    pub height: usize,
    pub quality: Q,
    pub keyframe_interval: Option<usize>,
}

pub struct HwRamEncoder {
    encoder: Encoder,
    pub format: DataFormat,
    pub pixfmt: AVPixelFormat,
    bitrate: u32, //kbs
    config: HwRamEncoderConfig,
}

impl EncoderApi for HwRamEncoder {
    fn new(cfg: EncoderCfg, _i444: bool) -> ResultType<Self>
    where
        Self: Sized,
    {
        match cfg {
            EncoderCfg::HWRAM(config) => {
                let rc = Self::rate_control(&config);
                let b = Self::convert_quality(&config.name, config.quality);
                let base_bitrate = base_bitrate(config.width as _, config.height as _);
                let mut bitrate = base_bitrate * b / 100;
                if bitrate <= 0 {
                    bitrate = base_bitrate;
                }
                bitrate = Self::check_bitrate_range(&config, bitrate);
                let gop = config.keyframe_interval.unwrap_or(DEFAULT_GOP as _) as i32;
                let ctx = EncodeContext {
                    name: config.name.clone(),
                    mc_name: config.mc_name.clone(),
                    width: config.width as _,
                    height: config.height as _,
                    pixfmt: DEFAULT_PIXFMT,
                    align: HW_STRIDE_ALIGN as _,
                    kbs: bitrate as i32,
                    fps: DEFAULT_FPS,
                    gop,
                    quality: DEFAULT_HW_QUALITY,
                    rc,
                    q: -1,
                    thread_count: codec_thread_num(16) as _, // ffmpeg's thread_count is used for cpu
                };
                let format = match Encoder::format_from_name(config.name.clone()) {
                    Ok(format) => format,
                    Err(_) => {
                        return Err(anyhow!(format!(
                            "failed to get format from name:{}",
                            config.name
                        )))
                    }
                };
                match Encoder::new(ctx.clone()) {
                    Ok(encoder) => Ok(HwRamEncoder {
                        encoder,
                        format,
                        pixfmt: ctx.pixfmt,
                        bitrate,
                        config,
                    }),
                    Err(_) => Err(anyhow!(format!("Failed to create encoder"))),
                }
            }
            _ => Err(anyhow!("encoder type mismatch")),
        }
    }

    fn encode_to_message(&mut self, input: EncodeInput, ms: i64) -> ResultType<VideoFrame> {
        let mut vf = VideoFrame::new();
        let mut frames = Vec::new();
        for frame in self
            .encode(input.yuv()?, ms)
            .with_context(|| "Failed to encode")?
        {
            frames.push(EncodedVideoFrame {
                data: Bytes::from(frame.data),
                pts: frame.pts,
                key: frame.key == 1,
                ..Default::default()
            });
        }
        if frames.len() > 0 {
            let frames = EncodedVideoFrames {
                frames: frames.into(),
                ..Default::default()
            };
            match self.format {
                DataFormat::H264 => vf.set_h264s(frames),
                DataFormat::H265 => vf.set_h265s(frames),
                _ => bail!("unsupported format: {:?}", self.format),
            }
            Ok(vf)
        } else {
            Err(anyhow!("no valid frame"))
        }
    }

    fn yuvfmt(&self) -> crate::EncodeYuvFormat {
        let pixfmt = if self.pixfmt == AVPixelFormat::AV_PIX_FMT_NV12 {
            Pixfmt::NV12
        } else {
            Pixfmt::I420
        };
        let stride = self
            .encoder
            .linesize
            .clone()
            .drain(..)
            .map(|i| i as usize)
            .collect();
        crate::EncodeYuvFormat {
            pixfmt,
            w: self.encoder.ctx.width as _,
            h: self.encoder.ctx.height as _,
            stride,
            u: self.encoder.offset[0] as _,
            v: if pixfmt == Pixfmt::NV12 {
                0
            } else {
                self.encoder.offset[1] as _
            },
        }
    }

    #[cfg(feature = "vram")]
    fn input_texture(&self) -> bool {
        false
    }

    fn set_quality(&mut self, quality: crate::codec::Quality) -> ResultType<()> {
        let b = Self::convert_quality(&self.config.name, quality);
        let mut bitrate = base_bitrate(self.config.width as _, self.config.height as _) * b / 100;
        if bitrate > 0 {
            bitrate = Self::check_bitrate_range(&self.config, bitrate);
            self.encoder.set_bitrate(bitrate as _).ok();
            self.bitrate = bitrate;
        }
        self.config.quality = quality;
        Ok(())
    }

    fn bitrate(&self) -> u32 {
        self.bitrate
    }

    fn support_abr(&self) -> bool {
        ["qsv", "vaapi"].iter().all(|&x| !self.config.name.contains(x))
    }

    fn support_changing_quality(&self) -> bool {
        ["vaapi"].iter().all(|&x| !self.config.name.contains(x))
    }

    fn latency_free(&self) -> bool {
        ["mediacodec", "videotoolbox"]
            .iter()
            .all(|&x| !self.config.name.contains(x))
    }

    fn is_hardware(&self) -> bool {
        true
    }

    fn disable(&self) {
        HwCodecConfig::clear(false, true);
    }
}

impl HwRamEncoder {
    pub fn try_get(format: CodecFormat) -> Option<CodecInfo> {
        let mut info = None;
        let best = CodecInfo::prioritized(HwCodecConfig::get().ram_encode);
        match format {
            CodecFormat::H264 => {
                if let Some(v) = best.h264 {
                    info = Some(v);
                }
            }
            CodecFormat::H265 => {
                if let Some(v) = best.h265 {
                    info = Some(v);
                }
            }
            _ => {}
        }
        info
    }

    pub fn encode(&mut self, yuv: &[u8], ms: i64) -> ResultType<Vec<EncodeFrame>> {
        match self.encoder.encode(yuv, ms) {
            Ok(v) => {
                let mut data = Vec::<EncodeFrame>::new();
                data.append(v);
                Ok(data)
            }
            Err(_) => Ok(Vec::<EncodeFrame>::new()),
        }
    }

    fn rate_control(_config: &HwRamEncoderConfig) -> RateControl {
        #[cfg(target_os = "android")]
        if _config.name.contains("mediacodec") {
            return RC_VBR;
        }
        RC_CBR
    }

    pub fn convert_quality(name: &str, quality: crate::codec::Quality) -> u32 {
        use crate::codec::Quality;
        let quality = match quality {
            Quality::Best => 150,
            Quality::Balanced => 100,
            Quality::Low => 50,
            Quality::Custom(b) => b,
        };
        let factor = if name.contains("mediacodec") {
            // https://stackoverflow.com/questions/26110337/what-are-valid-bit-rates-to-set-for-mediacodec?rq=3
            5
        } else {
            1
        };
        quality * factor
    }

    pub fn check_bitrate_range(_config: &HwRamEncoderConfig, bitrate: u32) -> u32 {
        #[cfg(target_os = "android")]
        if _config.name.contains("mediacodec") {
            let info = crate::android::ffi::get_codec_info();
            if let Some(info) = info {
                if let Some(codec) = info
                    .codecs
                    .iter()
                    .find(|c| Some(c.name.clone()) == _config.mc_name && c.is_encoder)
                {
                    if codec.max_bitrate > codec.min_bitrate {
                        if bitrate > codec.max_bitrate {
                            return codec.max_bitrate;
                        }
                        if bitrate < codec.min_bitrate {
                            return codec.min_bitrate;
                        }
                    }
                }
            }
        }
        bitrate
    }
}

pub struct HwRamDecoder {
    decoder: Decoder,
    pub info: CodecInfo,
}

impl HwRamDecoder {
    pub fn try_get(format: CodecFormat) -> Option<CodecInfo> {
        let mut info = None;
        let soft = CodecInfo::soft();
        match format {
            CodecFormat::H264 => {
                if let Some(v) = soft.h264 {
                    info = Some(v);
                }
            }
            CodecFormat::H265 => {
                if let Some(v) = soft.h265 {
                    info = Some(v);
                }
            }
            _ => {}
        }
        if enable_hwcodec_option() {
            let best = CodecInfo::prioritized(HwCodecConfig::get().ram_decode);
            match format {
                CodecFormat::H264 => {
                    if let Some(v) = best.h264 {
                        info = Some(v);
                    }
                }
                CodecFormat::H265 => {
                    if let Some(v) = best.h265 {
                        info = Some(v);
                    }
                }
                _ => {}
            }
        }
        info
    }

    pub fn new(format: CodecFormat) -> ResultType<Self> {
        let info = HwRamDecoder::try_get(format);
        log::info!("try create {info:?} ram decoder");
        let Some(info) = info else {
            bail!("unsupported format: {:?}", format);
        };
        let ctx = DecodeContext {
            name: info.name.clone(),
            device_type: info.hwdevice.clone(),
            thread_count: codec_thread_num(16) as _,
        };
        match Decoder::new(ctx) {
            Ok(decoder) => Ok(HwRamDecoder { decoder, info }),
            Err(_) => {
                HwCodecConfig::clear(false, false);
                Err(anyhow!(format!("Failed to create decoder")))
            }
        }
    }
    pub fn decode(&mut self, data: &[u8]) -> ResultType<Vec<HwRamDecoderImage>> {
        match self.decoder.decode(data) {
            Ok(v) => Ok(v.iter().map(|f| HwRamDecoderImage { frame: f }).collect()),
            Err(e) => Err(anyhow!(e)),
        }
    }
}

pub struct HwRamDecoderImage<'a> {
    frame: &'a DecodeFrame,
}

impl HwRamDecoderImage<'_> {
    // rgb [in/out] fmt and stride must be set in ImageRgb
    pub fn to_fmt(&self, rgb: &mut ImageRgb, i420: &mut Vec<u8>) -> ResultType<()> {
        let frame = self.frame;
        let width = frame.width;
        let height = frame.height;
        rgb.w = width as _;
        rgb.h = height as _;
        let dst_align = rgb.align();
        let bytes_per_row = (rgb.w * 4 + dst_align - 1) & !(dst_align - 1);
        rgb.raw.resize(rgb.h * bytes_per_row, 0);
        match frame.pixfmt {
            AVPixelFormat::AV_PIX_FMT_NV12 => {
                // I420ToARGB is much faster than NV12ToARGB in tests on Windows
                if cfg!(windows) {
                    let Ok((linesize_i420, offset_i420, len_i420)) = ffmpeg_linesize_offset_length(
                        AVPixelFormat::AV_PIX_FMT_YUV420P,
                        width as _,
                        height as _,
                        HW_STRIDE_ALIGN,
                    ) else {
                        bail!("failed to get i420 linesize, offset, length");
                    };
                    i420.resize(len_i420 as _, 0);
                    let i420_offset_y = unsafe { i420.as_ptr().add(0) as _ };
                    let i420_offset_u = unsafe { i420.as_ptr().add(offset_i420[0] as _) as _ };
                    let i420_offset_v = unsafe { i420.as_ptr().add(offset_i420[1] as _) as _ };
                    call_yuv!(NV12ToI420(
                        frame.data[0].as_ptr(),
                        frame.linesize[0],
                        frame.data[1].as_ptr(),
                        frame.linesize[1],
                        i420_offset_y,
                        linesize_i420[0],
                        i420_offset_u,
                        linesize_i420[1],
                        i420_offset_v,
                        linesize_i420[2],
                        width,
                        height,
                    ));
                    let f = match rgb.fmt() {
                        ImageFormat::ARGB => I420ToARGB,
                        ImageFormat::ABGR => I420ToABGR,
                        _ => bail!("unsupported format: {:?} -> {:?}", frame.pixfmt, rgb.fmt()),
                    };
                    call_yuv!(f(
                        i420_offset_y,
                        linesize_i420[0],
                        i420_offset_u,
                        linesize_i420[1],
                        i420_offset_v,
                        linesize_i420[2],
                        rgb.raw.as_mut_ptr(),
                        bytes_per_row as _,
                        width,
                        height,
                    ));
                } else {
                    let f = match rgb.fmt() {
                        ImageFormat::ARGB => NV12ToARGB,
                        ImageFormat::ABGR => NV12ToABGR,
                        _ => bail!("unsupported format: {:?} -> {:?}", frame.pixfmt, rgb.fmt()),
                    };
                    call_yuv!(f(
                        frame.data[0].as_ptr(),
                        frame.linesize[0],
                        frame.data[1].as_ptr(),
                        frame.linesize[1],
                        rgb.raw.as_mut_ptr(),
                        bytes_per_row as _,
                        width,
                        height,
                    ));
                }
            }
            AVPixelFormat::AV_PIX_FMT_YUV420P => {
                let f = match rgb.fmt() {
                    ImageFormat::ARGB => I420ToARGB,
                    ImageFormat::ABGR => I420ToABGR,
                    _ => bail!("unsupported format: {:?} -> {:?}", frame.pixfmt, rgb.fmt()),
                };
                call_yuv!(f(
                    frame.data[0].as_ptr(),
                    frame.linesize[0],
                    frame.data[1].as_ptr(),
                    frame.linesize[1],
                    frame.data[2].as_ptr(),
                    frame.linesize[2],
                    rgb.raw.as_mut_ptr(),
                    bytes_per_row as _,
                    width,
                    height,
                ));
            }
        }
        Ok(())
    }
}

#[cfg(target_os = "android")]
fn get_mime_type(codec: DataFormat) -> &'static str {
    match codec {
        DataFormat::VP8 => "video/x-vnd.on2.vp8",
        DataFormat::VP9 => "video/x-vnd.on2.vp9",
        DataFormat::AV1 => "video/av01",
        DataFormat::H264 => "video/avc",
        DataFormat::H265 => "video/hevc",
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct HwCodecConfig {
    #[serde(default)]
    pub signature: u64,
    #[serde(default)]
    pub ram_encode: Vec<CodecInfo>,
    #[serde(default)]
    pub ram_decode: Vec<CodecInfo>,
    #[cfg(feature = "vram")]
    #[serde(default)]
    pub vram_encode: Vec<hwcodec::vram::FeatureContext>,
    #[cfg(feature = "vram")]
    #[serde(default)]
    pub vram_decode: Vec<hwcodec::vram::DecodeContext>,
}

// HwCodecConfig2 is used to store the config in json format,
// confy can't serde HwCodecConfig successfully if the non-first struct Vec is empty due to old toml version.
// struct T { a: Vec<A>, b: Vec<String>} will fail if b is empty, but struct T { a: Vec<String>, b: Vec<String>} is ok.
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
struct HwCodecConfig2 {
    #[serde(default)]
    pub config: String,
}

// ipc server process start check process once, other process get from ipc server once
// install: --server start check process, check process send to --server,  ui get from --server
// portable: ui start check process, check process send to ui
// sciter and unilink: get from ipc server
impl HwCodecConfig {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub fn set(config: String) {
        let config = serde_json::from_str(&config).unwrap_or_default();
        log::info!("set hwcodec config");
        log::debug!("{config:?}");
        #[cfg(any(windows, target_os = "macos"))]
        hbb_common::config::common_store(
            &HwCodecConfig2 {
                config: serde_json::to_string_pretty(&config).unwrap_or_default(),
            },
            "_hwcodec",
        );
        *CONFIG.lock().unwrap() = Some(config);
        *CONFIG_SET_BY_IPC.lock().unwrap() = true;
    }

    pub fn get() -> HwCodecConfig {
        #[cfg(target_os = "android")]
        {
            let info = crate::android::ffi::get_codec_info();
            log::info!("all codec info: {info:?}");
            struct T {
                name_prefix: &'static str,
                data_format: DataFormat,
            }
            let ts = vec![
                T {
                    name_prefix: "h264",
                    data_format: DataFormat::H264,
                },
                T {
                    name_prefix: "hevc",
                    data_format: DataFormat::H265,
                },
            ];
            let mut e = vec![];
            if let Some(info) = info {
                ts.iter().for_each(|t| {
                    let codecs: Vec<_> = info
                        .codecs
                        .iter()
                        .filter(|c| {
                            c.is_encoder
                                && c.mime_type.as_str() == get_mime_type(t.data_format)
                                && c.nv12
                                && c.hw == Some(true) //only use hardware codec
                        })
                        .collect();
                    let screen_wh = std::cmp::max(info.w, info.h);
                    let mut best = None;
                    if let Some(codec) = codecs
                        .iter()
                        .find(|c| c.max_width >= screen_wh && c.max_height >= screen_wh)
                    {
                        best = Some(codec.name.clone());
                    } else {
                        // find the max resolution
                        let mut max_area = 0;
                        for codec in codecs.iter() {
                            if codec.max_width * codec.max_height > max_area {
                                best = Some(codec.name.clone());
                                max_area = codec.max_width * codec.max_height;
                            }
                        }
                    }
                    if let Some(best) = best {
                        e.push(CodecInfo {
                            name: format!("{}_mediacodec", t.name_prefix),
                            mc_name: Some(best),
                            format: t.data_format,
                            hwdevice: hwcodec::ffmpeg::AVHWDeviceType::AV_HWDEVICE_TYPE_NONE,
                            priority: 0,
                        });
                    }
                });
            }
            log::debug!("e: {e:?}");
            HwCodecConfig {
                ram_encode: e,
                ..Default::default()
            }
        }
        #[cfg(any(windows, target_os = "macos"))]
        {
            let config = CONFIG.lock().unwrap().clone();
            match config {
                Some(c) => c,
                None => {
                    log::info!("try load cached hwcodec config");
                    let c = hbb_common::config::common_load::<HwCodecConfig2>("_hwcodec");
                    let c: HwCodecConfig = serde_json::from_str(&c.config).unwrap_or_default();
                    let new_signature = hwcodec::common::get_gpu_signature();
                    if c.signature == new_signature {
                        log::debug!("load cached hwcodec config: {c:?}");
                        *CONFIG.lock().unwrap() = Some(c.clone());
                        c
                    } else {
                        log::info!(
                            "gpu signature changed, {} -> {}",
                            c.signature,
                            new_signature
                        );
                        HwCodecConfig::default()
                    }
                }
            }
        }
        #[cfg(target_os = "linux")]
        {
            CONFIG.lock().unwrap().clone().unwrap_or_default()
        }
        #[cfg(target_os = "ios")]
        {
            HwCodecConfig::default()
        }
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub fn get_set_value() -> Option<HwCodecConfig> {
        let set = CONFIG_SET_BY_IPC.lock().unwrap().clone();
        if set {
            CONFIG.lock().unwrap().clone()
        } else {
            None
        }
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub fn already_set() -> bool {
        CONFIG_SET_BY_IPC.lock().unwrap().clone()
    }

    pub fn clear(vram: bool, encode: bool) {
        log::info!("clear hwcodec config, vram: {vram}, encode: {encode}");
        #[cfg(target_os = "android")]
        crate::android::ffi::clear_codec_info();
        #[cfg(not(target_os = "android"))]
        {
            let mut c = CONFIG.lock().unwrap();
            if let Some(c) = c.as_mut() {
                if vram {
                    #[cfg(feature = "vram")]
                    if encode {
                        c.vram_encode = vec![];
                    } else {
                        c.vram_decode = vec![];
                    }
                } else {
                    if encode {
                        c.ram_encode = vec![];
                    } else {
                        c.ram_decode = vec![];
                    }
                }
            }
        }
        crate::codec::Encoder::update(crate::codec::EncodingUpdate::Check);
    }
}

pub fn check_available_hwcodec() -> String {
    let ctx = EncodeContext {
        name: String::from(""),
        mc_name: None,
        width: 1280,
        height: 720,
        pixfmt: DEFAULT_PIXFMT,
        align: HW_STRIDE_ALIGN as _,
        kbs: 0,
        fps: DEFAULT_FPS,
        gop: DEFAULT_GOP,
        quality: DEFAULT_HW_QUALITY,
        rc: RC_CBR,
        q: -1,
        thread_count: 4,
    };
    #[cfg(feature = "vram")]
    let vram = crate::vram::check_available_vram();
    #[cfg(feature = "vram")]
    let vram_string = vram.2;
    #[cfg(not(feature = "vram"))]
    let vram_string = "".to_owned();
    let c = HwCodecConfig {
        ram_encode: Encoder::available_encoders(ctx, Some(vram_string.clone())),
        ram_decode: Decoder::available_decoders(Some(vram_string)),
        #[cfg(feature = "vram")]
        vram_encode: vram.0,
        #[cfg(feature = "vram")]
        vram_decode: vram.1,
        signature: hwcodec::common::get_gpu_signature(),
    };
    log::debug!("{c:?}");
    serde_json::to_string(&c).unwrap_or_default()
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn start_check_process() {
    if !enable_hwcodec_option() || HwCodecConfig::already_set() {
        return;
    }
    use hbb_common::allow_err;
    use std::sync::Once;
    let f = || {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(_) = exe.file_name().to_owned() {
                let arg = "--check-hwcodec-config";
                if let Ok(mut child) = std::process::Command::new(exe).arg(arg).spawn() {
                    // wait up to 30 seconds, it maybe slow on windows startup for poorly performing machines
                    for _ in 0..30 {
                        std::thread::sleep(std::time::Duration::from_secs(1));
                        if let Ok(Some(_)) = child.try_wait() {
                            break;
                        }
                    }
                    allow_err!(child.kill());
                    std::thread::sleep(std::time::Duration::from_millis(30));
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            log::info!("Check hwcodec config, exit with: {status}")
                        }
                        Ok(None) => {
                            log::info!(
                                "Check hwcodec config, status not ready yet, let's really wait"
                            );
                            let res = child.wait();
                            log::info!("Check hwcodec config, wait result: {res:?}");
                        }
                        Err(e) => {
                            log::error!("Check hwcodec config, error attempting to wait: {e}")
                        }
                    }
                }
            }
        };
    };
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        std::thread::spawn(f);
    });
}
