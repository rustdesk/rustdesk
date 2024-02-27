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

use super::{
    display_service::{check_display_changed, get_display_info},
    service::ServiceTmpl,
    video_qos::VideoQoS,
    *,
};
#[cfg(target_os = "linux")]
use crate::common::SimpleCallOnReturn;
#[cfg(target_os = "linux")]
use crate::platform::linux::is_x11;
use crate::privacy_mode::{get_privacy_mode_conn_id, INVALID_PRIVACY_MODE_CONN_ID};
#[cfg(windows)]
use crate::{
    platform::windows::is_process_consent_running,
    privacy_mode::{is_current_privacy_mode_impl, PRIVACY_MODE_IMPL_WIN_MAG},
    ui_interface::is_installed,
};
use hbb_common::{
    anyhow::anyhow,
    tokio::sync::{
        mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
        Mutex as TokioMutex,
    },
};
#[cfg(feature = "gpucodec")]
use scrap::gpucodec::{GpuEncoder, GpuEncoderConfig};
#[cfg(feature = "hwcodec")]
use scrap::hwcodec::{HwEncoder, HwEncoderConfig};
#[cfg(not(windows))]
use scrap::Capturer;
use scrap::{
    aom::AomEncoderConfig,
    codec::{Encoder, EncoderCfg, Quality},
    record::{Recorder, RecorderContext},
    vpxcodec::{VpxEncoderConfig, VpxVideoCodecId},
    CodecName, Display, Frame, TraitCapturer,
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
pub const OPTION_REFRESH: &'static str = "refresh";

lazy_static::lazy_static! {
    static ref FRAME_FETCHED_NOTIFIER: (UnboundedSender<(i32, Option<Instant>)>, Arc<TokioMutex<UnboundedReceiver<(i32, Option<Instant>)>>>) = {
        let (tx, rx) = unbounded_channel();
        (tx, Arc::new(TokioMutex::new(rx)))
    };
    pub static ref VIDEO_QOS: Arc<Mutex<VideoQoS>> = Default::default();
    pub static ref IS_UAC_RUNNING: Arc<Mutex<bool>> = Default::default();
    pub static ref IS_FOREGROUND_WINDOW_ELEVATED: Arc<Mutex<bool>> = Default::default();
}

#[inline]
pub fn notify_video_frame_fetched(conn_id: i32, frame_tm: Option<Instant>) {
    FRAME_FETCHED_NOTIFIER.0.send((conn_id, frame_tm)).ok();
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

#[derive(Clone)]
pub struct VideoService {
    sp: GenericService,
    idx: usize,
}

impl Deref for VideoService {
    type Target = ServiceTmpl<ConnInner>;

    fn deref(&self) -> &Self::Target {
        &self.sp
    }
}

impl DerefMut for VideoService {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.sp
    }
}

pub fn get_service_name(idx: usize) -> String {
    format!("{}{}", NAME, idx)
}

pub fn new(idx: usize) -> GenericService {
    let vs = VideoService {
        sp: GenericService::new(get_service_name(idx), true),
        idx,
    };
    GenericService::run(&vs, run);
    vs.sp
}

// Capturer object is expensive, avoiding to create it frequently.
fn create_capturer(
    privacy_mode_id: i32,
    display: Display,
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
            if let Some(c1) = crate::privacy_mode::win_mag::create_capturer(
                privacy_mode_id,
                display.origin(),
                display.width(),
                display.height(),
            )? {
                c = Some(Box::new(c1));
            }
        }
    }

    match c {
        Some(c1) => return Ok(c1),
        None => {
            #[cfg(windows)]
            {
                log::debug!("Create capturer dxgi|gdi");
                return crate::portable_service::client::create_capturer(
                    _current,
                    display,
                    _portable_service_running,
                );
            }
            #[cfg(not(windows))]
            {
                log::debug!("Create capturer from scrap");
                return Ok(Box::new(
                    Capturer::new(display).with_context(|| "Failed to create capturer")?,
                ));
            }
        }
    };
}

// This function works on privacy mode. Windows only for now.
pub fn test_create_capturer(
    privacy_mode_id: i32,
    display_idx: usize,
    timeout_millis: u64,
) -> String {
    let test_begin = Instant::now();
    loop {
        let err = match Display::all() {
            Ok(mut displays) => {
                if displays.len() <= display_idx {
                    anyhow!(
                        "Failed to get display {}, the displays' count is {}",
                        display_idx,
                        displays.len()
                    )
                } else {
                    let display = displays.remove(display_idx);
                    match create_capturer(privacy_mode_id, display, display_idx, false) {
                        Ok(_) => return "".to_owned(),
                        Err(e) => e,
                    }
                }
            }
            Err(e) => e.into(),
        };
        if test_begin.elapsed().as_millis() >= timeout_millis as _ {
            return err.to_string();
        }
        std::thread::sleep(Duration::from_millis(300));
    }
}

// Note: This function is extremely expensive, do not call it frequently.
#[cfg(windows)]
fn check_uac_switch(privacy_mode_id: i32, capturer_privacy_mode_id: i32) -> ResultType<()> {
    if capturer_privacy_mode_id != INVALID_PRIVACY_MODE_CONN_ID
        && is_current_privacy_mode_impl(PRIVACY_MODE_IMPL_WIN_MAG)
    {
        if !is_installed() {
            if privacy_mode_id != capturer_privacy_mode_id {
                if !is_process_consent_running()? {
                    bail!("consent.exe is not running");
                }
            }
            if is_process_consent_running()? {
                bail!("consent.exe is running");
            }
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

fn get_capturer(current: usize, portable_service_running: bool) -> ResultType<CapturerInfo> {
    #[cfg(target_os = "linux")]
    {
        if !is_x11() {
            return super::wayland::get_capturer();
        }
    }

    let mut displays = Display::all()?;
    let ndisplay = displays.len();
    if ndisplay <= current {
        bail!(
            "Failed to get display {}, displays len: {}",
            current,
            ndisplay
        );
    }
    let display = displays.remove(current);

    #[cfg(target_os = "linux")]
    if let Display::X11(inner) = &display {
        if let Err(err) = inner.get_shm_status() {
            log::warn!(
                "MIT-SHM extension not working properly on select X11 server: {:?}",
                err
            );
        }
    }

    let (origin, width, height) = (display.origin(), display.width(), display.height());
    let name = display.name();
    log::debug!(
        "#displays={}, current={}, origin: {:?}, width={}, height={}, cpus={}/{}, name:{}",
        ndisplay,
        current,
        &origin,
        width,
        height,
        num_cpus::get_physical(),
        num_cpus::get(),
        &name,
    );

    let privacy_mode_id = get_privacy_mode_conn_id().unwrap_or(INVALID_PRIVACY_MODE_CONN_ID);
    #[cfg(not(windows))]
    let capturer_privacy_mode_id = privacy_mode_id;
    #[cfg(windows)]
    let mut capturer_privacy_mode_id = privacy_mode_id;
    #[cfg(windows)]
    {
        if capturer_privacy_mode_id != INVALID_PRIVACY_MODE_CONN_ID
            && is_current_privacy_mode_impl(PRIVACY_MODE_IMPL_WIN_MAG)
        {
            if !is_installed() {
                if is_process_consent_running()? {
                    capturer_privacy_mode_id = INVALID_PRIVACY_MODE_CONN_ID;
                }
            }
        }
    }
    log::debug!(
        "Try create capturer with capturer privacy mode id {}",
        capturer_privacy_mode_id,
    );

    if privacy_mode_id != INVALID_PRIVACY_MODE_CONN_ID {
        if privacy_mode_id != capturer_privacy_mode_id {
            log::info!("In privacy mode, but show UAC prompt window for now");
        } else {
            log::info!("In privacy mode, the peer side cannot watch the screen");
        }
    }
    let capturer = create_capturer(
        capturer_privacy_mode_id,
        display,
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

fn run(vs: VideoService) -> ResultType<()> {
    let _raii = Raii::new(vs.idx);
    // Wayland only support one video capturer for now. It is ok to call ensure_inited() here.
    //
    // ensure_inited() is needed because clear() may be called.
    // to-do: wayland ensure_inited should pass current display index.
    // But for now, we do not support multi-screen capture on wayland.
    #[cfg(target_os = "linux")]
    super::wayland::ensure_inited()?;
    #[cfg(target_os = "linux")]
    let _wayland_call_on_ret = SimpleCallOnReturn {
        b: true,
        f: Box::new(|| {
            super::wayland::clear();
        }),
    };

    #[cfg(windows)]
    let last_portable_service_running = crate::portable_service::client::running();
    #[cfg(not(windows))]
    let last_portable_service_running = false;

    let display_idx = vs.idx;
    let sp = vs.sp;
    let mut c = get_capturer(display_idx, last_portable_service_running)?;

    let mut video_qos = VIDEO_QOS.lock().unwrap();
    video_qos.refresh(None);
    let mut spf;
    let mut quality = video_qos.quality();
    let record_incoming = !Config::get_option("allow-auto-record-incoming").is_empty();
    let client_record = video_qos.record();
    drop(video_qos);
    let encoder_cfg = get_encoder_config(
        &c,
        display_idx,
        quality,
        client_record || record_incoming,
        last_portable_service_running,
    );
    Encoder::set_fallback(&encoder_cfg);
    let codec_name = Encoder::negotiated_codec();
    let recorder = get_recorder(c.width, c.height, &codec_name, record_incoming);
    let mut encoder;
    let use_i444 = Encoder::use_i444(&encoder_cfg);
    match Encoder::new(encoder_cfg.clone(), use_i444) {
        Ok(x) => encoder = x,
        Err(err) => bail!("Failed to create encoder: {}", err),
    }
    #[cfg(feature = "gpucodec")]
    c.set_output_texture(encoder.input_texture());
    VIDEO_QOS.lock().unwrap().store_bitrate(encoder.bitrate());
    VIDEO_QOS
        .lock()
        .unwrap()
        .set_support_abr(display_idx, encoder.support_abr());
    log::info!("initial quality: {quality:?}");

    if sp.is_option_true(OPTION_REFRESH) {
        sp.set_option_bool(OPTION_REFRESH, false);
    }

    let mut frame_controller = VideoFrameController::new();

    let start = time::Instant::now();
    let mut last_check_displays = time::Instant::now();
    #[cfg(windows)]
    let mut try_gdi = 1;
    #[cfg(windows)]
    log::info!("gdi: {}", c.is_gdi());
    #[cfg(windows)]
    start_uac_elevation_check();

    #[cfg(target_os = "linux")]
    let mut would_block_count = 0u32;
    let mut yuv = Vec::new();
    let mut mid_data = Vec::new();

    while sp.ok() {
        #[cfg(windows)]
        check_uac_switch(c.privacy_mode_id, c._capturer_privacy_mode_id)?;

        let mut video_qos = VIDEO_QOS.lock().unwrap();
        spf = video_qos.spf();
        if quality != video_qos.quality() {
            log::debug!("quality: {:?} -> {:?}", quality, video_qos.quality());
            quality = video_qos.quality();
            allow_err!(encoder.set_quality(quality));
            video_qos.store_bitrate(encoder.bitrate());
        }
        if client_record != video_qos.record() {
            bail!("SWITCH");
        }
        drop(video_qos);

        if sp.is_option_true(OPTION_REFRESH) {
            let _ = try_broadcast_display_changed(&sp, display_idx, &c);
            bail!("SWITCH");
        }
        if codec_name != Encoder::negotiated_codec() {
            bail!("SWITCH");
        }
        #[cfg(windows)]
        if last_portable_service_running != crate::portable_service::client::running() {
            bail!("SWITCH");
        }
        if Encoder::use_i444(&encoder_cfg) != use_i444 {
            bail!("SWITCH");
        }
        #[cfg(all(windows, feature = "gpucodec"))]
        if c.is_gdi() && (codec_name == CodecName::H264GPU || codec_name == CodecName::H265GPU) {
            log::info!("changed to gdi when using gpucodec");
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
            // This check may be redundant, but it is better to be safe.
            // The previous check in `sp.is_option_true(OPTION_REFRESH)` block may be enough.
            try_broadcast_display_changed(&sp, display_idx, &c)?;
        }

        frame_controller.reset();

        let res = match c.frame(spf) {
            Ok(frame) => {
                let time = now - start;
                let ms = (time.as_secs() * 1000 + time.subsec_millis() as u64) as i64;
                if frame.valid() {
                    let send_conn_ids = handle_one_frame(
                        display_idx,
                        &sp,
                        frame,
                        &mut yuv,
                        &mut mid_data,
                        ms,
                        &mut encoder,
                        recorder.clone(),
                    )?;
                    frame_controller.set_send(now, send_conn_ids);
                }
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
                    if !is_x11() {
                        if would_block_count >= 100 {
                            // to-do: Unknown reason for WouldBlock 100 times (seconds = 100 * 1 / fps)
                            // https://github.com/rustdesk/rustdesk/blob/63e6b2f8ab51743e77a151e2b7ff18816f5fa2fb/libs/scrap/src/common/wayland.rs#L81
                            //
                            // Do not reset the capturer for now, as it will cause the prompt to show every few minutes.
                            // https://github.com/rustdesk/rustdesk/issues/4276
                            //
                            // super::wayland::clear();
                            // bail!("Wayland capturer none 100 times, try restart capture");
                        }
                    }
                }
            }
            Err(err) => {
                // Get display information again immediately after error.
                crate::display_service::check_displays_changed().ok();
                // This check may be redundant, but it is better to be safe.
                // The previous check in `sp.is_option_true(OPTION_REFRESH)` block may be enough.
                try_broadcast_display_changed(&sp, display_idx, &c)?;

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

struct Raii(usize);

impl Raii {
    fn new(display_idx: usize) -> Self {
        Raii(display_idx)
    }
}

impl Drop for Raii {
    fn drop(&mut self) {
        #[cfg(feature = "gpucodec")]
        GpuEncoder::set_not_use(self.0, false);
        VIDEO_QOS.lock().unwrap().set_support_abr(self.0, true);
    }
}

fn get_encoder_config(
    c: &CapturerInfo,
    _display_idx: usize,
    quality: Quality,
    record: bool,
    _portable_service: bool,
) -> EncoderCfg {
    #[cfg(all(windows, feature = "gpucodec"))]
    if _portable_service || c.is_gdi() {
        log::info!("gdi:{}, portable:{}", c.is_gdi(), _portable_service);
        GpuEncoder::set_not_use(_display_idx, true);
    }
    #[cfg(feature = "gpucodec")]
    Encoder::update(scrap::codec::EncodingUpdate::Check);
    // https://www.wowza.com/community/t/the-correct-keyframe-interval-in-obs-studio/95162
    let keyframe_interval = if record { Some(240) } else { None };
    let negotiated_codec = Encoder::negotiated_codec();
    match negotiated_codec.clone() {
        CodecName::H264GPU | CodecName::H265GPU => {
            #[cfg(feature = "gpucodec")]
            if let Some(feature) = GpuEncoder::try_get(&c.device(), negotiated_codec.clone()) {
                EncoderCfg::GPU(GpuEncoderConfig {
                    device: c.device(),
                    width: c.width,
                    height: c.height,
                    quality,
                    feature,
                    keyframe_interval,
                })
            } else {
                handle_hw_encoder(
                    negotiated_codec.clone(),
                    c.width,
                    c.height,
                    quality as _,
                    keyframe_interval,
                )
            }
            #[cfg(not(feature = "gpucodec"))]
            handle_hw_encoder(
                negotiated_codec.clone(),
                c.width,
                c.height,
                quality as _,
                keyframe_interval,
            )
        }
        CodecName::H264HW(_name) | CodecName::H265HW(_name) => handle_hw_encoder(
            negotiated_codec.clone(),
            c.width,
            c.height,
            quality as _,
            keyframe_interval,
        ),
        name @ (CodecName::VP8 | CodecName::VP9) => EncoderCfg::VPX(VpxEncoderConfig {
            width: c.width as _,
            height: c.height as _,
            quality,
            codec: if name == CodecName::VP8 {
                VpxVideoCodecId::VP8
            } else {
                VpxVideoCodecId::VP9
            },
            keyframe_interval,
        }),
        CodecName::AV1 => EncoderCfg::AOM(AomEncoderConfig {
            width: c.width as _,
            height: c.height as _,
            quality,
            keyframe_interval,
        }),
    }
}

fn handle_hw_encoder(
    _name: CodecName,
    width: usize,
    height: usize,
    quality: Quality,
    keyframe_interval: Option<usize>,
) -> EncoderCfg {
    let f = || {
        #[cfg(feature = "hwcodec")]
        match _name {
            CodecName::H264GPU | CodecName::H265GPU => {
                if !scrap::codec::enable_hwcodec_option() {
                    return Err(());
                }
                let is_h265 = _name == CodecName::H265GPU;
                let best = HwEncoder::best();
                if let Some(h264) = best.h264 {
                    if !is_h265 {
                        return Ok(EncoderCfg::HW(HwEncoderConfig {
                            name: h264.name,
                            width,
                            height,
                            quality,
                            keyframe_interval,
                        }));
                    }
                }
                if let Some(h265) = best.h265 {
                    if is_h265 {
                        return Ok(EncoderCfg::HW(HwEncoderConfig {
                            name: h265.name,
                            width,
                            height,
                            quality,
                            keyframe_interval,
                        }));
                    }
                }
            }
            CodecName::H264HW(name) | CodecName::H265HW(name) => {
                return Ok(EncoderCfg::HW(HwEncoderConfig {
                    name,
                    width,
                    height,
                    quality,
                    keyframe_interval,
                }));
            }
            _ => {
                return Err(());
            }
        };

        Err(())
    };

    match f() {
        Ok(cfg) => cfg,
        _ => EncoderCfg::VPX(VpxEncoderConfig {
            width: width as _,
            height: height as _,
            quality,
            codec: VpxVideoCodecId::VP9,
            keyframe_interval,
        }),
    }
}

fn get_recorder(
    width: usize,
    height: usize,
    codec_name: &CodecName,
    record_incoming: bool,
) -> Arc<Mutex<Option<Recorder>>> {
    let recorder = if record_incoming {
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

    recorder
}

fn check_privacy_mode_changed(sp: &GenericService, privacy_mode_id: i32) -> ResultType<()> {
    let privacy_mode_id_2 = get_privacy_mode_conn_id().unwrap_or(INVALID_PRIVACY_MODE_CONN_ID);
    if privacy_mode_id != privacy_mode_id_2 {
        if privacy_mode_id_2 != INVALID_PRIVACY_MODE_CONN_ID {
            let msg_out = crate::common::make_privacy_mode_msg(
                back_notification::PrivacyModeState::PrvOnByOther,
                "".to_owned(),
            );
            sp.send_to_others(msg_out, privacy_mode_id_2);
        }
        bail!("SWITCH");
    }
    Ok(())
}

#[inline]
fn handle_one_frame(
    display: usize,
    sp: &GenericService,
    frame: Frame,
    yuv: &mut Vec<u8>,
    mid_data: &mut Vec<u8>,
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

    let frame = frame.to(encoder.yuvfmt(), yuv, mid_data)?;
    let mut send_conn_ids: HashSet<i32> = Default::default();
    match encoder.encode_to_message(frame, ms) {
        Ok(mut vf) => {
            vf.display = display as _;
            let mut msg = Message::new();
            msg.set_video_frame(vf);
            recorder
                .lock()
                .unwrap()
                .as_mut()
                .map(|r| r.write_message(&msg));
            send_conn_ids = sp.send_video_frame(msg);
        }
        Err(e) => match e.to_string().as_str() {
            scrap::codec::ENCODE_NEED_SWITCH => {
                bail!("SWITCH");
            }
            _ => {}
        },
    }
    Ok(send_conn_ids)
}

pub fn is_inited_msg() -> Option<Message> {
    #[cfg(target_os = "linux")]
    if !is_x11() {
        return super::wayland::is_inited();
    }
    None
}

#[inline]
pub fn refresh() {
    #[cfg(target_os = "android")]
    Display::refresh_size();
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

#[inline]
fn try_broadcast_display_changed(
    sp: &GenericService,
    display_idx: usize,
    cap: &CapturerInfo,
) -> ResultType<()> {
    if let Some(display) = check_display_changed(
        cap.ndisplay,
        cap.current,
        (cap.origin.0, cap.origin.1, cap.width, cap.height),
    ) {
        log::info!("Display {} changed", display);
        if let Some(msg_out) = make_display_changed_msg(display_idx, Some(display)) {
            let msg_out = Arc::new(msg_out);
            sp.send_shared(msg_out.clone());
            // switch display may occur before the first video frame, add snapshot to send to new subscribers
            sp.snapshot(move |sps| {
                sps.send_shared(msg_out.clone());
                Ok(())
            })?;
            bail!("SWITCH");
        }
    }
    Ok(())
}

pub fn make_display_changed_msg(
    display_idx: usize,
    opt_display: Option<DisplayInfo>,
) -> Option<Message> {
    let display = match opt_display {
        Some(d) => d,
        None => get_display_info(display_idx)?,
    };
    let mut misc = Misc::new();
    misc.set_switch_display(SwitchDisplay {
        display: display_idx as _,
        x: display.x,
        y: display.y,
        width: display.width,
        height: display.height,
        cursor_embedded: display_service::capture_cursor_embedded(),
        #[cfg(not(target_os = "android"))]
        resolutions: Some(SupportedResolutions {
            resolutions: if display.name.is_empty() {
                vec![]
            } else {
                crate::platform::resolutions(&display.name)
            },
            ..SupportedResolutions::default()
        })
        .into(),
        original_resolution: display.original_resolution,
        ..Default::default()
    });
    let mut msg_out = Message::new();
    msg_out.set_misc(misc);
    Some(msg_out)
}
