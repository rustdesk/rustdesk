use env_logger::{init_from_env, Env, DEFAULT_FILTER_ENV};
#[cfg(feature = "vram")]
use hwcodec::{
    common::MAX_GOP,
    vram::{DynamicContext, FeatureContext},
};
use hwcodec::{
    common::{DataFormat, Quality::*, RateControl::*},
    ffmpeg::AVPixelFormat::*,
    ffmpeg_ram::{
        decode::{DecodeContext, Decoder},
        encode::{EncodeContext, Encoder},
        ffmpeg_linesize_offset_length, CodecInfo,
    },
};
#[cfg(feature = "vram")]
use tool::Tool;

fn main() {
    init_from_env(Env::default().filter_or(DEFAULT_FILTER_ENV, "info"));
    let max_align = 16;
    setup_ram(max_align);
    #[cfg(feature = "vram")]
    setup_vram(max_align);
}

fn setup_ram(max_align: i32) {
    let encoders = Encoder::available_encoders(
        EncodeContext {
            name: String::from(""),
            mc_name: None,
            width: 1920,
            height: 1080,
            pixfmt: AV_PIX_FMT_NV12,
            align: 0,
            fps: 30,
            gop: 60,
            rc: RC_CBR,
            quality: Quality_Default,
            kbs: 0,
            q: -1,
            thread_count: 1,
        },
        None,
    );
    let decoders = Decoder::available_decoders();
    let h264_encoders = encoders
        .iter()
        .filter(|info| info.name.contains("h264"))
        .cloned()
        .collect::<Vec<_>>();
    let h265_encoders = encoders
        .iter()
        .filter(|info| info.name.contains("hevc"))
        .cloned()
        .collect::<Vec<_>>();
    let h264_decoders = decoders
        .iter()
        .filter(|info| info.format == DataFormat::H264)
        .cloned()
        .collect::<Vec<_>>();
    let h265_decoders = decoders
        .iter()
        .filter(|info| info.format == DataFormat::H265)
        .cloned()
        .collect::<Vec<_>>();

    let start_width = 1920;
    let start_height = 1080;
    let step = 2;

    for width in (start_width..=(start_width + max_align)).step_by(step) {
        for height in (start_height..=(start_height + max_align)).step_by(step) {
            for encode_info in &h264_encoders {
                test_ram(width, height, encode_info.clone(), h264_decoders[0].clone());
            }
            for decode_info in &h264_decoders {
                test_ram(width, height, h264_encoders[0].clone(), decode_info.clone());
            }
            for encode_info in &h265_encoders {
                test_ram(width, height, encode_info.clone(), h265_decoders[0].clone());
            }
            for decode_info in &h265_decoders {
                test_ram(width, height, h265_encoders[0].clone(), decode_info.clone());
            }
        }
    }
}

fn test_ram(width: i32, height: i32, encode_info: CodecInfo, decode_info: CodecInfo) {
    println!(
        "Test {}x{}: {} -> {}",
        width, height, encode_info.name, decode_info.name
    );
    let encode_ctx = EncodeContext {
        name: encode_info.name.clone(),
        mc_name: None,
        width,
        height,
        pixfmt: AV_PIX_FMT_NV12,
        align: 0,
        kbs: 0,
        fps: 30,
        gop: 60,
        quality: Quality_Default,
        rc: RC_CBR,
        thread_count: 1,
        q: -1,
    };
    let decode_ctx = DecodeContext {
        name: decode_info.name.clone(),
        device_type: decode_info.hwdevice,
        thread_count: 4,
    };
    let (_, _, len) = ffmpeg_linesize_offset_length(
        encode_ctx.pixfmt,
        encode_ctx.width as _,
        encode_ctx.height as _,
        encode_ctx.align as _,
    )
    .unwrap();
    let mut video_encoder = Encoder::new(encode_ctx).unwrap();
    let mut video_decoder = Decoder::new(decode_ctx).unwrap();
    let buf: Vec<u8> = vec![0; len as usize];
    let encode_frames = video_encoder.encode(&buf, 0).unwrap();
    assert_eq!(encode_frames.len(), 1);
    let docode_frames = video_decoder.decode(&encode_frames[0].data).unwrap();
    assert_eq!(docode_frames.len(), 1);
    assert_eq!(docode_frames[0].width, width);
    assert_eq!(docode_frames[0].height, height);
    println!(
        "Pass {}x{}: {} -> {} {:?}",
        width, height, encode_info.name, decode_info.name, decode_info.hwdevice
    )
}

#[cfg(feature = "vram")]
fn setup_vram(max_align: i32) {
    let encoders = hwcodec::vram::encode::available(DynamicContext {
        device: None,
        width: 1920,
        height: 1080,
        kbitrate: 1000,
        framerate: 30,
        gop: MAX_GOP as _,
    });
    let decoders = hwcodec::vram::decode::available();

    let start_width = 1920;
    let start_height = 1080;
    let step = 2;

    for width in (start_width..=(start_width + max_align)).step_by(step) {
        for height in (start_height..=(start_height + max_align)).step_by(step) {
            for encode_info in &encoders {
                if let Some(decoder) = decoders.iter().find(|d| {
                    d.luid == encode_info.luid && d.data_format == encode_info.data_format
                }) {
                    test_vram(width, height, encode_info.clone(), decoder.clone());
                }
            }
            for decode_info in &decoders {
                if let Some(encoder) = encoders.iter().find(|e| {
                    e.luid == decode_info.luid && e.data_format == decode_info.data_format
                }) {
                    test_vram(width, height, encoder.clone(), decode_info.clone());
                }
            }
        }
    }
}

#[cfg(feature = "vram")]
fn test_vram(
    width: i32,
    height: i32,
    encode_info: FeatureContext,
    decode_info: hwcodec::vram::DecodeContext,
) {
    println!(
        "Test {}x{}: {:?} {:?} -> {:?}",
        width, height, encode_info.data_format, encode_info.driver, decode_info.driver
    );

    let mut tool = Tool::new(encode_info.luid).unwrap();
    let encode_ctx = hwcodec::vram::EncodeContext {
        f: encode_info.clone(),
        d: hwcodec::vram::DynamicContext {
            device: Some(tool.device()),
            width,
            height,
            kbitrate: 1000,
            framerate: 30,
            gop: MAX_GOP as _,
        },
    };
    let mut encoder = hwcodec::vram::encode::Encoder::new(encode_ctx).unwrap();
    let mut decoder = hwcodec::vram::decode::Decoder::new(hwcodec::vram::DecodeContext {
        device: Some(tool.device()),
        ..decode_info.clone()
    })
    .unwrap();
    let encode_frames = encoder.encode(tool.get_texture(width, height), 0).unwrap();
    assert_eq!(encode_frames.len(), 1);
    let decoder_frames = decoder.decode(&encode_frames[0].data).unwrap();
    assert_eq!(decoder_frames.len(), 1);
    let (decoded_width, decoded_height) = tool.get_texture_size(decoder_frames[0].texture);
    assert_eq!(decoded_width, width);
    assert_eq!(decoded_height, height);
    println!(
        "Pass {}x{}: {:?} {:?} -> {:?}",
        width, height, encode_info.data_format, encode_info.driver, decode_info.driver
    );
}
