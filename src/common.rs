use std::{
    collections::HashMap,
    future::Future,
    sync::{Arc, Mutex, RwLock},
    task::Poll,
};

use serde_json::{json, Map, Value};

use hbb_common::{
    allow_err,
    anyhow::{anyhow, Context},
    bail, base64,
    bytes::Bytes,
    config::{self, Config, CONNECT_TIMEOUT, READ_TIMEOUT, RENDEZVOUS_PORT},
    futures::future::join_all,
    futures_util::future::poll_fn,
    get_version_number, log,
    message_proto::*,
    protobuf::{Enum, Message as _},
    rendezvous_proto::*,
    socket_client,
    sodiumoxide::crypto::{box_, secretbox, sign},
    tcp::FramedStream,
    timeout,
    tokio::{
        self,
        time::{Duration, Instant, Interval},
    },
    ResultType,
};

use crate::{
    hbbs_http::create_http_client_async,
    ui_interface::{get_option, set_option},
};

#[derive(Debug, Eq, PartialEq)]
pub enum GrabState {
    Ready,
    Run,
    Wait,
    Exit,
}

pub type NotifyMessageBox = fn(String, String, String, String) -> dyn Future<Output = ()>;

// the executable name of the portable version
pub const PORTABLE_APPNAME_RUNTIME_ENV_KEY: &str = "RUSTDESK_APPNAME";

pub const PLATFORM_WINDOWS: &str = "Windows";
pub const PLATFORM_LINUX: &str = "Linux";
pub const PLATFORM_MACOS: &str = "Mac OS";
pub const PLATFORM_ANDROID: &str = "Android";

pub const TIMER_OUT: Duration = Duration::from_secs(1);
pub const DEFAULT_KEEP_ALIVE: i32 = 60_000;

const MIN_VER_MULTI_UI_SESSION: &str = "1.2.4";

pub mod input {
    pub const MOUSE_TYPE_MOVE: i32 = 0;
    pub const MOUSE_TYPE_DOWN: i32 = 1;
    pub const MOUSE_TYPE_UP: i32 = 2;
    pub const MOUSE_TYPE_WHEEL: i32 = 3;
    pub const MOUSE_TYPE_TRACKPAD: i32 = 4;

    pub const MOUSE_BUTTON_LEFT: i32 = 0x01;
    pub const MOUSE_BUTTON_RIGHT: i32 = 0x02;
    pub const MOUSE_BUTTON_WHEEL: i32 = 0x04;
    pub const MOUSE_BUTTON_BACK: i32 = 0x08;
    pub const MOUSE_BUTTON_FORWARD: i32 = 0x10;
}

lazy_static::lazy_static! {
    pub static ref SOFTWARE_UPDATE_URL: Arc<Mutex<String>> = Default::default();
    pub static ref DEVICE_ID: Arc<Mutex<String>> = Default::default();
    pub static ref DEVICE_NAME: Arc<Mutex<String>> = Default::default();
}

lazy_static::lazy_static! {
    // Is server process, with "--server" args
    static ref IS_SERVER: bool = std::env::args().nth(1) == Some("--server".to_owned());
    // Is server logic running. The server code can invoked to run by the main process if --server is not running.
    static ref SERVER_RUNNING: Arc<RwLock<bool>> = Default::default();
    static ref IS_MAIN: bool = std::env::args().nth(1).map_or(true, |arg| !arg.starts_with("--"));
    static ref IS_CM: bool = std::env::args().nth(1) == Some("--cm".to_owned()) || std::env::args().nth(1) == Some("--cm-no-ui".to_owned());
}

pub struct SimpleCallOnReturn {
    pub b: bool,
    pub f: Box<dyn Fn() + 'static>,
}

impl Drop for SimpleCallOnReturn {
    fn drop(&mut self) {
        if self.b {
            (self.f)();
        }
    }
}

pub fn global_init() -> bool {
    #[cfg(target_os = "linux")]
    {
        if !crate::platform::linux::is_x11() {
            crate::server::wayland::init();
        }
    }
    true
}

pub fn global_clean() {}

#[inline]
pub fn set_server_running(b: bool) {
    *SERVER_RUNNING.write().unwrap() = b;
}

#[inline]
pub fn is_support_multi_ui_session(ver: &str) -> bool {
    is_support_multi_ui_session_num(hbb_common::get_version_number(ver))
}

#[inline]
pub fn is_support_multi_ui_session_num(ver: i64) -> bool {
    ver >= hbb_common::get_version_number(MIN_VER_MULTI_UI_SESSION)
}

// is server process, with "--server" args
#[inline]
pub fn is_server() -> bool {
    *IS_SERVER
}

#[inline]
pub fn is_main() -> bool {
    *IS_MAIN
}

#[inline]
pub fn is_cm() -> bool {
    *IS_CM
}

// Is server logic running.
#[inline]
pub fn is_server_running() -> bool {
    *SERVER_RUNNING.read().unwrap()
}

#[inline]
pub fn valid_for_numlock(evt: &KeyEvent) -> bool {
    if let Some(key_event::Union::ControlKey(ck)) = evt.union {
        let v = ck.value();
        (v >= ControlKey::Numpad0.value() && v <= ControlKey::Numpad9.value())
            || v == ControlKey::Decimal.value()
    } else {
        false
    }
}

/// Set sound input device.
pub fn set_sound_input(device: String) {
    let prior_device = get_option("audio-input".to_owned());
    if prior_device != device {
        log::info!("switch to audio input device {}", device);
        std::thread::spawn(move || {
            set_option("audio-input".to_owned(), device);
        });
    } else {
        log::info!("audio input is already set to {}", device);
    }
}

/// Get system's default sound input device name.
#[inline]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn get_default_sound_input() -> Option<String> {
    #[cfg(not(target_os = "linux"))]
    {
        use cpal::traits::{DeviceTrait, HostTrait};
        let host = cpal::default_host();
        let dev = host.default_input_device();
        return if let Some(dev) = dev {
            match dev.name() {
                Ok(name) => Some(name),
                Err(_) => None,
            }
        } else {
            None
        };
    }
    #[cfg(target_os = "linux")]
    {
        let input = crate::platform::linux::get_default_pa_source();
        return if let Some(input) = input {
            Some(input.1)
        } else {
            None
        };
    }
}

#[inline]
#[cfg(any(target_os = "android", target_os = "ios"))]
pub fn get_default_sound_input() -> Option<String> {
    None
}

#[cfg(feature = "use_rubato")]
pub fn resample_channels(
    data: &[f32],
    sample_rate0: u32,
    sample_rate: u32,
    channels: u16,
) -> Vec<f32> {
    use rubato::{
        InterpolationParameters, InterpolationType, Resampler, SincFixedIn, WindowFunction,
    };
    let params = InterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: InterpolationType::Nearest,
        oversampling_factor: 160,
        window: WindowFunction::BlackmanHarris2,
    };
    let mut resampler = SincFixedIn::<f64>::new(
        sample_rate as f64 / sample_rate0 as f64,
        params,
        data.len() / (channels as usize),
        channels as _,
    );
    let mut waves_in = Vec::new();
    if channels == 2 {
        waves_in.push(
            data.iter()
                .step_by(2)
                .map(|x| *x as f64)
                .collect::<Vec<_>>(),
        );
        waves_in.push(
            data.iter()
                .skip(1)
                .step_by(2)
                .map(|x| *x as f64)
                .collect::<Vec<_>>(),
        );
    } else {
        waves_in.push(data.iter().map(|x| *x as f64).collect::<Vec<_>>());
    }
    if let Ok(x) = resampler.process(&waves_in) {
        if x.is_empty() {
            Vec::new()
        } else if x.len() == 2 {
            x[0].chunks(1)
                .zip(x[1].chunks(1))
                .flat_map(|(a, b)| a.into_iter().chain(b))
                .map(|x| *x as f32)
                .collect()
        } else {
            x[0].iter().map(|x| *x as f32).collect()
        }
    } else {
        Vec::new()
    }
}

#[cfg(feature = "use_dasp")]
pub fn audio_resample(
    data: &[f32],
    sample_rate0: u32,
    sample_rate: u32,
    channels: u16,
) -> Vec<f32> {
    use dasp::{interpolate::linear::Linear, signal, Signal};
    let n = data.len() / (channels as usize);
    let n = n * sample_rate as usize / sample_rate0 as usize;
    if channels == 2 {
        let mut source = signal::from_interleaved_samples_iter::<_, [_; 2]>(data.iter().cloned());
        let a = source.next();
        let b = source.next();
        let interp = Linear::new(a, b);
        let mut data = Vec::with_capacity(n << 1);
        for x in source
            .from_hz_to_hz(interp, sample_rate0 as _, sample_rate as _)
            .take(n)
        {
            data.push(x[0]);
            data.push(x[1]);
        }
        data
    } else {
        let mut source = signal::from_iter(data.iter().cloned());
        let a = source.next();
        let b = source.next();
        let interp = Linear::new(a, b);
        source
            .from_hz_to_hz(interp, sample_rate0 as _, sample_rate as _)
            .take(n)
            .collect()
    }
}

#[cfg(feature = "use_samplerate")]
pub fn audio_resample(
    data: &[f32],
    sample_rate0: u32,
    sample_rate: u32,
    channels: u16,
) -> Vec<f32> {
    use samplerate::{convert, ConverterType};
    convert(
        sample_rate0 as _,
        sample_rate as _,
        channels as _,
        ConverterType::SincBestQuality,
        data,
    )
    .unwrap_or_default()
}

pub fn audio_rechannel(
    input: Vec<f32>,
    in_hz: u32,
    out_hz: u32,
    in_chan: u16,
    output_chan: u16,
) -> Vec<f32> {
    if in_chan == output_chan {
        return input;
    }
    let mut input = input;
    input.truncate(input.len() / in_chan as usize * in_chan as usize);
    match (in_chan, output_chan) {
        (1, 2) => audio_rechannel_1_2(&input, in_hz, out_hz),
        (1, 3) => audio_rechannel_1_3(&input, in_hz, out_hz),
        (1, 4) => audio_rechannel_1_4(&input, in_hz, out_hz),
        (1, 5) => audio_rechannel_1_5(&input, in_hz, out_hz),
        (1, 6) => audio_rechannel_1_6(&input, in_hz, out_hz),
        (1, 7) => audio_rechannel_1_7(&input, in_hz, out_hz),
        (1, 8) => audio_rechannel_1_8(&input, in_hz, out_hz),
        (2, 1) => audio_rechannel_2_1(&input, in_hz, out_hz),
        (2, 3) => audio_rechannel_2_3(&input, in_hz, out_hz),
        (2, 4) => audio_rechannel_2_4(&input, in_hz, out_hz),
        (2, 5) => audio_rechannel_2_5(&input, in_hz, out_hz),
        (2, 6) => audio_rechannel_2_6(&input, in_hz, out_hz),
        (2, 7) => audio_rechannel_2_7(&input, in_hz, out_hz),
        (2, 8) => audio_rechannel_2_8(&input, in_hz, out_hz),
        (3, 1) => audio_rechannel_3_1(&input, in_hz, out_hz),
        (3, 2) => audio_rechannel_3_2(&input, in_hz, out_hz),
        (3, 4) => audio_rechannel_3_4(&input, in_hz, out_hz),
        (3, 5) => audio_rechannel_3_5(&input, in_hz, out_hz),
        (3, 6) => audio_rechannel_3_6(&input, in_hz, out_hz),
        (3, 7) => audio_rechannel_3_7(&input, in_hz, out_hz),
        (3, 8) => audio_rechannel_3_8(&input, in_hz, out_hz),
        (4, 1) => audio_rechannel_4_1(&input, in_hz, out_hz),
        (4, 2) => audio_rechannel_4_2(&input, in_hz, out_hz),
        (4, 3) => audio_rechannel_4_3(&input, in_hz, out_hz),
        (4, 5) => audio_rechannel_4_5(&input, in_hz, out_hz),
        (4, 6) => audio_rechannel_4_6(&input, in_hz, out_hz),
        (4, 7) => audio_rechannel_4_7(&input, in_hz, out_hz),
        (4, 8) => audio_rechannel_4_8(&input, in_hz, out_hz),
        (5, 1) => audio_rechannel_5_1(&input, in_hz, out_hz),
        (5, 2) => audio_rechannel_5_2(&input, in_hz, out_hz),
        (5, 3) => audio_rechannel_5_3(&input, in_hz, out_hz),
        (5, 4) => audio_rechannel_5_4(&input, in_hz, out_hz),
        (5, 6) => audio_rechannel_5_6(&input, in_hz, out_hz),
        (5, 7) => audio_rechannel_5_7(&input, in_hz, out_hz),
        (5, 8) => audio_rechannel_5_8(&input, in_hz, out_hz),
        (6, 1) => audio_rechannel_6_1(&input, in_hz, out_hz),
        (6, 2) => audio_rechannel_6_2(&input, in_hz, out_hz),
        (6, 3) => audio_rechannel_6_3(&input, in_hz, out_hz),
        (6, 4) => audio_rechannel_6_4(&input, in_hz, out_hz),
        (6, 5) => audio_rechannel_6_5(&input, in_hz, out_hz),
        (6, 7) => audio_rechannel_6_7(&input, in_hz, out_hz),
        (6, 8) => audio_rechannel_6_8(&input, in_hz, out_hz),
        (7, 1) => audio_rechannel_7_1(&input, in_hz, out_hz),
        (7, 2) => audio_rechannel_7_2(&input, in_hz, out_hz),
        (7, 3) => audio_rechannel_7_3(&input, in_hz, out_hz),
        (7, 4) => audio_rechannel_7_4(&input, in_hz, out_hz),
        (7, 5) => audio_rechannel_7_5(&input, in_hz, out_hz),
        (7, 6) => audio_rechannel_7_6(&input, in_hz, out_hz),
        (7, 8) => audio_rechannel_7_8(&input, in_hz, out_hz),
        (8, 1) => audio_rechannel_8_1(&input, in_hz, out_hz),
        (8, 2) => audio_rechannel_8_2(&input, in_hz, out_hz),
        (8, 3) => audio_rechannel_8_3(&input, in_hz, out_hz),
        (8, 4) => audio_rechannel_8_4(&input, in_hz, out_hz),
        (8, 5) => audio_rechannel_8_5(&input, in_hz, out_hz),
        (8, 6) => audio_rechannel_8_6(&input, in_hz, out_hz),
        (8, 7) => audio_rechannel_8_7(&input, in_hz, out_hz),
        _ => input,
    }
}

macro_rules! audio_rechannel {
    ($name:ident, $in_channels:expr, $out_channels:expr) => {
        fn $name(input: &[f32], in_hz: u32, out_hz: u32) -> Vec<f32> {
            use fon::{chan::Ch32, Audio, Frame};
            let mut in_audio =
                Audio::<Ch32, $in_channels>::with_silence(in_hz, input.len() / $in_channels);
            for (x, y) in input.chunks_exact($in_channels).zip(in_audio.iter_mut()) {
                let mut f = Frame::<Ch32, $in_channels>::default();
                let mut i = 0;
                for c in f.channels_mut() {
                    *c = x[i].into();
                    i += 1;
                }
                *y = f;
            }
            Audio::<Ch32, $out_channels>::with_audio(out_hz, &in_audio)
                .as_f32_slice()
                .to_owned()
        }
    };
}

audio_rechannel!(audio_rechannel_1_2, 1, 2);
audio_rechannel!(audio_rechannel_1_3, 1, 3);
audio_rechannel!(audio_rechannel_1_4, 1, 4);
audio_rechannel!(audio_rechannel_1_5, 1, 5);
audio_rechannel!(audio_rechannel_1_6, 1, 6);
audio_rechannel!(audio_rechannel_1_7, 1, 7);
audio_rechannel!(audio_rechannel_1_8, 1, 8);
audio_rechannel!(audio_rechannel_2_1, 2, 1);
audio_rechannel!(audio_rechannel_2_3, 2, 3);
audio_rechannel!(audio_rechannel_2_4, 2, 4);
audio_rechannel!(audio_rechannel_2_5, 2, 5);
audio_rechannel!(audio_rechannel_2_6, 2, 6);
audio_rechannel!(audio_rechannel_2_7, 2, 7);
audio_rechannel!(audio_rechannel_2_8, 2, 8);
audio_rechannel!(audio_rechannel_3_1, 3, 1);
audio_rechannel!(audio_rechannel_3_2, 3, 2);
audio_rechannel!(audio_rechannel_3_4, 3, 4);
audio_rechannel!(audio_rechannel_3_5, 3, 5);
audio_rechannel!(audio_rechannel_3_6, 3, 6);
audio_rechannel!(audio_rechannel_3_7, 3, 7);
audio_rechannel!(audio_rechannel_3_8, 3, 8);
audio_rechannel!(audio_rechannel_4_1, 4, 1);
audio_rechannel!(audio_rechannel_4_2, 4, 2);
audio_rechannel!(audio_rechannel_4_3, 4, 3);
audio_rechannel!(audio_rechannel_4_5, 4, 5);
audio_rechannel!(audio_rechannel_4_6, 4, 6);
audio_rechannel!(audio_rechannel_4_7, 4, 7);
audio_rechannel!(audio_rechannel_4_8, 4, 8);
audio_rechannel!(audio_rechannel_5_1, 5, 1);
audio_rechannel!(audio_rechannel_5_2, 5, 2);
audio_rechannel!(audio_rechannel_5_3, 5, 3);
audio_rechannel!(audio_rechannel_5_4, 5, 4);
audio_rechannel!(audio_rechannel_5_6, 5, 6);
audio_rechannel!(audio_rechannel_5_7, 5, 7);
audio_rechannel!(audio_rechannel_5_8, 5, 8);
audio_rechannel!(audio_rechannel_6_1, 6, 1);
audio_rechannel!(audio_rechannel_6_2, 6, 2);
audio_rechannel!(audio_rechannel_6_3, 6, 3);
audio_rechannel!(audio_rechannel_6_4, 6, 4);
audio_rechannel!(audio_rechannel_6_5, 6, 5);
audio_rechannel!(audio_rechannel_6_7, 6, 7);
audio_rechannel!(audio_rechannel_6_8, 6, 8);
audio_rechannel!(audio_rechannel_7_1, 7, 1);
audio_rechannel!(audio_rechannel_7_2, 7, 2);
audio_rechannel!(audio_rechannel_7_3, 7, 3);
audio_rechannel!(audio_rechannel_7_4, 7, 4);
audio_rechannel!(audio_rechannel_7_5, 7, 5);
audio_rechannel!(audio_rechannel_7_6, 7, 6);
audio_rechannel!(audio_rechannel_7_8, 7, 8);
audio_rechannel!(audio_rechannel_8_1, 8, 1);
audio_rechannel!(audio_rechannel_8_2, 8, 2);
audio_rechannel!(audio_rechannel_8_3, 8, 3);
audio_rechannel!(audio_rechannel_8_4, 8, 4);
audio_rechannel!(audio_rechannel_8_5, 8, 5);
audio_rechannel!(audio_rechannel_8_6, 8, 6);
audio_rechannel!(audio_rechannel_8_7, 8, 7);

pub fn test_nat_type() {
    let mut i = 0;
    std::thread::spawn(move || loop {
        match test_nat_type_() {
            Ok(true) => break,
            Err(err) => {
                log::error!("test nat: {}", err);
            }
            _ => {}
        }
        if Config::get_nat_type() != 0 {
            break;
        }
        i = i * 2 + 1;
        if i > 300 {
            i = 300;
        }
        std::thread::sleep(std::time::Duration::from_secs(i));
    });
}

#[tokio::main(flavor = "current_thread")]
async fn test_nat_type_() -> ResultType<bool> {
    log::info!("Testing nat ...");
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let is_direct = crate::ipc::get_socks_async(1_000).await.is_none(); // sync socks BTW
    #[cfg(any(target_os = "android", target_os = "ios"))]
    let is_direct = Config::get_socks().is_none(); // sync socks BTW
    if !is_direct {
        Config::set_nat_type(NatType::SYMMETRIC as _);
        return Ok(true);
    }
    let start = std::time::Instant::now();
    let (rendezvous_server, _, _) = get_rendezvous_server(1_000).await;
    let server1 = rendezvous_server;
    let server2 = crate::increase_port(&server1, -1);
    let mut msg_out = RendezvousMessage::new();
    let serial = Config::get_serial();
    msg_out.set_test_nat_request(TestNatRequest {
        serial,
        ..Default::default()
    });
    let mut port1 = 0;
    let mut port2 = 0;
    let mut local_addr = None;
    for i in 0..2 {
        let server = if i == 0 { &*server1 } else { &*server2 };
        let mut socket =
            socket_client::connect_tcp_local(server, local_addr, CONNECT_TIMEOUT).await?;
        if i == 0 {
            // reuse the local addr is required for nat test
            local_addr = Some(socket.local_addr());
            Config::set_option(
                "local-ip-addr".to_owned(),
                socket.local_addr().ip().to_string(),
            );
        }
        socket.send(&msg_out).await?;
        if let Some(msg_in) = get_next_nonkeyexchange_msg(&mut socket, None).await {
            if let Some(rendezvous_message::Union::TestNatResponse(tnr)) = msg_in.union {
                log::debug!("Got nat response from {}: port={}", server, tnr.port);
                if i == 0 {
                    port1 = tnr.port;
                } else {
                    port2 = tnr.port;
                }
                if let Some(cu) = tnr.cu.as_ref() {
                    Config::set_option(
                        "rendezvous-servers".to_owned(),
                        cu.rendezvous_servers.join(","),
                    );
                    Config::set_serial(cu.serial);
                }
            }
        } else {
            break;
        }
    }
    let ok = port1 > 0 && port2 > 0;
    if ok {
        let t = if port1 == port2 {
            NatType::ASYMMETRIC
        } else {
            NatType::SYMMETRIC
        };
        Config::set_nat_type(t as _);
        log::info!("Tested nat type: {:?} in {:?}", t, start.elapsed());
    }
    Ok(ok)
}

pub async fn get_rendezvous_server(ms_timeout: u64) -> (String, Vec<String>, bool) {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    let (mut a, mut b) = get_rendezvous_server_(ms_timeout);
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let (mut a, mut b) = get_rendezvous_server_(ms_timeout).await;
    #[cfg(windows)]
    if let Ok(lic) = crate::platform::get_license_from_exe_name() {
        if !lic.host.is_empty() {
            a = lic.host;
        }
    }
    let mut b: Vec<String> = b
        .drain(..)
        .map(|x| socket_client::check_port(x, config::RENDEZVOUS_PORT))
        .collect();
    let c = if b.contains(&a) {
        b = b.drain(..).filter(|x| x != &a).collect();
        true
    } else {
        a = b.pop().unwrap_or(a);
        false
    };
    (a, b, c)
}

#[inline]
#[cfg(any(target_os = "android", target_os = "ios"))]
fn get_rendezvous_server_(_ms_timeout: u64) -> (String, Vec<String>) {
    (
        Config::get_rendezvous_server(),
        Config::get_rendezvous_servers(),
    )
}

#[inline]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
async fn get_rendezvous_server_(ms_timeout: u64) -> (String, Vec<String>) {
    crate::ipc::get_rendezvous_server(ms_timeout).await
}

#[inline]
#[cfg(any(target_os = "android", target_os = "ios"))]
pub async fn get_nat_type(_ms_timeout: u64) -> i32 {
    Config::get_nat_type()
}

#[inline]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub async fn get_nat_type(ms_timeout: u64) -> i32 {
    crate::ipc::get_nat_type(ms_timeout).await
}

// used for client to test which server is faster in case stop-servic=Y
#[tokio::main(flavor = "current_thread")]
async fn test_rendezvous_server_() {
    let servers = Config::get_rendezvous_servers();
    if servers.len() <= 1 {
        return;
    }
    let mut futs = Vec::new();
    for host in servers {
        futs.push(tokio::spawn(async move {
            let tm = std::time::Instant::now();
            if socket_client::connect_tcp(
                crate::check_port(&host, RENDEZVOUS_PORT),
                CONNECT_TIMEOUT,
            )
            .await
            .is_ok()
            {
                let elapsed = tm.elapsed().as_micros();
                Config::update_latency(&host, elapsed as _);
            } else {
                Config::update_latency(&host, -1);
            }
        }));
    }
    join_all(futs).await;
    Config::reset_online();
}

// #[cfg(any(target_os = "android", target_os = "ios", feature = "cli"))]
pub fn test_rendezvous_server() {
    std::thread::spawn(test_rendezvous_server_);
}

pub fn refresh_rendezvous_server() {
    #[cfg(any(target_os = "android", target_os = "ios", feature = "cli"))]
    test_rendezvous_server();
    #[cfg(not(any(target_os = "android", target_os = "ios", feature = "cli")))]
    std::thread::spawn(|| {
        if crate::ipc::test_rendezvous_server().is_err() {
            test_rendezvous_server();
        }
    });
}

pub fn run_me<T: AsRef<std::ffi::OsStr>>(args: Vec<T>) -> std::io::Result<std::process::Child> {
    #[cfg(target_os = "linux")]
    if let Ok(appdir) = std::env::var("APPDIR") {
        let appimage_cmd = std::path::Path::new(&appdir).join("AppRun");
        if appimage_cmd.exists() {
            log::info!("path: {:?}", appimage_cmd);
            return std::process::Command::new(appimage_cmd).args(&args).spawn();
        }
    }
    let cmd = std::env::current_exe()?;
    let mut cmd = std::process::Command::new(cmd);
    #[cfg(windows)]
    let mut force_foreground = false;
    #[cfg(windows)]
    {
        let arg_strs = args
            .iter()
            .map(|x| x.as_ref().to_string_lossy())
            .collect::<Vec<_>>();
        if arg_strs == vec!["--install"] || arg_strs == &["--noinstall"] {
            cmd.env(crate::platform::SET_FOREGROUND_WINDOW, "1");
            force_foreground = true;
        }
    }
    let result = cmd.args(&args).spawn();
    match result.as_ref() {
        Ok(_child) =>
        {
            #[cfg(windows)]
            if force_foreground {
                unsafe { winapi::um::winuser::AllowSetForegroundWindow(_child.id() as u32) };
            }
        }
        Err(err) => log::error!("run_me: {err:?}"),
    }
    result
}

#[inline]
pub fn username() -> String {
    // fix bug of whoami
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    return whoami::username().trim_end_matches('\0').to_owned();
    #[cfg(any(target_os = "android", target_os = "ios"))]
    return DEVICE_NAME.lock().unwrap().clone();
}

#[inline]
pub fn hostname() -> String {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        #[allow(unused_mut)]
        let mut name = whoami::hostname();
        // some time, there is .local, some time not, so remove it for osx
        #[cfg(target_os = "macos")]
        if name.ends_with(".local") {
            name = name.trim_end_matches(".local").to_owned();
        }
        name
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    return DEVICE_NAME.lock().unwrap().clone();
}

#[inline]
pub fn get_sysinfo() -> serde_json::Value {
    use hbb_common::sysinfo::System;
    let mut system = System::new();
    system.refresh_memory();
    system.refresh_cpu();
    let memory = system.total_memory();
    let memory = (memory as f64 / 1024. / 1024. / 1024. * 100.).round() / 100.;
    let cpus = system.cpus();
    let cpu_name = cpus.first().map(|x| x.brand()).unwrap_or_default();
    let cpu_name = cpu_name.trim_end();
    let cpu_freq = cpus.first().map(|x| x.frequency()).unwrap_or_default();
    let cpu_freq = (cpu_freq as f64 / 1024. * 100.).round() / 100.;
    let cpu = if cpu_freq > 0. {
        format!("{}, {}GHz, ", cpu_name, cpu_freq)
    } else {
        "".to_owned() // android
    };
    let num_cpus = num_cpus::get();
    let num_pcpus = num_cpus::get_physical();
    let mut os = system.distribution_id();
    os = format!("{} / {}", os, system.long_os_version().unwrap_or_default());
    #[cfg(windows)]
    {
        os = format!("{os} - {}", system.os_version().unwrap_or_default());
    }
    let hostname = hostname(); // sys.hostname() return localhost on android in my test
    use serde_json::json;
    #[cfg(any(target_os = "android", target_os = "ios"))]
    let out;
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let mut out;
    out = json!({
        "cpu": format!("{cpu}{num_cpus}/{num_pcpus} cores"),
        "memory": format!("{memory}GB"),
        "os": os,
        "hostname": hostname,
    });
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let username = crate::platform::get_active_username();
        if !username.is_empty() && (!cfg!(windows) || username != "SYSTEM") {
            out["username"] = json!(username);
        }
    }
    out
}

#[inline]
pub fn check_port<T: std::string::ToString>(host: T, port: i32) -> String {
    hbb_common::socket_client::check_port(host, port)
}

#[inline]
pub fn increase_port<T: std::string::ToString>(host: T, offset: i32) -> String {
    hbb_common::socket_client::increase_port(host, offset)
}

pub const POSTFIX_SERVICE: &'static str = "_service";

#[inline]
pub fn is_control_key(evt: &KeyEvent, key: &ControlKey) -> bool {
    if let Some(key_event::Union::ControlKey(ck)) = evt.union {
        ck.value() == key.value()
    } else {
        false
    }
}

#[inline]
pub fn is_modifier(evt: &KeyEvent) -> bool {
    if let Some(key_event::Union::ControlKey(ck)) = evt.union {
        let v = ck.value();
        v == ControlKey::Alt.value()
            || v == ControlKey::Shift.value()
            || v == ControlKey::Control.value()
            || v == ControlKey::Meta.value()
            || v == ControlKey::RAlt.value()
            || v == ControlKey::RShift.value()
            || v == ControlKey::RControl.value()
            || v == ControlKey::RWin.value()
    } else {
        false
    }
}

pub fn check_software_update() {
    std::thread::spawn(move || allow_err!(check_software_update_()));
}

#[tokio::main(flavor = "current_thread")]
async fn check_software_update_() -> hbb_common::ResultType<()> {
    let url = "https://github.com/rustdesk/rustdesk/releases/latest";
    let latest_release_response = create_http_client_async().get(url).send().await?;
    let latest_release_version = latest_release_response
        .url()
        .path()
        .rsplit('/')
        .next()
        .unwrap_or_default();

    let response_url = latest_release_response.url().to_string();

    if get_version_number(&latest_release_version) > get_version_number(crate::VERSION) {
        #[cfg(feature = "flutter")]
        {
            let mut m = HashMap::new();
            m.insert("name", "check_software_update_finish");
            m.insert("url", &response_url);
            if let Ok(data) = serde_json::to_string(&m) {
                let _ = crate::flutter::push_global_event(crate::flutter::APP_TYPE_MAIN, data);
            }
        }
        *SOFTWARE_UPDATE_URL.lock().unwrap() = response_url;
    }
    Ok(())
}

#[inline]
pub fn get_app_name() -> String {
    hbb_common::config::APP_NAME.read().unwrap().clone()
}

#[inline]
pub fn is_rustdesk() -> bool {
    hbb_common::config::APP_NAME.read().unwrap().eq("RustDesk")
}

#[inline]
pub fn get_uri_prefix() -> String {
    format!("{}://", get_app_name().to_lowercase())
}

#[cfg(target_os = "macos")]
pub fn get_full_name() -> String {
    format!(
        "{}.{}",
        hbb_common::config::ORG.read().unwrap(),
        hbb_common::config::APP_NAME.read().unwrap(),
    )
}

pub fn is_setup(name: &str) -> bool {
    name.to_lowercase().ends_with("install.exe")
}

pub fn get_custom_rendezvous_server(custom: String) -> String {
    #[cfg(windows)]
    if let Ok(lic) = crate::platform::windows::get_license_from_exe_name() {
        if !lic.host.is_empty() {
            return lic.host.clone();
        }
    }
    if !custom.is_empty() {
        return custom;
    }
    if !config::PROD_RENDEZVOUS_SERVER.read().unwrap().is_empty() {
        return config::PROD_RENDEZVOUS_SERVER.read().unwrap().clone();
    }
    "".to_owned()
}

pub fn get_api_server(api: String, custom: String) -> String {
    #[cfg(windows)]
    if let Ok(lic) = crate::platform::windows::get_license_from_exe_name() {
        if !lic.api.is_empty() {
            return lic.api.clone();
        }
    }
    if !api.is_empty() {
        return api.to_owned();
    }
    let api = option_env!("API_SERVER").unwrap_or_default();
    if !api.is_empty() {
        return api.into();
    }
    let s0 = get_custom_rendezvous_server(custom);
    if !s0.is_empty() {
        let s = crate::increase_port(&s0, -2);
        if s == s0 {
            return format!("http://{}:{}", s, config::RENDEZVOUS_PORT - 2);
        } else {
            return format!("http://{}", s);
        }
    }
    "http://sy.dl999.cn".to_owned()
}

pub fn get_audit_server(api: String, custom: String, typ: String) -> String {
    let url = get_api_server(api, custom);
    if url.is_empty() || url.contains("rustdesk.com") {
        return "".to_owned();
    }
    format!("{}/api/audit/{}", url, typ)
}

pub async fn post_request(url: String, body: String, header: &str) -> ResultType<String> {
    let mut req = create_http_client_async().post(url);
    if !header.is_empty() {
        let tmp: Vec<&str> = header.split(": ").collect();
        if tmp.len() == 2 {
            req = req.header(tmp[0], tmp[1]);
        }
    }
    req = req.header("Content-Type", "application/json");
    let to = std::time::Duration::from_secs(12);
    Ok(req.body(body).timeout(to).send().await?.text().await?)
}

#[tokio::main(flavor = "current_thread")]
pub async fn post_request_sync(url: String, body: String, header: &str) -> ResultType<String> {
    post_request(url, body, header).await
}

#[tokio::main(flavor = "current_thread")]
pub async fn http_request_sync(
    url: String,
    method: String,
    body: Option<String>,
    header: String,
) -> ResultType<String> {
    let http_client = create_http_client_async();
    let mut http_client = match method.as_str() {
        "get" => http_client.get(url),
        "post" => http_client.post(url),
        "put" => http_client.put(url),
        "delete" => http_client.delete(url),
        _ => return Err(anyhow!("The HTTP request method is not supported!")),
    };
    let v = serde_json::from_str(header.as_str())?;

    if let Value::Object(obj) = v {
        for (key, value) in obj.iter() {
            http_client = http_client.header(key, value.as_str().unwrap_or_default());
        }
    } else {
        return Err(anyhow!("HTTP header information parsing failed!"));
    }

    if let Some(b) = body {
        http_client = http_client.body(b);
    }

    let response = http_client
        .timeout(std::time::Duration::from_secs(12))
        .send()
        .await?;

    // Serialize response headers
    let mut response_headers = serde_json::map::Map::new();
    for (key, value) in response.headers() {
        response_headers.insert(
            key.to_string(),
            serde_json::json!(value.to_str().unwrap_or("")),
        );
    }

    let status_code = response.status().as_u16();
    let response_body = response.text().await?;

    // Construct the JSON object
    let mut result = serde_json::map::Map::new();
    result.insert("status_code".to_string(), serde_json::json!(status_code));
    result.insert(
        "headers".to_string(),
        serde_json::Value::Object(response_headers),
    );
    result.insert("body".to_string(), serde_json::json!(response_body));

    // Convert map to JSON string
    serde_json::to_string(&result).map_err(|e| anyhow!("Failed to serialize response: {}", e))
}

#[inline]
pub fn make_privacy_mode_msg_with_details(
    state: back_notification::PrivacyModeState,
    details: String,
    impl_key: String,
) -> Message {
    let mut misc = Misc::new();
    let mut back_notification = BackNotification {
        details,
        impl_key,
        ..Default::default()
    };
    back_notification.set_privacy_mode_state(state);
    misc.set_back_notification(back_notification);
    let mut msg_out = Message::new();
    msg_out.set_misc(misc);
    msg_out
}

#[inline]
pub fn make_privacy_mode_msg(
    state: back_notification::PrivacyModeState,
    impl_key: String,
) -> Message {
    make_privacy_mode_msg_with_details(state, "".to_owned(), impl_key)
}

pub fn is_keyboard_mode_supported(
    keyboard_mode: &KeyboardMode,
    version_number: i64,
    peer_platform: &str,
) -> bool {
    match keyboard_mode {
        KeyboardMode::Legacy => true,
        KeyboardMode::Map => {
            if peer_platform.to_lowercase() == crate::PLATFORM_ANDROID.to_lowercase() {
                false
            } else {
                version_number >= hbb_common::get_version_number("1.2.0")
            }
        }
        KeyboardMode::Translate => version_number >= hbb_common::get_version_number("1.2.0"),
        KeyboardMode::Auto => version_number >= hbb_common::get_version_number("1.2.0"),
    }
}

pub fn get_supported_keyboard_modes(version: i64, peer_platform: &str) -> Vec<KeyboardMode> {
    KeyboardMode::iter()
        .filter(|&mode| is_keyboard_mode_supported(mode, version, peer_platform))
        .map(|&mode| mode)
        .collect::<Vec<_>>()
}

pub fn make_fd_to_json(id: i32, path: String, entries: &Vec<FileEntry>) -> String {
    let fd_json = _make_fd_to_json(id, path, entries);
    serde_json::to_string(&fd_json).unwrap_or("".into())
}

pub fn _make_fd_to_json(id: i32, path: String, entries: &Vec<FileEntry>) -> Map<String, Value> {
    use serde_json::json;
    let mut fd_json = serde_json::Map::new();
    fd_json.insert("id".into(), json!(id));
    fd_json.insert("path".into(), json!(path));

    let mut entries_out = vec![];
    for entry in entries {
        let mut entry_map = serde_json::Map::new();
        entry_map.insert("entry_type".into(), json!(entry.entry_type.value()));
        entry_map.insert("name".into(), json!(entry.name));
        entry_map.insert("size".into(), json!(entry.size));
        entry_map.insert("modified_time".into(), json!(entry.modified_time));
        entries_out.push(entry_map);
    }
    fd_json.insert("entries".into(), json!(entries_out));
    fd_json
}

pub fn make_vec_fd_to_json(fds: &[FileDirectory]) -> String {
    let mut fd_jsons = vec![];

    for fd in fds.iter() {
        let fd_json = _make_fd_to_json(fd.id, fd.path.clone(), &fd.entries);
        fd_jsons.push(fd_json);
    }

    serde_json::to_string(&fd_jsons).unwrap_or("".into())
}

pub fn make_empty_dirs_response_to_json(res: &ReadEmptyDirsResponse) -> String {
    let mut map: Map<String, Value> = serde_json::Map::new();
    map.insert("path".into(), json!(res.path));

    let mut fd_jsons = vec![];

    for fd in res.empty_dirs.iter() {
        let fd_json = _make_fd_to_json(fd.id, fd.path.clone(), &fd.entries);
        fd_jsons.push(fd_json);
    }
    map.insert("empty_dirs".into(), fd_jsons.into());

    serde_json::to_string(&map).unwrap_or("".into())
}

/// The function to handle the url scheme sent by the system.
///
/// 1. Try to send the url scheme from ipc.
/// 2. If failed to send the url scheme, we open a new main window to handle this url scheme.
pub fn handle_url_scheme(url: String) {
    #[cfg(not(target_os = "ios"))]
    if let Err(err) = crate::ipc::send_url_scheme(url.clone()) {
        log::debug!("Send the url to the existing flutter process failed, {}. Let's open a new program to handle this.", err);
        let _ = crate::run_me(vec![url]);
    }
}

#[inline]
pub fn encode64<T: AsRef<[u8]>>(input: T) -> String {
    #[allow(deprecated)]
    base64::encode(input)
}

#[inline]
pub fn decode64<T: AsRef<[u8]>>(input: T) -> Result<Vec<u8>, base64::DecodeError> {
    #[allow(deprecated)]
    base64::decode(input)
}

pub async fn get_key(sync: bool) -> String {
    #[cfg(windows)]
    if let Ok(lic) = crate::platform::windows::get_license_from_exe_name() {
        if !lic.key.is_empty() {
            return lic.key;
        }
    }
    #[cfg(target_os = "ios")]
    let mut key = Config::get_option("key");
    #[cfg(not(target_os = "ios"))]
    let mut key = if sync {
        Config::get_option("key")
    } else {
        let mut options = crate::ipc::get_options_async().await;
        options.remove("key").unwrap_or_default()
    };
    if key.is_empty() {
        key = config::RS_PUB_KEY.to_owned();
    }
    key
}

pub fn pk_to_fingerprint(pk: Vec<u8>) -> String {
    let s: String = pk.iter().map(|u| format!("{:02x}", u)).collect();
    s.chars()
        .enumerate()
        .map(|(i, c)| {
            if i > 0 && i % 4 == 0 {
                format!(" {}", c)
            } else {
                format!("{}", c)
            }
        })
        .collect()
}

#[inline]
pub async fn get_next_nonkeyexchange_msg(
    conn: &mut FramedStream,
    timeout: Option<u64>,
) -> Option<RendezvousMessage> {
    let timeout = timeout.unwrap_or(READ_TIMEOUT);
    for _ in 0..2 {
        if let Some(Ok(bytes)) = conn.next_timeout(timeout).await {
            if let Ok(msg_in) = RendezvousMessage::parse_from_bytes(&bytes) {
                match &msg_in.union {
                    Some(rendezvous_message::Union::KeyExchange(_)) => {
                        continue;
                    }
                    _ => {
                        return Some(msg_in);
                    }
                }
            }
        }
        break;
    }
    None
}

#[allow(unused_mut)]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn check_process(arg: &str, mut same_uid: bool) -> bool {
    #[cfg(target_os = "macos")]
    if !crate::platform::is_root() && !same_uid {
        log::warn!("Can not get other process's command line arguments on macos without root");
        same_uid = true;
    }
    use hbb_common::sysinfo::System;
    let mut sys = System::new();
    sys.refresh_processes();
    let mut path = std::env::current_exe().unwrap_or_default();
    if let Ok(linked) = path.read_link() {
        path = linked;
    }
    let path = path.to_string_lossy().to_lowercase();
    let my_uid = sys
        .process((std::process::id() as usize).into())
        .map(|x| x.user_id())
        .unwrap_or_default();
    for (_, p) in sys.processes().iter() {
        let mut cur_path = p.exe().to_path_buf();
        if let Ok(linked) = cur_path.read_link() {
            cur_path = linked;
        }
        if cur_path.to_string_lossy().to_lowercase() != path {
            continue;
        }
        if p.pid().to_string() == std::process::id().to_string() {
            continue;
        }
        if same_uid && p.user_id() != my_uid {
            continue;
        }
        // on mac, p.cmd() get "/Applications/RustDesk.app/Contents/MacOS/RustDesk", "XPC_SERVICE_NAME=com.carriez.RustDesk_server"
        let parg = if p.cmd().len() <= 1 { "" } else { &p.cmd()[1] };
        if arg.is_empty() {
            if !parg.starts_with("--") {
                return true;
            }
        } else if arg == parg {
            return true;
        }
    }
    false
}

pub async fn secure_tcp(conn: &mut FramedStream, key: &str) -> ResultType<()> {
    let rs_pk = get_rs_pk(key);
    let Some(rs_pk) = rs_pk else {
        bail!("Handshake failed: invalid public key from rendezvous server");
    };
    match timeout(READ_TIMEOUT, conn.next()).await? {
        Some(Ok(bytes)) => {
            if let Ok(msg_in) = RendezvousMessage::parse_from_bytes(&bytes) {
                match msg_in.union {
                    Some(rendezvous_message::Union::KeyExchange(ex)) => {
                        if ex.keys.len() != 1 {
                            bail!("Handshake failed: invalid key exchange message");
                        }
                        let their_pk_b = sign::verify(&ex.keys[0], &rs_pk)
                            .map_err(|_| anyhow!("Signature mismatch in key exchange"))?;
                        let (asymmetric_value, symmetric_value, key) = create_symmetric_key_msg(
                            get_pk(&their_pk_b)
                                .context("Wrong their public length in key exchange")?,
                        );
                        let mut msg_out = RendezvousMessage::new();
                        msg_out.set_key_exchange(KeyExchange {
                            keys: vec![asymmetric_value, symmetric_value],
                            ..Default::default()
                        });
                        timeout(CONNECT_TIMEOUT, conn.send(&msg_out)).await??;
                        conn.set_key(key);
                        log::info!("Connection secured");
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
    Ok(())
}

#[inline]
fn get_pk(pk: &[u8]) -> Option<[u8; 32]> {
    if pk.len() == 32 {
        let mut tmp = [0u8; 32];
        tmp[..].copy_from_slice(&pk);
        Some(tmp)
    } else {
        None
    }
}

#[inline]
pub fn get_rs_pk(str_base64: &str) -> Option<sign::PublicKey> {
    if let Ok(pk) = crate::decode64(str_base64) {
        get_pk(&pk).map(|x| sign::PublicKey(x))
    } else {
        None
    }
}

pub fn decode_id_pk(signed: &[u8], key: &sign::PublicKey) -> ResultType<(String, [u8; 32])> {
    let res = IdPk::parse_from_bytes(
        &sign::verify(signed, key).map_err(|_| anyhow!("Signature mismatch"))?,
    )?;
    if let Some(pk) = get_pk(&res.pk) {
        Ok((res.id, pk))
    } else {
        bail!("Wrong their public length");
    }
}

pub fn create_symmetric_key_msg(their_pk_b: [u8; 32]) -> (Bytes, Bytes, secretbox::Key) {
    let their_pk_b = box_::PublicKey(their_pk_b);
    let (our_pk_b, out_sk_b) = box_::gen_keypair();
    let key = secretbox::gen_key();
    let nonce = box_::Nonce([0u8; box_::NONCEBYTES]);
    let sealed_key = box_::seal(&key.0, &nonce, &their_pk_b, &out_sk_b);
    (Vec::from(our_pk_b.0).into(), sealed_key.into(), key)
}

#[inline]
pub fn using_public_server() -> bool {
    option_env!("RENDEZVOUS_SERVER").unwrap_or("").is_empty()
        && crate::get_custom_rendezvous_server(get_option("custom-rendezvous-server")).is_empty()
}

pub struct ThrottledInterval {
    interval: Interval,
    next_tick: Instant,
    min_interval: Duration,
}

impl ThrottledInterval {
    pub fn new(i: Interval) -> ThrottledInterval {
        let period = i.period();
        ThrottledInterval {
            interval: i,
            next_tick: Instant::now(),
            min_interval: Duration::from_secs_f64(period.as_secs_f64() * 0.9),
        }
    }

    pub async fn tick(&mut self) -> Instant {
        let instant = poll_fn(|cx| self.poll_tick(cx));
        instant.await
    }

    pub fn poll_tick(&mut self, cx: &mut std::task::Context<'_>) -> Poll<Instant> {
        match self.interval.poll_tick(cx) {
            Poll::Ready(instant) => {
                let now = Instant::now();
                if self.next_tick <= now {
                    self.next_tick = now + self.min_interval;
                    Poll::Ready(instant)
                } else {
                    // This call is required since tokio 1.27
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

pub type RustDeskInterval = ThrottledInterval;

#[inline]
pub fn rustdesk_interval(i: Interval) -> ThrottledInterval {
    ThrottledInterval::new(i)
}

pub fn load_custom_client() {
    #[cfg(debug_assertions)]
    if let Ok(data) = std::fs::read_to_string("./custom.txt") {
        read_custom_client(data.trim());
        return;
    }
    let Some(path) = std::env::current_exe().map_or(None, |x| x.parent().map(|x| x.to_path_buf()))
    else {
        return;
    };
    #[cfg(target_os = "macos")]
    let path = path.join("../Resources");
    let path = path.join("custom.txt");
    if path.is_file() {
        let Ok(data) = std::fs::read_to_string(&path) else {
            log::error!("Failed to read custom client config");
            return;
        };
        read_custom_client(&data.trim());
    }
}

fn read_custom_client_advanced_settings(
    settings: serde_json::Value,
    map_display_settings: &HashMap<String, &&str>,
    map_local_settings: &HashMap<String, &&str>,
    map_settings: &HashMap<String, &&str>,
    map_buildin_settings: &HashMap<String, &&str>,
    is_override: bool,
) {
    let mut display_settings = if is_override {
        config::OVERWRITE_DISPLAY_SETTINGS.write().unwrap()
    } else {
        config::DEFAULT_DISPLAY_SETTINGS.write().unwrap()
    };
    let mut local_settings = if is_override {
        config::OVERWRITE_LOCAL_SETTINGS.write().unwrap()
    } else {
        config::DEFAULT_LOCAL_SETTINGS.write().unwrap()
    };
    let mut server_settings = if is_override {
        config::OVERWRITE_SETTINGS.write().unwrap()
    } else {
        config::DEFAULT_SETTINGS.write().unwrap()
    };
    let mut buildin_settings = config::BUILTIN_SETTINGS.write().unwrap();

    if let Some(settings) = settings.as_object() {
        for (k, v) in settings {
            let Some(v) = v.as_str() else {
                continue;
            };
            if let Some(k2) = map_display_settings.get(k) {
                display_settings.insert(k2.to_string(), v.to_owned());
            } else if let Some(k2) = map_local_settings.get(k) {
                local_settings.insert(k2.to_string(), v.to_owned());
            } else if let Some(k2) = map_settings.get(k) {
                server_settings.insert(k2.to_string(), v.to_owned());
            } else if let Some(k2) = map_buildin_settings.get(k) {
                buildin_settings.insert(k2.to_string(), v.to_owned());
            } else {
                let k2 = k.replace("_", "-");
                let k = k2.replace("-", "_");
                // display
                display_settings.insert(k.clone(), v.to_owned());
                display_settings.insert(k2.clone(), v.to_owned());
                // local
                local_settings.insert(k.clone(), v.to_owned());
                local_settings.insert(k2.clone(), v.to_owned());
                // server
                server_settings.insert(k.clone(), v.to_owned());
                server_settings.insert(k2.clone(), v.to_owned());
                // buildin
                buildin_settings.insert(k.clone(), v.to_owned());
                buildin_settings.insert(k2.clone(), v.to_owned());
            }
        }
    }
}

#[inline]
#[cfg(target_os = "macos")]
pub fn get_dst_align_rgba() -> usize {
    // https://developer.apple.com/forums/thread/712709
    // Memory alignment should be multiple of 64.
    if crate::ui_interface::use_texture_render() {
        64
    } else {
        1
    }
}

#[inline]
#[cfg(not(target_os = "macos"))]
pub fn get_dst_align_rgba() -> usize {
    1
}

pub fn read_custom_client(config: &str) {
    let Ok(data) = decode64(config) else {
        log::error!("Failed to decode custom client config");
        return;
    };
    const KEY: &str = "5Qbwsde3unUcJBtrx9ZkvUmwFNoExHzpryHuPUdqlWM=";
    let Some(pk) = get_rs_pk(KEY) else {
        log::error!("Failed to parse public key of custom client");
        return;
    };
    let Ok(data) = sign::verify(&data, &pk) else {
        log::error!("Failed to dec custom client config");
        return;
    };
    let Ok(mut data) =
        serde_json::from_slice::<std::collections::HashMap<String, serde_json::Value>>(&data)
    else {
        log::error!("Failed to parse custom client config");
        return;
    };

    if let Some(app_name) = data.remove("app-name") {
        if let Some(app_name) = app_name.as_str() {
            *config::APP_NAME.write().unwrap() = app_name.to_owned();
        }
    }

    let mut map_display_settings = HashMap::new();
    for s in config::keys::KEYS_DISPLAY_SETTINGS {
        map_display_settings.insert(s.replace("_", "-"), s);
    }
    let mut map_local_settings = HashMap::new();
    for s in config::keys::KEYS_LOCAL_SETTINGS {
        map_local_settings.insert(s.replace("_", "-"), s);
    }
    let mut map_settings = HashMap::new();
    for s in config::keys::KEYS_SETTINGS {
        map_settings.insert(s.replace("_", "-"), s);
    }
    let mut buildin_settings = HashMap::new();
    for s in config::keys::KEYS_BUILDIN_SETTINGS {
        buildin_settings.insert(s.replace("_", "-"), s);
    }
    if let Some(default_settings) = data.remove("default-settings") {
        read_custom_client_advanced_settings(
            default_settings,
            &map_display_settings,
            &map_local_settings,
            &map_settings,
            &buildin_settings,
            false,
        );
    }
    if let Some(overwrite_settings) = data.remove("override-settings") {
        read_custom_client_advanced_settings(
            overwrite_settings,
            &map_display_settings,
            &map_local_settings,
            &map_settings,
            &buildin_settings,
            true,
        );
    }
    for (k, v) in data {
        if let Some(v) = v.as_str() {
            config::HARD_SETTINGS
                .write()
                .unwrap()
                .insert(k, v.to_owned());
        };
    }
}

#[inline]
pub fn is_empty_uni_link(arg: &str) -> bool {
    let prefix = crate::get_uri_prefix();
    if !arg.starts_with(&prefix) {
        return false;
    }
    arg[prefix.len()..].chars().all(|c| c == '/')
}

pub fn get_hwid() -> Bytes {
    use sha2::{Digest, Sha256};

    let uuid = hbb_common::get_uuid();
    let mut hasher = Sha256::new();
    hasher.update(&uuid);
    Bytes::from(hasher.finalize().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbb_common::tokio::{
        self,
        time::{interval, interval_at, sleep, Duration, Instant, Interval},
    };
    use std::collections::HashSet;

    #[inline]
    fn get_timestamp_secs() -> u128 {
        (std::time::SystemTime::UNIX_EPOCH
            .elapsed()
            .unwrap()
            .as_millis()
            + 500)
            / 1000
    }

    fn interval_maker() -> Interval {
        interval(Duration::from_secs(1))
    }

    fn interval_at_maker() -> Interval {
        interval_at(
            Instant::now() + Duration::from_secs(1),
            Duration::from_secs(1),
        )
    }

    // ThrottledInterval tick at the same time as tokio interval, if no sleeps
    #[allow(non_snake_case)]
    #[tokio::test]
    async fn test_RustDesk_interval() {
        let base_intervals = [interval_maker, interval_at_maker];
        for maker in base_intervals.into_iter() {
            let mut tokio_timer = maker();
            let mut tokio_times = Vec::new();
            let mut timer = rustdesk_interval(maker());
            let mut times = Vec::new();
            loop {
                tokio::select! {
                    _ = timer.tick() => {
                        if tokio_times.len() >= 10 && times.len() >= 10 {
                            break;
                        }
                        times.push(get_timestamp_secs());
                    }
                    _ = tokio_timer.tick() => {
                        if tokio_times.len() >= 10 && times.len() >= 10 {
                            break;
                        }
                        tokio_times.push(get_timestamp_secs());
                    }
                }
            }
            assert_eq!(times, tokio_times);
        }
    }

    #[tokio::test]
    async fn test_tokio_time_interval_sleep() {
        let mut timer = interval_maker();
        let mut times = Vec::new();
        sleep(Duration::from_secs(3)).await;
        loop {
            tokio::select! {
                _ = timer.tick() => {
                    times.push(get_timestamp_secs());
                    if times.len() == 5 {
                        break;
                    }
                }
            }
        }
        let times2: HashSet<u128> = HashSet::from_iter(times.clone());
        assert_eq!(times.len(), times2.len() + 3);
    }

    // ThrottledInterval tick less times than tokio interval, if there're sleeps
    #[allow(non_snake_case)]
    #[tokio::test]
    async fn test_RustDesk_interval_sleep() {
        let base_intervals = [interval_maker, interval_at_maker];
        for (i, maker) in base_intervals.into_iter().enumerate() {
            let mut timer = rustdesk_interval(maker());
            let mut times = Vec::new();
            sleep(Duration::from_secs(3)).await;
            loop {
                tokio::select! {
                    _ = timer.tick() => {
                        times.push(get_timestamp_secs());
                        if times.len() == 5 {
                            break;
                        }
                    }
                }
            }
            // No multiple ticks in the `interval` time.
            // Values in "times" are unique and are less than normal tokio interval.
            // See previous test (test_tokio_time_interval_sleep) for comparison.
            let times2: HashSet<u128> = HashSet::from_iter(times.clone());
            assert_eq!(times.len(), times2.len(), "test: {}", i);
        }
    }

    #[test]
    fn test_duration_multiplication() {
        let dur = Duration::from_secs(1);

        assert_eq!(dur * 2, Duration::from_secs(2));
        assert_eq!(
            Duration::from_secs_f64(dur.as_secs_f64() * 0.9),
            Duration::from_millis(900)
        );
        assert_eq!(
            Duration::from_secs_f64(dur.as_secs_f64() * 0.923),
            Duration::from_millis(923)
        );
        assert_eq!(
            Duration::from_secs_f64(dur.as_secs_f64() * 0.923 * 1e-3),
            Duration::from_micros(923)
        );
        assert_eq!(
            Duration::from_secs_f64(dur.as_secs_f64() * 0.923 * 1e-6),
            Duration::from_nanos(923)
        );
        assert_eq!(
            Duration::from_secs_f64(dur.as_secs_f64() * 0.923 * 1e-9),
            Duration::from_nanos(1)
        );
        assert_eq!(
            Duration::from_secs_f64(dur.as_secs_f64() * 0.5 * 1e-9),
            Duration::from_nanos(1)
        );
        assert_eq!(
            Duration::from_secs_f64(dur.as_secs_f64() * 0.499 * 1e-9),
            Duration::from_nanos(0)
        );
    }
}

#[inline]
pub fn get_builtin_option(key: &str) -> String {
    config::BUILTIN_SETTINGS
        .read()
        .unwrap()
        .get(key)
        .cloned()
        .unwrap_or_default()
}
