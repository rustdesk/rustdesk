use env_logger::{init_from_env, Env, DEFAULT_FILTER_ENV};
use hwcodec::{
    common::{Quality::*, RateControl::*},
    ffmpeg::{AVHWDeviceType::*, AVPixelFormat::*},
    ffmpeg_ram::{
        decode::{DecodeContext, Decoder},
        encode::{EncodeContext, Encoder},
        ffmpeg_linesize_offset_length,
    },
};
use std::{
    fs::File,
    io::{Read, Write},
};

fn main() {
    init_from_env(Env::default().filter_or(DEFAULT_FILTER_ENV, "info"));

    let encode_ctx = EncodeContext {
        name: String::from("h264_nvenc"),
        mc_name: None,
        width: 1920,
        height: 1080,
        pixfmt: AV_PIX_FMT_NV12,
        align: 0,
        kbs: 0,
        fps: 30,
        gop: 60,
        quality: Quality_Default,
        rc: RC_DEFAULT,
        thread_count: 4,
        q: -1,
    };
    let decode_ctx = DecodeContext {
        name: String::from("hevc"),
        device_type: AV_HWDEVICE_TYPE_D3D11VA,
        thread_count: 4,
    };
    let _ = std::thread::spawn(move || test_encode_decode(encode_ctx, decode_ctx)).join();
}

fn test_encode_decode(encode_ctx: EncodeContext, decode_ctx: DecodeContext) {
    let size: usize;
    if let Ok((_, _, len)) = ffmpeg_linesize_offset_length(
        encode_ctx.pixfmt,
        encode_ctx.width as _,
        encode_ctx.height as _,
        encode_ctx.align as _,
    ) {
        size = len as _;
    } else {
        return;
    }

    let mut video_encoder = Encoder::new(encode_ctx).unwrap();
    let mut video_decoder = Decoder::new(decode_ctx).unwrap();

    let mut yuv_file = File::open("input/1920_1080_decoded.yuv").unwrap();
    let mut encode_file = File::create("output/1920_1080.265").unwrap();
    let mut decode_file = File::create("output/1920_1080_decode.yuv").unwrap();

    let mut buf = vec![0; size + 64];
    let mut encode_sum = 0;
    let mut decode_sum = 0;
    let mut encode_size = 0;
    let mut counter = 0;

    let mut f = |data: &[u8]| {
        let now = std::time::Instant::now();
        if let Ok(encode_frames) = video_encoder.encode(data, 0) {
            log::info!("encode:{:?}", now.elapsed());
            encode_sum += now.elapsed().as_micros();
            for encode_frame in encode_frames.iter() {
                encode_size += encode_frame.data.len();
                encode_file.write_all(&encode_frame.data).unwrap();
                encode_file.flush().unwrap();

                let now = std::time::Instant::now();
                if let Ok(docode_frames) = video_decoder.decode(&encode_frame.data) {
                    log::info!("decode:{:?}", now.elapsed());
                    decode_sum += now.elapsed().as_micros();
                    counter += 1;
                    for decode_frame in docode_frames {
                        log::info!("decode_frame:{}", decode_frame);
                        for data in decode_frame.data.iter() {
                            decode_file.write_all(data).unwrap();
                            decode_file.flush().unwrap();
                        }
                    }
                }
            }
        }
    };

    loop {
        match yuv_file.read(&mut buf[..size]) {
            Ok(n) => {
                if n > 0 {
                    f(&buf[..n]);
                } else {
                    break;
                }
            }
            Err(e) => {
                log::info!("{:?}", e);
                break;
            }
        }
    }
    log::info!(
        "counter:{}, encode_avg:{}us, decode_avg:{}us, size_avg:{}",
        counter,
        encode_sum / counter,
        decode_sum / counter,
        encode_size / counter as usize,
    );
}
