use env_logger::{init_from_env, Env, DEFAULT_FILTER_ENV};
use hwcodec::{
    common::{Quality::*, RateControl::*, MAX_GOP},
    ffmpeg::{
        AVHWDeviceType::{self, *},
        AVPixelFormat::*,
    },
    ffmpeg_ram::{
        decode::{DecodeContext, Decoder},
        encode::{EncodeContext, Encoder},
    },
};
use std::{
    fs::File,
    io::{Read, Write},
};

fn main() {
    let gpu = true;
    let h264 = true;
    let hw_type = if gpu { "gpu" } else { "hw" };
    let file_type = if h264 { "h264" } else { "h265" };
    let codec = if h264 { "h264" } else { "hevc" };

    init_from_env(Env::default().filter_or(DEFAULT_FILTER_ENV, "info"));
    let device_type = AV_HWDEVICE_TYPE_CUDA;
    let decode_ctx = DecodeContext {
        name: String::from(codec),
        device_type,
        thread_count: 4,
    };
    let mut video_decoder = Decoder::new(decode_ctx).unwrap();

    decode_encode(
        &mut video_decoder,
        0,
        hw_type,
        file_type,
        1600,
        900,
        h264,
        device_type,
    );
    decode_encode(
        &mut video_decoder,
        1,
        hw_type,
        file_type,
        1440,
        900,
        h264,
        device_type,
    );
}

fn decode_encode(
    video_decoder: &mut Decoder,
    index: usize,
    hw_type: &str,
    file_type: &str,
    width: usize,
    height: usize,
    h264: bool,
    device_type: AVHWDeviceType,
) {
    let input_enc_filename = format!("input/data_and_line/{hw_type}_{width}_{height}.{file_type}");
    let len_filename = format!("input/data_and_line/{hw_type}_{width}_{height}_{file_type}.txt");
    let enc_ctx = EncodeContext {
        name: if h264 {
            "h264_nvenc".to_owned()
        } else {
            "hevc_nvenc".to_owned()
        },
        mc_name: None,
        width: width as _,
        height: height as _,
        pixfmt: if device_type == AV_HWDEVICE_TYPE_NONE {
            AV_PIX_FMT_YUV420P
        } else {
            AV_PIX_FMT_NV12
        },
        align: 0,
        kbs: 1_000,
        fps: 30,
        gop: MAX_GOP as _,
        quality: Quality_Default,
        rc: RC_DEFAULT,
        thread_count: 4,
        q: -1,
    };
    let mut video_encoder = Encoder::new(enc_ctx).unwrap();
    let mut encode_file =
        File::create(format!("output/{hw_type}_{width}_{height}.{file_type}")).unwrap();

    let mut yuv_file =
        File::create(format!("output/{hw_type}_{width}_{height}_decode.yuv")).unwrap();

    let mut file_lens = File::open(len_filename).unwrap();
    let mut file = File::open(input_enc_filename).unwrap();
    let mut file_lens_buf = Vec::new();
    file_lens.read_to_end(&mut file_lens_buf).unwrap();
    let file_lens_str = String::from_utf8_lossy(&file_lens_buf).to_string();
    let lens: Vec<usize> = file_lens_str
        .split(",")
        .filter(|e| !e.is_empty())
        .map(|e| e.parse().unwrap())
        .collect();
    for i in 0..lens.len() {
        let mut buf = vec![0; lens[i]];
        file.read(&mut buf).unwrap();
        let frames = video_decoder.decode(&buf).unwrap();
        println!(
            "file{}, w:{}, h:{}, fmt:{:?}, linesize:{:?}",
            index, frames[0].width, frames[0].height, frames[0].pixfmt, frames[0].linesize
        );
        assert!(frames.len() == 1);
        let mut encode_buf = Vec::new();
        for d in &mut frames[0].data {
            encode_buf.append(d);
        }
        yuv_file.write_all(&encode_buf).unwrap();
        let frames = video_encoder.encode(&encode_buf, 0).unwrap();
        assert_eq!(frames.len(), 1);
        for f in frames {
            encode_file.write_all(&f.data).unwrap();
        }
    }
}
