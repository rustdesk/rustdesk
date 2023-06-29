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

use super::{video_qos::VideoQoS, *};
#[cfg(all(windows, feature = "virtual_display_driver"))]
use crate::virtual_display_manager;
#[cfg(windows)]
use crate::{platform::windows::is_process_consent_running, privacy_win_mag};
#[cfg(windows)]
use hbb_common::get_version_number;
use hbb_common::{
    protobuf::MessageField,
    tokio::sync::{
        mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
        Mutex as TokioMutex,
    },
};
#[cfg(not(windows))]
use scrap::Capturer;
use scrap::{
    aom::AomEncoderConfig,
    codec::{Encoder, EncoderCfg, HwEncoderConfig},
    record::{Recorder, RecorderContext},
    vpxcodec::{VpxEncoderConfig, VpxVideoCodecId},
    CodecName, Display, TraitCapturer,
};
#[cfg(windows)]
use std::sync::Once;
use std::{
    collections::HashSet,
    io::ErrorKind::WouldBlock,
    ops::{Deref, DerefMut},
    time::{self, Duration, Instant},
};

pub const NAME: &'static str = "video";

lazy_static::lazy_static! {
    pub static ref CURRENT_DISPLAY: Arc<Mutex<usize>> = Arc::new(Mutex::new(usize::MAX));
    static ref LAST_ACTIVE: Arc<Mutex<Instant>> = Arc::new(Mutex::new(Instant::now()));
    static ref SWITCH: Arc<Mutex<bool>> = Default::default();
    static ref FRAME_FETCHED_NOTIFIER: (UnboundedSender<(i32, Option<Instant>)>, Arc<TokioMutex<UnboundedReceiver<(i32, Option<Instant>)>>>) = {
        let (tx, rx) = unbounded_channel();
        (tx, Arc::new(TokioMutex::new(rx)))
    };
    static ref PRIVACY_MODE_CONN_ID: Mutex<i32> = Mutex::new(0);
    static ref IS_CAPTURER_MAGNIFIER_SUPPORTED: bool = is_capturer_mag_supported();
    pub static ref VIDEO_QOS: Arc<Mutex<VideoQoS>> = Default::default();
    pub static ref IS_UAC_RUNNING: Arc<Mutex<bool>> = Default::default();
    pub static ref IS_FOREGROUND_WINDOW_ELEVATED: Arc<Mutex<bool>> = Default::default();
    pub static ref LAST_SYNC_DISPLAYS: Arc<RwLock<Vec<DisplayInfo>>> = Default::default();
    static ref ORIGINAL_RESOLUTIONS: Arc<RwLock<HashMap<String, (i32, i32)>>> = Default::default();
}

// Not virtual display
#[inline]
fn set_original_resolution_(display_name: &str, wh: (i32, i32)) -> (i32, i32) {
    let mut original_resolutions = ORIGINAL_RESOLUTIONS.write().unwrap();
    match original_resolutions.get(display_name) {
        Some(r) => r.clone(),
        None => {
            original_resolutions.insert(display_name.to_owned(), wh.clone());
            wh
        }
    }
}

// Not virtual display
#[inline]
fn get_original_resolution_(display_name: &str) -> Option<(i32, i32)> {
    ORIGINAL_RESOLUTIONS
        .read()
        .unwrap()
        .get(display_name)
        .map(|r| r.clone())
}

// Not virtual display
#[inline]
fn get_or_set_original_resolution_(display_name: &str, wh: (i32, i32)) -> (i32, i32) {
    let r = get_original_resolution_(display_name);
    if let Some(r) = r {
        return r;
    }
    set_original_resolution_(display_name, wh)
}

// Not virtual display
#[inline]
fn update_get_original_resolution_(display_name: &str, w: usize, h: usize) -> Resolution {
    let wh = get_or_set_original_resolution_(display_name, (w as _, h as _));
    Resolution {
        width: wh.0,
        height: wh.1,
        ..Default::default()
    }
}

#[inline]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn reset_resolutions() {
    for (name, (w, h)) in ORIGINAL_RESOLUTIONS.read().unwrap().iter() {
        if let Err(e) = crate::platform::change_resolution(name, *w as _, *h as _) {
            log::error!(
                "Failed to reset resolution of display '{}' to ({},{}): {}",
                name,
                w,
                h,
                e
            );
        }
    }
}

#[inline]
fn is_capturer_mag_supported() -> bool {
    #[cfg(windows)]
    return scrap::CapturerMag::is_supported();
    #[cfg(not(windows))]
    false
}

#[inline]
pub fn capture_cursor_embedded() -> bool {
    scrap::is_cursor_embedded()
}

#[inline]
pub fn notify_video_frame_fetched(conn_id: i32, frame_tm: Option<Instant>) {
    FRAME_FETCHED_NOTIFIER.0.send((conn_id, frame_tm)).unwrap()
}

#[inline]
pub fn set_privacy_mode_conn_id(conn_id: i32) {
    *PRIVACY_MODE_CONN_ID.lock().unwrap() = conn_id
}

#[inline]
pub fn get_privacy_mode_conn_id() -> i32 {
    *PRIVACY_MODE_CONN_ID.lock().unwrap()
}

#[inline]
pub fn is_privacy_mode_supported() -> bool {
    #[cfg(windows)]
    return *IS_CAPTURER_MAGNIFIER_SUPPORTED
        && get_version_number(&crate::VERSION) > get_version_number("1.1.9");
    #[cfg(not(windows))]
    return false;
}

struct VideoFrameController {
    cur: Instant,
    send_conn_ids: HashSet<i32>,
}

impl VideoFrameController {
    fn new() -> Self {
        Self {
            cur: Instant::now(),
            send_conn_ids: HashSet::new(),
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

    #[tokio::main(flavor = "current_thread")]
    async fn try_wait_next(&mut self, fetched_conn_ids: &mut HashSet<i32>, timeout_millis: u64) {
        if self.send_conn_ids.is_empty() {
            return;
        }

        let timeout_dur = Duration::from_millis(timeout_millis as u64);
        match tokio::time::timeout(timeout_dur, FRAME_FETCHED_NOTIFIER.1.lock().await.recv()).await
        {
            Err(_) => {
                // break if timeout
                // log::error!("blocking wait frame receiving timeout {}", timeout_millis);
            }
            Ok(Some((id, instant))) => {
                if let Some(tm) = instant {
                    log::trace!("Channel recv latency: {}", tm.elapsed().as_secs_f32());
                }
                fetched_conn_ids.insert(id);
            }
            Ok(None) => {
                // this branch would never be reached
            }
        }
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
    last_height: usize,
) -> bool {
    #[cfg(target_os = "linux")]
    {
        // wayland do not support changing display for now
        if !scrap::is_x11() {
            return false;
        }
    }

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
            if d.width() != last_width || d.height() != last_height {
                return true;
            };
        }
    }

    return false;
}

// Capturer object is expensive, avoiding to create it frequently.
fn create_capturer(
    privacy_mode_id: i32,
    display: Display,
    use_yuv: bool,
    _current: usize,
    _portable_service_running: bool,
) -> ResultType<Box<dyn TraitCapturer>> {
    #[cfg(not(windows))]
    let c: Option<Box<dyn TraitCapturer>> = None;
    #[cfg(windows)]
    let mut c: Option<Box<dyn TraitCapturer>> = None;
    if privacy_mode_id > 0 {
        #[cfg(windows)]
        {
            match scrap::CapturerMag::new(
                display.origin(),
                display.width(),
                display.height(),
                use_yuv,
            ) {
                Ok(mut c1) => {
                    let mut ok = false;
                    let check_begin = Instant::now();
                    while check_begin.elapsed().as_secs() < 5 {
                        match c1.exclude("", privacy_win_mag::PRIVACY_WINDOW_NAME) {
                            Ok(false) => {
                                ok = false;
                                std::thread::sleep(std::time::Duration::from_millis(500));
                            }
                            Err(e) => {
                                bail!(
                                    "Failed to exclude privacy window {} - {}, err: {}",
                                    "",
                                    privacy_win_mag::PRIVACY_WINDOW_NAME,
                                    e
                                );
                            }
                            _ => {
                                ok = true;
                                break;
                            }
                        }
                    }
                    if !ok {
                        bail!(
                            "Failed to exclude privacy window {} - {} ",
                            "",
                            privacy_win_mag::PRIVACY_WINDOW_NAME
                        );
                    }
                    log::debug!("Create magnifier capture for {}", privacy_mode_id);
                    c = Some(Box::new(c1));
                }
                Err(e) => {
                    bail!(format!("Failed to create magnifier capture {}", e));
                }
            }
        }
    }

    match c {
        Some(c1) => return Ok(c1),
        None => {
            log::debug!("Create capturer dxgi|gdi");
            #[cfg(windows)]
            return crate::portable_service::client::create_capturer(
                _current,
                display,
                use_yuv,
                _portable_service_running,
            );
            #[cfg(not(windows))]
            return Ok(Box::new(
                Capturer::new(display, use_yuv).with_context(|| "Failed to create capturer")?,
            ));
        }
    };
}

// This function works on privacy mode. Windows only for now.
pub fn test_create_capturer(privacy_mode_id: i32, timeout_millis: u64) -> bool {
    let test_begin = Instant::now();
    while test_begin.elapsed().as_millis() < timeout_millis as _ {
        if let Ok((_, current, display)) = get_current_display() {
            if let Ok(_) = create_capturer(privacy_mode_id, display, true, current, false) {
                return true;
            }
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    false
}

#[cfg(windows)]
fn check_uac_switch(privacy_mode_id: i32, capturer_privacy_mode_id: i32) -> ResultType<()> {
    if capturer_privacy_mode_id != 0 {
        if privacy_mode_id != capturer_privacy_mode_id {
            if !is_process_consent_running()? {
                bail!("consent.exe is running");
            }
        }
        if is_process_consent_running()? {
            bail!("consent.exe is running");
        }
    }
    Ok(())
}

pub(super) struct CapturerInfo {
    pub origin: (i32, i32),
    pub width: usize,
    pub height: usize,
    pub ndisplay: usize,
    pub current: usize,
    pub privacy_mode_id: i32,
    pub _capturer_privacy_mode_id: i32,
    pub capturer: Box<dyn TraitCapturer>,
}

impl Deref for CapturerInfo {
    type Target = Box<dyn TraitCapturer>;

    fn deref(&self) -> &Self::Target {
        &self.capturer
    }
}

impl DerefMut for CapturerInfo {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.capturer
    }
}

fn get_capturer(use_yuv: bool, portable_service_running: bool) -> ResultType<CapturerInfo> {
    #[cfg(target_os = "linux")]
    {
        if !scrap::is_x11() {
            return super::wayland::get_capturer();
        }
    }

    let (ndisplay, current, display) = get_current_display()?;
    let (origin, width, height) = (display.origin(), display.width(), display.height());
    log::debug!(
        "#displays={}, current={}, origin: {:?}, width={}, height={}, cpus={}/{}, name:{}",
        ndisplay,
        current,
        &origin,
        width,
        height,
        num_cpus::get_physical(),
        num_cpus::get(),
        display.name(),
    );

    let privacy_mode_id = *PRIVACY_MODE_CONN_ID.lock().unwrap();
    #[cfg(not(windows))]
    let capturer_privacy_mode_id = privacy_mode_id;
    #[cfg(windows)]
    let mut capturer_privacy_mode_id = privacy_mode_id;
    #[cfg(windows)]
    if capturer_privacy_mode_id != 0 {
        if is_process_consent_running()? {
            capturer_privacy_mode_id = 0;
        }
    }
    log::debug!(
        "Try create capturer with capturer privacy mode id {}",
        capturer_privacy_mode_id,
    );

    if privacy_mode_id != 0 {
        if privacy_mode_id != capturer_privacy_mode_id {
            log::info!("In privacy mode, but show UAC prompt window for now");
        } else {
            log::info!("In privacy mode, the peer side cannot watch the screen");
        }
    }
    let capturer = create_capturer(
        capturer_privacy_mode_id,
        display,
        use_yuv,
        current,
        portable_service_running,
    )?;
    Ok(CapturerInfo {
        origin,
        width,
        height,
        ndisplay,
        current,
        privacy_mode_id,
        _capturer_privacy_mode_id: capturer_privacy_mode_id,
        capturer,
    })
}

fn check_displays_new() -> Option<Vec<Display>> {
    let displays = try_get_displays().ok()?;
    let last_sync_displays = &*LAST_SYNC_DISPLAYS.read().unwrap();

    if displays.len() != last_sync_displays.len() {
        Some(displays)
    } else {
        for i in 0..displays.len() {
            if displays[i].height() != (last_sync_displays[i].height as usize) {
                return Some(displays);
            }
            if displays[i].width() != (last_sync_displays[i].width as usize) {
                return Some(displays);
            }
            if displays[i].origin() != (last_sync_displays[i].x, last_sync_displays[i].y) {
                return Some(displays);
            }
        }
        None
    }
}

fn check_get_displays_changed_msg() -> Option<Message> {
    let displays = check_displays_new()?;
    let (current, displays) = get_displays_2(&displays);
    let mut pi = PeerInfo {
        conn_id: crate::SYNC_PEER_INFO_DISPLAYS,
        ..Default::default()
    };
    pi.displays = displays.clone();
    pi.current_display = current as _;
    let mut msg_out = Message::new();
    msg_out.set_peer_info(pi);
    *LAST_SYNC_DISPLAYS.write().unwrap() = displays;
    Some(msg_out)
}

#[cfg(all(windows, feature = "virtual_display_driver"))]
pub fn try_plug_out_virtual_display() {
    let _res = virtual_display_manager::plug_out_headless();
}

fn run(sp: GenericService) -> ResultType<()> {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let _wake_lock = get_wake_lock();

    // ensure_inited() is needed because release_resource() may be called.
    #[cfg(target_os = "linux")]
    super::wayland::ensure_inited()?;
    #[cfg(windows)]
    let last_portable_service_running = crate::portable_service::client::running();
    #[cfg(not(windows))]
    let last_portable_service_running = false;

    let mut c = get_capturer(true, last_portable_service_running)?;

    let mut video_qos = VIDEO_QOS.lock().unwrap();
    video_qos.set_size(c.width as _, c.height as _);
    let mut spf = video_qos.spf();
    let bitrate = video_qos.generate_bitrate()?;
    let abr = video_qos.check_abr_config();
    drop(video_qos);
    log::info!("init bitrate={}, abr enabled:{}", bitrate, abr);

    let encoder_cfg = match Encoder::negotiated_codec() {
        scrap::CodecName::H264(name) | scrap::CodecName::H265(name) => {
            EncoderCfg::HW(HwEncoderConfig {
                name,
                width: c.width,
                height: c.height,
                bitrate: bitrate as _,
            })
        }
        name @ (scrap::CodecName::VP8 | scrap::CodecName::VP9) => {
            EncoderCfg::VPX(VpxEncoderConfig {
                width: c.width as _,
                height: c.height as _,
                bitrate,
                codec: if name == scrap::CodecName::VP8 {
                    VpxVideoCodecId::VP8
                } else {
                    VpxVideoCodecId::VP9
                },
            })
        }
        scrap::CodecName::AV1 => EncoderCfg::AOM(AomEncoderConfig {
            width: c.width as _,
            height: c.height as _,
            bitrate: bitrate as _,
        }),
    };

    let mut encoder;
    match Encoder::new(encoder_cfg) {
        Ok(x) => encoder = x,
        Err(err) => bail!("Failed to create encoder: {}", err),
    }
    c.set_use_yuv(encoder.use_yuv());

    if *SWITCH.lock().unwrap() {
        log::debug!("Broadcasting display switch");
        let mut misc = Misc::new();
        let display_name = get_current_display_name().unwrap_or_default();
        let original_resolution = get_original_resolution(&display_name, c.width, c.height);
        misc.set_switch_display(SwitchDisplay {
            display: c.current as _,
            x: c.origin.0 as _,
            y: c.origin.1 as _,
            width: c.width as _,
            height: c.height as _,
            cursor_embedded: capture_cursor_embedded(),
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            resolutions: Some(SupportedResolutions {
                resolutions: if display_name.is_empty() {
                    vec![]
                } else {
                    crate::platform::resolutions(&display_name)
                },
                ..SupportedResolutions::default()
            })
            .into(),
            original_resolution,
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
    let codec_name = Encoder::negotiated_codec();
    let recorder = get_recorder(c.width, c.height, &codec_name);
    #[cfg(windows)]
    start_uac_elevation_check();

    #[cfg(target_os = "linux")]
    let mut would_block_count = 0u32;

    while sp.ok() {
        #[cfg(windows)]
        check_uac_switch(c.privacy_mode_id, c._capturer_privacy_mode_id)?;

        let mut video_qos = VIDEO_QOS.lock().unwrap();
        if video_qos.check_if_updated() && video_qos.target_bitrate > 0 {
            log::debug!(
                "qos is updated, target_bitrate:{}, fps:{}",
                video_qos.target_bitrate,
                video_qos.fps
            );
            allow_err!(encoder.set_bitrate(video_qos.target_bitrate));
            spf = video_qos.spf();
        }
        drop(video_qos);

        if *SWITCH.lock().unwrap() {
            bail!("SWITCH");
        }
        if c.current != *CURRENT_DISPLAY.lock().unwrap() {
            *SWITCH.lock().unwrap() = true;
            bail!("SWITCH");
        }
        if codec_name != Encoder::negotiated_codec() {
            bail!("SWITCH");
        }
        #[cfg(windows)]
        if last_portable_service_running != crate::portable_service::client::running() {
            bail!("SWITCH");
        }
        check_privacy_mode_changed(&sp, c.privacy_mode_id)?;
        #[cfg(windows)]
        {
            if crate::platform::windows::desktop_changed()
                && !crate::portable_service::client::running()
            {
                bail!("Desktop changed");
            }
        }
        let now = time::Instant::now();
        if last_check_displays.elapsed().as_millis() > 1000 {
            last_check_displays = now;

            // Capturer on macos does not return Err event the solution is changed.
            #[cfg(target_os = "macos")]
            if check_display_changed(c.ndisplay, c.current, c.width, c.height) {
                log::info!("Displays changed");
                *SWITCH.lock().unwrap() = true;
                bail!("SWITCH");
            }

            if let Some(msg_out) = check_get_displays_changed_msg() {
                sp.send(msg_out);
                log::info!("Displays changed");
                *SWITCH.lock().unwrap() = true;
                bail!("SWITCH");
            }
        }

        *LAST_ACTIVE.lock().unwrap() = now;

        frame_controller.reset();

        #[cfg(any(target_os = "android", target_os = "ios"))]
        let res = match c.frame(spf) {
            Ok(frame) => {
                let time = now - start;
                let ms = (time.as_secs() * 1000 + time.subsec_millis() as u64) as i64;
                match frame {
                    scrap::Frame::RAW(data) => {
                        if data.len() != 0 {
                            let send_conn_ids =
                                handle_one_frame(&sp, data, ms, &mut encoder, recorder.clone())?;
                            frame_controller.set_send(now, send_conn_ids);
                        }
                    }
                    _ => {}
                };
                Ok(())
            }
            Err(err) => Err(err),
        };

        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        let res = match c.frame(spf) {
            Ok(frame) => {
                let time = now - start;
                let ms = (time.as_secs() * 1000 + time.subsec_millis() as u64) as i64;
                let send_conn_ids =
                    handle_one_frame(&sp, &frame, ms, &mut encoder, recorder.clone())?;
                frame_controller.set_send(now, send_conn_ids);
                #[cfg(windows)]
                {
                    try_gdi = 0;
                }
                Ok(())
            }
            Err(err) => Err(err),
        };

        match res {
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
                #[cfg(target_os = "linux")]
                {
                    would_block_count += 1;
                    if !scrap::is_x11() {
                        if would_block_count >= 100 {
                            // to-do: Unknown reason for WouldBlock 100 times (seconds = 100 * 1 / fps)
                            // https://github.com/rustdesk/rustdesk/blob/63e6b2f8ab51743e77a151e2b7ff18816f5fa2fb/libs/scrap/src/common/wayland.rs#L81
                            //
                            // Do not reset the capturer for now, as it will cause the prompt to show every few minutes.
                            // https://github.com/rustdesk/rustdesk/issues/4276
                            //
                            // super::wayland::release_resource();
                            // bail!("Wayland capturer none 100 times, try restart capture");
                        }
                    }
                }
            }
            Err(err) => {
                if check_display_changed(c.ndisplay, c.current, c.width, c.height) {
                    log::info!("Displays changed");
                    *SWITCH.lock().unwrap() = true;
                    bail!("SWITCH");
                }

                #[cfg(windows)]
                if !c.is_gdi() {
                    c.set_gdi();
                    log::info!("dxgi error, fall back to gdi: {:?}", err);
                    continue;
                }

                return Err(err.into());
            }
            _ => {
                #[cfg(target_os = "linux")]
                {
                    would_block_count = 0;
                }
            }
        }

        let mut fetched_conn_ids = HashSet::new();
        let timeout_millis = 3_000u64;
        let wait_begin = Instant::now();
        while wait_begin.elapsed().as_millis() < timeout_millis as _ {
            check_privacy_mode_changed(&sp, c.privacy_mode_id)?;
            #[cfg(windows)]
            check_uac_switch(c.privacy_mode_id, c._capturer_privacy_mode_id)?;
            frame_controller.try_wait_next(&mut fetched_conn_ids, 300);
            // break if all connections have received current frame
            if fetched_conn_ids.len() >= frame_controller.send_conn_ids.len() {
                break;
            }
        }

        let elapsed = now.elapsed();
        // may need to enable frame(timeout)
        log::trace!("{:?} {:?}", time::Instant::now(), elapsed);
        if elapsed < spf {
            std::thread::sleep(spf - elapsed);
        }
    }

    #[cfg(target_os = "linux")]
    if !scrap::is_x11() {
        super::wayland::release_resource();
    }

    Ok(())
}

fn get_recorder(
    width: usize,
    height: usize,
    codec_name: &CodecName,
) -> Arc<Mutex<Option<Recorder>>> {
    #[cfg(not(target_os = "ios"))]
    let recorder = if !Config::get_option("allow-auto-record-incoming").is_empty() {
        use crate::hbbs_http::record_upload;

        let tx = if record_upload::is_enable() {
            let (tx, rx) = std::sync::mpsc::channel();
            record_upload::run(rx);
            Some(tx)
        } else {
            None
        };
        Recorder::new(RecorderContext {
            server: true,
            id: Config::get_id(),
            default_dir: crate::ui_interface::default_video_save_directory(),
            filename: "".to_owned(),
            width,
            height,
            format: codec_name.into(),
            tx,
        })
        .map_or(Default::default(), |r| Arc::new(Mutex::new(Some(r))))
    } else {
        Default::default()
    };
    #[cfg(target_os = "ios")]
    let recorder: Arc<Mutex<Option<Recorder>>> = Default::default();

    recorder
}

fn check_privacy_mode_changed(sp: &GenericService, privacy_mode_id: i32) -> ResultType<()> {
    let privacy_mode_id_2 = *PRIVACY_MODE_CONN_ID.lock().unwrap();
    if privacy_mode_id != privacy_mode_id_2 {
        if privacy_mode_id_2 != 0 {
            let msg_out = crate::common::make_privacy_mode_msg(
                back_notification::PrivacyModeState::PrvOnByOther,
            );
            sp.send_to_others(msg_out, privacy_mode_id_2);
        }
        bail!("SWITCH");
    }
    Ok(())
}

#[inline]
fn handle_one_frame(
    sp: &GenericService,
    frame: &[u8],
    ms: i64,
    encoder: &mut Encoder,
    recorder: Arc<Mutex<Option<Recorder>>>,
) -> ResultType<HashSet<i32>> {
    sp.snapshot(|sps| {
        // so that new sub and old sub share the same encoder after switch
        if sps.has_subscribes() {
            bail!("SWITCH");
        }
        Ok(())
    })?;

    let mut send_conn_ids: HashSet<i32> = Default::default();
    if let Ok(msg) = encoder.encode_to_message(frame, ms) {
        #[cfg(not(target_os = "ios"))]
        recorder
            .lock()
            .unwrap()
            .as_mut()
            .map(|r| r.write_message(&msg));
        send_conn_ids = sp.send_video_frame(msg);
    }
    Ok(send_conn_ids)
}

#[inline]
fn get_original_resolution(display_name: &str, w: usize, h: usize) -> MessageField<Resolution> {
    #[cfg(all(windows, feature = "virtual_display_driver"))]
    let is_virtual_display = crate::virtual_display_manager::is_virtual_display(&display_name);
    #[cfg(not(all(windows, feature = "virtual_display_driver")))]
    let is_virtual_display = false;
    Some(if is_virtual_display {
        Resolution {
            width: 0,
            height: 0,
            ..Default::default()
        }
    } else {
        update_get_original_resolution_(&display_name, w, h)
    })
    .into()
}

pub(super) fn get_displays_2(all: &Vec<Display>) -> (usize, Vec<DisplayInfo>) {
    let mut displays = Vec::new();
    let mut primary = 0;
    for (i, d) in all.iter().enumerate() {
        if d.is_primary() {
            primary = i;
        }
        let display_name = d.name();
        let original_resolution = get_original_resolution(&display_name, d.width(), d.height());
        displays.push(DisplayInfo {
            x: d.origin().0 as _,
            y: d.origin().1 as _,
            width: d.width() as _,
            height: d.height() as _,
            name: display_name,
            online: d.is_online(),
            cursor_embedded: false,
            original_resolution,
            ..Default::default()
        });
    }
    let mut lock = CURRENT_DISPLAY.lock().unwrap();
    if *lock >= displays.len() {
        *lock = primary
    }
    (*lock, displays)
}

pub fn is_inited_msg() -> Option<Message> {
    #[cfg(target_os = "linux")]
    if !scrap::is_x11() {
        return super::wayland::is_inited();
    }
    None
}

// switch to primary display if long time (30 seconds) no users
#[inline]
pub fn try_reset_current_display() {
    if LAST_ACTIVE.lock().unwrap().elapsed().as_secs() >= 30 {
        *CURRENT_DISPLAY.lock().unwrap() = usize::MAX;
    }
    *LAST_ACTIVE.lock().unwrap() = time::Instant::now();
}

pub async fn get_displays() -> ResultType<(usize, Vec<DisplayInfo>)> {
    #[cfg(target_os = "linux")]
    {
        if !scrap::is_x11() {
            return super::wayland::get_displays().await;
        }
    }
    Ok(get_displays_2(&try_get_displays()?))
}

pub async fn switch_display(i: i32) {
    let i = i as usize;
    if let Ok((_, displays)) = get_displays().await {
        if i < displays.len() {
            *CURRENT_DISPLAY.lock().unwrap() = i;
        }
    }
}

#[inline]
pub fn refresh() {
    #[cfg(target_os = "android")]
    Display::refresh_size();
    *SWITCH.lock().unwrap() = true;
}

fn get_primary() -> usize {
    #[cfg(target_os = "linux")]
    {
        if !scrap::is_x11() {
            return match super::wayland::get_primary() {
                Ok(n) => n,
                Err(_) => 0,
            };
        }
    }

    if let Ok(all) = try_get_displays() {
        for (i, d) in all.iter().enumerate() {
            if d.is_primary() {
                return i;
            }
        }
    }
    0
}

#[inline]
pub async fn switch_to_primary() {
    switch_display(get_primary() as _).await;
}

#[inline]
#[cfg(not(all(windows, feature = "virtual_display_driver")))]
fn try_get_displays() -> ResultType<Vec<Display>> {
    Ok(Display::all()?)
}

#[inline]
#[cfg(all(windows, feature = "virtual_display_driver"))]
fn no_displays(displays: &Vec<Display>) -> bool {
    let display_len = displays.len();
    if display_len == 0 {
        true
    } else if display_len == 1 {
        let display = &displays[0];
        let dummy_display_side_max_size = 800;
        display.width() <= dummy_display_side_max_size
            && display.height() <= dummy_display_side_max_size
    } else {
        false
    }
}

#[cfg(all(windows, feature = "virtual_display_driver"))]
fn try_get_displays() -> ResultType<Vec<Display>> {
    let mut displays = Display::all()?;
    if no_displays(&displays) {
        log::debug!("no displays, create virtual display");
        if let Err(e) = virtual_display_manager::plug_in_headless() {
            log::error!("plug in headless failed {}", e);
        } else {
            displays = Display::all()?;
        }
    }
    Ok(displays)
}

pub(super) fn get_current_display_2(mut all: Vec<Display>) -> ResultType<(usize, usize, Display)> {
    let mut current = *CURRENT_DISPLAY.lock().unwrap() as usize;
    if all.len() == 0 {
        bail!("No displays");
    }
    let n = all.len();
    if current >= n {
        current = 0;
        for (i, d) in all.iter().enumerate() {
            if d.is_primary() {
                current = i;
                break;
            }
        }
        *CURRENT_DISPLAY.lock().unwrap() = current;
    }
    return Ok((n, current, all.remove(current)));
}

#[inline]
pub fn get_current_display() -> ResultType<(usize, usize, Display)> {
    get_current_display_2(try_get_displays()?)
}

// `try_reset_current_display` is needed because `get_displays` may change the current display,
// which may cause the mismatch of current display and the current display name.
#[inline]
pub fn get_current_display_name() -> ResultType<String> {
    Ok(get_current_display_2(try_get_displays()?)?.2.name())
}

#[cfg(windows)]
fn start_uac_elevation_check() {
    static START: Once = Once::new();
    START.call_once(|| {
        if !crate::platform::is_installed() && !crate::platform::is_root() {
            std::thread::spawn(|| loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
                if let Ok(uac) = is_process_consent_running() {
                    *IS_UAC_RUNNING.lock().unwrap() = uac;
                }
                if !crate::platform::is_elevated(None).unwrap_or(false) {
                    if let Ok(elevated) = crate::platform::is_foreground_window_elevated() {
                        *IS_FOREGROUND_WINDOW_ELEVATED.lock().unwrap() = elevated;
                    }
                }
            });
        }
    });
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn get_wake_lock() -> crate::platform::WakeLock {
    let (display, idle, sleep) = if cfg!(windows) {
        (true, false, false)
    } else if cfg!(linux) {
        (false, false, true)
    } else {
        //macos
        (true, false, false)
    };
    crate::platform::WakeLock::new(display, idle, sleep)
}
