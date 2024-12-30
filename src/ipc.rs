use crate::{
    privacy_mode::PrivacyModeState,
    ui_interface::{get_local_option, set_local_option},
};
use bytes::Bytes;
use parity_tokio_ipc::{
    Connection as Conn, ConnectionClient as ConnClient, Endpoint, Incoming, SecurityAttributes,
};
use serde_derive::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::atomic::{AtomicBool, Ordering},
};
#[cfg(not(windows))]
use std::{fs::File, io::prelude::*};

#[cfg(all(feature = "flutter", feature = "plugin_framework"))]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::plugin::ipc::Plugin;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub use clipboard::ClipboardFile;
use hbb_common::{
    allow_err, bail, bytes,
    bytes_codec::BytesCodec,
    config::{self, Config, Config2},
    futures::StreamExt as _,
    futures_util::sink::SinkExt,
    log, password_security as password, timeout,
    tokio::{
        self,
        io::{AsyncRead, AsyncWrite},
    },
    tokio_util::codec::Framed,
    ResultType,
};

use crate::{common::is_server, privacy_mode, rendezvous_mediator::RendezvousMediator};

// IPC actions here.
pub const IPC_ACTION_CLOSE: &str = "close";
pub static EXIT_RECV_CLOSE: AtomicBool = AtomicBool::new(true);

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "t", content = "c")]
pub enum FS {
    ReadEmptyDirs {
        dir: String,
        include_hidden: bool,
    },
    ReadDir {
        dir: String,
        include_hidden: bool,
    },
    RemoveDir {
        path: String,
        id: i32,
        recursive: bool,
    },
    RemoveFile {
        path: String,
        id: i32,
        file_num: i32,
    },
    CreateDir {
        path: String,
        id: i32,
    },
    NewWrite {
        path: String,
        id: i32,
        file_num: i32,
        files: Vec<(String, u64)>,
        overwrite_detection: bool,
        total_size: u64,
        conn_id: i32,
    },
    CancelWrite {
        id: i32,
    },
    WriteBlock {
        id: i32,
        file_num: i32,
        data: Bytes,
        compressed: bool,
    },
    WriteDone {
        id: i32,
        file_num: i32,
    },
    WriteError {
        id: i32,
        file_num: i32,
        err: String,
    },
    WriteOffset {
        id: i32,
        file_num: i32,
        offset_blk: u32,
    },
    CheckDigest {
        id: i32,
        file_num: i32,
        file_size: u64,
        last_modified: u64,
        is_upload: bool,
    },
    Rename {
        id: i32,
        path: String,
        new_name: String,
    },
}

#[cfg(target_os = "windows")]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "t")]
pub struct ClipboardNonFile {
    pub compress: bool,
    pub content: bytes::Bytes,
    pub content_len: usize,
    pub next_raw: bool,
    pub width: i32,
    pub height: i32,
    // message.proto: ClipboardFormat
    pub format: i32,
    pub special_name: String,
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "t", content = "c")]
pub enum DataKeyboard {
    Sequence(String),
    KeyDown(enigo::Key),
    KeyUp(enigo::Key),
    KeyClick(enigo::Key),
    GetKeyState(enigo::Key),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "t", content = "c")]
pub enum DataKeyboardResponse {
    GetKeyState(bool),
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "t", content = "c")]
pub enum DataMouse {
    MoveTo(i32, i32),
    MoveRelative(i32, i32),
    Down(enigo::MouseButton),
    Up(enigo::MouseButton),
    Click(enigo::MouseButton),
    ScrollX(i32),
    ScrollY(i32),
    Refresh,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "t", content = "c")]
pub enum DataControl {
    Resolution {
        minx: i32,
        maxx: i32,
        miny: i32,
        maxy: i32,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "t", content = "c")]
pub enum DataPortableService {
    Ping,
    Pong,
    ConnCount(Option<usize>),
    Mouse((Vec<u8>, i32)),
    Pointer((Vec<u8>, i32)),
    Key(Vec<u8>),
    RequestStart,
    WillClose,
    CmShowElevation(bool),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "t", content = "c")]
pub enum Data {
    Login {
        id: i32,
        is_file_transfer: bool,
        peer_id: String,
        name: String,
        authorized: bool,
        port_forward: String,
        keyboard: bool,
        clipboard: bool,
        audio: bool,
        file: bool,
        file_transfer_enabled: bool,
        restart: bool,
        recording: bool,
        block_input: bool,
        from_switch: bool,
    },
    ChatMessage {
        text: String,
    },
    SwitchPermission {
        name: String,
        enabled: bool,
    },
    SystemInfo(Option<String>),
    ClickTime(i64),
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    MouseMoveTime(i64),
    Authorize,
    Close,
    #[cfg(windows)]
    SAS,
    UserSid(Option<u32>),
    OnlineStatus(Option<(i64, bool)>),
    Config((String, Option<String>)),
    Options(Option<HashMap<String, String>>),
    NatType(Option<i32>),
    ConfirmedKey(Option<(Vec<u8>, Vec<u8>)>),
    RawMessage(Vec<u8>),
    Socks(Option<config::Socks5Server>),
    FS(FS),
    Test,
    SyncConfig(Option<Box<(Config, Config2)>>),
    #[cfg(target_os = "windows")]
    ClipboardFile(ClipboardFile),
    ClipboardFileEnabled(bool),
    #[cfg(target_os = "windows")]
    ClipboardNonFile(Option<(String, Vec<ClipboardNonFile>)>),
    PrivacyModeState((i32, PrivacyModeState, String)),
    TestRendezvousServer,
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    Keyboard(DataKeyboard),
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    KeyboardResponse(DataKeyboardResponse),
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    Mouse(DataMouse),
    Control(DataControl),
    Theme(String),
    Language(String),
    Empty,
    Disconnected,
    DataPortableService(DataPortableService),
    SwitchSidesRequest(String),
    SwitchSidesBack,
    UrlLink(String),
    VoiceCallIncoming,
    StartVoiceCall,
    VoiceCallResponse(bool),
    CloseVoiceCall(String),
    #[cfg(all(feature = "flutter", feature = "plugin_framework"))]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    Plugin(Plugin),
    #[cfg(windows)]
    SyncWinCpuUsage(Option<f64>),
    FileTransferLog((String, String)),
    #[cfg(windows)]
    ControlledSessionCount(usize),
    CmErr(String),
    CheckHwcodec,
    #[cfg(feature = "flutter")]
    VideoConnCount(Option<usize>),
    // Although the key is not neccessary, it is used to avoid hardcoding the key.
    WaylandScreencastRestoreToken((String, String)),
    HwCodecConfig(Option<String>),
    RemoveTrustedDevices(Vec<Bytes>),
    ClearTrustedDevices,
}

#[tokio::main(flavor = "current_thread")]
pub async fn start(postfix: &str) -> ResultType<()> {
    let mut incoming = new_listener(postfix).await?;
    loop {
        if let Some(result) = incoming.next().await {
            match result {
                Ok(stream) => {
                    let mut stream = Connection::new(stream);
                    let postfix = postfix.to_owned();
                    tokio::spawn(async move {
                        loop {
                            match stream.next().await {
                                Err(err) => {
                                    log::trace!("ipc '{}' connection closed: {}", postfix, err);
                                    break;
                                }
                                Ok(Some(data)) => {
                                    handle(data, &mut stream).await;
                                }
                                _ => {}
                            }
                        }
                    });
                }
                Err(err) => {
                    log::error!("Couldn't get client: {:?}", err);
                }
            }
        }
    }
}

pub async fn new_listener(postfix: &str) -> ResultType<Incoming> {
    let path = Config::ipc_path(postfix);
    #[cfg(not(any(windows, target_os = "android", target_os = "ios")))]
    check_pid(postfix).await;
    let mut endpoint = Endpoint::new(path.clone());
    match SecurityAttributes::allow_everyone_create() {
        Ok(attr) => endpoint.set_security_attributes(attr),
        Err(err) => log::error!("Failed to set ipc{} security: {}", postfix, err),
    };
    match endpoint.incoming() {
        Ok(incoming) => {
            log::info!("Started ipc{} server at path: {}", postfix, &path);
            #[cfg(not(windows))]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o0777)).ok();
                write_pid(postfix);
            }
            Ok(incoming)
        }
        Err(err) => {
            log::error!(
                "Failed to start ipc{} server at path {}: {}",
                postfix,
                path,
                err
            );
            Err(err.into())
        }
    }
}

pub struct CheckIfRestart(String, Vec<String>, String, String);

impl CheckIfRestart {
    pub fn new() -> CheckIfRestart {
        CheckIfRestart(
            Config::get_option("stop-service"),
            Config::get_rendezvous_servers(),
            Config::get_option("audio-input"),
            Config::get_option("voice-call-input"),
        )
    }
}
impl Drop for CheckIfRestart {
    fn drop(&mut self) {
        if self.0 != Config::get_option("stop-service")
            || self.1 != Config::get_rendezvous_servers()
        {
            RendezvousMediator::restart();
        }
        if self.2 != Config::get_option("audio-input") {
            crate::audio_service::restart();
        }
        if self.3 != Config::get_option("voice-call-input") {
            crate::audio_service::set_voice_call_input_device(
                Some(Config::get_option("voice-call-input")),
                true,
            )
        }
    }
}

async fn handle(data: Data, stream: &mut Connection) {
    match data {
        Data::SystemInfo(_) => {
            let info = format!(
                "log_path: {}, config: {}, username: {}",
                Config::log_path().to_str().unwrap_or(""),
                Config::file().to_str().unwrap_or(""),
                crate::username(),
            );
            allow_err!(stream.send(&Data::SystemInfo(Some(info))).await);
        }
        Data::ClickTime(_) => {
            let t = crate::server::CLICK_TIME.load(Ordering::SeqCst);
            allow_err!(stream.send(&Data::ClickTime(t)).await);
        }
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        Data::MouseMoveTime(_) => {
            let t = crate::server::MOUSE_MOVE_TIME.load(Ordering::SeqCst);
            allow_err!(stream.send(&Data::MouseMoveTime(t)).await);
        }
        Data::Close => {
            log::info!("Receive close message");
            if EXIT_RECV_CLOSE.load(Ordering::SeqCst) {
                #[cfg(not(target_os = "android"))]
                crate::server::input_service::fix_key_down_timeout_at_exit();
                if is_server() {
                    let _ = privacy_mode::turn_off_privacy(0, Some(PrivacyModeState::OffByPeer));
                }
                #[cfg(any(target_os = "macos", target_os = "linux"))]
                if crate::is_main() {
                    // below part is for main windows can be reopen during rustdesk installation and installing service from UI
                    // this make new ipc server (domain socket) can be created.
                    std::fs::remove_file(&Config::ipc_path("")).ok();
                    #[cfg(target_os = "linux")]
                    {
                        hbb_common::sleep((crate::platform::SERVICE_INTERVAL * 2) as f32 / 1000.0)
                            .await;
                        // https://github.com/rustdesk/rustdesk/discussions/9254
                        crate::run_me::<&str>(vec!["--no-server"]).ok();
                    }
                    #[cfg(target_os = "macos")]
                    {
                        // our launchagent interval is 1 second
                        hbb_common::sleep(1.5).await;
                        std::process::Command::new("open")
                            .arg("-n")
                            .arg(&format!("/Applications/{}.app", crate::get_app_name()))
                            .spawn()
                            .ok();
                    }
                    // leave above open a little time
                    hbb_common::sleep(0.3).await;
                    // in case below exit failed
                    crate::platform::quit_gui();
                }
                std::process::exit(-1); // to make sure --server luauchagent process can restart because SuccessfulExit used
            }
        }
        Data::OnlineStatus(_) => {
            let x = config::get_online_state();
            let confirmed = Config::get_key_confirmed();
            allow_err!(stream.send(&Data::OnlineStatus(Some((x, confirmed)))).await);
        }
        Data::ConfirmedKey(None) => {
            let out = if Config::get_key_confirmed() {
                Some(Config::get_key_pair())
            } else {
                None
            };
            allow_err!(stream.send(&Data::ConfirmedKey(out)).await);
        }
        Data::Socks(s) => match s {
            None => {
                allow_err!(stream.send(&Data::Socks(Config::get_socks())).await);
            }
            Some(data) => {
                if data.proxy.is_empty() {
                    Config::set_socks(None);
                } else {
                    Config::set_socks(Some(data));
                }
                crate::common::test_nat_type();
                RendezvousMediator::restart();
                log::info!("socks updated");
            }
        },
        #[cfg(feature = "flutter")]
        Data::VideoConnCount(None) => {
            let n = crate::server::AUTHED_CONNS
                .lock()
                .unwrap()
                .iter()
                .filter(|x| x.1 == crate::server::AuthConnType::Remote)
                .count();
            allow_err!(stream.send(&Data::VideoConnCount(Some(n))).await);
        }
        Data::Config((name, value)) => match value {
            None => {
                let value;
                if name == "id" {
                    value = Some(Config::get_id());
                } else if name == "temporary-password" {
                    value = Some(password::temporary_password());
                } else if name == "permanent-password" {
                    value = Some(Config::get_permanent_password());
                } else if name == "salt" {
                    value = Some(Config::get_salt());
                } else if name == "rendezvous_server" {
                    value = Some(format!(
                        "{},{}",
                        Config::get_rendezvous_server(),
                        Config::get_rendezvous_servers().join(",")
                    ));
                } else if name == "rendezvous_servers" {
                    value = Some(Config::get_rendezvous_servers().join(","));
                } else if name == "fingerprint" {
                    value = if Config::get_key_confirmed() {
                        Some(crate::common::pk_to_fingerprint(Config::get_key_pair().1))
                    } else {
                        None
                    };
                } else if name == "hide_cm" {
                    value = if crate::hbbs_http::sync::is_pro() {
                        Some(hbb_common::password_security::hide_cm().to_string())
                    } else {
                        None
                    };
                } else if name == "voice-call-input" {
                    value = crate::audio_service::get_voice_call_input_device();
                } else if name == "unlock-pin" {
                    value = Some(Config::get_unlock_pin());
                } else if name == "trusted-devices" {
                    value = Some(Config::get_trusted_devices_json());
                } else {
                    value = None;
                }
                allow_err!(stream.send(&Data::Config((name, value))).await);
            }
            Some(value) => {
                if name == "id" {
                    Config::set_key_confirmed(false);
                    Config::set_id(&value);
                } else if name == "temporary-password" {
                    password::update_temporary_password();
                } else if name == "permanent-password" {
                    Config::set_permanent_password(&value);
                } else if name == "salt" {
                    Config::set_salt(&value);
                } else if name == "voice-call-input" {
                    crate::audio_service::set_voice_call_input_device(Some(value), true);
                } else if name == "unlock-pin" {
                    Config::set_unlock_pin(&value);
                } else {
                    return;
                }
                log::info!("{} updated", name);
            }
        },
        Data::Options(value) => match value {
            None => {
                let v = Config::get_options();
                allow_err!(stream.send(&Data::Options(Some(v))).await);
            }
            Some(value) => {
                let _chk = CheckIfRestart::new();
                if let Some(v) = value.get("privacy-mode-impl-key") {
                    crate::privacy_mode::switch(v);
                }
                Config::set_options(value);
                allow_err!(stream.send(&Data::Options(None)).await);
            }
        },
        Data::NatType(_) => {
            let t = Config::get_nat_type();
            allow_err!(stream.send(&Data::NatType(Some(t))).await);
        }
        Data::SyncConfig(Some(configs)) => {
            let (config, config2) = *configs;
            let _chk = CheckIfRestart::new();
            Config::set(config);
            Config2::set(config2);
            allow_err!(stream.send(&Data::SyncConfig(None)).await);
        }
        Data::SyncConfig(None) => {
            allow_err!(
                stream
                    .send(&Data::SyncConfig(Some(
                        (Config::get(), Config2::get()).into()
                    )))
                    .await
            );
        }
        #[cfg(windows)]
        Data::SyncWinCpuUsage(None) => {
            allow_err!(
                stream
                    .send(&Data::SyncWinCpuUsage(
                        hbb_common::platform::windows::cpu_uage_one_minute()
                    ))
                    .await
            );
        }
        Data::TestRendezvousServer => {
            crate::test_rendezvous_server();
        }
        Data::SwitchSidesRequest(id) => {
            let uuid = uuid::Uuid::new_v4();
            crate::server::insert_switch_sides_uuid(id, uuid.clone());
            allow_err!(
                stream
                    .send(&Data::SwitchSidesRequest(uuid.to_string()))
                    .await
            );
        }
        #[cfg(all(feature = "flutter", feature = "plugin_framework"))]
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        Data::Plugin(plugin) => crate::plugin::ipc::handle_plugin(plugin, stream).await,
        #[cfg(windows)]
        Data::ControlledSessionCount(_) => {
            allow_err!(
                stream
                    .send(&Data::ControlledSessionCount(
                        crate::Connection::alive_conns().len()
                    ))
                    .await
            );
        }
        #[cfg(feature = "hwcodec")]
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        Data::CheckHwcodec => {
            scrap::hwcodec::start_check_process();
        }
        #[cfg(feature = "hwcodec")]
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        Data::HwCodecConfig(c) => {
            match c {
                None => {
                    let v = match scrap::hwcodec::HwCodecConfig::get_set_value() {
                        Some(v) => Some(serde_json::to_string(&v).unwrap_or_default()),
                        None => None,
                    };
                    allow_err!(stream.send(&Data::HwCodecConfig(v)).await);
                }
                Some(v) => {
                    // --server and portable
                    scrap::hwcodec::HwCodecConfig::set(v);
                }
            }
        }
        Data::WaylandScreencastRestoreToken((key, value)) => {
            let v = if value == "get" {
                let opt = get_local_option(key.clone());
                #[cfg(not(target_os = "linux"))]
                {
                    Some(opt)
                }
                #[cfg(target_os = "linux")]
                {
                    let v = if opt.is_empty() {
                        if scrap::wayland::pipewire::is_rdp_session_hold() {
                            "fake token".to_string()
                        } else {
                            "".to_owned()
                        }
                    } else {
                        opt
                    };
                    Some(v)
                }
            } else if value == "clear" {
                set_local_option(key.clone(), "".to_owned());
                #[cfg(target_os = "linux")]
                scrap::wayland::pipewire::close_session();
                Some("".to_owned())
            } else {
                None
            };
            if let Some(v) = v {
                allow_err!(
                    stream
                        .send(&Data::WaylandScreencastRestoreToken((key, v)))
                        .await
                );
            }
        }
        Data::RemoveTrustedDevices(v) => {
            Config::remove_trusted_devices(&v);
        }
        Data::ClearTrustedDevices => {
            Config::clear_trusted_devices();
        }
        _ => {}
    }
}

pub async fn connect(ms_timeout: u64, postfix: &str) -> ResultType<ConnectionTmpl<ConnClient>> {
    let path = Config::ipc_path(postfix);
    let client = timeout(ms_timeout, Endpoint::connect(&path)).await??;
    Ok(ConnectionTmpl::new(client))
}

#[cfg(target_os = "linux")]
#[tokio::main(flavor = "current_thread")]
pub async fn start_pa() {
    use crate::audio_service::AUDIO_DATA_SIZE_U8;

    match new_listener("_pa").await {
        Ok(mut incoming) => {
            loop {
                if let Some(result) = incoming.next().await {
                    match result {
                        Ok(stream) => {
                            let mut stream = Connection::new(stream);
                            let mut device: String = "".to_owned();
                            if let Some(Ok(Some(Data::Config((_, Some(x)))))) =
                                stream.next_timeout2(1000).await
                            {
                                device = x;
                            }
                            if !device.is_empty() {
                                device = crate::platform::linux::get_pa_source_name(&device);
                            }
                            if device.is_empty() {
                                device = crate::platform::linux::get_pa_monitor();
                            }
                            if device.is_empty() {
                                continue;
                            }
                            let spec = pulse::sample::Spec {
                                format: pulse::sample::Format::F32le,
                                channels: 2,
                                rate: crate::platform::PA_SAMPLE_RATE,
                            };
                            log::info!("pa monitor: {:?}", device);
                            // systemctl --user status pulseaudio.service
                            let mut buf: Vec<u8> = vec![0; AUDIO_DATA_SIZE_U8];
                            match psimple::Simple::new(
                                None,                             // Use the default server
                                &crate::get_app_name(),           // Our application’s name
                                pulse::stream::Direction::Record, // We want a record stream
                                Some(&device),                    // Use the default device
                                "record",                         // Description of our stream
                                &spec,                            // Our sample format
                                None,                             // Use default channel map
                                None, // Use default buffering attributes
                            ) {
                                Ok(s) => loop {
                                    if let Ok(_) = s.read(&mut buf) {
                                        let out =
                                            if buf.iter().filter(|x| **x != 0).next().is_none() {
                                                vec![]
                                            } else {
                                                buf.clone()
                                            };
                                        if let Err(err) = stream.send_raw(out.into()).await {
                                            log::error!("Failed to send audio data:{}", err);
                                            break;
                                        }
                                    }
                                },
                                Err(err) => {
                                    log::error!("Could not create simple pulse: {}", err);
                                }
                            }
                        }
                        Err(err) => {
                            log::error!("Couldn't get pa client: {:?}", err);
                        }
                    }
                }
            }
        }
        Err(err) => {
            log::error!("Failed to start pa ipc server: {}", err);
        }
    }
}

#[inline]
#[cfg(not(windows))]
fn get_pid_file(postfix: &str) -> String {
    let path = Config::ipc_path(postfix);
    format!("{}.pid", path)
}

#[cfg(not(any(windows, target_os = "android", target_os = "ios")))]
async fn check_pid(postfix: &str) {
    let pid_file = get_pid_file(postfix);
    if let Ok(mut file) = File::open(&pid_file) {
        let mut content = String::new();
        file.read_to_string(&mut content).ok();
        let pid = content.parse::<usize>().unwrap_or(0);
        if pid > 0 {
            use hbb_common::sysinfo::System;
            let mut sys = System::new();
            sys.refresh_processes();
            if let Some(p) = sys.process(pid.into()) {
                if let Some(current) = sys.process((std::process::id() as usize).into()) {
                    if current.name() == p.name() {
                        // double check with connect
                        if connect(1000, postfix).await.is_ok() {
                            return;
                        }
                    }
                }
            }
        }
    }
    // if not remove old ipc file, the new ipc creation will fail
    // if we remove a ipc file, but the old ipc process is still running,
    // new connection to the ipc will connect to new ipc, old connection to old ipc still keep alive
    std::fs::remove_file(&Config::ipc_path(postfix)).ok();
}

#[inline]
#[cfg(not(windows))]
fn write_pid(postfix: &str) {
    let path = get_pid_file(postfix);
    if let Ok(mut file) = File::create(&path) {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o0777)).ok();
        file.write_all(&std::process::id().to_string().into_bytes())
            .ok();
    }
}

pub struct ConnectionTmpl<T> {
    inner: Framed<T, BytesCodec>,
}

pub type Connection = ConnectionTmpl<Conn>;

impl<T> ConnectionTmpl<T>
where
    T: AsyncRead + AsyncWrite + std::marker::Unpin,
{
    pub fn new(conn: T) -> Self {
        Self {
            inner: Framed::new(conn, BytesCodec::new()),
        }
    }

    pub async fn send(&mut self, data: &Data) -> ResultType<()> {
        let v = serde_json::to_vec(data)?;
        self.inner.send(bytes::Bytes::from(v)).await?;
        Ok(())
    }

    async fn send_config(&mut self, name: &str, value: String) -> ResultType<()> {
        self.send(&Data::Config((name.to_owned(), Some(value))))
            .await
    }

    pub async fn next_timeout(&mut self, ms_timeout: u64) -> ResultType<Option<Data>> {
        Ok(timeout(ms_timeout, self.next()).await??)
    }

    pub async fn next_timeout2(&mut self, ms_timeout: u64) -> Option<ResultType<Option<Data>>> {
        if let Ok(x) = timeout(ms_timeout, self.next()).await {
            Some(x)
        } else {
            None
        }
    }

    pub async fn next(&mut self) -> ResultType<Option<Data>> {
        match self.inner.next().await {
            Some(res) => {
                let bytes = res?;
                if let Ok(s) = std::str::from_utf8(&bytes) {
                    if let Ok(data) = serde_json::from_str::<Data>(s) {
                        return Ok(Some(data));
                    }
                }
                return Ok(None);
            }
            _ => {
                bail!("reset by the peer");
            }
        }
    }

    pub async fn send_raw(&mut self, data: Bytes) -> ResultType<()> {
        self.inner.send(data).await?;
        Ok(())
    }

    pub async fn next_raw(&mut self) -> ResultType<bytes::BytesMut> {
        match self.inner.next().await {
            Some(Ok(res)) => Ok(res),
            _ => {
                bail!("reset by the peer");
            }
        }
    }
}

#[tokio::main(flavor = "current_thread")]
pub async fn get_config(name: &str) -> ResultType<Option<String>> {
    get_config_async(name, 1_000).await
}

async fn get_config_async(name: &str, ms_timeout: u64) -> ResultType<Option<String>> {
    let mut c = connect(ms_timeout, "").await?;
    c.send(&Data::Config((name.to_owned(), None))).await?;
    if let Some(Data::Config((name2, value))) = c.next_timeout(ms_timeout).await? {
        if name == name2 {
            return Ok(value);
        }
    }
    return Ok(None);
}

pub async fn set_config_async(name: &str, value: String) -> ResultType<()> {
    let mut c = connect(1000, "").await?;
    c.send_config(name, value).await?;
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
pub async fn set_data(data: &Data) -> ResultType<()> {
    set_data_async(data).await
}

async fn set_data_async(data: &Data) -> ResultType<()> {
    let mut c = connect(1000, "").await?;
    c.send(data).await?;
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
pub async fn set_config(name: &str, value: String) -> ResultType<()> {
    set_config_async(name, value).await
}

pub fn update_temporary_password() -> ResultType<()> {
    set_config("temporary-password", "".to_owned())
}

pub fn get_permanent_password() -> String {
    if let Ok(Some(v)) = get_config("permanent-password") {
        Config::set_permanent_password(&v);
        v
    } else {
        Config::get_permanent_password()
    }
}

pub fn get_fingerprint() -> String {
    get_config("fingerprint")
        .unwrap_or_default()
        .unwrap_or_default()
}

pub fn set_permanent_password(v: String) -> ResultType<()> {
    Config::set_permanent_password(&v);
    set_config("permanent-password", v)
}

#[cfg(feature = "flutter")]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn set_unlock_pin(v: String, translate: bool) -> ResultType<()> {
    let v = v.trim().to_owned();
    let min_len = 4;
    let max_len = crate::ui_interface::max_encrypt_len();
    let len = v.chars().count();
    if !v.is_empty() {
        if len < min_len {
            let err = if translate {
                crate::lang::translate(
                    "Requires at least {".to_string() + &format!("{min_len}") + "} characters",
                )
            } else {
                // Sometimes, translated can't show normally in command line
                format!("Requires at least {} characters", min_len)
            };
            bail!(err);
        }
        if len > max_len {
            bail!("No more than {max_len} characters");
        }
    }
    Config::set_unlock_pin(&v);
    set_config("unlock-pin", v)
}

#[cfg(feature = "flutter")]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn get_unlock_pin() -> String {
    if let Ok(Some(v)) = get_config("unlock-pin") {
        Config::set_unlock_pin(&v);
        v
    } else {
        Config::get_unlock_pin()
    }
}

#[cfg(feature = "flutter")]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn get_trusted_devices() -> String {
    if let Ok(Some(v)) = get_config("trusted-devices") {
        v
    } else {
        Config::get_trusted_devices_json()
    }
}

#[cfg(feature = "flutter")]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn remove_trusted_devices(hwids: Vec<Bytes>) {
    Config::remove_trusted_devices(&hwids);
    allow_err!(set_data(&Data::RemoveTrustedDevices(hwids)));
}

#[cfg(feature = "flutter")]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn clear_trusted_devices() {
    Config::clear_trusted_devices();
    allow_err!(set_data(&Data::ClearTrustedDevices));
}

pub fn get_id() -> String {
    if let Ok(Some(v)) = get_config("id") {
        // update salt also, so that next time reinstallation not causing first-time auto-login failure
        if let Ok(Some(v2)) = get_config("salt") {
            Config::set_salt(&v2);
        }
        if v != Config::get_id() {
            Config::set_key_confirmed(false);
            Config::set_id(&v);
        }
        v
    } else {
        Config::get_id()
    }
}

pub async fn get_rendezvous_server(ms_timeout: u64) -> (String, Vec<String>) {
    if let Ok(Some(v)) = get_config_async("rendezvous_server", ms_timeout).await {
        let mut urls = v.split(",");
        let a = urls.next().unwrap_or_default().to_owned();
        let b: Vec<String> = urls.map(|x| x.to_owned()).collect();
        (a, b)
    } else {
        (
            Config::get_rendezvous_server(),
            Config::get_rendezvous_servers(),
        )
    }
}

async fn get_options_(ms_timeout: u64) -> ResultType<HashMap<String, String>> {
    let mut c = connect(ms_timeout, "").await?;
    c.send(&Data::Options(None)).await?;
    if let Some(Data::Options(Some(value))) = c.next_timeout(ms_timeout).await? {
        Config::set_options(value.clone());
        Ok(value)
    } else {
        Ok(Config::get_options())
    }
}

pub async fn get_options_async() -> HashMap<String, String> {
    get_options_(1000).await.unwrap_or(Config::get_options())
}

#[tokio::main(flavor = "current_thread")]
pub async fn get_options() -> HashMap<String, String> {
    get_options_async().await
}

pub async fn get_option_async(key: &str) -> String {
    if let Some(v) = get_options_async().await.get(key) {
        v.clone()
    } else {
        "".to_owned()
    }
}

pub fn set_option(key: &str, value: &str) {
    let mut options = get_options();
    if value.is_empty() {
        options.remove(key);
    } else {
        options.insert(key.to_owned(), value.to_owned());
    }
    set_options(options).ok();
}

#[tokio::main(flavor = "current_thread")]
pub async fn set_options(value: HashMap<String, String>) -> ResultType<()> {
    if let Ok(mut c) = connect(1000, "").await {
        c.send(&Data::Options(Some(value.clone()))).await?;
        // do not put below before connect, because we need to check should_exit
        c.next_timeout(1000).await.ok();
    }
    Config::set_options(value);
    Ok(())
}

#[inline]
async fn get_nat_type_(ms_timeout: u64) -> ResultType<i32> {
    let mut c = connect(ms_timeout, "").await?;
    c.send(&Data::NatType(None)).await?;
    if let Some(Data::NatType(Some(value))) = c.next_timeout(ms_timeout).await? {
        Config::set_nat_type(value);
        Ok(value)
    } else {
        Ok(Config::get_nat_type())
    }
}

pub async fn get_nat_type(ms_timeout: u64) -> i32 {
    get_nat_type_(ms_timeout)
        .await
        .unwrap_or(Config::get_nat_type())
}

pub async fn get_rendezvous_servers(ms_timeout: u64) -> Vec<String> {
    if let Ok(Some(v)) = get_config_async("rendezvous_servers", ms_timeout).await {
        return v.split(',').map(|x| x.to_owned()).collect();
    }
    return Config::get_rendezvous_servers();
}

#[inline]
async fn get_socks_(ms_timeout: u64) -> ResultType<Option<config::Socks5Server>> {
    let mut c = connect(ms_timeout, "").await?;
    c.send(&Data::Socks(None)).await?;
    if let Some(Data::Socks(value)) = c.next_timeout(ms_timeout).await? {
        Config::set_socks(value.clone());
        Ok(value)
    } else {
        Ok(Config::get_socks())
    }
}

pub async fn get_socks_async(ms_timeout: u64) -> Option<config::Socks5Server> {
    get_socks_(ms_timeout).await.unwrap_or(Config::get_socks())
}

#[tokio::main(flavor = "current_thread")]
pub async fn get_socks() -> Option<config::Socks5Server> {
    get_socks_async(1_000).await
}

#[tokio::main(flavor = "current_thread")]
pub async fn set_socks(value: config::Socks5Server) -> ResultType<()> {
    Config::set_socks(if value.proxy.is_empty() {
        None
    } else {
        Some(value.clone())
    });
    connect(1_000, "")
        .await?
        .send(&Data::Socks(Some(value)))
        .await?;
    Ok(())
}

pub fn get_proxy_status() -> bool {
    Config::get_socks().is_some()
}
#[tokio::main(flavor = "current_thread")]
pub async fn test_rendezvous_server() -> ResultType<()> {
    let mut c = connect(1000, "").await?;
    c.send(&Data::TestRendezvousServer).await?;
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
pub async fn send_url_scheme(url: String) -> ResultType<()> {
    connect(1_000, "_url")
        .await?
        .send(&Data::UrlLink(url))
        .await?;
    Ok(())
}

// Emit `close` events to ipc.
pub fn close_all_instances() -> ResultType<bool> {
    match crate::ipc::send_url_scheme(IPC_ACTION_CLOSE.to_owned()) {
        Ok(_) => Ok(true),
        Err(err) => Err(err),
    }
}

#[tokio::main(flavor = "current_thread")]
pub async fn connect_to_user_session(usid: Option<u32>) -> ResultType<()> {
    let mut stream = crate::ipc::connect(1000, crate::POSTFIX_SERVICE).await?;
    timeout(1000, stream.send(&crate::ipc::Data::UserSid(usid))).await??;
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
pub async fn notify_server_to_check_hwcodec() -> ResultType<()> {
    connect(1_000, "").await?.send(&&Data::CheckHwcodec).await?;
    Ok(())
}

#[cfg(feature = "hwcodec")]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::main(flavor = "current_thread")]
pub async fn get_hwcodec_config_from_server() -> ResultType<()> {
    if !scrap::codec::enable_hwcodec_option() || scrap::hwcodec::HwCodecConfig::already_set() {
        return Ok(());
    }
    let mut c = connect(50, "").await?;
    c.send(&Data::HwCodecConfig(None)).await?;
    if let Some(Data::HwCodecConfig(v)) = c.next_timeout(50).await? {
        match v {
            Some(v) => {
                scrap::hwcodec::HwCodecConfig::set(v);
                return Ok(());
            }
            None => {
                bail!("hwcodec config is none");
            }
        }
    }
    bail!("failed to get hwcodec config");
}

#[cfg(feature = "hwcodec")]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn client_get_hwcodec_config_thread(wait_sec: u64) {
    static ONCE: std::sync::Once = std::sync::Once::new();
    if !crate::platform::is_installed()
        || !scrap::codec::enable_hwcodec_option()
        || scrap::hwcodec::HwCodecConfig::already_set()
    {
        return;
    }
    ONCE.call_once(move || {
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(1));
            let mut intervals: Vec<u64> = vec![wait_sec, 3, 3, 6, 9];
            for i in intervals.drain(..) {
                if i > 0 {
                    std::thread::sleep(std::time::Duration::from_secs(i));
                }
                if get_hwcodec_config_from_server().is_ok() {
                    break;
                }
            }
        });
    });
}

#[cfg(feature = "hwcodec")]
#[tokio::main(flavor = "current_thread")]
pub async fn hwcodec_process() {
    let s = scrap::hwcodec::check_available_hwcodec();
    for _ in 0..5 {
        match crate::ipc::connect(1000, "").await {
            Ok(mut conn) => {
                match conn
                    .send(&crate::ipc::Data::HwCodecConfig(Some(s.clone())))
                    .await
                {
                    Ok(()) => {
                        log::info!("send ok");
                        break;
                    }
                    Err(e) => {
                        log::error!("send failed: {e:?}");
                    }
                }
            }
            Err(e) => {
                log::error!("connect failed: {e:?}");
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

#[tokio::main(flavor = "current_thread")]
pub async fn get_wayland_screencast_restore_token(key: String) -> ResultType<String> {
    let v = handle_wayland_screencast_restore_token(key, "get".to_owned()).await?;
    Ok(v.unwrap_or_default())
}

#[tokio::main(flavor = "current_thread")]
pub async fn clear_wayland_screencast_restore_token(key: String) -> ResultType<bool> {
    if let Some(v) = handle_wayland_screencast_restore_token(key, "clear".to_owned()).await? {
        return Ok(v.is_empty());
    }
    return Ok(false);
}

async fn handle_wayland_screencast_restore_token(
    key: String,
    value: String,
) -> ResultType<Option<String>> {
    let ms_timeout = 1_000;
    let mut c = connect(ms_timeout, "").await?;
    c.send(&Data::WaylandScreencastRestoreToken((key, value)))
        .await?;
    if let Some(Data::WaylandScreencastRestoreToken((_key, v))) = c.next_timeout(ms_timeout).await?
    {
        return Ok(Some(v));
    }
    return Ok(None);
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn verify_ffi_enum_data_size() {
        println!("{}", std::mem::size_of::<Data>());
        assert!(std::mem::size_of::<Data>() < 96);
    }
}
