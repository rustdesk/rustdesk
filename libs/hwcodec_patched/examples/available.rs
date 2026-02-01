use env_logger::{init_from_env, Env, DEFAULT_FILTER_ENV};
use hwcodec::{
    common::{get_gpu_signature, Quality::*, RateControl::*},
    ffmpeg::AVPixelFormat,
    ffmpeg_ram::{
        decode::Decoder,
        encode::{EncodeContext, Encoder},
    },
};

fn main() {
    let start = std::time::Instant::now();
    init_from_env(Env::default().filter_or(DEFAULT_FILTER_ENV, "info"));
    ram();
    #[cfg(feature = "vram")]
    vram();
    log::info!(
        "signature: {:?}, elapsed: {:?}",
        get_gpu_signature(),
        start.elapsed()
    );
}

fn ram() {
    println!("ram:");
    println!("encoders:");
    let ctx = EncodeContext {
        name: String::from(""),
        mc_name: None,
        width: 1280,
        height: 720,
        pixfmt: AVPixelFormat::AV_PIX_FMT_NV12,
        align: 0,
        kbs: 1000,
        fps: 30,
        gop: i32::MAX,
        quality: Quality_Default,
        rc: RC_CBR,
        q: -1,
        thread_count: 1,
    };
    let encoders = Encoder::available_encoders(ctx.clone(), None);
    encoders.iter().map(|e| println!("{:?}", e)).count();
    println!("decoders:");
    let decoders = Decoder::available_decoders();
    decoders.iter().map(|e| println!("{:?}", e)).count();
}

#[cfg(feature = "vram")]
fn vram() {
    use hwcodec::common::MAX_GOP;
    use hwcodec::vram::{decode, encode, DynamicContext};
    println!("vram:");
    println!("encoders:");
    let encoders = encode::available(DynamicContext {
        width: 1920,
        height: 1080,
        kbitrate: 5000,
        framerate: 30,
        gop: MAX_GOP as _,
        device: None,
    });
    encoders.iter().map(|e| println!("{:?}", e)).count();
    println!("decoders:");
    let decoders = decode::available();
    decoders.iter().map(|e| println!("{:?}", e)).count();
}
