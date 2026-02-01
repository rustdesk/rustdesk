use env_logger::{init_from_env, Env, DEFAULT_FILTER_ENV};
use hwcodec::{
    common::{Quality::*, RateControl::*},
    ffmpeg::AVPixelFormat,
    ffmpeg_ram::{
        decode::{DecodeContext, Decoder},
        encode::{EncodeContext, Encoder},
        CodecInfo, CodecInfos,
    },
};
use rand::random;
use std::io::Write;
use std::time::Instant;

fn main() {
    init_from_env(Env::default().filter_or(DEFAULT_FILTER_ENV, "info"));

    let ctx = EncodeContext {
        name: String::from(""),
        mc_name: None,
        width: 1920,
        height: 1080,
        pixfmt: AVPixelFormat::AV_PIX_FMT_NV12,
        align: 0,
        kbs: 5000,
        fps: 30,
        gop: 60,
        quality: Quality_Default,
        rc: RC_DEFAULT,
        thread_count: 4,
        q: -1,
    };
    let yuv_count = 10;
    println!("benchmark");
    let yuvs = prepare_yuv(ctx.width as _, ctx.height as _, yuv_count);

    let encoders = Encoder::available_encoders(ctx.clone(), None);
    log::info!("encoders: {:?}", encoders);
    let best = CodecInfo::prioritized(encoders.clone());
    for info in encoders {
        test_encoder(info.clone(), ctx.clone(), &yuvs, is_best(&best, &info));
    }

    let (h264s, h265s) = prepare_h26x(best, ctx.clone(), &yuvs);

    let decoders = Decoder::available_decoders();
    log::info!("decoders: {:?}", decoders);
    let best = CodecInfo::prioritized(decoders.clone());
    for info in decoders {
        let h26xs = if info.name.contains("h264") {
            &h264s
        } else {
            &h265s
        };
        if h26xs.len() == yuv_count {
            test_decoder(info.clone(), h26xs, is_best(&best, &info));
        }
    }
}

fn test_encoder(info: CodecInfo, ctx: EncodeContext, yuvs: &Vec<Vec<u8>>, best: bool) {
    let mut ctx = ctx;
    ctx.name = info.name;
    let mut encoder = Encoder::new(ctx.clone()).unwrap();
    let start = Instant::now();
    for yuv in yuvs {
        let _ = encoder
            .encode(yuv, start.elapsed().as_millis() as _)
            .unwrap();
    }
    println!(
        "{}{}: {:?}",
        if best { "*" } else { "" },
        ctx.name,
        start.elapsed() / yuvs.len() as _
    );
}

fn test_decoder(info: CodecInfo, h26xs: &Vec<Vec<u8>>, best: bool) {
    let ctx = DecodeContext {
        name: info.name,
        device_type: info.hwdevice,
        thread_count: 4,
    };

    let mut decoder = Decoder::new(ctx.clone()).unwrap();
    let start = Instant::now();
    let mut cnt = 0;
    for h26x in h26xs {
        let _ = decoder.decode(h26x).unwrap();
        cnt += 1;
    }
    let device = format!("{:?}", ctx.device_type).to_lowercase();
    let device = device.split("_").last().unwrap();
    println!(
        "{}{} {}: {:?}",
        if best { "*" } else { "" },
        ctx.name,
        device,
        start.elapsed() / cnt
    );
}

fn prepare_yuv(width: usize, height: usize, count: usize) -> Vec<Vec<u8>> {
    let mut ret = vec![];
    for index in 0..count {
        let linesize = width * 3 / 2;
        let mut yuv = vec![0u8; linesize * height];
        for y in 0..height {
            for x in 0..linesize {
                yuv[linesize * y + x] = random();
            }
        }
        ret.push(yuv);
        print!("\rprepare {}/{}", index + 1, count);
        std::io::stdout().flush().ok();
    }
    println!();
    ret
}

fn prepare_h26x(
    best: CodecInfos,
    ctx: EncodeContext,
    yuvs: &Vec<Vec<u8>>,
) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
    let f = |info: Option<CodecInfo>| {
        let mut h26xs = vec![];
        if let Some(info) = info {
            let mut ctx = ctx.clone();
            ctx.name = info.name;
            let mut encoder = Encoder::new(ctx).unwrap();
            for yuv in yuvs {
                let h26x = encoder.encode(yuv, 0).unwrap();
                for frame in h26x {
                    h26xs.push(frame.data.to_vec());
                }
            }
        }
        h26xs
    };
    (f(best.h264), f(best.h265))
}

fn is_best(best: &CodecInfos, info: &CodecInfo) -> bool {
    Some(info.clone()) == best.h264 || Some(info.clone()) == best.h265
}
