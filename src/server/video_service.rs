// 24FPS (actually 23.976FPS) is what video professionals ages ago determined to be the
// slowest playback rate that still looks smooth enough to feel real.
// Our eyes can see a slight difference and even though 30FPS actually shows
// more information and is more realistic.
// 60FPS is commonly used in game, teamviewer 12 support this for video editing user.

// how to capture with mouse cursor:
// https://docs.microsoft.com/zh-cn/windows/win32/direct3ddxgi/desktop-dup-api?redirectedfrom=MSDN

// 实现了硬件编解码和音频抓取，还绘制了鼠标
// https://github.com/PHZ76/DesktopSharing

// dxgi memory leak issue
// https://stackoverflow.com/questions/47801238/memory-leak-in-creating-direct2d-device
// but per my test, it is more related to AcquireNextFrame,
// https://forums.developer.nvidia.com/t/dxgi-outputduplication-memory-leak-when-using-nv-but-not-amd-drivers/108582

// to-do:
// https://slhck.info/video/2017/03/01/rate-control.html

use super::*;
use scrap::{Capturer, Config, Display, EncodeFrame, Encoder, VideoCodecId, STRIDE_ALIGN};
use std::{
    io::ErrorKind::WouldBlock,
    time::{self, Instant},
};

const WAIT_BASE: i32 = 17;
pub const NAME: &'static str = "video";

lazy_static::lazy_static! {
    static ref CURRENT_DISPLAY: Arc<Mutex<usize>> = Arc::new(Mutex::new(usize::MAX));
    static ref LAST_ACTIVE: Arc<Mutex<Instant>> = Arc::new(Mutex::new(Instant::now()));
    static ref SWITCH: Arc<Mutex<bool>> = Default::default();
    static ref INTERNAL_LATENCIES: Arc<Mutex<HashMap<i32, i64>>> = Default::default();
    static ref TEST_LATENCIES: Arc<Mutex<HashMap<i32, i64>>> = Default::default();
    static ref IMAGE_QUALITIES: Arc<Mutex<HashMap<i32, i32>>> = Default::default();
}

pub fn new() -> GenericService {
    let sp = GenericService::new(NAME, true);
    sp.run(run);
    sp
}

fn run(sp: GenericService) -> ResultType<()> {
    let fps = 30;
    let spf = time::Duration::from_secs_f32(1. / (fps as f32));
    let (ndisplay, current, display) = get_current_display()?;
    let (origin, width, height) = (display.origin(), display.width(), display.height());
    log::debug!(
        "#displays={}, current={}, origin: {:?}, width={}, height={}",
        ndisplay,
        current,
        &origin,
        width,
        height
    );
    // Capturer object is expensive, avoiding to create it frequently.
    let mut c = Capturer::new(display, true).with_context(|| "Failed to create capturer")?;

    let q = get_image_quality();
    let (bitrate, rc_min_quantizer, rc_max_quantizer, speed) = get_quality(width, height, q);
    log::info!("bitrate={}, rc_min_quantizer={}", bitrate, rc_min_quantizer);
    let mut wait = WAIT_BASE;
    let cfg = Config {
        width: width as _,
        height: height as _,
        timebase: [1, 1000], // Output timestamp precision
        bitrate,
        codec: VideoCodecId::VP9,
        rc_min_quantizer,
        rc_max_quantizer,
        speed,
    };
    let mut vpx;
    let mut n = ((width * height) as f64 / (1920 * 1080) as f64).round() as u32;
    if n < 1 {
        n = 1;
    }
    match Encoder::new(&cfg, n) {
        Ok(x) => vpx = x,
        Err(err) => bail!("Failed to create encoder: {}", err),
    }

    if *SWITCH.lock().unwrap() {
        log::debug!("Broadcasting display switch");
        let mut misc = Misc::new();
        misc.set_switch_display(SwitchDisplay {
            display: current as _,
            x: origin.0 as _,
            y: origin.1 as _,
            width: width as _,
            height: height as _,
            ..Default::default()
        });
        let mut msg_out = Message::new();
        msg_out.set_misc(misc);
        *SWITCH.lock().unwrap() = false;
        sp.send(msg_out);
    }
    
    let mut crc = (0, 0);
    let start = time::Instant::now();
    let mut last_sent = time::Instant::now();
    let mut last_check_displays = time::Instant::now();
    #[cfg(windows)]
    let mut try_gdi = true;
    #[cfg(windows)]
    log::info!("gdi: {}", c.is_gdi());
    while sp.ok() {
        if *SWITCH.lock().unwrap() {
            bail!("SWITCH");
        }
        if current != *CURRENT_DISPLAY.lock().unwrap() {
            *SWITCH.lock().unwrap() = true;
            bail!("SWITCH");
        }
        if get_image_quality() != q {
            bail!("SWITCH");
        }
        #[cfg(windows)]
        {
            if crate::platform::windows::desktop_changed() {
                bail!("Desktop changed");
            }
        }
        let now = time::Instant::now();
        if last_check_displays.elapsed().as_millis() > 1000 {
            last_check_displays = now;
            if ndisplay != get_display_num() {
                log::info!("Displays changed");
                *SWITCH.lock().unwrap() = true;
                bail!("SWITCH");
            }
        }
        *LAST_ACTIVE.lock().unwrap() = now;
        if get_latency() < 1000 || last_sent.elapsed().as_millis() > 1000 {
            match c.frame(wait as _) {
                Ok(frame) => {
                    let time = now - start;
                    let ms = (time.as_secs() * 1000 + time.subsec_millis() as u64) as i64;
                    handle_one_frame(&sp, &frame, ms, &mut crc, &mut vpx)?;
                    last_sent = now;
                    #[cfg(windows)]
                    {
                        try_gdi = false;
                    }
                }
                Err(ref e) if e.kind() == WouldBlock => {
                    // https://github.com/NVIDIA/video-sdk-samples/tree/master/nvEncDXGIOutputDuplicationSample
                    wait = WAIT_BASE - now.elapsed().as_millis() as i32;
                    if wait < 0 {
                        wait = 0
                    }
                    #[cfg(windows)]
                    if try_gdi && !c.is_gdi() {
                        c.set_gdi();
                        try_gdi = false;
                        log::info!("No image, fall back to gdi");
                    }
                    continue;
                }
                Err(err) => {
                    return Err(err.into());
                }
            }
        }
        let elapsed = now.elapsed();
        // may need to enable frame(timeout)
        log::trace!("{:?} {:?}", time::Instant::now(), elapsed);
        if elapsed < spf {
            std::thread::sleep(spf - elapsed);
        }
    }
    Ok(())
}

#[inline]
fn create_msg(vp9s: Vec<VP9>) -> Message {
    let mut msg_out = Message::new();
    let mut vf = VideoFrame::new();
    vf.set_vp9s(VP9s {
        frames: vp9s.into(),
        ..Default::default()
    });
    msg_out.set_video_frame(vf);
    msg_out
}

#[inline]
fn create_frame(frame: &EncodeFrame) -> VP9 {
    VP9 {
        data: frame.data.to_vec(),
        key: frame.key,
        pts: frame.pts,
        ..Default::default()
    }
}

#[inline]
fn handle_one_frame(
    sp: &GenericService,
    frame: &[u8],
    ms: i64,
    crc: &mut (u32, u32),
    vpx: &mut Encoder,
) -> ResultType<()> {
    sp.snapshot(|sps| {
        // so that new sub and old sub share the same encoder after switch
        if sps.has_subscribes() {
            bail!("SWITCH");
        }
        Ok(())
    })?;
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(frame);
    let checksum = hasher.finalize();
    if checksum != crc.0 {
        crc.0 = checksum;
        crc.1 = 0;
    } else {
        crc.1 += 1;
    }
    if crc.1 <= 180 && crc.1 % 5 == 0 {
        let mut frames = Vec::new();
        for ref frame in vpx
            .encode(ms, frame, STRIDE_ALIGN)
            .with_context(|| "Failed to encode")?
        {
            frames.push(create_frame(frame));
        }
        for ref frame in vpx.flush().with_context(|| "Failed to flush")? {
            frames.push(create_frame(frame));
        }
        // to-do: flush periodically, e.g. 1 second
        if frames.len() > 0 {
            sp.send(create_msg(frames));
        }
    }
    Ok(())
}

fn get_display_num() -> usize {
    if let Ok(d) = Display::all() {
        d.len()
    } else {
        0
    }
}

pub fn get_displays() -> ResultType<(usize, Vec<DisplayInfo>)> {
    // switch to primary display if long time (30 seconds) no users
    if LAST_ACTIVE.lock().unwrap().elapsed().as_secs() >= 30 {
        *CURRENT_DISPLAY.lock().unwrap() = usize::MAX;
    }
    let mut displays = Vec::new();
    let mut primary = 0;
    for (i, d) in Display::all()?.iter().enumerate() {
        if d.is_primary() {
            primary = i;
        }
        displays.push(DisplayInfo {
            x: d.origin().0 as _,
            y: d.origin().1 as _,
            width: d.width() as _,
            height: d.height() as _,
            name: d.name(),
            online: d.is_online(),
            ..Default::default()
        });
    }
    let mut lock = CURRENT_DISPLAY.lock().unwrap();
    if *lock >= displays.len() {
        *lock = primary
    }
    Ok((*lock, displays))
}

pub fn switch_display(i: i32) {
    let i = i as usize;
    if let Ok((_, displays)) = get_displays() {
        if i < displays.len() {
            *CURRENT_DISPLAY.lock().unwrap() = i;
        }
    }
}

pub fn refresh() {
    *SWITCH.lock().unwrap() = true;
}

fn get_primary() -> usize {
    if let Ok(all) = Display::all() {
        for (i, d) in all.iter().enumerate() {
            if d.is_primary() {
                return i;
            }
        }
    }
    0
}

pub fn switch_to_primary() {
    switch_display(get_primary() as _);
}

fn get_current_display() -> ResultType<(usize, usize, Display)> {
    let mut current = *CURRENT_DISPLAY.lock().unwrap() as usize;
    let mut displays = Display::all()?;
    if displays.len() == 0 {
        bail!("No displays");
    }
    let n = displays.len();
    if current >= n {
        current = 0;
        for (i, d) in displays.iter().enumerate() {
            if d.is_primary() {
                current = i;
                break;
            }
        }
        *CURRENT_DISPLAY.lock().unwrap() = current;
    }
    return Ok((n, current, displays.remove(current)));
}

#[inline]
fn update_latency(id: i32, latency: i64, latencies: &mut HashMap<i32, i64>) {
    if latency <= 0 {
        latencies.remove(&id);
    } else {
        latencies.insert(id, latency);
    }
}

pub fn update_test_latency(id: i32, latency: i64) {
    update_latency(id, latency, &mut *TEST_LATENCIES.lock().unwrap());
}

pub fn update_internal_latency(id: i32, latency: i64) {
    update_latency(id, latency, &mut *INTERNAL_LATENCIES.lock().unwrap());
}

pub fn get_latency() -> i64 {
    INTERNAL_LATENCIES
        .lock()
        .unwrap()
        .values()
        .max()
        .unwrap_or(&0)
        .clone()
}

fn convert_quality(q: i32) -> i32 {
    let q = {
        if q == ImageQuality::Balanced.value() {
            (100 * 2 / 3, 12)
        } else if q == ImageQuality::Low.value() {
            (100 / 2, 18)
        } else if q == ImageQuality::Best.value() {
            (100, 12)
        } else {
            let bitrate = q >> 8 & 0xFF;
            let quantizer = q & 0xFF;
            (bitrate * 2, (100 - quantizer) * 36 / 100)
        }
    };
    if q.0 <= 0 {
        0
    } else {
        q.0 << 8 | q.1
    }
}

pub fn update_image_quality(id: i32, q: Option<i32>) {
    match q {
        Some(q) => {
            let q = convert_quality(q);
            if q > 0 {
                IMAGE_QUALITIES.lock().unwrap().insert(id, q);
            } else {
                IMAGE_QUALITIES.lock().unwrap().remove(&id);
            }
        }
        None => {
            IMAGE_QUALITIES.lock().unwrap().remove(&id);
        }
    }
}

fn get_image_quality() -> i32 {
    IMAGE_QUALITIES
        .lock()
        .unwrap()
        .values()
        .min()
        .unwrap_or(&convert_quality(ImageQuality::Balanced.value()))
        .clone()
}

#[inline]
fn get_quality(w: usize, h: usize, q: i32) -> (u32, u32, u32, i32) {
    // https://www.nvidia.com/en-us/geforce/guides/broadcasting-guide/
    let bitrate = q >> 8 & 0xFF;
    let quantizer = q & 0xFF;
    let b = ((w * h) / 1000) as u32;
    (bitrate as u32 * b / 100, quantizer as _, 56, 7)
}
