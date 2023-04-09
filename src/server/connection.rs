use super::{input_service::*, *};
#[cfg(windows)]
use crate::clipboard_file::*;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::common::update_clipboard;
#[cfg(all(target_os = "linux", feature = "linux_headless"))]
#[cfg(not(any(feature = "flatpak", feature = "appimage")))]
use crate::platform::linux_desktop_manager;
#[cfg(windows)]
use crate::portable_service::client as portable_client;
use crate::{
    client::{
        new_voice_call_request, new_voice_call_response, start_audio_thread, MediaData, MediaSender,
    },
    common::{get_default_sound_input, set_sound_input},
    video_service,
};
#[cfg(any(target_os = "android", target_os = "ios"))]
use crate::{common::DEVICE_NAME, flutter::connection_manager::start_channel};
use crate::{ipc, VERSION};
use cidr_utils::cidr::IpCidr;
#[cfg(all(target_os = "linux", feature = "linux_headless"))]
#[cfg(not(any(feature = "flatpak", feature = "appimage")))]
use hbb_common::platform::linux::run_cmds;
use hbb_common::{
    config::Config,
    fs,
    fs::can_enable_overwrite_detection,
    futures::{SinkExt, StreamExt},
    get_time, get_version_number,
    message_proto::{option_message::BoolOption, permission_info::Permission},
    password_security::{self as password, ApproveMode},
    sleep, timeout,
    tokio::{
        net::TcpStream,
        sync::mpsc,
        time::{self, Duration, Instant, Interval},
    },
    tokio_util::codec::{BytesCodec, Framed},
};
#[cfg(any(target_os = "android", target_os = "ios"))]
use scrap::android::call_main_service_mouse_input;
use serde_json::{json, value::Value};
use sha2::{Digest, Sha256};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use std::sync::atomic::Ordering;
use std::{
    num::NonZeroI64,
    sync::{atomic::AtomicI64, mpsc as std_mpsc},
};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use system_shutdown;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
use std::collections::HashSet;

pub type Sender = mpsc::UnboundedSender<(Instant, Arc<Message>)>;

lazy_static::lazy_static! {
    static ref LOGIN_FAILURES: Arc::<Mutex<HashMap<String, (i32, i32, i32)>>> = Default::default();
    static ref SESSIONS: Arc::<Mutex<HashMap<String, Session>>> = Default::default();
    static ref ALIVE_CONNS: Arc::<Mutex<Vec<i32>>> = Default::default();
    static ref SWITCH_SIDES_UUID: Arc::<Mutex<HashMap<String, (Instant, uuid::Uuid)>>> = Default::default();
}
pub static CLICK_TIME: AtomicI64 = AtomicI64::new(0);
pub static MOUSE_MOVE_TIME: AtomicI64 = AtomicI64::new(0);

pub const LOGIN_MSG_DESKTOP_NOT_INITED: &str = "Desktop env is not inited";
pub const LOGIN_MSG_DESKTOP_SESSION_NOT_READY: &str = "Desktop session not ready";
pub const LOGIN_MSG_DESKTOP_XSESSION_FAILED: &str = "Desktop xsession failed";
pub const LOGIN_MSG_DESKTOP_SESSION_ANOTHER_USER: &str = "Desktop session another user login";
pub const LOGIN_MSG_DESKTOP_XORG_NOT_FOUND: &str = "Desktop xorg not found";
// ls /usr/share/xsessions/
pub const LOGIN_MSG_DESKTOP_NO_DESKTOP: &str = "Desktop none";
pub const LOGIN_MSG_DESKTOP_SESSION_NOT_READY_PASSWORD_EMPTY: &str =
    "Desktop session not ready, password empty";
pub const LOGIN_MSG_DESKTOP_SESSION_NOT_READY_PASSWORD_WRONG: &str =
    "Desktop session not ready, password wrong";
pub const LOGIN_MSG_PASSWORD_EMPTY: &str = "Empty Password";
pub const LOGIN_MSG_PASSWORD_WRONG: &str = "Wrong Password";
pub const LOGIN_MSG_NO_PASSWORD_ACCESS: &str = "No Password Access";
pub const LOGIN_MSG_OFFLINE: &str = "Offline";

#[derive(Clone, Default)]
pub struct ConnInner {
    id: i32,
    tx: Option<Sender>,
    tx_video: Option<Sender>,
}

enum MessageInput {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    Mouse((MouseEvent, i32)),
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    Key((KeyEvent, bool)),
    BlockOn,
    BlockOff,
}

#[derive(Clone, Debug)]
struct Session {
    name: String,
    session_id: u64,
    last_recv_time: Arc<Mutex<Instant>>,
    random_password: String,
}

pub struct Connection {
    inner: ConnInner,
    stream: super::Stream,
    server: super::ServerPtrWeak,
    hash: Hash,
    read_jobs: Vec<fs::TransferJob>,
    timer: Interval,
    file_timer: Interval,
    file_transfer: Option<(String, bool)>,
    port_forward_socket: Option<Framed<TcpStream, BytesCodec>>,
    port_forward_address: String,
    tx_to_cm: mpsc::UnboundedSender<ipc::Data>,
    authorized: bool,
    keyboard: bool,
    clipboard: bool,
    audio: bool,
    file: bool,
    restart: bool,
    recording: bool,
    last_test_delay: i64,
    lock_after_session_end: bool,
    show_remote_cursor: bool,
    // by peer
    ip: String,
    // by peer
    disable_keyboard: bool,
    // by peer
    disable_clipboard: bool,
    // by peer
    disable_audio: bool,
    // by peer
    enable_file_transfer: bool,
    // by peer
    audio_sender: Option<MediaSender>,
    // audio by the remote peer/client
    tx_input: std_mpsc::Sender<MessageInput>,
    // handle input messages
    video_ack_required: bool,
    peer_info: (String, String),
    server_audit_conn: String,
    server_audit_file: String,
    lr: LoginRequest,
    last_recv_time: Arc<Mutex<Instant>>,
    chat_unanswered: bool,
    #[cfg(windows)]
    portable: PortableState,
    from_switch: bool,
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    origin_resolution: HashMap<String, Resolution>,
    voice_call_request_timestamp: Option<NonZeroI64>,
    audio_input_device_before_voice_call: Option<String>,
    options_in_login: Option<OptionMessage>,
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pressed_modifiers: HashSet<rdev::Key>,
    #[cfg(all(target_os = "linux", feature = "linux_headless"))]
    #[cfg(not(any(feature = "flatpak", feature = "appimage")))]
    rx_cm_stream_ready: mpsc::Receiver<()>,
    #[cfg(all(target_os = "linux", feature = "linux_headless"))]
    #[cfg(not(any(feature = "flatpak", feature = "appimage")))]
    tx_desktop_ready: mpsc::Sender<()>,
}

impl ConnInner {
    pub fn new(id: i32, tx: Option<Sender>, tx_video: Option<Sender>) -> Self {
        Self { id, tx, tx_video }
    }
}

impl Subscriber for ConnInner {
    #[inline]
    fn id(&self) -> i32 {
        self.id
    }

    #[inline]
    fn send(&mut self, msg: Arc<Message>) {
        match &msg.union {
            Some(message::Union::VideoFrame(_)) => {
                self.tx_video.as_mut().map(|tx| {
                    allow_err!(tx.send((Instant::now(), msg)));
                });
            }
            _ => {
                self.tx.as_mut().map(|tx| {
                    allow_err!(tx.send((Instant::now(), msg)));
                });
            }
        }
    }
}

const TEST_DELAY_TIMEOUT: Duration = Duration::from_secs(1);
const SEC30: Duration = Duration::from_secs(30);
const H1: Duration = Duration::from_secs(3600);
const MILLI1: Duration = Duration::from_millis(1);
const SEND_TIMEOUT_VIDEO: u64 = 12_000;
const SEND_TIMEOUT_OTHER: u64 = SEND_TIMEOUT_VIDEO * 10;
const SESSION_TIMEOUT: Duration = Duration::from_secs(30);

impl Connection {
    pub async fn start(
        addr: SocketAddr,
        stream: super::Stream,
        id: i32,
        server: super::ServerPtrWeak,
    ) {
        let hash = Hash {
            salt: Config::get_salt(),
            challenge: Config::get_auto_password(6),
            ..Default::default()
        };
        ALIVE_CONNS.lock().unwrap().push(id);
        let (tx_from_cm_holder, mut rx_from_cm) = mpsc::unbounded_channel::<ipc::Data>();
        // holding tx_from_cm_holder to avoid cpu burning of rx_from_cm.recv when all sender closed
        let tx_from_cm = tx_from_cm_holder.clone();
        let (tx_to_cm, rx_to_cm) = mpsc::unbounded_channel::<ipc::Data>();
        let (tx, mut rx) = mpsc::unbounded_channel::<(Instant, Arc<Message>)>();
        let (tx_video, mut rx_video) = mpsc::unbounded_channel::<(Instant, Arc<Message>)>();
        let (tx_input, _rx_input) = std_mpsc::channel();
        let mut hbbs_rx = crate::hbbs_http::sync::signal_receiver();
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        let (tx_cm_stream_ready, _rx_cm_stream_ready) = mpsc::channel(1);
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        let (_tx_desktop_ready, rx_desktop_ready) = mpsc::channel(1);

        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        let tx_cloned = tx.clone();
        let mut conn = Self {
            inner: ConnInner {
                id,
                tx: Some(tx),
                tx_video: Some(tx_video),
            },
            stream,
            server,
            hash,
            read_jobs: Vec::new(),
            timer: time::interval(SEC30),
            file_timer: time::interval(SEC30),
            file_transfer: None,
            port_forward_socket: None,
            port_forward_address: "".to_owned(),
            tx_to_cm,
            authorized: false,
            keyboard: Connection::permission("enable-keyboard"),
            clipboard: Connection::permission("enable-clipboard"),
            audio: Connection::permission("enable-audio"),
            file: Connection::permission("enable-file-transfer"),
            restart: Connection::permission("enable-remote-restart"),
            recording: Connection::permission("enable-record-session"),
            last_test_delay: 0,
            lock_after_session_end: false,
            show_remote_cursor: false,
            ip: "".to_owned(),
            disable_audio: false,
            enable_file_transfer: false,
            disable_clipboard: false,
            disable_keyboard: false,
            tx_input,
            video_ack_required: false,
            peer_info: Default::default(),
            server_audit_conn: "".to_owned(),
            server_audit_file: "".to_owned(),
            lr: Default::default(),
            last_recv_time: Arc::new(Mutex::new(Instant::now())),
            chat_unanswered: false,
            #[cfg(windows)]
            portable: Default::default(),
            from_switch: false,
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            origin_resolution: Default::default(),
            audio_sender: None,
            voice_call_request_timestamp: None,
            audio_input_device_before_voice_call: None,
            options_in_login: None,
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            pressed_modifiers: Default::default(),
            #[cfg(all(target_os = "linux", feature = "linux_headless"))]
            #[cfg(not(any(feature = "flatpak", feature = "appimage")))]
            rx_cm_stream_ready: _rx_cm_stream_ready,
            #[cfg(all(target_os = "linux", feature = "linux_headless"))]
            #[cfg(not(any(feature = "flatpak", feature = "appimage")))]
            tx_desktop_ready: _tx_desktop_ready,
        };
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        tokio::spawn(async move {
            if let Err(err) =
                start_ipc(rx_to_cm, tx_from_cm, rx_desktop_ready, tx_cm_stream_ready).await
            {
                log::error!("ipc to connection manager exit: {}", err);
            }
        });
        #[cfg(target_os = "android")]
        start_channel(rx_to_cm, tx_from_cm);

        if !conn.on_open(addr).await {
            return;
        }
        if !conn.keyboard {
            conn.send_permission(Permission::Keyboard, false).await;
        }
        if !conn.clipboard {
            conn.send_permission(Permission::Clipboard, false).await;
        }
        if !conn.audio {
            conn.send_permission(Permission::Audio, false).await;
        }
        if !conn.file {
            conn.send_permission(Permission::File, false).await;
        }
        if !conn.restart {
            conn.send_permission(Permission::Restart, false).await;
        }
        if !conn.recording {
            conn.send_permission(Permission::Recording, false).await;
        }
        let mut test_delay_timer =
            time::interval_at(Instant::now() + TEST_DELAY_TIMEOUT, TEST_DELAY_TIMEOUT);
        let mut last_recv_time = Instant::now();

        conn.stream.set_send_timeout(
            if conn.file_transfer.is_some() || conn.port_forward_socket.is_some() {
                SEND_TIMEOUT_OTHER
            } else {
                SEND_TIMEOUT_VIDEO
            },
        );

        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        std::thread::spawn(move || Self::handle_input(_rx_input, tx_cloned));
        let mut second_timer = time::interval(Duration::from_secs(1));

        loop {
            tokio::select! {
                // biased; // video has higher priority // causing test_delay_timer failed while transferring big file

                Some(data) = rx_from_cm.recv() => {
                    match data {
                        ipc::Data::Authorize => {
                            conn.send_logon_response().await;
                            if conn.port_forward_socket.is_some() {
                                break;
                            }
                        }
                        ipc::Data::Close => {
                            conn.chat_unanswered = false; // seen
                            conn.send_close_reason_no_retry("").await;
                            conn.on_close("connection manager", true).await;
                            break;
                        }
                        ipc::Data::ChatMessage{text} => {
                            let mut misc = Misc::new();
                            misc.set_chat_message(ChatMessage {
                                text,
                                ..Default::default()
                            });
                            let mut msg_out = Message::new();
                            msg_out.set_misc(misc);
                            conn.send(msg_out).await;
                            conn.chat_unanswered = false;
                        }
                        ipc::Data::SwitchPermission{name, enabled} => {
                            log::info!("Change permission {} -> {}", name, enabled);
                            if &name == "keyboard" {
                                conn.keyboard = enabled;
                                conn.send_permission(Permission::Keyboard, enabled).await;
                                if let Some(s) = conn.server.upgrade() {
                                    s.write().unwrap().subscribe(
                                        NAME_CURSOR,
                                        conn.inner.clone(), enabled || conn.show_remote_cursor);
                                }
                            } else if &name == "clipboard" {
                                conn.clipboard = enabled;
                                conn.send_permission(Permission::Clipboard, enabled).await;
                                if let Some(s) = conn.server.upgrade() {
                                    s.write().unwrap().subscribe(
                                        super::clipboard_service::NAME,
                                        conn.inner.clone(), conn.clipboard_enabled() && conn.peer_keyboard_enabled());
                                }
                            } else if &name == "audio" {
                                conn.audio = enabled;
                                conn.send_permission(Permission::Audio, enabled).await;
                                if let Some(s) = conn.server.upgrade() {
                                    s.write().unwrap().subscribe(
                                        super::audio_service::NAME,
                                        conn.inner.clone(), conn.audio_enabled());
                                }
                            } else if &name == "file" {
                                conn.file = enabled;
                                conn.send_permission(Permission::File, enabled).await;
                                conn.send_to_cm(ipc::Data::ClipboardFileEnabled(conn.file_transfer_enabled()));
                            } else if &name == "restart" {
                                conn.restart = enabled;
                                conn.send_permission(Permission::Restart, enabled).await;
                            } else if &name == "recording" {
                                conn.recording = enabled;
                                conn.send_permission(Permission::Recording, enabled).await;
                            }
                        }
                        ipc::Data::RawMessage(bytes) => {
                            allow_err!(conn.stream.send_raw(bytes).await);
                        }
                        #[cfg(windows)]
                        ipc::Data::ClipboardFile(_clip) => {
                            if conn.file_transfer_enabled() {
                                allow_err!(conn.stream.send(&clip_2_msg(_clip)).await);
                            }
                        }
                        ipc::Data::PrivacyModeState((_, state)) => {
                            let msg_out = match state {
                                ipc::PrivacyModeState::OffSucceeded => {
                                    video_service::set_privacy_mode_conn_id(0);
                                    crate::common::make_privacy_mode_msg(
                                        back_notification::PrivacyModeState::PrvOffSucceeded,
                                    )
                                }
                                ipc::PrivacyModeState::OffFailed => {
                                    crate::common::make_privacy_mode_msg(
                                        back_notification::PrivacyModeState::PrvOffFailed,
                                    )
                                }
                                ipc::PrivacyModeState::OffByPeer => {
                                    video_service::set_privacy_mode_conn_id(0);
                                    crate::common::make_privacy_mode_msg(
                                        back_notification::PrivacyModeState::PrvOffByPeer,
                                    )
                                }
                                ipc::PrivacyModeState::OffUnknown => {
                                    video_service::set_privacy_mode_conn_id(0);
                                     crate::common::make_privacy_mode_msg(
                                        back_notification::PrivacyModeState::PrvOffUnknown,
                                    )
                                }
                            };
                            conn.send(msg_out).await;
                        }
                        #[cfg(windows)]
                        ipc::Data::DataPortableService(ipc::DataPortableService::RequestStart) => {
                            if let Err(e) = portable_client::start_portable_service(portable_client::StartPara::Direct) {
                                log::error!("Failed to start portable service from cm:{:?}", e);
                            }
                        }
                        ipc::Data::SwitchSidesBack => {
                            let mut misc = Misc::new();
                            misc.set_switch_back(SwitchBack::default());
                            let mut msg = Message::new();
                            msg.set_misc(misc);
                            conn.send(msg).await;
                        }
                        ipc::Data::VoiceCallResponse(accepted) => {
                            conn.handle_voice_call(accepted).await;
                        }
                        ipc::Data::CloseVoiceCall(_reason) => {
                            log::debug!("Close the voice call from the ipc.");
                            conn.close_voice_call().await;
                            // Notify the peer that we closed the voice call.
                            let msg = new_voice_call_request(false);
                            conn.send(msg).await;
                        }
                        _ => {}
                    }
                },
                res = conn.stream.next() => {
                    if let Some(res) = res {
                        match res {
                            Err(err) => {
                                conn.on_close(&err.to_string(), true).await;
                                break;
                            },
                            Ok(bytes) => {
                                last_recv_time = Instant::now();
                                *conn.last_recv_time.lock().unwrap() = Instant::now();
                                if let Ok(msg_in) = Message::parse_from_bytes(&bytes) {
                                    if !conn.on_message(msg_in).await {
                                        break;
                                    }
                                }
                            }
                        }
                    } else {
                        conn.on_close("Reset by the peer", true).await;
                        break;
                    }
                },
                _ = conn.file_timer.tick() => {
                    if !conn.read_jobs.is_empty() {
                        if let Err(err) = fs::handle_read_jobs(&mut conn.read_jobs, &mut conn.stream).await {
                            conn.on_close(&err.to_string(), false).await;
                            break;
                        }
                    } else {
                        conn.file_timer = time::interval_at(Instant::now() + SEC30, SEC30);
                    }
                }
                Ok(conns) = hbbs_rx.recv() => {
                    if conns.contains(&id) {
                        conn.send_close_reason_no_retry("Closed manually by web console").await;
                        conn.on_close("web console", true).await;
                        break;
                    }
                }
                Some((instant, value)) = rx_video.recv() => {
                    if !conn.video_ack_required {
                        video_service::notify_video_frame_fetched(id, Some(instant.into()));
                    }
                    if let Err(err) = conn.stream.send(&value as &Message).await {
                        conn.on_close(&err.to_string(), false).await;
                        break;
                    }
                },
                Some((instant, value)) = rx.recv() => {
                    let latency = instant.elapsed().as_millis() as i64;
                    let msg: &Message = &value;

                    if latency > 1000 {
                        match &msg.union {
                            Some(message::Union::AudioFrame(_)) => {
                                // log::info!("audio frame latency {}", instant.elapsed().as_secs_f32());
                                continue;
                            }
                            _ => {}
                        }
                    }
                    match &msg.union {
                        Some(message::Union::Misc(m)) => {
                            match &m.union {
                                Some(misc::Union::StopService(_)) => {
                                    conn.send_close_reason_no_retry("").await;
                                    conn.on_close("stop service", true).await;
                                    break;
                                }
                                _ => {},
                            }
                        }
                        _ => {}
                    }
                    if let Err(err) = conn.stream.send(msg).await {
                        conn.on_close(&err.to_string(), false).await;
                        break;
                    }
                },
                _ = second_timer.tick() => {
                    #[cfg(windows)]
                    conn.portable_check();
                }
                _ = test_delay_timer.tick() => {
                    if last_recv_time.elapsed() >= SEC30 {
                        conn.on_close("Timeout", true).await;
                        break;
                    }
                    let time = get_time();
                    if time > 0 && conn.last_test_delay == 0 {
                        conn.last_test_delay = time;
                        let mut msg_out = Message::new();
                        let qos = video_service::VIDEO_QOS.lock().unwrap();
                        msg_out.set_test_delay(TestDelay{
                            time,
                            last_delay:qos.current_delay,
                            target_bitrate:qos.target_bitrate,
                            ..Default::default()
                        });
                        conn.inner.send(msg_out.into());
                    }
                }
            }
        }

        let video_privacy_conn_id = video_service::get_privacy_mode_conn_id();
        if video_privacy_conn_id == id {
            video_service::set_privacy_mode_conn_id(0);
            let _ = privacy_mode::turn_off_privacy(id);
        } else if video_privacy_conn_id == 0 {
            let _ = privacy_mode::turn_off_privacy(0);
        }
        video_service::notify_video_frame_fetched(id, None);
        scrap::codec::Encoder::update(id, scrap::codec::EncodingUpdate::Remove);
        video_service::VIDEO_QOS.lock().unwrap().reset();
        if conn.authorized {
            password::update_temporary_password();
        }
        if let Err(err) = conn.try_port_forward_loop(&mut rx_from_cm).await {
            conn.on_close(&err.to_string(), false).await;
        }

        conn.post_conn_audit(json!({
            "action": "close",
        }));
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        conn.reset_resolution();
        ALIVE_CONNS.lock().unwrap().retain(|&c| c != id);
        if let Some(s) = conn.server.upgrade() {
            let mut s = s.write().unwrap();
            s.remove_connection(&conn.inner);
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            try_stop_record_cursor_pos();
        }
        log::info!("#{} connection loop exited", id);
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    fn handle_input(receiver: std_mpsc::Receiver<MessageInput>, tx: Sender) {
        let mut block_input_mode = false;
        #[cfg(target_os = "windows")]
        {
            rdev::set_dw_mouse_extra_info(enigo::ENIGO_INPUT_EXTRA_VALUE);
            rdev::set_dw_keyboard_extra_info(enigo::ENIGO_INPUT_EXTRA_VALUE);
        }
        #[cfg(target_os = "macos")]
        reset_input_ondisconn();
        loop {
            match receiver.recv_timeout(std::time::Duration::from_millis(500)) {
                Ok(v) => match v {
                    MessageInput::Mouse((msg, id)) => {
                        handle_mouse(&msg, id);
                    }
                    MessageInput::Key((mut msg, press)) => {
                        // todo: press and down have similar meanings.
                        if press && msg.mode.unwrap() == KeyboardMode::Legacy {
                            msg.down = true;
                        }
                        handle_key(&msg);
                        if press && msg.mode.unwrap() == KeyboardMode::Legacy {
                            msg.down = false;
                            handle_key(&msg);
                        }
                    }
                    MessageInput::BlockOn => {
                        if crate::platform::block_input(true) {
                            block_input_mode = true;
                        } else {
                            Self::send_block_input_error(
                                &tx,
                                back_notification::BlockInputState::BlkOnFailed,
                            );
                        }
                    }
                    MessageInput::BlockOff => {
                        if crate::platform::block_input(false) {
                            block_input_mode = false;
                        } else {
                            Self::send_block_input_error(
                                &tx,
                                back_notification::BlockInputState::BlkOffFailed,
                            );
                        }
                    }
                },
                Err(err) => {
                    #[cfg(not(any(target_os = "android", target_os = "ios")))]
                    if block_input_mode {
                        let _ = crate::platform::block_input(true);
                    }
                    if std_mpsc::RecvTimeoutError::Disconnected == err {
                        break;
                    }
                }
            }
        }
        #[cfg(target_os = "linux")]
        clear_remapped_keycode();
        log::info!("Input thread exited");
    }

    async fn try_port_forward_loop(
        &mut self,
        rx_from_cm: &mut mpsc::UnboundedReceiver<Data>,
    ) -> ResultType<()> {
        let mut last_recv_time = Instant::now();
        if let Some(mut forward) = self.port_forward_socket.take() {
            log::info!("Running port forwarding loop");
            self.stream.set_raw();
            let mut hbbs_rx = crate::hbbs_http::sync::signal_receiver();
            loop {
                tokio::select! {
                    Some(data) = rx_from_cm.recv() => {
                        match data {
                            ipc::Data::Close => {
                                bail!("Close requested from selection manager");
                            }
                            _ => {}
                        }
                    }
                    res = forward.next() => {
                        if let Some(res) = res {
                            last_recv_time = Instant::now();
                            self.stream.send_bytes(res?.into()).await?;
                        } else {
                            bail!("Forward reset by the peer");
                        }
                    },
                    res = self.stream.next() => {
                        if let Some(res) = res {
                            last_recv_time = Instant::now();
                            timeout(SEND_TIMEOUT_OTHER, forward.send(res?)).await??;
                        } else {
                            bail!("Stream reset by the peer");
                        }
                    },
                    _ = self.timer.tick() => {
                        if last_recv_time.elapsed() >= H1 {
                            bail!("Timeout");
                        }
                    }
                    Ok(conns) = hbbs_rx.recv() => {
                        if conns.contains(&self.inner.id) {
                            // todo: check reconnect
                            bail!("Closed manually by the web console");
                        }
                    }
                }
            }
        }
        Ok(())
    }

    async fn send_permission(&mut self, permission: Permission, enabled: bool) {
        let mut misc = Misc::new();
        misc.set_permission_info(PermissionInfo {
            permission: permission.into(),
            enabled,
            ..Default::default()
        });
        let mut msg_out = Message::new();
        msg_out.set_misc(misc);
        self.send(msg_out).await;
    }

    async fn on_open(&mut self, addr: SocketAddr) -> bool {
        log::debug!("#{} Connection opened from {}.", self.inner.id, addr);
        let whitelist: Vec<String> = Config::get_option("whitelist")
            .split(",")
            .filter(|x| !x.is_empty())
            .map(|x| x.to_owned())
            .collect();
        if !whitelist.is_empty()
            && whitelist
                .iter()
                .filter(|x| x == &"0.0.0.0")
                .next()
                .is_none()
            && whitelist
                .iter()
                .filter(|x| IpCidr::from_str(x).map_or(false, |y| y.contains(addr.ip())))
                .next()
                .is_none()
        {
            self.send_login_error("Your ip is blocked by the peer")
                .await;
            Self::post_alarm_audit(
                AlarmAuditType::IpWhitelist, //"ip whitelist",
                true,
                json!({
                            "ip":addr.ip(),
                }),
            );
            sleep(1.).await;
            return false;
        }
        self.ip = addr.ip().to_string();
        let mut msg_out = Message::new();
        msg_out.set_hash(self.hash.clone());
        self.send(msg_out).await;
        self.get_api_server();
        self.post_conn_audit(json!({
            "ip": addr.ip(),
            "action": "new",
        }));
        true
    }

    fn get_api_server(&mut self) {
        self.server_audit_conn = crate::get_audit_server(
            Config::get_option("api-server"),
            Config::get_option("custom-rendezvous-server"),
            "conn".to_owned(),
        );
        self.server_audit_file = crate::get_audit_server(
            Config::get_option("api-server"),
            Config::get_option("custom-rendezvous-server"),
            "file".to_owned(),
        );
    }

    fn post_conn_audit(&self, v: Value) {
        if self.server_audit_conn.is_empty() {
            return;
        }
        let url = self.server_audit_conn.clone();
        let mut v = v;
        v["id"] = json!(Config::get_id());
        v["uuid"] = json!(crate::encode64(hbb_common::get_uuid()));
        v["conn_id"] = json!(self.inner.id);
        tokio::spawn(async move {
            allow_err!(Self::post_audit_async(url, v).await);
        });
    }

    fn post_file_audit(
        &self,
        r#type: FileAuditType,
        path: &str,
        files: Vec<(String, i64)>,
        info: Value,
    ) {
        if self.server_audit_file.is_empty() {
            return;
        }
        let url = self.server_audit_file.clone();
        let file_num = files.len();
        let mut files = files;
        files.sort_by(|a, b| b.1.cmp(&a.1));
        files.truncate(10);
        let is_file = files.len() == 1 && files[0].0.is_empty();
        let mut info = info;
        info["ip"] = json!(self.ip.clone());
        info["name"] = json!(self.lr.my_name.clone());
        info["num"] = json!(file_num);
        info["files"] = json!(files);
        let v = json!({
            "id":json!(Config::get_id()),
            "uuid":json!(crate::encode64(hbb_common::get_uuid())),
            "peer_id":json!(self.lr.my_id),
            "type": r#type as i8,
            "path":path,
            "is_file":is_file,
            "info":json!(info).to_string(),
        });
        tokio::spawn(async move {
            allow_err!(Self::post_audit_async(url, v).await);
        });
    }

    pub fn post_alarm_audit(typ: AlarmAuditType, from_remote: bool, info: Value) {
        let url = crate::get_audit_server(
            Config::get_option("api-server"),
            Config::get_option("custom-rendezvous-server"),
            "alarm".to_owned(),
        );
        if url.is_empty() {
            return;
        }
        let mut v = Value::default();
        v["id"] = json!(Config::get_id());
        v["uuid"] = json!(crate::encode64(hbb_common::get_uuid()));
        v["typ"] = json!(typ as i8);
        v["from_remote"] = json!(from_remote);
        v["info"] = serde_json::Value::String(info.to_string());
        tokio::spawn(async move {
            allow_err!(Self::post_audit_async(url, v).await);
        });
    }

    #[inline]
    async fn post_audit_async(url: String, v: Value) -> ResultType<String> {
        crate::post_request(url, v.to_string(), "").await
    }

    async fn send_logon_response(&mut self) {
        if self.authorized {
            return;
        }
        let conn_type = if self.file_transfer.is_some() {
            1
        } else if self.port_forward_socket.is_some() {
            2
        } else {
            0
        };
        self.post_conn_audit(json!({"peer": self.peer_info, "type": conn_type}));
        #[allow(unused_mut)]
        let mut username = crate::platform::get_active_username();
        let mut res = LoginResponse::new();
        let mut pi = PeerInfo {
            username: username.clone(),
            conn_id: self.inner.id,
            version: VERSION.to_owned(),
            ..Default::default()
        };

        #[cfg(not(target_os = "android"))]
        {
            pi.hostname = whoami::hostname();
            pi.platform = whoami::platform().to_string();
        }
        #[cfg(target_os = "android")]
        {
            pi.hostname = DEVICE_NAME.lock().unwrap().clone();
            pi.platform = "Android".into();
        }
        #[cfg(target_os = "linux")]
        {
            let mut platform_additions = serde_json::Map::new();
            if crate::platform::current_is_wayland() {
                platform_additions.insert("is_wayland".into(), json!(true));
            }
            #[cfg(feature = "linux_headless")]
            #[cfg(not(any(feature = "flatpak", feature = "appimage")))]
            if linux_desktop_manager::is_headless() {
                platform_additions.insert("headless".into(), json!(true));
            }
            if !platform_additions.is_empty() {
                pi.platform_additions =
                    serde_json::to_string(&platform_additions).unwrap_or("".into());
            }
        }

        pi.encoding = Some(scrap::codec::Encoder::supported_encoding()).into();

        if self.port_forward_socket.is_some() {
            let mut msg_out = Message::new();
            res.set_peer_info(pi);
            msg_out.set_login_response(res);
            self.send(msg_out).await;
            return;
        }
        #[cfg(target_os = "linux")]
        if !self.file_transfer.is_some() && !self.port_forward_socket.is_some() {
            let dtype = crate::platform::linux::get_display_server();
            if dtype != crate::platform::linux::DISPLAY_SERVER_X11
                && dtype != crate::platform::linux::DISPLAY_SERVER_WAYLAND
            {
                res.set_error(format!(
                    "Unsupported display server type \"{}\", x11 or wayland expected",
                    dtype
                ));
                let mut msg_out = Message::new();
                msg_out.set_login_response(res);
                self.send(msg_out).await;
                return;
            }
        }
        #[allow(unused_mut)]
        let mut sas_enabled = false;
        #[cfg(windows)]
        if crate::platform::is_root() {
            sas_enabled = true;
        }
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        if self.file_transfer.is_some() {
            if crate::platform::is_prelogin() || self.tx_to_cm.send(ipc::Data::Test).is_err() {
                username = "".to_owned();
            }
        }
        self.authorized = true;

        pi.username = username;
        pi.sas_enabled = sas_enabled;
        pi.features = Some(Features {
            privacy_mode: video_service::is_privacy_mode_supported(),
            ..Default::default()
        })
        .into();
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            pi.resolutions = Some(SupportedResolutions {
                resolutions: video_service::get_current_display_name()
                    .map(|name| crate::platform::resolutions(&name))
                    .unwrap_or(vec![]),
                ..Default::default()
            })
            .into();
        }

        let mut sub_service = false;
        if self.file_transfer.is_some() {
            res.set_peer_info(pi);
        } else {
            try_activate_screen();
            if let Some(msg_out) = super::video_service::is_inited_msg() {
                self.send(msg_out).await;
            }

            match super::video_service::get_displays().await {
                Err(err) => {
                    res.set_error(format!("{}", err));
                }
                Ok((current, displays)) => {
                    pi.displays = displays.clone();
                    pi.current_display = current as _;
                    res.set_peer_info(pi);
                    sub_service = true;
                    *super::video_service::LAST_SYNC_DISPLAYS.write().unwrap() = displays;
                }
            }
        }
        let mut msg_out = Message::new();
        msg_out.set_login_response(res);
        self.send(msg_out).await;
        if let Some(o) = self.options_in_login.take() {
            self.update_options(&o).await;
        }
        if let Some((dir, show_hidden)) = self.file_transfer.clone() {
            let dir = if !dir.is_empty() && std::path::Path::new(&dir).is_dir() {
                &dir
            } else {
                ""
            };
            self.read_dir(dir, show_hidden);
        } else if sub_service {
            if let Some(s) = self.server.upgrade() {
                let mut noperms = Vec::new();
                if !self.peer_keyboard_enabled() && !self.show_remote_cursor {
                    noperms.push(NAME_CURSOR);
                }
                if !self.show_remote_cursor {
                    noperms.push(NAME_POS);
                }
                if !self.clipboard_enabled() || !self.peer_keyboard_enabled() {
                    noperms.push(super::clipboard_service::NAME);
                }
                if !self.audio_enabled() {
                    noperms.push(super::audio_service::NAME);
                }
                let mut s = s.write().unwrap();
                #[cfg(not(any(target_os = "android", target_os = "ios")))]
                try_start_record_cursor_pos();
                s.add_connection(self.inner.clone(), &noperms);
            }
        }
    }

    fn peer_keyboard_enabled(&self) -> bool {
        self.keyboard && !self.disable_keyboard
    }

    fn clipboard_enabled(&self) -> bool {
        self.clipboard && !self.disable_clipboard
    }

    fn audio_enabled(&self) -> bool {
        self.audio && !self.disable_audio
    }

    fn file_transfer_enabled(&self) -> bool {
        self.file && self.enable_file_transfer
    }

    fn try_start_cm(&mut self, peer_id: String, name: String, authorized: bool) {
        self.peer_info = (peer_id.clone(), name.clone());
        self.send_to_cm(ipc::Data::Login {
            id: self.inner.id(),
            is_file_transfer: self.file_transfer.is_some(),
            port_forward: self.port_forward_address.clone(),
            peer_id,
            name,
            authorized,
            keyboard: self.keyboard,
            clipboard: self.clipboard,
            audio: self.audio,
            file: self.file,
            file_transfer_enabled: self.file_transfer_enabled(),
            restart: self.restart,
            recording: self.recording,
            from_switch: self.from_switch,
        });
    }

    #[inline]
    fn send_to_cm(&mut self, data: ipc::Data) {
        self.tx_to_cm.send(data).ok();
    }

    #[inline]
    fn send_fs(&mut self, data: ipc::FS) {
        self.send_to_cm(ipc::Data::FS(data));
    }

    async fn send_login_error<T: std::string::ToString>(&mut self, err: T) {
        let mut msg_out = Message::new();
        let mut res = LoginResponse::new();
        res.set_error(err.to_string());
        msg_out.set_login_response(res);
        self.send(msg_out).await;
    }

    #[inline]
    pub fn send_block_input_error(s: &Sender, state: back_notification::BlockInputState) {
        let mut misc = Misc::new();
        let mut back_notification = BackNotification::new();
        back_notification.set_block_input_state(state);
        misc.set_back_notification(back_notification);
        let mut msg_out = Message::new();
        msg_out.set_misc(misc);
        s.send((Instant::now(), Arc::new(msg_out))).ok();
    }

    #[inline]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    fn input_mouse(&self, msg: MouseEvent, conn_id: i32) {
        self.tx_input.send(MessageInput::Mouse((msg, conn_id))).ok();
    }

    #[inline]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    fn input_key(&self, msg: KeyEvent, press: bool) {
        self.tx_input.send(MessageInput::Key((msg, press))).ok();
    }

    fn validate_one_password(&self, password: String) -> bool {
        if password.len() == 0 {
            return false;
        }
        let mut hasher = Sha256::new();
        hasher.update(password);
        hasher.update(&self.hash.salt);
        let mut hasher2 = Sha256::new();
        hasher2.update(&hasher.finalize()[..]);
        hasher2.update(&self.hash.challenge);
        hasher2.finalize()[..] == self.lr.password[..]
    }

    fn validate_password(&mut self) -> bool {
        if password::temporary_enabled() {
            let password = password::temporary_password();
            if self.validate_one_password(password.clone()) {
                SESSIONS.lock().unwrap().insert(
                    self.lr.my_id.clone(),
                    Session {
                        name: self.lr.my_name.clone(),
                        session_id: self.lr.session_id,
                        last_recv_time: self.last_recv_time.clone(),
                        random_password: password,
                    },
                );
                return true;
            }
        }
        if password::permanent_enabled() {
            if self.validate_one_password(Config::get_permanent_password()) {
                return true;
            }
        }
        false
    }

    fn is_recent_session(&mut self) -> bool {
        let session = SESSIONS
            .lock()
            .unwrap()
            .get(&self.lr.my_id)
            .map(|s| s.to_owned());
        SESSIONS
            .lock()
            .unwrap()
            .retain(|_, s| s.last_recv_time.lock().unwrap().elapsed() < SESSION_TIMEOUT);
        if let Some(session) = session {
            if session.name == self.lr.my_name
                && session.session_id == self.lr.session_id
                && !self.lr.password.is_empty()
                && self.validate_one_password(session.random_password.clone())
            {
                SESSIONS.lock().unwrap().insert(
                    self.lr.my_id.clone(),
                    Session {
                        name: self.lr.my_name.clone(),
                        session_id: self.lr.session_id,
                        last_recv_time: self.last_recv_time.clone(),
                        random_password: session.random_password,
                    },
                );
                return true;
            }
        }
        false
    }

    pub fn permission(enable_prefix_option: &str) -> bool {
        #[cfg(feature = "flutter")]
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            let access_mode = Config::get_option("access-mode");
            if access_mode == "full" {
                return true;
            } else if access_mode == "view" {
                return false;
            }
        }
        return Config::get_option(enable_prefix_option).is_empty();
    }

    async fn handle_login_request_without_validation(&mut self, lr: &LoginRequest) {
        self.lr = lr.clone();
        if let Some(o) = lr.option.as_ref() {
            self.options_in_login = Some(o.clone());
            if let Some(q) = o.supported_decoding.clone().take() {
                scrap::codec::Encoder::update(
                    self.inner.id(),
                    scrap::codec::EncodingUpdate::New(q),
                );
            } else {
                scrap::codec::Encoder::update(
                    self.inner.id(),
                    scrap::codec::EncodingUpdate::NewOnlyVP9,
                );
            }
        } else {
            scrap::codec::Encoder::update(
                self.inner.id(),
                scrap::codec::EncodingUpdate::NewOnlyVP9,
            );
        }
        self.video_ack_required = lr.video_ack_required;
    }

    async fn on_message(&mut self, msg: Message) -> bool {
        if let Some(message::Union::LoginRequest(lr)) = msg.union {
            self.handle_login_request_without_validation(&lr).await;
            if self.authorized {
                return true;
            }
            match lr.union {
                Some(login_request::Union::FileTransfer(ft)) => {
                    if !Connection::permission("enable-file-transfer") {
                        self.send_login_error("No permission of file transfer")
                            .await;
                        sleep(1.).await;
                        return false;
                    }
                    self.file_transfer = Some((ft.dir, ft.show_hidden));
                }
                Some(login_request::Union::PortForward(mut pf)) => {
                    let mut is_rdp = false;
                    if pf.host == "RDP" && pf.port == 0 {
                        pf.host = "localhost".to_owned();
                        pf.port = 3389;
                        is_rdp = true;
                    }
                    if is_rdp && !Connection::permission("enable-rdp")
                        || !is_rdp && !Connection::permission("enable-tunnel")
                    {
                        if is_rdp {
                            self.send_login_error("No permission of RDP").await;
                        } else {
                            self.send_login_error("No permission of IP tunneling").await;
                        }
                        sleep(1.).await;
                        return false;
                    }
                    if pf.host.is_empty() {
                        pf.host = "localhost".to_owned();
                    }
                    let mut addr = format!("{}:{}", pf.host, pf.port);
                    self.port_forward_address = addr.clone();
                    match timeout(3000, TcpStream::connect(&addr)).await {
                        Ok(Ok(sock)) => {
                            self.port_forward_socket = Some(Framed::new(sock, BytesCodec::new()));
                        }
                        _ => {
                            if is_rdp {
                                addr = "RDP".to_owned();
                            }
                            self.send_login_error(format!(
                                "Failed to access remote {}, please make sure if it is open",
                                addr
                            ))
                            .await;
                            return false;
                        }
                    }
                }
                _ => {}
            }

            #[cfg(all(target_os = "linux", feature = "linux_headless"))]
            #[cfg(not(any(feature = "flatpak", feature = "appimage")))]
            let desktop_err = match lr.os_login.as_ref() {
                Some(os_login) => {
                    linux_desktop_manager::try_start_desktop(&os_login.username, &os_login.password)
                }
                None => linux_desktop_manager::try_start_desktop("", ""),
            };
            #[cfg(all(target_os = "linux", feature = "linux_headless"))]
            #[cfg(not(any(feature = "flatpak", feature = "appimage")))]
            let is_headless = linux_desktop_manager::is_headless();
            #[cfg(all(target_os = "linux", feature = "linux_headless"))]
            #[cfg(not(any(feature = "flatpak", feature = "appimage")))]
            let wait_ipc_timeout = 10_000;

            // If err is LOGIN_MSG_DESKTOP_SESSION_NOT_READY, just keep this msg and go on checking password.
            #[cfg(all(target_os = "linux", feature = "linux_headless"))]
            #[cfg(not(any(feature = "flatpak", feature = "appimage")))]
            if !desktop_err.is_empty() && desktop_err != LOGIN_MSG_DESKTOP_SESSION_NOT_READY {
                self.send_login_error(desktop_err).await;
                return true;
            }

            if !hbb_common::is_ipv4_str(&lr.username) && lr.username != Config::get_id() {
                self.send_login_error(LOGIN_MSG_OFFLINE).await;
            } else if password::approve_mode() == ApproveMode::Click
                || password::approve_mode() == ApproveMode::Both && !password::has_valid_password()
            {
                self.try_start_cm(lr.my_id, lr.my_name, false);
                if hbb_common::get_version_number(&lr.version)
                    >= hbb_common::get_version_number("1.2.0")
                {
                    self.send_login_error(LOGIN_MSG_NO_PASSWORD_ACCESS).await;
                }
                return true;
            } else if password::approve_mode() == ApproveMode::Password
                && !password::has_valid_password()
            {
                self.send_login_error("Connection not allowed").await;
                return false;
            } else if self.is_recent_session() {
                #[cfg(all(target_os = "linux", feature = "linux_headless"))]
                #[cfg(not(any(feature = "flatpak", feature = "appimage")))]
                if desktop_err.is_empty() {
                    #[cfg(target_os = "linux")]
                    if is_headless {
                        self.tx_desktop_ready.send(()).await.ok();
                        let _res = timeout(wait_ipc_timeout, self.rx_cm_stream_ready.recv()).await;
                    }
                    self.try_start_cm(lr.my_id, lr.my_name, true);
                    self.send_logon_response().await;
                    if self.port_forward_socket.is_some() {
                        return false;
                    }
                } else {
                    self.send_login_error(desktop_err).await;
                }
                #[cfg(not(all(target_os = "linux", feature = "linux_headless")))]
                {
                    self.try_start_cm(lr.my_id, lr.my_name, true);
                    self.send_logon_response().await;
                    if self.port_forward_socket.is_some() {
                        return false;
                    }
                }
            } else if lr.password.is_empty() {
                #[cfg(all(target_os = "linux", feature = "linux_headless"))]
                #[cfg(not(any(feature = "flatpak", feature = "appimage")))]
                if desktop_err.is_empty() {
                    self.try_start_cm(lr.my_id, lr.my_name, false);
                } else {
                    self.send_login_error(LOGIN_MSG_DESKTOP_SESSION_NOT_READY_PASSWORD_EMPTY)
                        .await;
                }
                #[cfg(not(all(target_os = "linux", feature = "linux_headless")))]
                self.try_start_cm(lr.my_id, lr.my_name, false);
            } else {
                let mut failure = LOGIN_FAILURES
                    .lock()
                    .unwrap()
                    .get(&self.ip)
                    .map(|x| x.clone())
                    .unwrap_or((0, 0, 0));
                let time = (get_time() / 60_000) as i32;
                if failure.2 > 30 {
                    self.send_login_error("Too many wrong password attempts")
                        .await;
                    Self::post_alarm_audit(
                        AlarmAuditType::ManyWrongPassword,
                        true,
                        json!({
                                    "ip":self.ip,
                        }),
                    );
                } else if time == failure.0 && failure.1 > 6 {
                    self.send_login_error("Please try 1 minute later").await;
                    Self::post_alarm_audit(
                        AlarmAuditType::FrequentAttempt,
                        true,
                        json!({
                                    "ip":self.ip,
                        }),
                    );
                } else if !self.validate_password() {
                    if failure.0 == time {
                        failure.1 += 1;
                        failure.2 += 1;
                    } else {
                        failure.0 = time;
                        failure.1 = 1;
                        failure.2 += 1;
                    }
                    LOGIN_FAILURES
                        .lock()
                        .unwrap()
                        .insert(self.ip.clone(), failure);
                    #[cfg(all(target_os = "linux", feature = "linux_headless"))]
                    #[cfg(not(any(feature = "flatpak", feature = "appimage")))]
                    if desktop_err.is_empty() {
                        self.send_login_error(LOGIN_MSG_PASSWORD_WRONG).await;
                        self.try_start_cm(lr.my_id, lr.my_name, false);
                    } else {
                        self.send_login_error(LOGIN_MSG_DESKTOP_SESSION_NOT_READY_PASSWORD_WRONG)
                            .await;
                    }
                    #[cfg(not(all(target_os = "linux", feature = "linux_headless")))]
                    {
                        self.send_login_error(LOGIN_MSG_PASSWORD_WRONG).await;
                        self.try_start_cm(lr.my_id, lr.my_name, false);
                    }
                } else {
                    if failure.0 != 0 {
                        LOGIN_FAILURES.lock().unwrap().remove(&self.ip);
                    }
                    #[cfg(all(target_os = "linux", feature = "linux_headless"))]
                    #[cfg(not(any(feature = "flatpak", feature = "appimage")))]
                    if desktop_err.is_empty() {
                        #[cfg(target_os = "linux")]
                        if is_headless {
                            self.tx_desktop_ready.send(()).await.ok();
                            let _res =
                                timeout(wait_ipc_timeout, self.rx_cm_stream_ready.recv()).await;
                        }
                        self.send_logon_response().await;
                        self.try_start_cm(lr.my_id, lr.my_name, true);
                        if self.port_forward_socket.is_some() {
                            return false;
                        }
                    } else {
                        self.send_login_error(desktop_err).await;
                    }
                    #[cfg(not(all(target_os = "linux", feature = "linux_headless")))]
                    {
                        self.send_logon_response().await;
                        self.try_start_cm(lr.my_id, lr.my_name, true);
                        if self.port_forward_socket.is_some() {
                            return false;
                        }
                    }
                }
            }
        } else if let Some(message::Union::TestDelay(t)) = msg.union {
            if t.from_client {
                let mut msg_out = Message::new();
                msg_out.set_test_delay(t);
                self.inner.send(msg_out.into());
            } else {
                self.last_test_delay = 0;
                let new_delay = (get_time() - t.time) as u32;
                video_service::VIDEO_QOS
                    .lock()
                    .unwrap()
                    .update_network_delay(new_delay);
            }
        } else if let Some(message::Union::SwitchSidesResponse(_s)) = msg.union {
            #[cfg(feature = "flutter")]
            if let Some(lr) = _s.lr.clone().take() {
                self.handle_login_request_without_validation(&lr).await;
                SWITCH_SIDES_UUID
                    .lock()
                    .unwrap()
                    .retain(|_, v| v.0.elapsed() < Duration::from_secs(10));
                let uuid_old = SWITCH_SIDES_UUID.lock().unwrap().remove(&lr.my_id);
                if let Ok(uuid) = uuid::Uuid::from_slice(_s.uuid.to_vec().as_ref()) {
                    if let Some((_instant, uuid_old)) = uuid_old {
                        if uuid == uuid_old {
                            self.from_switch = true;
                            self.try_start_cm(lr.my_id.clone(), lr.my_name.clone(), true);
                            self.send_logon_response().await;
                        }
                    }
                }
            }
        } else if self.authorized {
            match msg.union {
                Some(message::Union::MouseEvent(me)) => {
                    #[cfg(any(target_os = "android", target_os = "ios"))]
                    if let Err(e) = call_main_service_mouse_input(me.mask, me.x, me.y) {
                        log::debug!("call_main_service_mouse_input fail:{}", e);
                    }
                    #[cfg(not(any(target_os = "android", target_os = "ios")))]
                    if self.peer_keyboard_enabled() {
                        if is_left_up(&me) {
                            CLICK_TIME.store(get_time(), Ordering::SeqCst);
                        } else {
                            MOUSE_MOVE_TIME.store(get_time(), Ordering::SeqCst);
                        }
                        self.input_mouse(me, self.inner.id());
                    }
                }
                #[cfg(any(target_os = "android", target_os = "ios"))]
                Some(message::Union::KeyEvent(..)) => {}
                #[cfg(not(any(target_os = "android", target_os = "ios")))]
                Some(message::Union::KeyEvent(me)) => {
                    if self.peer_keyboard_enabled() {
                        if is_enter(&me) {
                            CLICK_TIME.store(get_time(), Ordering::SeqCst);
                        }
                        // handle all down as press
                        // fix unexpected repeating key on remote linux, seems also fix abnormal alt/shift, which
                        // make sure all key are released
                        let is_press = if cfg!(target_os = "linux") {
                            (me.press || me.down) && !crate::is_modifier(&me)
                        } else {
                            me.press
                        };

                        let key = match me.mode.enum_value_or_default() {
                            KeyboardMode::Map => {
                                Some(crate::keyboard::keycode_to_rdev_key(me.chr()))
                            }
                            KeyboardMode::Translate => {
                                if let Some(key_event::Union::Chr(code)) = me.union {
                                    Some(crate::keyboard::keycode_to_rdev_key(code & 0x0000FFFF))
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        }
                        .filter(crate::keyboard::is_modifier);

                        if let Some(key) = key {
                            if is_press {
                                self.pressed_modifiers.insert(key);
                            } else {
                                self.pressed_modifiers.remove(&key);
                            }
                        }

                        if is_press {
                            match me.union {
                                Some(key_event::Union::Unicode(_))
                                | Some(key_event::Union::Seq(_)) => {
                                    self.input_key(me, false);
                                }
                                _ => {
                                    self.input_key(me, true);
                                }
                            }
                        } else {
                            self.input_key(me, false);
                        }
                    }
                }
                Some(message::Union::Clipboard(_cb)) =>
                {
                    #[cfg(not(any(target_os = "android", target_os = "ios")))]
                    if self.clipboard {
                        update_clipboard(_cb, None);
                    }
                }
                Some(message::Union::Cliprdr(_clip)) => {
                    if self.file_transfer_enabled() {
                        #[cfg(windows)]
                        if let Some(clip) = msg_2_clip(_clip) {
                            self.send_to_cm(ipc::Data::ClipboardFile(clip))
                        }
                    }
                }
                Some(message::Union::FileAction(fa)) => {
                    if self.file_transfer.is_some() {
                        match fa.union {
                            Some(file_action::Union::ReadDir(rd)) => {
                                self.read_dir(&rd.path, rd.include_hidden);
                            }
                            Some(file_action::Union::AllFiles(f)) => {
                                match fs::get_recursive_files(&f.path, f.include_hidden) {
                                    Err(err) => {
                                        self.send(fs::new_error(f.id, err, -1)).await;
                                    }
                                    Ok(files) => {
                                        self.send(fs::new_dir(f.id, f.path, files)).await;
                                    }
                                }
                            }
                            Some(file_action::Union::Send(s)) => {
                                let id = s.id;
                                let od = can_enable_overwrite_detection(get_version_number(
                                    &self.lr.version,
                                ));
                                let path = s.path.clone();
                                match fs::TransferJob::new_read(
                                    id,
                                    "".to_string(),
                                    path.clone(),
                                    s.file_num,
                                    s.include_hidden,
                                    false,
                                    od,
                                ) {
                                    Err(err) => {
                                        self.send(fs::new_error(id, err, 0)).await;
                                    }
                                    Ok(job) => {
                                        self.send(fs::new_dir(id, path, job.files().to_vec()))
                                            .await;
                                        let mut files = job.files().to_owned();
                                        self.read_jobs.push(job);
                                        self.file_timer = time::interval(MILLI1);
                                        self.post_file_audit(
                                            FileAuditType::RemoteSend,
                                            &s.path,
                                            files
                                                .drain(..)
                                                .map(|f| (f.name, f.size as _))
                                                .collect(),
                                            json!({}),
                                        );
                                    }
                                }
                            }
                            Some(file_action::Union::Receive(r)) => {
                                // note: 1.1.10 introduced identical file detection, which breaks original logic of send/recv files
                                // whenever got send/recv request, check peer version to ensure old version of rustdesk
                                let od = can_enable_overwrite_detection(get_version_number(
                                    &self.lr.version,
                                ));
                                self.send_fs(ipc::FS::NewWrite {
                                    path: r.path.clone(),
                                    id: r.id,
                                    file_num: r.file_num,
                                    files: r
                                        .files
                                        .to_vec()
                                        .drain(..)
                                        .map(|f| (f.name, f.modified_time))
                                        .collect(),
                                    overwrite_detection: od,
                                });
                                self.post_file_audit(
                                    FileAuditType::RemoteReceive,
                                    &r.path,
                                    r.files
                                        .to_vec()
                                        .drain(..)
                                        .map(|f| (f.name, f.size as _))
                                        .collect(),
                                    json!({}),
                                );
                            }
                            Some(file_action::Union::RemoveDir(d)) => {
                                self.send_fs(ipc::FS::RemoveDir {
                                    path: d.path,
                                    id: d.id,
                                    recursive: d.recursive,
                                });
                            }
                            Some(file_action::Union::RemoveFile(f)) => {
                                self.send_fs(ipc::FS::RemoveFile {
                                    path: f.path,
                                    id: f.id,
                                    file_num: f.file_num,
                                });
                            }
                            Some(file_action::Union::Create(c)) => {
                                self.send_fs(ipc::FS::CreateDir {
                                    path: c.path,
                                    id: c.id,
                                });
                            }
                            Some(file_action::Union::Cancel(c)) => {
                                self.send_fs(ipc::FS::CancelWrite { id: c.id });
                                fs::remove_job(c.id, &mut self.read_jobs);
                            }
                            Some(file_action::Union::SendConfirm(r)) => {
                                if let Some(job) = fs::get_job(r.id, &mut self.read_jobs) {
                                    job.confirm(&r);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Some(message::Union::FileResponse(fr)) => match fr.union {
                    Some(file_response::Union::Block(block)) => {
                        self.send_fs(ipc::FS::WriteBlock {
                            id: block.id,
                            file_num: block.file_num,
                            data: block.data,
                            compressed: block.compressed,
                        });
                    }
                    Some(file_response::Union::Done(d)) => {
                        self.send_fs(ipc::FS::WriteDone {
                            id: d.id,
                            file_num: d.file_num,
                        });
                    }
                    Some(file_response::Union::Digest(d)) => self.send_fs(ipc::FS::CheckDigest {
                        id: d.id,
                        file_num: d.file_num,
                        file_size: d.file_size,
                        last_modified: d.last_modified,
                        is_upload: true,
                    }),
                    Some(file_response::Union::Error(e)) => {
                        self.send_fs(ipc::FS::WriteError {
                            id: e.id,
                            file_num: e.file_num,
                            err: e.error,
                        });
                    }
                    _ => {}
                },
                Some(message::Union::Misc(misc)) => match misc.union {
                    Some(misc::Union::SwitchDisplay(s)) => {
                        video_service::switch_display(s.display).await;
                    }
                    Some(misc::Union::ChatMessage(c)) => {
                        self.send_to_cm(ipc::Data::ChatMessage { text: c.text });
                        self.chat_unanswered = true;
                    }
                    Some(misc::Union::Option(o)) => {
                        self.update_options(&o).await;
                    }
                    Some(misc::Union::RefreshVideo(r)) => {
                        if r {
                            super::video_service::refresh();
                        }
                    }
                    Some(misc::Union::VideoReceived(_)) => {
                        video_service::notify_video_frame_fetched(
                            self.inner.id,
                            Some(Instant::now().into()),
                        );
                    }
                    Some(misc::Union::CloseReason(_)) => {
                        self.on_close("Peer close", true).await;
                        SESSIONS.lock().unwrap().remove(&self.lr.my_id);
                        return false;
                    }

                    Some(misc::Union::RestartRemoteDevice(_)) =>
                    {
                        #[cfg(not(any(target_os = "android", target_os = "ios")))]
                        if self.restart {
                            match system_shutdown::reboot() {
                                Ok(_) => log::info!("Restart by the peer"),
                                Err(e) => log::error!("Failed to restart:{}", e),
                            }
                        }
                    }
                    Some(misc::Union::ElevationRequest(r)) => match r.union {
                        Some(elevation_request::Union::Direct(_)) => {
                            #[cfg(windows)]
                            {
                                let mut err = "No need to elevate".to_string();
                                if !crate::platform::is_installed() && !portable_client::running() {
                                    err = portable_client::start_portable_service(
                                        portable_client::StartPara::Direct,
                                    )
                                    .err()
                                    .map_or("".to_string(), |e| e.to_string());
                                }
                                let mut misc = Misc::new();
                                misc.set_elevation_response(err);
                                let mut msg = Message::new();
                                msg.set_misc(misc);
                                self.send(msg).await;
                            }
                        }
                        Some(elevation_request::Union::Logon(_r)) => {
                            #[cfg(windows)]
                            {
                                let mut err = "No need to elevate".to_string();
                                if !crate::platform::is_installed() && !portable_client::running() {
                                    err = portable_client::start_portable_service(
                                        portable_client::StartPara::Logon(_r.username, _r.password),
                                    )
                                    .err()
                                    .map_or("".to_string(), |e| e.to_string());
                                }
                                let mut misc = Misc::new();
                                misc.set_elevation_response(err);
                                let mut msg = Message::new();
                                msg.set_misc(misc);
                                self.send(msg).await;
                            }
                        }
                        _ => {}
                    },
                    Some(misc::Union::AudioFormat(format)) => {
                        if !self.disable_audio {
                            // Drop the audio sender previously.
                            drop(std::mem::replace(&mut self.audio_sender, None));
                            self.audio_sender = Some(start_audio_thread());
                            allow_err!(self
                                .audio_sender
                                .as_ref()
                                .unwrap()
                                .send(MediaData::AudioFormat(format)));
                        }
                    }
                    #[cfg(feature = "flutter")]
                    Some(misc::Union::SwitchSidesRequest(s)) => {
                        if let Ok(uuid) = uuid::Uuid::from_slice(&s.uuid.to_vec()[..]) {
                            crate::run_me(vec![
                                "--connect",
                                &self.lr.my_id,
                                "--switch_uuid",
                                uuid.to_string().as_ref(),
                            ])
                            .ok();
                            self.on_close("switch sides", false).await;
                            return false;
                        }
                    }
                    #[cfg(not(any(target_os = "android", target_os = "ios")))]
                    Some(misc::Union::ChangeResolution(r)) => {
                        if self.keyboard {
                            if let Ok(name) = video_service::get_current_display_name() {
                                if let Ok(current) = crate::platform::current_resolution(&name) {
                                    if let Err(e) = crate::platform::change_resolution(
                                        &name,
                                        r.width as _,
                                        r.height as _,
                                    ) {
                                        log::error!("change resolution failed:{:?}", e);
                                    } else {
                                        if !self.origin_resolution.contains_key(&name) {
                                            self.origin_resolution.insert(name, current);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                },
                Some(message::Union::AudioFrame(frame)) => {
                    if !self.disable_audio {
                        if let Some(sender) = &self.audio_sender {
                            allow_err!(sender.send(MediaData::AudioFrame(Box::new(frame))));
                        } else {
                            log::warn!(
                                "Processing audio frame without the voice call audio sender."
                            );
                        }
                    }
                }
                Some(message::Union::VoiceCallRequest(request)) => {
                    if request.is_connect {
                        self.voice_call_request_timestamp = Some(
                            NonZeroI64::new(request.req_timestamp)
                                .unwrap_or(NonZeroI64::new(get_time()).unwrap()),
                        );
                        // Notify the connection manager.
                        self.send_to_cm(Data::VoiceCallIncoming);
                    } else {
                        self.close_voice_call().await;
                    }
                }
                Some(message::Union::VoiceCallResponse(_response)) => {
                    // TODO: Maybe we can do a voice call from cm directly.
                }
                _ => {}
            }
        }
        true
    }

    pub async fn handle_voice_call(&mut self, accepted: bool) {
        if let Some(ts) = self.voice_call_request_timestamp.take() {
            let msg = new_voice_call_response(ts.get(), accepted);
            if accepted {
                // Backup the default input device.
                let audio_input_device = Config::get_option("audio-input");
                log::debug!("Backup the sound input device {}", audio_input_device);
                self.audio_input_device_before_voice_call = Some(audio_input_device);
                // Switch to default input device
                let default_sound_device = get_default_sound_input();
                if let Some(device) = default_sound_device {
                    set_sound_input(device);
                }
                self.send_to_cm(Data::StartVoiceCall);
            } else {
                self.send_to_cm(Data::CloseVoiceCall("".to_owned()));
            }
            self.send(msg).await;
        } else {
            log::warn!("Possible a voice call attack.");
        }
    }

    pub async fn close_voice_call(&mut self) {
        // Restore to the prior audio device.
        if let Some(sound_input) =
            std::mem::replace(&mut self.audio_input_device_before_voice_call, None)
        {
            set_sound_input(sound_input);
        }
        // Notify the connection manager that the voice call has been closed.
        self.send_to_cm(Data::CloseVoiceCall("".to_owned()));
    }

    async fn update_options(&mut self, o: &OptionMessage) {
        log::info!("Option update: {:?}", o);
        if let Ok(q) = o.image_quality.enum_value() {
            let image_quality;
            if let ImageQuality::NotSet = q {
                if o.custom_image_quality > 0 {
                    image_quality = o.custom_image_quality;
                } else {
                    image_quality = -1;
                }
            } else {
                image_quality = q.value();
            }
            if image_quality > 0 {
                video_service::VIDEO_QOS
                    .lock()
                    .unwrap()
                    .update_image_quality(image_quality);
            }
        }
        if o.custom_fps > 0 {
            video_service::VIDEO_QOS
                .lock()
                .unwrap()
                .update_user_fps(o.custom_fps as _);
        }
        if let Some(q) = o.supported_decoding.clone().take() {
            scrap::codec::Encoder::update(self.inner.id(), scrap::codec::EncodingUpdate::New(q));
        }
        if let Ok(q) = o.lock_after_session_end.enum_value() {
            if q != BoolOption::NotSet {
                self.lock_after_session_end = q == BoolOption::Yes;
            }
        }
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        if let Ok(q) = o.show_remote_cursor.enum_value() {
            if q != BoolOption::NotSet {
                self.show_remote_cursor = q == BoolOption::Yes;
                if let Some(s) = self.server.upgrade() {
                    s.write().unwrap().subscribe(
                        NAME_CURSOR,
                        self.inner.clone(),
                        self.peer_keyboard_enabled() || self.show_remote_cursor,
                    );
                    s.write().unwrap().subscribe(
                        NAME_POS,
                        self.inner.clone(),
                        self.show_remote_cursor,
                    );
                }
            }
        }
        if let Ok(q) = o.disable_audio.enum_value() {
            if q != BoolOption::NotSet {
                self.disable_audio = q == BoolOption::Yes;
                if let Some(s) = self.server.upgrade() {
                    s.write().unwrap().subscribe(
                        super::audio_service::NAME,
                        self.inner.clone(),
                        self.audio_enabled(),
                    );
                }
            }
        }
        #[cfg(windows)]
        if let Ok(q) = o.enable_file_transfer.enum_value() {
            if q != BoolOption::NotSet {
                self.enable_file_transfer = q == BoolOption::Yes;
                self.send_to_cm(ipc::Data::ClipboardFileEnabled(
                    self.file_transfer_enabled(),
                ));
            }
        }
        if let Ok(q) = o.disable_clipboard.enum_value() {
            if q != BoolOption::NotSet {
                self.disable_clipboard = q == BoolOption::Yes;
                if let Some(s) = self.server.upgrade() {
                    s.write().unwrap().subscribe(
                        super::clipboard_service::NAME,
                        self.inner.clone(),
                        self.clipboard_enabled() && self.peer_keyboard_enabled(),
                    );
                }
            }
        }
        if let Ok(q) = o.disable_keyboard.enum_value() {
            if q != BoolOption::NotSet {
                self.disable_keyboard = q == BoolOption::Yes;
                if let Some(s) = self.server.upgrade() {
                    s.write().unwrap().subscribe(
                        super::clipboard_service::NAME,
                        self.inner.clone(),
                        self.clipboard_enabled() && self.peer_keyboard_enabled(),
                    );
                    s.write().unwrap().subscribe(
                        NAME_CURSOR,
                        self.inner.clone(),
                        self.peer_keyboard_enabled() || self.show_remote_cursor,
                    );
                }
            }
        }
        if let Ok(q) = o.privacy_mode.enum_value() {
            if self.keyboard {
                match q {
                    BoolOption::Yes => {
                        let msg_out = if !video_service::is_privacy_mode_supported() {
                            crate::common::make_privacy_mode_msg(
                                back_notification::PrivacyModeState::PrvNotSupported,
                            )
                        } else {
                            match privacy_mode::turn_on_privacy(self.inner.id) {
                                Ok(true) => {
                                    if video_service::test_create_capturer(self.inner.id, 5_000) {
                                        video_service::set_privacy_mode_conn_id(self.inner.id);
                                        crate::common::make_privacy_mode_msg(
                                            back_notification::PrivacyModeState::PrvOnSucceeded,
                                        )
                                    } else {
                                        log::error!(
                                            "Wait privacy mode timeout, turn off privacy mode"
                                        );
                                        video_service::set_privacy_mode_conn_id(0);
                                        let _ = privacy_mode::turn_off_privacy(self.inner.id);
                                        crate::common::make_privacy_mode_msg(
                                            back_notification::PrivacyModeState::PrvOnFailed,
                                        )
                                    }
                                }
                                Ok(false) => crate::common::make_privacy_mode_msg(
                                    back_notification::PrivacyModeState::PrvOnFailedPlugin,
                                ),
                                Err(e) => {
                                    log::error!("Failed to turn on privacy mode. {}", e);
                                    if video_service::get_privacy_mode_conn_id() == 0 {
                                        let _ = privacy_mode::turn_off_privacy(0);
                                    }
                                    crate::common::make_privacy_mode_msg(
                                        back_notification::PrivacyModeState::PrvOnFailed,
                                    )
                                }
                            }
                        };
                        self.send(msg_out).await;
                    }
                    BoolOption::No => {
                        let msg_out = if !video_service::is_privacy_mode_supported() {
                            crate::common::make_privacy_mode_msg(
                                back_notification::PrivacyModeState::PrvNotSupported,
                            )
                        } else {
                            video_service::set_privacy_mode_conn_id(0);
                            privacy_mode::turn_off_privacy(self.inner.id)
                        };
                        self.send(msg_out).await;
                    }
                    _ => {}
                }
            }
        }
        if self.keyboard {
            if let Ok(q) = o.block_input.enum_value() {
                match q {
                    BoolOption::Yes => {
                        self.tx_input.send(MessageInput::BlockOn).ok();
                    }
                    BoolOption::No => {
                        self.tx_input.send(MessageInput::BlockOff).ok();
                    }
                    _ => {}
                }
            }
        }
    }

    async fn on_close(&mut self, reason: &str, lock: bool) {
        log::info!("#{} Connection closed: {}", self.inner.id(), reason);
        if lock && self.lock_after_session_end && self.keyboard {
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            lock_screen().await;
        }
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        let data = if self.chat_unanswered {
            ipc::Data::Disconnected
        } else {
            ipc::Data::Close
        };
        #[cfg(any(target_os = "android", target_os = "ios"))]
        let data = ipc::Data::Close;
        self.tx_to_cm.send(data).ok();
        self.port_forward_socket.take();
    }

    // The `reason` should be consistent with `check_if_retry` if not empty
    async fn send_close_reason_no_retry(&mut self, reason: &str) {
        let mut misc = Misc::new();
        if reason.is_empty() {
            misc.set_close_reason("Closed manually by the peer".to_string());
        } else {
            misc.set_close_reason(reason.to_string());
        }
        let mut msg_out = Message::new();
        msg_out.set_misc(misc);
        self.send(msg_out).await;
        SESSIONS.lock().unwrap().remove(&self.lr.my_id);
    }

    fn read_dir(&mut self, dir: &str, include_hidden: bool) {
        let dir = dir.to_string();
        self.send_fs(ipc::FS::ReadDir {
            dir,
            include_hidden,
        });
    }

    #[inline]
    async fn send(&mut self, msg: Message) {
        allow_err!(self.stream.send(&msg).await);
    }

    pub fn alive_conns() -> Vec<i32> {
        ALIVE_CONNS.lock().unwrap().clone()
    }

    #[cfg(windows)]
    fn portable_check(&mut self) {
        if self.portable.is_installed
            || self.file_transfer.is_some()
            || self.port_forward_socket.is_some()
        {
            return;
        }
        let running = portable_client::running();
        let show_elevation = !running;
        self.send_to_cm(ipc::Data::DataPortableService(
            ipc::DataPortableService::CmShowElevation(show_elevation),
        ));
        if self.authorized {
            let p = &mut self.portable;
            if running != p.last_running {
                p.last_running = running;
                let mut misc = Misc::new();
                misc.set_portable_service_running(running);
                let mut msg = Message::new();
                msg.set_misc(misc);
                self.inner.send(msg.into());
            }
            let uac = crate::video_service::IS_UAC_RUNNING.lock().unwrap().clone();
            if p.last_uac != uac {
                p.last_uac = uac;
                if !uac || !running {
                    let mut misc = Misc::new();
                    misc.set_uac(uac);
                    let mut msg = Message::new();
                    msg.set_misc(misc);
                    self.inner.send(msg.into());
                }
            }
            let foreground_window_elevated = crate::video_service::IS_FOREGROUND_WINDOW_ELEVATED
                .lock()
                .unwrap()
                .clone();
            if p.last_foreground_window_elevated != foreground_window_elevated {
                p.last_foreground_window_elevated = foreground_window_elevated;
                if !foreground_window_elevated || !running {
                    let mut misc = Misc::new();
                    misc.set_foreground_window_elevated(foreground_window_elevated);
                    let mut msg = Message::new();
                    msg.set_misc(misc);
                    self.inner.send(msg.into());
                }
            }
        }
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    fn reset_resolution(&self) {
        self.origin_resolution
            .iter()
            .map(|(name, r)| {
                if let Err(e) =
                    crate::platform::change_resolution(&name, r.width as _, r.height as _)
                {
                    log::error!("change resolution failed:{:?}", e);
                }
            })
            .count();
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    fn release_pressed_modifiers(&mut self) {
        for modifier in self.pressed_modifiers.iter() {
            rdev::simulate(&rdev::EventType::KeyRelease(*modifier)).ok();
        }
        self.pressed_modifiers.clear();
    }
}

pub fn insert_switch_sides_uuid(id: String, uuid: uuid::Uuid) {
    SWITCH_SIDES_UUID
        .lock()
        .unwrap()
        .insert(id, (tokio::time::Instant::now(), uuid));
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
async fn start_ipc(
    mut rx_to_cm: mpsc::UnboundedReceiver<ipc::Data>,
    tx_from_cm: mpsc::UnboundedSender<ipc::Data>,
    mut _rx_desktop_ready: mpsc::Receiver<()>,
    tx_stream_ready: mpsc::Sender<()>,
) -> ResultType<()> {
    loop {
        if !crate::platform::is_prelogin() {
            break;
        }
        sleep(1.).await;
    }
    let mut stream = None;
    if let Ok(s) = crate::ipc::connect(1000, "_cm").await {
        stream = Some(s);
    } else {
        let mut args = vec!["--cm"];
        if password::hide_cm() {
            args.push("--hide");
        };

        #[cfg(target_os = "linux")]
        #[cfg(not(feature = "linux_headless"))]
        let user = None;
        #[cfg(all(target_os = "linux", feature = "linux_headless"))]
        #[cfg(any(feature = "flatpak", feature = "appimage"))]
        let user = None;
        #[cfg(all(target_os = "linux", feature = "linux_headless"))]
        #[cfg(not(any(feature = "flatpak", feature = "appimage")))]
        let mut user = None;
        // Cm run as user, wait until desktop session is ready.
        #[cfg(all(target_os = "linux", feature = "linux_headless"))]
        #[cfg(not(any(feature = "flatpak", feature = "appimage")))]
        if linux_desktop_manager::is_headless() {
            let mut username = linux_desktop_manager::get_username();
            loop {
                if !username.is_empty() {
                    break;
                }
                let _res = timeout(1_000, _rx_desktop_ready.recv()).await;
                username = linux_desktop_manager::get_username();
            }
            let uid = {
                let output = run_cmds(&format!("id -u {}", &username))?;
                let output = output.trim();
                if output.is_empty() || !output.parse::<i32>().is_ok() {
                    bail!("Invalid username {}", &username);
                }
                output.to_string()
            };
            user = Some((uid, username));
            args = vec!["--cm-no-ui"];
        }
        let run_done;
        if crate::platform::is_root() {
            let mut res = Ok(None);
            for _ in 0..10 {
                #[cfg(not(target_os = "linux"))]
                {
                    log::debug!("Start cm");
                    res = crate::platform::run_as_user(args.clone());
                }
                #[cfg(target_os = "linux")]
                {
                    log::debug!("Start cm");
                    res = crate::platform::run_as_user(args.clone(), user.clone());
                }
                if res.is_ok() {
                    break;
                }
                sleep(1.).await;
            }
            if let Some(task) = res? {
                super::CHILD_PROCESS.lock().unwrap().push(task);
            }
            run_done = true;
        } else {
            run_done = false;
        }
        if !run_done {
            log::debug!("Start cm");
            super::CHILD_PROCESS
                .lock()
                .unwrap()
                .push(crate::run_me(args)?);
        }
        for _ in 0..20 {
            sleep(0.3).await;
            if let Ok(s) = crate::ipc::connect(1000, "_cm").await {
                stream = Some(s);
                break;
            }
        }
        if stream.is_none() {
            bail!("Failed to connect to connection manager");
        }
    }

    let _res = tx_stream_ready.send(()).await;
    let mut stream = stream.unwrap();
    loop {
        tokio::select! {
            res = stream.next() => {
                match res {
                    Err(err) => {
                        return Err(err.into());
                    }
                    Ok(Some(data)) => {
                        match data {
                            ipc::Data::ClickTime(_)=> {
                                let ct = CLICK_TIME.load(Ordering::SeqCst);
                                let data = ipc::Data::ClickTime(ct);
                                stream.send(&data).await?;
                            }
                            _ => {
                                tx_from_cm.send(data)?;
                            }
                        }
                    }
                    _ => {}
                }
            }
            res = rx_to_cm.recv() => {
                match res {
                    Some(data) => {
                        if let Data::FS(ipc::FS::WriteBlock{id,
                            file_num,
                            data,
                            compressed}) = data {
                                stream.send(&Data::FS(ipc::FS::WriteBlock{id, file_num, data: Bytes::new(), compressed})).await?;
                                stream.send_raw(data).await?;
                        } else {
                            stream.send(&data).await?;
                        }
                    }
                    None => {
                        bail!("expected");
                    }
                }
            }
        }
    }
}

// in case screen is sleep and blank, here to activate it
fn try_activate_screen() {
    #[cfg(windows)]
    std::thread::spawn(|| {
        mouse_move_relative(-6, -6);
        std::thread::sleep(std::time::Duration::from_millis(30));
        mouse_move_relative(6, 6);
    });
}

mod privacy_mode {
    use super::*;

    pub(super) fn turn_off_privacy(_conn_id: i32) -> Message {
        #[cfg(windows)]
        {
            use crate::win_privacy::*;

            let res = turn_off_privacy(_conn_id, None);
            match res {
                Ok(_) => crate::common::make_privacy_mode_msg(
                    back_notification::PrivacyModeState::PrvOffSucceeded,
                ),
                Err(e) => {
                    log::error!("Failed to turn off privacy mode {}", e);
                    crate::common::make_privacy_mode_msg(
                        back_notification::PrivacyModeState::PrvOffFailed,
                    )
                }
            }
        }
        #[cfg(not(windows))]
        {
            crate::common::make_privacy_mode_msg(back_notification::PrivacyModeState::PrvOffFailed)
        }
    }

    pub(super) fn turn_on_privacy(_conn_id: i32) -> ResultType<bool> {
        #[cfg(windows)]
        {
            let plugin_exist = crate::win_privacy::turn_on_privacy(_conn_id)?;
            Ok(plugin_exist)
        }
        #[cfg(not(windows))]
        {
            Ok(true)
        }
    }
}

pub enum AlarmAuditType {
    IpWhitelist = 0,
    ManyWrongPassword = 1,
    FrequentAttempt = 2,
}

pub enum FileAuditType {
    RemoteSend = 0,
    RemoteReceive = 1,
}

#[cfg(windows)]
pub struct PortableState {
    pub last_uac: bool,
    pub last_foreground_window_elevated: bool,
    pub last_running: bool,
    pub is_installed: bool,
}

#[cfg(windows)]
impl Default for PortableState {
    fn default() -> Self {
        Self {
            is_installed: crate::platform::is_installed(),
            last_uac: Default::default(),
            last_foreground_window_elevated: Default::default(),
            last_running: Default::default(),
        }
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        self.release_pressed_modifiers();
    }
}
