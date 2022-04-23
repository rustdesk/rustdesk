// 24FPS (actually 23.976FPS) is what video professionals ages ago determined to be the
// slowest playback rate that still looks smooth enough to feel real.
// Our eyes can see a slight difference and even though 30FPS actually shows
// more information and is more realistic.
// 60FPS is commonly used in game, teamviewer 12 support this for video editing user.

// how to capture with mouse cursor:
// https://docs.microsoft.com/zh-cn/windows/win32/direct3ddxgi/desktop-dup-api?redirectedfrom=MSDN

// RECORD: The following Project has implemented audio capture, hardware codec and mouse cursor drawn.
// https://github.com/PHZ76/DesktopSharing

// dxgi memory leak issue
// https://stackoverflow.com/questions/47801238/memory-leak-in-creating-direct2d-device
// but per my test, it is more related to AcquireNextFrame,
// https://forums.developer.nvidia.com/t/dxgi-outputduplication-memory-leak-when-using-nv-but-not-amd-drivers/108582

// to-do:
// https://slhck.info/video/2017/03/01/rate-control.html

use super::*;
use hbb_common::tokio::{
    runtime::Runtime,
    sync::{
        mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
        Mutex as TokioMutex,
    },
};
use scrap::{Capturer, Config, Display, EncodeFrame, Encoder, VideoCodecId, STRIDE_ALIGN};
use std::{
    collections::HashSet,
    io::ErrorKind::WouldBlock,
    time::{self, Duration, Instant},
};
use virtual_display;

pub const NAME: &'static str = "video";

lazy_static::lazy_static! {
    static ref CURRENT_DISPLAY: Arc<Mutex<usize>> = Arc::new(Mutex::new(usize::MAX));
    static ref LAST_ACTIVE: Arc<Mutex<Instant>> = Arc::new(Mutex::new(Instant::now()));
    static ref SWITCH: Arc<Mutex<bool>> = Default::default();
    static ref TEST_LATENCIES: Arc<Mutex<HashMap<i32, i64>>> = Default::default();
    static ref IMAGE_QUALITIES: Arc<Mutex<HashMap<i32, i32>>> = Default::default();
    static ref FRAME_FETCHED_NOTIFIER: (UnboundedSender<(i32, Option<Instant>)>, Arc<TokioMutex<UnboundedReceiver<(i32, Option<Instant>)>>>) = {
        let (tx, rx) = unbounded_channel();
        (tx, Arc::new(TokioMutex::new(rx)))
    };
}

pub fn notify_video_frame_feched(conn_id: i32, frame_tm: Option<Instant>) {
    FRAME_FETCHED_NOTIFIER.0.send((conn_id, frame_tm)).unwrap()
}

struct VideoFrameController {
    cur: Instant,
    send_conn_ids: HashSet<i32>,
    rt: Runtime,
}

impl VideoFrameController {
    fn new() -> Self {
        Self {
            cur: Instant::now(),
            send_conn_ids: HashSet::new(),
            rt: Runtime::new().unwrap(),
        }
    }

    fn reset(&mut self) {
        self.send_conn_ids.clear();
    }

    fn set_send(&mut self, tm: Instant, conn_ids: HashSet<i32>) {
        if !conn_ids.is_empty() {
            self.cur = tm;
            self.send_conn_ids = conn_ids;
        }
    }

    fn blocking_wait_next(&mut self, timeout_millis: u128) {
        if self.send_conn_ids.is_empty() {
            return;
        }

        let send_conn_ids = self.send_conn_ids.clone();
        self.rt.block_on(async move {
            let mut fetched_conn_ids = HashSet::new();
            let begin = Instant::now();
            while begin.elapsed().as_millis() < timeout_millis {
                let timeout_dur =
                    Duration::from_millis((timeout_millis - begin.elapsed().as_millis()) as u64);
                match tokio::time::timeout(
                    timeout_dur,
                    FRAME_FETCHED_NOTIFIER.1.lock().await.recv(),
                )
                .await
                {
                    Err(_) => {
                        // break if timeout
                        // log::error!("blocking wait frame receiving timeout {}", timeout_millis);
                        break;
                    }
                    Ok(Some((id, instant))) => {
                        if let Some(tm) = instant {
                            log::trace!("Channel recv latency: {}", tm.elapsed().as_secs_f32());
                        }
                        fetched_conn_ids.insert(id);

                        // break if all connections have received current frame
                        if fetched_conn_ids.len() >= send_conn_ids.len() {
                            break;
                        }
                    }
                    Ok(None) => {
                        // this branch would nerver be reached
                    }
                }
            }
        });
    }
}

pub fn new() -> GenericService {
    let sp = GenericService::new(NAME, true);
    sp.run(run);
    sp
}

fn check_display_changed(
    last_n: usize,
    last_current: usize,
    last_width: usize,
    last_hegiht: usize,
) -> bool {
    let displays = match try_get_displays() {
        Ok(d) => d,
        _ => return false,
    };

    let n = displays.len();
    if n != last_n {
        return true;
    };

    for (i, d) in displays.iter().enumerate() {
        if d.is_primary() {
            if i != last_current {
                return true;
            };
            if d.width() != last_width || d.height() != last_hegiht {
                return true;
            };
        }
    }

    return false;
}

fn run(sp: GenericService) -> ResultType<()> {
    let num_displays = Display::all()?.len();
    if num_displays == 0 {
        // Device may sometimes be uninstalled by user in "Device Manager" Window.
        // Closing device will clear the instance data.
        virtual_display::close_device();
    } else if num_displays > 1 {
        // Try close device, if display device changed.
        if virtual_display::is_device_created() {
            virtual_display::close_device();
        }
    }

    let fps = 30;
    let wait = 1000 / fps;
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
    match Encoder::new(&cfg, (num_cpus::get() / 2) as _) {
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

    let mut frame_controller = VideoFrameController::new();

    let start = time::Instant::now();
    let mut last_check_displays = time::Instant::now();
    #[cfg(windows)]
    let mut try_gdi = 1;
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

        frame_controller.reset();

        match c.frame(wait as _) {
            Ok(frame) => {
                let time = now - start;
                let ms = (time.as_secs() * 1000 + time.subsec_millis() as u64) as i64;
                let send_conn_ids = handle_one_frame(&sp, &frame, ms, &mut vpx)?;
                frame_controller.set_send(now, send_conn_ids);
                #[cfg(windows)]
                {
                    try_gdi = 0;
                }
            }
            Err(ref e) if e.kind() == WouldBlock => {
                #[cfg(windows)]
                if try_gdi > 0 && !c.is_gdi() {
                    if try_gdi > 3 {
                        c.set_gdi();
                        try_gdi = 0;
                        log::info!("No image, fall back to gdi");
                    }
                    try_gdi += 1;
                }
            }
            Err(err) => {
                if check_display_changed(ndisplay, current, width, height) {
                    log::info!("Displays changed");
                    *SWITCH.lock().unwrap() = true;
                    bail!("SWITCH");
                }

                return Err(err.into());
            }
        }

        // i love 3, 6, 8
        frame_controller.blocking_wait_next(3_000);

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
    vpx: &mut Encoder,
) -> ResultType<HashSet<i32>> {
    sp.snapshot(|sps| {
        // so that new sub and old sub share the same encoder after switch
        if sps.has_subscribes() {
            bail!("SWITCH");
        }
        Ok(())
    })?;

    let mut send_conn_ids: HashSet<i32> = Default::default();
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
        send_conn_ids = sp.send_video_frame(create_msg(frames));
    }
    Ok(send_conn_ids)
}

fn get_display_num() -> usize {
    if let Ok(d) = try_get_displays() {
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
    for (i, d) in try_get_displays()?.iter().enumerate() {
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
    if let Ok(all) = try_get_displays() {
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

fn try_get_displays() -> ResultType<Vec<Display>> {
    let mut displays = Display::all()?;
    if displays.len() == 0 {
        log::debug!("no displays, create virtual display");
        // Try plugin monitor
        if !virtual_display::is_device_created() {
            if let Err(e) = virtual_display::create_device() {
                log::debug!("Create device failed {}", e);
            }
        }
        if virtual_display::is_device_created() {
            if let Err(e) = virtual_display::plug_in_monitor() {
                log::debug!("Plug in monitor failed {}", e);
            } else {
                if let Err(e) = virtual_display::update_monitor_modes() {
                    log::debug!("Update monitor modes failed {}", e);
                }
            }
        }
        displays = Display::all()?;
    } else if displays.len() > 1 {
        // If more than one displays exists, close RustDeskVirtualDisplay
        if virtual_display::is_device_created() {
            virtual_display::close_device()
        }
    }
    Ok(displays)
}

fn get_current_display() -> ResultType<(usize, usize, Display)> {
    let mut current = *CURRENT_DISPLAY.lock().unwrap() as usize;
    let mut displays = try_get_displays()?;
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
