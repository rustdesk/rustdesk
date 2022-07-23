extern crate docopt;
extern crate quest;
extern crate repng;
extern crate scrap;
extern crate serde;
extern crate webm;

use std::fs::{File, OpenOptions};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::{io, thread};

use docopt::Docopt;
use scrap::codec::{EncoderApi, EncoderCfg};
use webm::mux;
use webm::mux::Track;

use scrap::vpxcodec as vpx_encode;
use scrap::{TraitCapturer, Capturer, Display, STRIDE_ALIGN};

const USAGE: &'static str = "
Simple WebM screen capture.

Usage:
  record-screen <path> [--time=<s>] [--fps=<fps>] [--bv=<kbps>] [--ba=<kbps>] [--codec CODEC]
  record-screen (-h | --help)

Options:
  -h --help      Show this screen.
  --time=<s>     Recording duration in seconds.
  --fps=<fps>    Frames per second [default: 30].
  --bv=<kbps>    Video bitrate in kilobits per second [default: 5000].
  --ba=<kbps>    Audio bitrate in kilobits per second [default: 96].
  --codec CODEC  Configure the codec used. [default: vp9]
                 Valid values: vp8, vp9.
";

#[derive(Debug, serde::Deserialize)]
struct Args {
    arg_path: PathBuf,
    flag_codec: Codec,
    flag_time: Option<u64>,
    flag_fps: u64,
    flag_bv: u32,
}

#[derive(Debug, serde::Deserialize)]
enum Codec {
    Vp8,
    Vp9,
}

fn main() -> io::Result<()> {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());

    let duration = args.flag_time.map(Duration::from_secs);

    let d = Display::primary().unwrap();
    let (width, height) = (d.width() as u32, d.height() as u32);

    // Setup the multiplexer.

    let out = match {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&args.arg_path)
    } {
        Ok(file) => file,
        Err(ref e) if e.kind() == io::ErrorKind::AlreadyExists => {
            if loop {
                quest::ask("Overwrite the existing file? [y/N] ");
                if let Some(b) = quest::yesno(false)? {
                    break b;
                }
            } {
                File::create(&args.arg_path)?
            } else {
                return Ok(());
            }
        }
        Err(e) => return Err(e.into()),
    };

    let mut webm =
        mux::Segment::new(mux::Writer::new(out)).expect("Could not initialize the multiplexer.");

    let (vpx_codec, mux_codec) = match args.flag_codec {
        Codec::Vp8 => (vpx_encode::VpxVideoCodecId::VP8, mux::VideoCodecId::VP8),
        Codec::Vp9 => (vpx_encode::VpxVideoCodecId::VP9, mux::VideoCodecId::VP9),
    };

    let mut vt = webm.add_video_track(width, height, None, mux_codec);

    // Setup the encoder.

    let mut vpx = vpx_encode::VpxEncoder::new(EncoderCfg::VPX(vpx_encode::VpxEncoderConfig {
        width,
        height,
        timebase: [1, 1000],
        bitrate: args.flag_bv,
        codec: vpx_codec,
        num_threads: 0,
    }))
    .unwrap();

    // Start recording.

    let start = Instant::now();
    let stop = Arc::new(AtomicBool::new(false));

    thread::spawn({
        let stop = stop.clone();
        move || {
            let _ = quest::ask("Recording! Press ⏎ to stop.");
            let _ = quest::text();
            stop.store(true, Ordering::Release);
        }
    });

    let spf = Duration::from_nanos(1_000_000_000 / args.flag_fps);

    // Capturer object is expensive, avoiding to create it frequently.
    let mut c = Capturer::new(d, true).unwrap();
    while !stop.load(Ordering::Acquire) {
        let now = Instant::now();
        let time = now - start;

        if Some(true) == duration.map(|d| time > d) {
            break;
        }

        if let Ok(frame) = c.frame(Duration::from_millis(0)) {
            let ms = time.as_secs() * 1000 + time.subsec_millis() as u64;

            for frame in vpx.encode(ms as i64, &frame, STRIDE_ALIGN).unwrap() {
                vt.add_frame(frame.data, frame.pts as u64 * 1_000_000, frame.key);
            }
        }

        let dt = now.elapsed();
        if dt < spf {
            thread::sleep(spf - dt);
        }
    }

    // End things.

    let _ = webm.finalize(None);

    Ok(())
}
