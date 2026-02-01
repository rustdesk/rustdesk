use capture::dxgi;
use env_logger::{init_from_env, Env, DEFAULT_FILTER_ENV};
use hwcodec::common::{DataFormat, Driver, MAX_GOP};
use hwcodec::vram::{
    decode::Decoder, encode::Encoder, DecodeContext, DynamicContext, EncodeContext, FeatureContext,
};
use render::Render;
use std::{
    io::Write,
    path::PathBuf,
    time::{Duration, Instant},
};

fn main() {
    init_from_env(Env::default().filter_or(DEFAULT_FILTER_ENV, "trace"));
    let luid = 69524; // 63444; // 59677
    unsafe {
        // one luid create render failed on my pc, wouldn't happen in rustdesk
        let data_format = DataFormat::H265;
        let mut capturer = dxgi::Capturer::new(luid).unwrap();
        let mut render = Render::new(luid, false).unwrap();

        let en_ctx = EncodeContext {
            f: FeatureContext {
                driver: Driver::FFMPEG,
                vendor: Driver::NV,
                data_format,
                luid,
            },
            d: DynamicContext {
                device: Some(capturer.device()),
                width: capturer.width(),
                height: capturer.height(),
                kbitrate: 5000,
                framerate: 30,
                gop: MAX_GOP as _,
            },
        };
        let de_ctx = DecodeContext {
            device: Some(render.device()),
            driver: Driver::FFMPEG,
            vendor: Driver::NV,
            data_format,
            luid,
        };

        let mut dec = Decoder::new(de_ctx).unwrap();
        let mut enc = Encoder::new(en_ctx).unwrap();
        let filename = PathBuf::from(".\\1.264");
        let mut file = std::fs::File::create(filename).unwrap();
        let mut dup_sum = Duration::ZERO;
        let mut enc_sum = Duration::ZERO;
        let mut dec_sum = Duration::ZERO;
        let mut pts_instant = Instant::now();
        loop {
            let start = Instant::now();
            let texture = capturer.capture(100);
            if texture.is_null() {
                continue;
            }
            dup_sum += start.elapsed();
            let start = Instant::now();
            let frame = enc
                .encode(texture, pts_instant.elapsed().as_millis() as _)
                .unwrap();
            enc_sum += start.elapsed();
            for f in frame {
                file.write_all(&mut f.data).unwrap();
                let start = Instant::now();
                let frames = dec.decode(&f.data).unwrap();
                dec_sum += start.elapsed();
                for f in frames {
                    render.render(f.texture).unwrap();
                }
            }
        }
    }
}
