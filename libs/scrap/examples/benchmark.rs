use docopt::Docopt;
use hbb_common::{
    env_logger::{init_from_env, Env, DEFAULT_FILTER_ENV},
    log,
};
use scrap::{
    aom::{AomDecoder, AomEncoder, AomEncoderConfig},
    codec::{EncoderApi, EncoderCfg},
    Capturer, Display, TraitCapturer, VpxDecoder, VpxDecoderConfig, VpxEncoder, VpxEncoderConfig,
    VpxVideoCodecId::{self, *},
    STRIDE_ALIGN,
};
use std::{
    io::Write,
    time::{Duration, Instant},
};

// cargo run --package scrap --example benchmark --release --features hwcodec

const USAGE: &'static str = "
Codec benchmark.

Usage:
  benchmark [--count=COUNT] [--quality=QUALITY] [--i444]
  benchmark (-h | --help)

Options:
  -h --help             Show this screen.
  --count=COUNT         Capture frame count [default: 100].
  --quality=QUALITY     Video quality [default: 1.0].
  --i444                I444.
";

#[derive(Debug, serde::Deserialize, Clone, Copy)]
struct Args {
    flag_count: usize,
    flag_quality: f32,
    flag_i444: bool,
}

fn main() {
    init_from_env(Env::default().filter_or(DEFAULT_FILTER_ENV, "info"));
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());
    let quality = args.flag_quality;
    let yuv_count = args.flag_count;
    let mut index = 0;
    let mut displays = Display::all().unwrap();
    for i in 0..displays.len() {
        if displays[i].is_primary() {
            index = i;
            break;
        }
    }
    let d = displays.remove(index);
    let mut c = Capturer::new(d).unwrap();
    let width = c.width();
    let height = c.height();

    println!(
        "benchmark {}x{} quality:{:?}, i444:{:?}",
        width, height, quality, args.flag_i444
    );
    [VP8, VP9].map(|codec| {
        test_vpx(
            &mut c,
            codec,
            width,
            height,
            quality,
            yuv_count,
            if codec == VP8 { false } else { args.flag_i444 },
        )
    });
    test_av1(&mut c, width, height, quality, yuv_count, args.flag_i444);
    #[cfg(feature = "hwcodec")]
    {
        hw::test(&mut c, width, height, quality, yuv_count);
    }
}

fn test_vpx(
    c: &mut Capturer,
    codec_id: VpxVideoCodecId,
    width: usize,
    height: usize,
    quality: f32,
    yuv_count: usize,
    i444: bool,
) {
    let config = EncoderCfg::VPX(VpxEncoderConfig {
        width: width as _,
        height: height as _,
        quality,
        codec: codec_id,
        keyframe_interval: None,
    });
    let mut encoder = VpxEncoder::new(config, i444).unwrap();
    let mut vpxs = vec![];
    let start = Instant::now();
    let mut size = 0;
    let mut yuv = Vec::new();
    let mut mid_data = Vec::new();
    let mut counter = 0;
    let mut time_sum = Duration::ZERO;
    loop {
        match c.frame(std::time::Duration::from_millis(30)) {
            Ok(frame) => {
                let tmp_timer = Instant::now();
                let frame = frame.to(encoder.yuvfmt(), &mut yuv, &mut mid_data).unwrap();
                let yuv = frame.yuv().unwrap();
                for ref frame in encoder
                    .encode(start.elapsed().as_millis() as _, &yuv, STRIDE_ALIGN)
                    .unwrap()
                {
                    size += frame.data.len();
                    vpxs.push(frame.data.to_vec());
                    counter += 1;
                    print!("\r{codec_id:?} {}/{}", counter, yuv_count);
                    std::io::stdout().flush().ok();
                }
                for ref frame in encoder.flush().unwrap() {
                    size += frame.data.len();
                    vpxs.push(frame.data.to_vec());
                    counter += 1;
                    print!("\r{codec_id:?} {}/{}", counter, yuv_count);
                    std::io::stdout().flush().ok();
                }
                time_sum += tmp_timer.elapsed();
            }
            Err(e) => {
                log::error!("{e:?}");
            }
        }
        if counter >= yuv_count {
            println!();
            break;
        }
    }

    assert_eq!(vpxs.len(), yuv_count);
    println!(
        "{:?} encode: {:?}, {} byte",
        codec_id,
        time_sum / yuv_count as _,
        size / yuv_count
    );

    let mut decoder = VpxDecoder::new(VpxDecoderConfig { codec: codec_id }).unwrap();
    let start = Instant::now();
    for vpx in vpxs {
        let _ = decoder.decode(&vpx);
        let _ = decoder.flush();
    }
    println!(
        "{:?} decode: {:?}",
        codec_id,
        start.elapsed() / yuv_count as _
    );
}

fn test_av1(
    c: &mut Capturer,
    width: usize,
    height: usize,
    quality: f32,
    yuv_count: usize,
    i444: bool,
) {
    let config = EncoderCfg::AOM(AomEncoderConfig {
        width: width as _,
        height: height as _,
        quality,
        keyframe_interval: None,
    });
    let mut encoder = AomEncoder::new(config, i444).unwrap();
    let start = Instant::now();
    let mut size = 0;
    let mut av1s: Vec<Vec<u8>> = vec![];
    let mut yuv = Vec::new();
    let mut mid_data = Vec::new();
    let mut counter = 0;
    let mut time_sum = Duration::ZERO;
    loop {
        match c.frame(std::time::Duration::from_millis(30)) {
            Ok(frame) => {
                let tmp_timer = Instant::now();
                let frame = frame.to(encoder.yuvfmt(), &mut yuv, &mut mid_data).unwrap();
                let yuv = frame.yuv().unwrap();
                for ref frame in encoder
                    .encode(start.elapsed().as_millis() as _, &yuv, STRIDE_ALIGN)
                    .unwrap()
                {
                    size += frame.data.len();
                    av1s.push(frame.data.to_vec());
                    counter += 1;
                    print!("\rAV1 {}/{}", counter, yuv_count);
                    std::io::stdout().flush().ok();
                }
                time_sum += tmp_timer.elapsed();
            }
            Err(e) => {
                log::error!("{e:?}");
            }
        }
        if counter >= yuv_count {
            println!();
            break;
        }
    }
    assert_eq!(av1s.len(), yuv_count);
    println!(
        "AV1 encode: {:?}, {} byte",
        time_sum / yuv_count as _,
        size / yuv_count
    );
    let mut decoder = AomDecoder::new().unwrap();
    let start = Instant::now();
    for av1 in av1s {
        let _ = decoder.decode(&av1);
        let _ = decoder.flush();
    }
    println!("AV1 decode: {:?}", start.elapsed() / yuv_count as _);
}

#[cfg(feature = "hwcodec")]
mod hw {
    use hwcodec::ffmpeg_ram::CodecInfo;
    use scrap::{
        hwcodec::{HwRamDecoder, HwRamEncoder, HwRamEncoderConfig},
        CodecFormat,
    };

    use super::*;

    pub fn test(c: &mut Capturer, width: usize, height: usize, quality: f32, yuv_count: usize) {
        let mut h264s = Vec::new();
        let mut h265s = Vec::new();
        if let Some(info) = HwRamEncoder::try_get(CodecFormat::H264) {
            test_encoder(width, height, quality, info, c, yuv_count, &mut h264s);
        }
        if let Some(info) = HwRamEncoder::try_get(CodecFormat::H265) {
            test_encoder(width, height, quality, info, c, yuv_count, &mut h265s);
        }
        test_decoder(CodecFormat::H264, &h264s);
        test_decoder(CodecFormat::H265, &h265s);
    }

    fn test_encoder(
        width: usize,
        height: usize,
        quality: f32,
        info: CodecInfo,
        c: &mut Capturer,
        yuv_count: usize,
        h26xs: &mut Vec<Vec<u8>>,
    ) {
        let mut encoder = HwRamEncoder::new(
            EncoderCfg::HWRAM(HwRamEncoderConfig {
                name: info.name.clone(),
                mc_name: None,
                width,
                height,
                quality,
                keyframe_interval: None,
            }),
            false,
        )
        .unwrap();
        let mut size = 0;

        let mut yuv = Vec::new();
        let mut mid_data = Vec::new();
        let mut counter = 0;
        let mut time_sum = Duration::ZERO;
        let start = std::time::Instant::now();
        loop {
            match c.frame(std::time::Duration::from_millis(30)) {
                Ok(frame) => {
                    let tmp_timer = Instant::now();
                    let frame = frame.to(encoder.yuvfmt(), &mut yuv, &mut mid_data).unwrap();
                    let yuv = frame.yuv().unwrap();
                    for ref frame in encoder
                        .encode(&yuv, start.elapsed().as_millis() as _)
                        .unwrap()
                    {
                        size += frame.data.len();

                        h26xs.push(frame.data.to_vec());
                        counter += 1;
                        print!("\r{:?} {}/{}", info.name, counter, yuv_count);
                        std::io::stdout().flush().ok();
                    }
                    time_sum += tmp_timer.elapsed();
                }
                Err(e) => {
                    log::error!("{e:?}");
                }
            }
            if counter >= yuv_count {
                println!();
                break;
            }
        }
        println!(
            "{}: {:?}, {} byte",
            info.name,
            time_sum / yuv_count as u32,
            size / yuv_count,
        );
    }

    fn test_decoder(format: CodecFormat, h26xs: &Vec<Vec<u8>>) {
        let mut decoder = HwRamDecoder::new(format).unwrap();
        let start = Instant::now();
        let mut cnt = 0;
        for h26x in h26xs {
            let _ = decoder.decode(h26x).unwrap();
            cnt += 1;
        }
        let device = format!("{:?}", decoder.info.hwdevice).to_lowercase();
        let device = device.split("_").last().unwrap();
        println!(
            "{} {}: {:?}",
            decoder.info.name,
            device,
            start.elapsed() / cnt
        );
    }
}
