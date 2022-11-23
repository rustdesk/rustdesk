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
#[cfg(windows)]
use crate::portable_service::client::PORTABLE_SERVICE_RUNNING;
use hbb_common::tokio::sync::{
    mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    Mutex as TokioMutex,
};
#[cfg(not(windows))]
use scrap::Capturer;
use scrap::{
    codec::{Encoder, EncoderCfg, HwEncoderConfig},
    record::{Recorder, RecorderContext},
    vpxcodec::{VpxEncoderConfig, VpxVideoCodecId},
    Display, TraitCapturer,
};
#[cfg(windows)]
use std::sync::Once;
use std::{
    collections::HashSet,
    io::ErrorKind::WouldBlock,
    ops::{Deref, DerefMut},
    time::{self, Duration, Instant},
};
#[cfg(windows)]
use virtual_display;

pub const SCRAP_UBUNTU_HIGHER_REQUIRED: &str = "Wayland requires Ubuntu 21.04 or higher version.";
pub const SCRAP_OTHER_VERSION_OR_X11_REQUIRED: &str =
    "Wayland requires higher version of linux distro. Please try X11 desktop or change your OS.";
pub const SCRAP_X11_REQUIRED: &str = "x11 expected";
pub const SCRAP_X11_REF_URL: &str = "https://rustdesk.com/docs/en/manual/linux/#x11-required";

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
}

fn is_capturer_mag_supported() -> bool {
    #[cfg(windows)]
    return scrap::CapturerMag::is_supported();
    #[cfg(not(windows))]
    false
}

pub fn notify_video_frame_feched(conn_id: i32, frame_tm: Option<Instant>) {
    FRAME_FETCHED_NOTIFIER.0.send((conn_id, frame_tm)).unwrap()
}

pub fn set_privacy_mode_conn_id(conn_id: i32) {
    *PRIVACY_MODE_CONN_ID.lock().unwrap() = conn_id
}

pub fn get_privacy_mode_conn_id() -> i32 {
    *PRIVACY_MODE_CONN_ID.lock().unwrap()
}

pub fn is_privacy_mode_supported() -> bool {
    #[cfg(windows)]
    return *IS_CAPTURER_MAGNIFIER_SUPPORTED;
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
                // this branch would nerver be reached
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
    last_hegiht: usize,
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
            if d.width() != last_width || d.height() != last_hegiht {
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
            use crate::ui::win_privacy::*;

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
                        match c1.exclude("", PRIVACY_WINDOW_NAME) {
                            Ok(false) => {
                                ok = false;
                                std::thread::sleep(std::time::Duration::from_millis(500));
                            }
                            Err(e) => {
                                bail!(
                                    "Failed to exclude privacy window {} - {}, err: {}",
                                    "",
                                    PRIVACY_WINDOW_NAME,
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
                            PRIVACY_WINDOW_NAME
                        );
                    }
                    log::debug!("Create maginifier capture for {}", privacy_mode_id);
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

#[cfg(windows)]
fn ensure_close_virtual_device() -> ResultType<()> {
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
    Ok(())
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
fn check_uac_switch(privacy_mode_id: i32, captuerer_privacy_mode_id: i32) -> ResultType<()> {
    if captuerer_privacy_mode_id != 0 {
        if privacy_mode_id != captuerer_privacy_mode_id {
            if !crate::ui::win_privacy::is_process_consent_running()? {
                bail!("consent.exe is running");
            }
        }
        if crate::ui::win_privacy::is_process_consent_running()? {
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
    pub _captuerer_privacy_mode_id: i32,
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
        "#displays={}, current={}, origin: {:?}, width={}, height={}, cpus={}/{}",
        ndisplay,
        current,
        &origin,
        width,
        height,
        num_cpus::get_physical(),
        num_cpus::get(),
    );

    let privacy_mode_id = *PRIVACY_MODE_CONN_ID.lock().unwrap();
    #[cfg(not(windows))]
    let captuerer_privacy_mode_id = privacy_mode_id;
    #[cfg(windows)]
    let mut captuerer_privacy_mode_id = privacy_mode_id;
    #[cfg(windows)]
    if captuerer_privacy_mode_id != 0 {
        if crate::ui::win_privacy::is_process_consent_running()? {
            captuerer_privacy_mode_id = 0;
        }
    }
    log::debug!(
        "Try create capturer with captuerer privacy mode id {}",
        captuerer_privacy_mode_id,
    );

    if privacy_mode_id != captuerer_privacy_mode_id {
        log::info!("In privacy mode, but show UAC prompt window for now");
    } else {
        log::info!("In privacy mode, the peer side cannot watch the screen");
    }
    let capturer = create_capturer(
        captuerer_privacy_mode_id,
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
        _captuerer_privacy_mode_id: captuerer_privacy_mode_id,
        capturer,
    })
}

fn run(sp: GenericService) -> ResultType<()> {
    #[cfg(windows)]
    ensure_close_virtual_device()?;

    // ensure_inited() is needed because release_resouce() may be called.
    #[cfg(target_os = "linux")]
    super::wayland::ensure_inited()?;
    #[cfg(windows)]
    let last_portable_service_running = PORTABLE_SERVICE_RUNNING.lock().unwrap().clone();
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

    let encoder_cfg = match Encoder::current_hw_encoder_name() {
        Some(codec_name) => EncoderCfg::HW(HwEncoderConfig {
            codec_name,
            width: c.width,
            height: c.height,
            bitrate: bitrate as _,
        }),
        None => EncoderCfg::VPX(VpxEncoderConfig {
            width: c.width as _,
            height: c.height as _,
            timebase: [1, 1000], // Output timestamp precision
            bitrate,
            codec: VpxVideoCodecId::VP9,
            num_threads: (num_cpus::get() / 2) as _,
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
        misc.set_switch_display(SwitchDisplay {
            display: c.current as _,
            x: c.origin.0 as _,
            y: c.origin.1 as _,
            width: c.width as _,
            height: c.height as _,
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
    let codec_name = Encoder::current_hw_encoder_name();
    #[cfg(not(target_os = "ios"))]
    let recorder = if !Config::get_option("allow-auto-record-incoming").is_empty() {
        Recorder::new(RecorderContext {
            id: "local".to_owned(),
            default_dir: crate::ui_interface::default_video_save_directory(),
            filename: "".to_owned(),
            width: c.width,
            height: c.height,
            codec_id: scrap::record::RecordCodecID::VP9,
        })
        .map_or(Default::default(), |r| Arc::new(Mutex::new(Some(r))))
    } else {
        Default::default()
    };
    #[cfg(target_os = "ios")]
    let recorder: Arc<Mutex<Option<Recorder>>> = Default::default();
    #[cfg(windows)]
    start_uac_elevation_check();

    #[cfg(target_os = "linux")]
    let mut would_block_count = 0u32;

    while sp.ok() {
        #[cfg(windows)]
        check_uac_switch(c.privacy_mode_id, c._captuerer_privacy_mode_id)?;

        let mut video_qos = VIDEO_QOS.lock().unwrap();
        if video_qos.check_if_updated() {
            log::debug!(
                "qos is updated, target_bitrate:{}, fps:{}",
                video_qos.target_bitrate,
                video_qos.fps
            );
            encoder.set_bitrate(video_qos.target_bitrate).unwrap();
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
        if codec_name != Encoder::current_hw_encoder_name() {
            bail!("SWITCH");
        }
        #[cfg(windows)]
        if last_portable_service_running != PORTABLE_SERVICE_RUNNING.lock().unwrap().clone() {
            bail!("SWITCH");
        }
        check_privacy_mode_changed(&sp, c.privacy_mode_id)?;
        #[cfg(windows)]
        {
            if crate::platform::windows::desktop_changed()
                && !PORTABLE_SERVICE_RUNNING.lock().unwrap().clone()
            {
                bail!("Desktop changed");
            }
        }
        let now = time::Instant::now();
        if last_check_displays.elapsed().as_millis() > 1000 {
            last_check_displays = now;
            if c.ndisplay != get_display_num() {
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
                    scrap::Frame::VP9(data) => {
                        let send_conn_ids = handle_one_frame_encoded(&sp, data, ms)?;
                        frame_controller.set_send(now, send_conn_ids);
                    }
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
                        if would_block_count >= 1000 {
                            // For now, the user should choose and agree screen sharing agiain.
                            // to-do: Remember choice, attendless...
                            // super::wayland::release_resouce();
                            // bail!("Wayland capturer none 100 times, try restart captuere");
                            log::error!("Wayland capturer none 1000 times, try restart captuere");
                            would_block_count = 0;
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
            check_uac_switch(c.privacy_mode_id, c._captuerer_privacy_mode_id)?;
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
    Ok(())
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
#[cfg(any(target_os = "android", target_os = "ios"))]
fn create_msg(vp9s: Vec<EncodedVideoFrame>) -> Message {
    let mut msg_out = Message::new();
    let mut vf = VideoFrame::new();
    vf.set_vp9s(EncodedVideoFrames {
        frames: vp9s.into(),
        ..Default::default()
    });
    vf.timestamp = hbb_common::get_time();
    msg_out.set_video_frame(vf);
    msg_out
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
#[cfg(any(target_os = "android", target_os = "ios"))]
pub fn handle_one_frame_encoded(
    sp: &GenericService,
    frame: &[u8],
    ms: i64,
) -> ResultType<HashSet<i32>> {
    sp.snapshot(|sps| {
        // so that new sub and old sub share the same encoder after switch
        if sps.has_subscribes() {
            bail!("SWITCH");
        }
        Ok(())
    })?;
    let mut send_conn_ids: HashSet<i32> = Default::default();
    let vp9_frame = EncodedVideoFrame {
        data: frame.to_vec().into(),
        key: true,
        pts: ms,
        ..Default::default()
    };
    send_conn_ids = sp.send_video_frame(create_msg(vec![vp9_frame]));
    Ok(send_conn_ids)
}

fn get_display_num() -> usize {
    #[cfg(target_os = "linux")]
    {
        if !scrap::is_x11() {
            return if let Ok(n) = super::wayland::get_display_num() {
                n
            } else {
                0
            };
        }
    }

    if let Ok(d) = try_get_displays() {
        d.len()
    } else {
        0
    }
}

pub(super) fn get_displays_2(all: &Vec<Display>) -> (usize, Vec<DisplayInfo>) {
    let mut displays = Vec::new();
    let mut primary = 0;
    for (i, d) in all.iter().enumerate() {
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
    (*lock, displays)
}

pub fn is_inited_msg() -> Option<Message> {
    #[cfg(target_os = "linux")]
    if !scrap::is_x11() {
        return super::wayland::is_inited();
    }
    None
}

pub async fn get_displays() -> ResultType<(usize, Vec<DisplayInfo>)> {
    #[cfg(target_os = "linux")]
    {
        if !scrap::is_x11() {
            return super::wayland::get_displays().await;
        }
    }
    // switch to primary display if long time (30 seconds) no users
    if LAST_ACTIVE.lock().unwrap().elapsed().as_secs() >= 30 {
        *CURRENT_DISPLAY.lock().unwrap() = usize::MAX;
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

pub async fn switch_to_primary() {
    switch_display(get_primary() as _).await;
}

#[cfg(not(windows))]
fn try_get_displays() -> ResultType<Vec<Display>> {
    Ok(Display::all()?)
}

#[cfg(windows)]
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

pub fn get_current_display() -> ResultType<(usize, usize, Display)> {
    get_current_display_2(try_get_displays()?)
}

#[cfg(windows)]
fn start_uac_elevation_check() {
    static START: Once = Once::new();
    START.call_once(|| {
        if !crate::platform::is_installed()
            && !crate::platform::is_root()
            && !crate::platform::is_elevated(None).map_or(false, |b| b)
        {
            std::thread::spawn(|| loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
                if let Ok(uac) = crate::ui::win_privacy::is_process_consent_running() {
                    *IS_UAC_RUNNING.lock().unwrap() = uac;
                }
                if let Ok(elevated) = crate::platform::is_foreground_window_elevated() {
                    *IS_FOREGROUND_WINDOW_ELEVATED.lock().unwrap() = elevated;
                }
            });
        }
    });
}
