use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex, RwLock,
    },
};

use flutter_rust_bridge::{StreamSink, ZeroCopyBuffer};

use hbb_common::{
    allow_err, bail,
    compress::decompress,
    config::{Config, LocalConfig, PeerConfig, TransferSerde},
    fs::{
        self, can_enable_overwrite_detection, get_job, get_string, new_send_confirm,
        transform_windows_path, DigestCheckResult,
    },
    log,
    message_proto::*,
    protobuf::Message as _,
    rendezvous_proto::ConnType,
    tokio::{
        self,
        sync::mpsc,
        time::{self, Duration, Instant, Interval},
    },
    ResultType, Stream,
};

use crate::common::{self, make_fd_to_json, CLIPBOARD_INTERVAL};

#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::common::{check_clipboard, update_clipboard, ClipboardContext};

use crate::{client::*, flutter_ffi::EventToUI, make_fd_flutter};

pub(super) const APP_TYPE_MAIN: &str = "main";
pub(super) const APP_TYPE_DESKTOP_REMOTE: &str = "remote";
pub(super) const APP_TYPE_DESKTOP_FILE_TRANSFER: &str = "file transfer";

const MILLI1: Duration = Duration::from_millis(1);

lazy_static::lazy_static! {
    pub static ref SESSIONS: RwLock<HashMap<String,Session>> = Default::default();
    pub static ref GLOBAL_EVENT_STREAM: RwLock<HashMap<String, StreamSink<String>>> = Default::default(); // rust to dart event channel
}

static SERVER_CLIPBOARD_ENABLED: AtomicBool = AtomicBool::new(true);
static SERVER_KEYBOARD_ENABLED: AtomicBool = AtomicBool::new(true);


#[derive(Clone)]
pub struct Session {
    id: String,
    sender: Arc<RwLock<Option<mpsc::UnboundedSender<Data>>>>, // UI to rust
    lc: Arc<RwLock<LoginConfigHandler>>,
    events2ui: Arc<RwLock<Option<StreamSink<EventToUI>>>>,
}

impl Session {
    /// Create a new remote session with the given id.
    ///
    /// # Arguments
    ///
    /// * `id` - The identifier of the remote session with prefix. Regex: [\w]*[\_]*[\d]+
    /// * `is_file_transfer` - If the session is used for file transfer.
    /// * `is_port_forward` - If the session is used for port forward.
    pub fn add(id: &str, is_file_transfer: bool, is_port_forward: bool) -> ResultType<()> {
        // TODO check same id
        let session_id = get_session_id(id.to_owned());
        LocalConfig::set_remote_id(&session_id);
        // TODO close
        // Self::close();
        let session = Session {
            id: session_id.clone(),
            sender: Default::default(),
            lc: Default::default(),
            events2ui: Arc::new(RwLock::new(None)),
        };
        session.lc.write().unwrap().initialize(
            session_id.clone(),
            is_file_transfer,
            is_port_forward,
        );
        SESSIONS
            .write()
            .unwrap()
            .insert(id.to_owned(), session.clone());
        Ok(())
    }

    /// Create a new remote session with the given id.
    ///
    /// # Arguments
    ///
    /// * `id` - The identifier of the remote session with prefix. Regex: [\w]*[\_]*[\d]+
    /// * `events2ui` - The events channel to ui.
    pub fn start(id: &str, events2ui: StreamSink<EventToUI>) -> ResultType<()> {
        if let Some(session) = SESSIONS.write().unwrap().get_mut(id) {
            *session.events2ui.write().unwrap() = Some(events2ui);
            let session = session.clone();
            std::thread::spawn(move || {
                let is_file_transfer = session.lc.read().unwrap().is_file_transfer;
                let is_port_forward = session.lc.read().unwrap().is_port_forward;
                Connection::start(session, is_file_transfer, is_port_forward);
            });
            Ok(())
        } else {
            bail!("No session with peer id {}", id)
        }
    }


    /// Get the option of the current session.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the option to get. Currently only `remote_dir` is supported.
    pub fn get_option(&self, name: &str) -> String {
        if name == "remote_dir" {
            return self.lc.read().unwrap().get_remote_dir();
        }
        self.lc.read().unwrap().get_option(name)
    }

    /// Set the option of the current session.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the option to set. Currently only `remote_dir` is supported.
    /// * `value` - The value of the option to set.
    pub fn set_option(&self, name: String, value: String) {
        let mut value = value;
        let mut lc = self.lc.write().unwrap();
        if name == "remote_dir" {
            value = lc.get_all_remote_dir(value);
        }
        lc.set_option(name, value);
    }

    /// Input the OS password.
    pub fn input_os_password(&self, pass: String, activate: bool) {
        input_os_password(pass, activate, self.clone());
    }

    pub fn restart_remote_device(&self) {
        let mut lc = self.lc.write().unwrap();
        lc.restarting_remote_device = true;
        let msg = lc.restart_remote_device();
        self.send_msg(msg);
    }

    /// Toggle an option.
    pub fn toggle_option(&self, name: &str) {
        let msg = self.lc.write().unwrap().toggle_option(name.to_owned());
        if let Some(msg) = msg {
            self.send_msg(msg);
        }
    }

    /// Send a refresh command.
    pub fn refresh(&self) {
        self.send(Data::Message(LoginConfigHandler::refresh()));
    }

    /// Get image quality.
    pub fn get_image_quality(&self) -> String {
        self.lc.read().unwrap().image_quality.clone()
    }

    /// Set image quality.
    pub fn set_image_quality(&self, value: &str) {
        let msg = self
            .lc
            .write()
            .unwrap()
            .save_image_quality(value.to_owned());
        if let Some(msg) = msg {
            self.send_msg(msg);
        }
    }

    /// Get the status of a toggle option.
    /// Return `None` if the option is not found.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the option to get.
    pub fn get_toggle_option(&self, name: &str) -> bool {
        self.lc.write().unwrap().get_toggle_option(name)
    }

    /// Login.
    ///
    /// # Arguments
    ///
    /// * `password` - The password to login.
    /// * `remember` - If the password should be remembered.
    pub fn login(&self, password: &str, remember: bool) {
        self.send(Data::Login((password.to_owned(), remember)));
    }

    /// Close the session.
    pub fn close(&self) {
        self.send(Data::Close);
    }

    /// Reconnect to the current session.
    pub fn reconnect(&self) {
        self.send(Data::Close);
        let session = self.clone();
        std::thread::spawn(move || {
            Connection::start(session, false, false);
        });
    }

    /// Get `remember` flag in [`LoginConfigHandler`].
    pub fn get_remember(&self) -> bool {
        self.lc.read().unwrap().remember
    }

    /// Send message over the current session.
    ///
    /// # Arguments
    ///
    /// * `msg` - The message to send.
    #[inline]
    pub fn send_msg(&self, msg: Message) {
        self.send(Data::Message(msg));
    }

    /// Send chat message over the current session.
    ///
    /// # Arguments
    ///
    /// * `text` - The message to send.
    pub fn send_chat(&self, text: String) {
        let mut misc = Misc::new();
        misc.set_chat_message(ChatMessage {
            text,
            ..Default::default()
        });
        let mut msg_out = Message::new();
        msg_out.set_misc(misc);
        self.send_msg(msg_out);
    }

    /// Push an event to the event queue.
    /// An event is stored as json in the event queue.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the event.
    /// * `event` - Fields of the event content.
    fn push_event(&self, name: &str, event: Vec<(&str, &str)>) {
        let mut h: HashMap<&str, &str> = event.iter().cloned().collect();
        assert!(h.get("name").is_none());
        h.insert("name", name);
        let out = serde_json::ser::to_string(&h).unwrap_or("".to_owned());
        if let Some(stream) = &*self.events2ui.read().unwrap() {
            stream.add(EventToUI::Event(out));
        }
    }

    /// Get platform of peer.
    #[inline]
    fn peer_platform(&self) -> String {
        self.lc.read().unwrap().info.platform.clone()
    }

    /// Quick method for sending a ctrl_alt_del command.
    pub fn ctrl_alt_del(&self) {
        if self.peer_platform() == "Windows" {
            let k = Key::ControlKey(ControlKey::CtrlAltDel);
            self.key_down_or_up(1, k, false, false, false, false);
        } else {
            let k = Key::ControlKey(ControlKey::Delete);
            self.key_down_or_up(3, k, true, true, false, false);
        }
    }

    /// Switch the display.
    ///
    /// # Arguments
    ///
    /// * `display` - The display to switch to.
    pub fn switch_display(&self, display: i32) {
        let mut misc = Misc::new();
        misc.set_switch_display(SwitchDisplay {
            display,
            ..Default::default()
        });
        let mut msg_out = Message::new();
        msg_out.set_misc(misc);
        self.send_msg(msg_out);
    }

    /// Send lock screen command.
    pub fn lock_screen(&self) {
        let k = Key::ControlKey(ControlKey::LockScreen);
        self.key_down_or_up(1, k, false, false, false, false);
    }

    /// Send key input command.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the key.
    /// * `down` - Whether the key is down or up.
    /// * `press` - If the key is simply being pressed(Down+Up).
    /// * `alt` - If the alt key is also pressed.
    /// * `ctrl` - If the ctrl key is also pressed.
    /// * `shift` - If the shift key is also pressed.
    /// * `command` - If the command key is also pressed.
    pub fn input_key(
        &self,
        name: &str,
        down: bool,
        press: bool,
        alt: bool,
        ctrl: bool,
        shift: bool,
        command: bool,
    ) {
        let chars: Vec<char> = name.chars().collect();
        if chars.len() == 1 {
            let key = Key::_Raw(chars[0] as _);
            self._input_key(key, down, press, alt, ctrl, shift, command);
        } else {
            if let Some(key) = KEY_MAP.get(name) {
                self._input_key(key.clone(), down, press, alt, ctrl, shift, command);
            }
        }
    }

    /// Input a string of text.
    /// String is parsed into individual key presses.
    ///
    /// # Arguments
    ///
    /// * `value` - The text to input. TODO &str -> String
    pub fn input_string(&self, value: &str) {
        let mut key_event = KeyEvent::new();
        key_event.set_seq(value.to_owned());
        let mut msg_out = Message::new();
        msg_out.set_key_event(key_event);
        self.send_msg(msg_out);
    }

    fn _input_key(
        &self,
        key: Key,
        down: bool,
        press: bool,
        alt: bool,
        ctrl: bool,
        shift: bool,
        command: bool,
    ) {
        let v = if press {
            3
        } else if down {
            1
        } else {
            0
        };
        self.key_down_or_up(v, key, alt, ctrl, shift, command);
    }

    pub fn send_mouse(
        &self,
        mask: i32,
        x: i32,
        y: i32,
        alt: bool,
        ctrl: bool,
        shift: bool,
        command: bool,
    ) {
        send_mouse(mask, x, y, alt, ctrl, shift, command, self);
    }

    fn key_down_or_up(
        &self,
        down_or_up: i32,
        key: Key,
        alt: bool,
        ctrl: bool,
        shift: bool,
        command: bool,
    ) {
        let mut down_or_up = down_or_up;
        let mut key_event = KeyEvent::new();
        match key {
            Key::Chr(chr) => {
                key_event.set_chr(chr);
            }
            Key::ControlKey(key) => {
                key_event.set_control_key(key.clone());
            }
            Key::_Raw(raw) => {
                if raw > 'z' as u32 || raw < 'a' as u32 {
                    key_event.set_unicode(raw);
                    if down_or_up == 0 {
                        // ignore up, avoiding trigger twice
                        return;
                    }
                    down_or_up = 1; // if press, turn into down for avoiding trigger twice on server side
                } else {
                    // to make ctrl+c works on windows
                    key_event.set_chr(raw);
                }
            }
        }
        if alt {
            key_event.modifiers.push(ControlKey::Alt.into());
        }
        if shift {
            key_event.modifiers.push(ControlKey::Shift.into());
        }
        if ctrl {
            key_event.modifiers.push(ControlKey::Control.into());
        }
        if command {
            key_event.modifiers.push(ControlKey::Meta.into());
        }
        if down_or_up == 1 {
            key_event.down = true;
        } else if down_or_up == 3 {
            key_event.press = true;
        }
        let mut msg_out = Message::new();
        msg_out.set_key_event(key_event);
        log::debug!("{:?}", msg_out);
        self.send_msg(msg_out);
    }

    pub fn load_config(&self) -> PeerConfig {
        load_config(&self.id)
    }

    pub fn save_config(&self, config: &PeerConfig) {
        config.store(&self.id);
    }

    pub fn get_platform(&self, is_remote: bool) -> String {
        if is_remote {
            self.lc.read().unwrap().info.platform.clone()
        } else {
            whoami::platform().to_string()
        }
    }

    pub fn load_last_jobs(&self) {
        let pc = self.load_config();
        if pc.transfer.write_jobs.is_empty() && pc.transfer.read_jobs.is_empty() {
            // no last jobs
            return;
        }
        let mut cnt = 1;
        for job_str in pc.transfer.read_jobs.iter() {
            if !job_str.is_empty() {
                self.push_event("load_last_job", vec![("value", job_str)]);
                cnt += 1;
                println!("restore read_job: {:?}", job_str);
            }
        }
        for job_str in pc.transfer.write_jobs.iter() {
            if !job_str.is_empty() {
                self.push_event("load_last_job", vec![("value", job_str)]);
                cnt += 1;
                println!("restore write_job: {:?}", job_str);
            }
        }
    }

    fn update_quality_status(&self, status: QualityStatus) {
        const NULL: String = String::new();
        self.push_event(
            "update_quality_status",
            vec![
                ("speed", &status.speed.map_or(NULL, |it| it)),
                ("fps", &status.fps.map_or(NULL, |it| it.to_string())),
                ("delay", &status.delay.map_or(NULL, |it| it.to_string())),
                (
                    "target_bitrate",
                    &status.target_bitrate.map_or(NULL, |it| it.to_string()),
                ),
                (
                    "codec_format",
                    &status.codec_format.map_or(NULL, |it| it.to_string()),
                ),
            ],
        );
    }

    pub fn remove_port_forward(&mut self, port: i32) {
        let mut config = self.load_config();
        config.port_forwards = config
            .port_forwards
            .drain(..)
            .filter(|x| x.0 != port)
            .collect();
        self.save_config(&config);
        self.send(Data::RemovePortForward(port));
    }

    pub fn add_port_forward(&mut self, port: i32, remote_host: String, remote_port: i32) {
        let mut config = self.load_config();
        if config
            .port_forwards
            .iter()
            .filter(|x| x.0 == port)
            .next()
            .is_some()
        {
            return;
        }
        let pf = (port, remote_host, remote_port);
        config.port_forwards.push(pf.clone());
        self.save_config(&config);
        self.send(Data::AddPortForward(pf));
    }

    fn on_error(&self, err: &str) {
        self.msgbox("error", "Error", err);
    }
}

impl FileManager for Session {}

#[async_trait]
impl Interface for Session {
    fn send(&self, data: Data) {
        if let Some(sender) = self.sender.read().unwrap().as_ref() {
            sender.send(data).ok();
        }
    }

    fn is_file_transfer(&self) -> bool {
        todo!()
    }

    fn is_port_forward(&self) -> bool {
        todo!()
    }

    fn is_rdp(&self) -> bool {
        todo!()
    }

    fn msgbox(&self, msgtype: &str, title: &str, text: &str) {
        let has_retry = if check_if_retry(msgtype, title, text) {
            "true"
        } else {
            ""
        };
        self.push_event(
            "msgbox",
            vec![
                ("type", msgtype),
                ("title", title),
                ("text", text),
                ("hasRetry", has_retry),
            ],
        );
    }

    fn handle_login_error(&mut self, err: &str) -> bool {
        self.lc.write().unwrap().handle_login_error(err, self)
    }

    fn handle_peer_info(&mut self, pi: PeerInfo) {
        let mut lc = self.lc.write().unwrap();
        let username = lc.get_username(&pi);
        let mut displays = Vec::new();
        let mut current = pi.current_display as usize;

        if lc.is_file_transfer {
            if pi.username.is_empty() {
                self.msgbox(
                    "error",
                    "Error",
                    "No active console user logged on, please connect and logon first.",
                );
                return;
            }
        } else {
            if pi.displays.is_empty() {
                self.msgbox("error", "Remote Error", "No Display");
            }
            for ref d in pi.displays.iter() {
                let mut h: HashMap<&str, i32> = Default::default();
                h.insert("x", d.x);
                h.insert("y", d.y);
                h.insert("width", d.width);
                h.insert("height", d.height);
                displays.push(h);
            }
            if current >= pi.displays.len() {
                current = 0;
            }
        }
        let displays = serde_json::ser::to_string(&displays).unwrap_or("".to_owned());
        self.push_event(
            "peer_info",
            vec![
                ("username", &username),
                ("hostname", &pi.hostname),
                ("platform", &pi.platform),
                ("sas_enabled", &pi.sas_enabled.to_string()),
                ("displays", &displays),
                ("version", &pi.version),
                ("current_display", &current.to_string()),
                ("is_file_transfer", &lc.is_file_transfer.to_string()),
            ],
        );
        lc.handle_peer_info(username, pi);
        let p = lc.should_auto_login();
        if !p.is_empty() {
            input_os_password(p, true, self.clone());
        }
    }

    fn set_force_relay(&mut self, direct: bool, received: bool) {
        let mut lc = self.lc.write().unwrap();
        lc.force_relay = false;
        if direct && !received {
            let errno = errno::errno().0;
            log::info!("errno is {}", errno);
            // TODO: check mac and ios
            if cfg!(windows) && errno == 10054 || !cfg!(windows) && errno == 104 {
                lc.force_relay = true;
                lc.set_option("force-always-relay".to_owned(), "Y".to_owned());
            }
        }
    }

    fn is_force_relay(&self) -> bool {
        self.lc.read().unwrap().force_relay
    }

    async fn handle_hash(&mut self, pass: &str, hash: Hash, peer: &mut Stream) {
        handle_hash(self.lc.clone(), pass, hash, self, peer).await;
    }

    async fn handle_login_from_ui(&mut self, password: String, remember: bool, peer: &mut Stream) {
        handle_login_from_ui(self.lc.clone(), password, remember, peer).await;
    }

    async fn handle_test_delay(&mut self, t: TestDelay, peer: &mut Stream) {
        if !t.from_client {
            self.update_quality_status(QualityStatus {
                delay: Some(t.last_delay as _),
                target_bitrate: Some(t.target_bitrate as _),
                ..Default::default()
            });
            handle_test_delay(t, peer).await;
        }
    }
}


struct Connection {
    video_handler: VideoHandler,
    audio_handler: AudioHandler,
    session: Session,
    first_frame: bool,
    read_jobs: Vec<fs::TransferJob>,
    write_jobs: Vec<fs::TransferJob>,
    timer: Interval,
    last_update_jobs_status: (Instant, HashMap<i32, u64>),
    data_count: Arc<AtomicUsize>,
    frame_count: Arc<AtomicUsize>,
    video_format: CodecFormat,
}

impl Connection {
    // TODO: Similar to remote::start_clipboard
    // merge the code
    fn start_clipboard(
        tx_protobuf: mpsc::UnboundedSender<Data>,
        lc: Arc<RwLock<LoginConfigHandler>>,
    ) -> Option<std::sync::mpsc::Sender<()>> {
        let (tx, rx) = std::sync::mpsc::channel();
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        match ClipboardContext::new() {
            Ok(mut ctx) => {
                let old_clipboard: Arc<Mutex<String>> = Default::default();
                // ignore clipboard update before service start
                check_clipboard(&mut ctx, Some(&old_clipboard));
                std::thread::spawn(move || loop {
                    std::thread::sleep(Duration::from_millis(CLIPBOARD_INTERVAL));
                    match rx.try_recv() {
                        Ok(_) | Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                            log::debug!("Exit clipboard service of client");
                            break;
                        }
                        _ => {}
                    }
                    if !SERVER_CLIPBOARD_ENABLED.load(Ordering::SeqCst)
                        || !SERVER_KEYBOARD_ENABLED.load(Ordering::SeqCst)
                        || lc.read().unwrap().disable_clipboard
                    {
                        continue;
                    }
                    if let Some(msg) = check_clipboard(&mut ctx, Some(&old_clipboard)) {
                        tx_protobuf.send(Data::Message(msg)).ok();
                    }
                });
            }
            Err(err) => {
                log::error!("Failed to start clipboard service of client: {}", err);
            }
        }
        Some(tx)
    }

    /// Create a new connection.
    ///
    /// # Arguments
    ///
    /// * `session` - The session to create a new connection for.
    /// * `is_file_transfer` - Whether the connection is for file transfer.
    /// * `is_port_forward` - Whether the connection is for port forward.
    #[tokio::main(flavor = "current_thread")]
    async fn start(session: Session, is_file_transfer: bool, is_port_forward: bool) {
        let mut last_recv_time = Instant::now();
        let (sender, mut receiver) = mpsc::unbounded_channel::<Data>();
        let mut stop_clipboard = None;
        if !is_file_transfer && !is_port_forward {
            stop_clipboard = Self::start_clipboard(sender.clone(), session.lc.clone());
        }
        *session.sender.write().unwrap() = Some(sender.clone());
        let conn_type = if is_file_transfer {
            session.lc.write().unwrap().is_file_transfer = true;
            ConnType::FILE_TRANSFER
        } else if is_port_forward {
            ConnType::PORT_FORWARD // TODO: RDP
        } else {
            ConnType::DEFAULT_CONN
        };
        let key = Config::get_option("key");
        let token = Config::get_option("access_token");

        // TODO rdp & cli args
        let is_rdp = false;
        let args: Vec<i32> = Vec::new();

        if is_port_forward {
            if is_rdp {
                // let port = handler
                //     .get_option("rdp_port".to_owned())
                //     .parse::<i32>()
                //     .unwrap_or(3389);
                // std::env::set_var(
                //     "rdp_username",
                //     handler.get_option("rdp_username".to_owned()),
                // );
                // std::env::set_var(
                //     "rdp_password",
                //     handler.get_option("rdp_password".to_owned()),
                // );
                // log::info!("Remote rdp port: {}", port);
                // start_one_port_forward(handler, 0, "".to_owned(), port, receiver, &key, &token).await;
            } else if args.len() == 0 {
                let pfs = session.lc.read().unwrap().port_forwards.clone();
                let mut queues = HashMap::<i32, mpsc::UnboundedSender<Data>>::new();
                for d in pfs {
                    sender.send(Data::AddPortForward(d)).ok();
                }
                loop {
                    match receiver.recv().await {
                        Some(Data::AddPortForward((port, remote_host, remote_port))) => {
                            if port <= 0 || remote_port <= 0 {
                                continue;
                            }
                            let (sender, receiver) = mpsc::unbounded_channel::<Data>();
                            queues.insert(port, sender);
                            let handler = session.clone();
                            let key = key.clone();
                            let token = token.clone();
                            tokio::spawn(async move {
                                start_one_port_forward(
                                    handler,
                                    port,
                                    remote_host,
                                    remote_port,
                                    receiver,
                                    &key,
                                    &token,
                                )
                                .await;
                            });
                        }
                        Some(Data::RemovePortForward(port)) => {
                            if let Some(s) = queues.remove(&port) {
                                s.send(Data::Close).ok();
                            }
                        }
                        Some(Data::Close) => {
                            break;
                        }
                        Some(d) => {
                            for (_, s) in queues.iter() {
                                s.send(d.clone()).ok();
                            }
                        }
                        _ => {}
                    }
                }
            } else {
                // let port = handler.args[0].parse::<i32>().unwrap_or(0);
                // if handler.args.len() != 3
                //     || handler.args[2].parse::<i32>().unwrap_or(0) <= 0
                //     || port <= 0
                // {
                //     handler.on_error("Invalid arguments, usage:<br><br> rustdesk --port-forward remote-id listen-port remote-host remote-port");
                // }
                // let remote_host = handler.args[1].clone();
                // let remote_port = handler.args[2].parse::<i32>().unwrap_or(0);
                // start_one_port_forward(
                //     handler,
                //     port,
                //     remote_host,
                //     remote_port,
                //     receiver,
                //     &key,
                //     &token,
                // )
                // .await;
            }
            return;
        }

        let latency_controller = LatencyController::new();
        let latency_controller_cl = latency_controller.clone();

        let mut conn = Connection {
            video_handler: VideoHandler::new(latency_controller),
            audio_handler: AudioHandler::new(latency_controller_cl),
            session: session.clone(),
            first_frame: false,
            read_jobs: Vec::new(),
            write_jobs: Vec::new(),
            timer: time::interval(SEC30),
            last_update_jobs_status: (Instant::now(), Default::default()),
            data_count: Arc::new(AtomicUsize::new(0)),
            frame_count: Arc::new(AtomicUsize::new(0)),
            video_format: CodecFormat::Unknown,
        };

        match Client::start(&session.id, &key, &token, conn_type, session.clone()).await {
            Ok((mut peer, direct)) => {
                SERVER_KEYBOARD_ENABLED.store(true, Ordering::SeqCst);
                SERVER_CLIPBOARD_ENABLED.store(true, Ordering::SeqCst);

                session.push_event(
                    "connection_ready",
                    vec![
                        ("secure", &peer.is_secured().to_string()),
                        ("direct", &direct.to_string()),
                    ],
                );

                let mut status_timer = time::interval(Duration::new(1, 0));

                loop {
                    tokio::select! {
                        res = peer.next() => {
                            if let Some(res) = res {
                                match res {
                                    Err(err) => {
                                        log::error!("Connection closed: {}", err);
                                        session.msgbox("error", "Connection Error", &err.to_string());
                                        break;
                                    }
                                    Ok(ref bytes) => {
                                        last_recv_time = Instant::now();
                                        conn.data_count.fetch_add(bytes.len(), Ordering::Relaxed);
                                        if !conn.handle_msg_from_peer(bytes, &mut peer).await {
                                            break
                                        }
                                    }
                                }
                            } else {
                                if session.lc.read().unwrap().restarting_remote_device {
                                    log::info!("Restart remote device");
                                    session.msgbox("restarting", "Restarting Remote Device", "remote_restarting_tip");
                                } else {
                                    log::info!("Reset by the peer");
                                    session.msgbox("error", "Connection Error", "Reset by the peer");
                                }
                                break;
                            }
                        }
                        d = receiver.recv() => {
                            if let Some(d) = d {
                                if !conn.handle_msg_from_ui(d, &mut peer).await {
                                    break;
                                }
                            }
                        }
                        _ = conn.timer.tick() => {
                            if last_recv_time.elapsed() >= SEC30 {
                                session.msgbox("error", "Connection Error", "Timeout");
                                break;
                            }
                            if !conn.read_jobs.is_empty() {
                                if let Err(err) = fs::handle_read_jobs(&mut conn.read_jobs, &mut peer).await {
                                    log::debug!("Connection Error: {}", err);
                                    break;
                                }
                                conn.update_jobs_status();
                            } else {
                                conn.timer = time::interval_at(Instant::now() + SEC30, SEC30);
                            }
                        }
                        _ = status_timer.tick() => {
                            let speed = conn.data_count.swap(0, Ordering::Relaxed);
                            let speed = format!("{:.2}kB/s", speed as f32 / 1024 as f32);
                            let fps = conn.frame_count.swap(0, Ordering::Relaxed) as _;
                            conn.session.update_quality_status(QualityStatus {
                                speed:Some(speed),
                                fps:Some(fps),
                                ..Default::default()
                            });
                        }
                    }
                }
                log::debug!("Exit io_loop of id={}", session.id);
            }
            Err(err) => {
                session.msgbox("error", "Connection Error", &err.to_string());
            }
        }

        if let Some(stop) = stop_clipboard {
            stop.send(()).ok();
        }
        SERVER_KEYBOARD_ENABLED.store(false, Ordering::SeqCst);
        SERVER_CLIPBOARD_ENABLED.store(false, Ordering::SeqCst);
    }

    /// Handle message from peer.
    /// Return false if the connection should be closed.
    ///
    /// The message is handled by [`Message`], see [`message::Union`] for possible types.
    async fn handle_msg_from_peer(&mut self, data: &[u8], peer: &mut Stream) -> bool {
        if let Ok(msg_in) = Message::parse_from_bytes(&data) {
            match msg_in.union {
                Some(message::Union::VideoFrame(vf)) => {
                    if !self.first_frame {
                        self.first_frame = true;
                        common::send_opts_after_login(&self.session.lc.read().unwrap(), peer).await;
                    }
                    let incomming_format = CodecFormat::from(&vf);
                    if self.video_format != incomming_format {
                        self.video_format = incomming_format.clone();
                        self.session.update_quality_status(QualityStatus {
                            codec_format: Some(incomming_format),
                            ..Default::default()
                        })
                    };
                    if let Ok(true) = self.video_handler.handle_frame(vf) {
                        if let Some(stream) = &*self.session.events2ui.read().unwrap() {
                            self.frame_count.fetch_add(1, Ordering::Relaxed);
                            stream.add(EventToUI::Rgba(ZeroCopyBuffer(
                                self.video_handler.rgb.clone(),
                            )));
                        }
                    }
                }
                Some(message::Union::Hash(hash)) => {
                    self.session.handle_hash("", hash, peer).await;
                }
                Some(message::Union::LoginResponse(lr)) => match lr.union {
                    Some(login_response::Union::Error(err)) => {
                        if !self.session.handle_login_error(&err) {
                            return false;
                        }
                    }
                    Some(login_response::Union::PeerInfo(pi)) => {
                        self.session.handle_peer_info(pi);
                    }
                    _ => {}
                },
                Some(message::Union::Clipboard(cb)) => {
                    if !self.session.lc.read().unwrap().disable_clipboard {
                        let content = if cb.compress {
                            decompress(&cb.content)
                        } else {
                            cb.content.into()
                        };
                        if let Ok(content) = String::from_utf8(content) {
                            self.session
                                .push_event("clipboard", vec![("content", &content)]);
                        }
                    }
                }
                Some(message::Union::CursorData(cd)) => {
                    let colors = hbb_common::compress::decompress(&cd.colors);
                    self.session.push_event(
                        "cursor_data",
                        vec![
                            ("id", &cd.id.to_string()),
                            ("hotx", &cd.hotx.to_string()),
                            ("hoty", &cd.hoty.to_string()),
                            ("width", &cd.width.to_string()),
                            ("height", &cd.height.to_string()),
                            (
                                "colors",
                                &serde_json::ser::to_string(&colors).unwrap_or("".to_owned()),
                            ),
                        ],
                    );
                }
                Some(message::Union::CursorId(id)) => {
                    self.session
                        .push_event("cursor_id", vec![("id", &id.to_string())]);
                }
                Some(message::Union::CursorPosition(cp)) => {
                    self.session.push_event(
                        "cursor_position",
                        vec![("x", &cp.x.to_string()), ("y", &cp.y.to_string())],
                    );
                }
                Some(message::Union::FileResponse(fr)) => {
                    match fr.union {
                        Some(file_response::Union::Dir(fd)) => {
                            let mut entries = fd.entries.to_vec();
                            if self.session.peer_platform() == "Windows" {
                                transform_windows_path(&mut entries);
                            }
                            let id = fd.id;
                            self.session.push_event(
                                "file_dir",
                                vec![("value", &make_fd_to_json(fd)), ("is_local", "false")],
                            );
                            if let Some(job) = fs::get_job(id, &mut self.write_jobs) {
                                job.set_files(entries);
                            }
                        }
                        Some(file_response::Union::Block(block)) => {
                            if let Some(job) = fs::get_job(block.id, &mut self.write_jobs) {
                                if let Err(_err) = job.write(block, None).await {
                                    // to-do: add "skip" for writing job
                                }
                                self.update_jobs_status();
                            }
                        }
                        Some(file_response::Union::Done(d)) => {
                            if let Some(job) = fs::get_job(d.id, &mut self.write_jobs) {
                                job.modify_time();
                                fs::remove_job(d.id, &mut self.write_jobs);
                            }
                            self.handle_job_status(d.id, d.file_num, None);
                        }
                        Some(file_response::Union::Error(e)) => {
                            self.handle_job_status(e.id, e.file_num, Some(e.error));
                        }
                        Some(file_response::Union::Digest(digest)) => {
                            if digest.is_upload {
                                if let Some(job) = fs::get_job(digest.id, &mut self.read_jobs) {
                                    if let Some(file) = job.files().get(digest.file_num as usize) {
                                        let read_path = get_string(&job.join(&file.name));
                                        let overwrite_strategy = job.default_overwrite_strategy();
                                        if let Some(overwrite) = overwrite_strategy {
                                            let req = FileTransferSendConfirmRequest {
                                                id: digest.id,
                                                file_num: digest.file_num,
                                                union: Some(if overwrite {
                                                    file_transfer_send_confirm_request::Union::OffsetBlk(0)
                                                } else {
                                                    file_transfer_send_confirm_request::Union::Skip(
                                                        true,
                                                    )
                                                }),
                                                ..Default::default()
                                            };
                                            job.confirm(&req);
                                            let msg = new_send_confirm(req);
                                            allow_err!(peer.send(&msg).await);
                                        } else {
                                            self.handle_override_file_confirm(
                                                digest.id,
                                                digest.file_num,
                                                read_path,
                                                true,
                                            );
                                        }
                                    }
                                }
                            } else {
                                if let Some(job) = fs::get_job(digest.id, &mut self.write_jobs) {
                                    if let Some(file) = job.files().get(digest.file_num as usize) {
                                        let write_path = get_string(&job.join(&file.name));
                                        let overwrite_strategy = job.default_overwrite_strategy();
                                        match fs::is_write_need_confirmation(&write_path, &digest) {
                                            Ok(res) => match res {
                                                DigestCheckResult::IsSame => {
                                                    let msg= new_send_confirm(FileTransferSendConfirmRequest {
                                                    id: digest.id,
                                                    file_num: digest.file_num,
                                                    union: Some(file_transfer_send_confirm_request::Union::Skip(true)),
                                                    ..Default::default()
                                                });
                                                    self.session.send_msg(msg);
                                                }
                                                DigestCheckResult::NeedConfirm(digest) => {
                                                    if let Some(overwrite) = overwrite_strategy {
                                                        let msg = new_send_confirm(
                                                            FileTransferSendConfirmRequest {
                                                                id: digest.id,
                                                                file_num: digest.file_num,
                                                                union: Some(if overwrite {
                                                                    file_transfer_send_confirm_request::Union::OffsetBlk(0)
                                                                } else {
                                                                    file_transfer_send_confirm_request::Union::Skip(true)
                                                                }),
                                                                ..Default::default()
                                                            },
                                                        );
                                                        self.session.send_msg(msg);
                                                    } else {
                                                        self.handle_override_file_confirm(
                                                            digest.id,
                                                            digest.file_num,
                                                            write_path.to_string(),
                                                            false,
                                                        );
                                                    }
                                                }
                                                DigestCheckResult::NoSuchFile => {
                                                    let msg = new_send_confirm(
                                                    FileTransferSendConfirmRequest {
                                                        id: digest.id,
                                                        file_num: digest.file_num,
                                                        union: Some(file_transfer_send_confirm_request::Union::OffsetBlk(0)),
                                                        ..Default::default()
                                                    },
                                                );
                                                    self.session.send_msg(msg);
                                                }
                                            },
                                            Err(err) => {
                                                println!("error recving digest: {}", err);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Some(message::Union::Misc(misc)) => match misc.union {
                    Some(misc::Union::AudioFormat(f)) => {
                        self.audio_handler.handle_format(f); //
                    }
                    Some(misc::Union::ChatMessage(c)) => {
                        self.session
                            .push_event("chat_client_mode", vec![("text", &c.text)]);
                    }
                    Some(misc::Union::PermissionInfo(p)) => {
                        log::info!("Change permission {:?} -> {}", p.permission, p.enabled);
                        use permission_info::Permission;
                        self.session.push_event(
                            "permission",
                            vec![(
                                match p.permission.enum_value_or_default() {
                                    Permission::Keyboard => "keyboard",
                                    Permission::Clipboard => "clipboard",
                                    Permission::Audio => "audio",
                                    Permission::Restart => "restart",
                                    _ => "",
                                },
                                &p.enabled.to_string(),
                            )],
                        );
                    }
                    Some(misc::Union::SwitchDisplay(s)) => {
                        self.video_handler.reset();
                        self.session.push_event(
                            "switch_display",
                            vec![
                                ("display", &s.display.to_string()),
                                ("x", &s.x.to_string()),
                                ("y", &s.y.to_string()),
                                ("width", &s.width.to_string()),
                                ("height", &s.height.to_string()),
                            ],
                        );
                    }
                    Some(misc::Union::CloseReason(c)) => {
                        self.session.msgbox("error", "Connection Error", &c);
                        return false;
                    }
                    Some(misc::Union::BackNotification(notification)) => {
                        if !self.handle_back_notification(notification).await {
                            return false;
                        }
                    }
                    _ => {}
                },
                Some(message::Union::TestDelay(t)) => {
                    self.session.handle_test_delay(t, peer).await;
                }
                Some(message::Union::AudioFrame(frame)) => {
                    if !self.session.lc.read().unwrap().disable_audio {
                        self.audio_handler.handle_frame(frame);
                    }
                }
                Some(message::Union::FileAction(action)) => match action.union {
                    Some(file_action::Union::SendConfirm(c)) => {
                        if let Some(job) = fs::get_job(c.id, &mut self.read_jobs) {
                            job.confirm(&c);
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        true
    }

    async fn handle_back_notification(&mut self, notification: BackNotification) -> bool {
        match notification.union {
            Some(back_notification::Union::BlockInputState(state)) => {
                self.handle_back_msg_block_input(
                    state.enum_value_or(back_notification::BlockInputState::BlkStateUnknown),
                )
                .await;
            }
            Some(back_notification::Union::PrivacyModeState(state)) => {
                if !self
                    .handle_back_msg_privacy_mode(
                        state.enum_value_or(back_notification::PrivacyModeState::PrvStateUnknown),
                    )
                    .await
                {
                    return false;
                }
            }
            _ => {}
        }
        true
    }

    #[inline(always)]
    fn update_block_input_state(&mut self, on: bool) {
        self.session.push_event(
            "update_block_input_state",
            [("input_state", if on { "on" } else { "off" })].into(),
        );
    }

    async fn handle_back_msg_block_input(&mut self, state: back_notification::BlockInputState) {
        match state {
            back_notification::BlockInputState::BlkOnSucceeded => {
                self.update_block_input_state(true);
            }
            back_notification::BlockInputState::BlkOnFailed => {
                self.session
                    .msgbox("custom-error", "Block user input", "Failed");
                self.update_block_input_state(false);
            }
            back_notification::BlockInputState::BlkOffSucceeded => {
                self.update_block_input_state(false);
            }
            back_notification::BlockInputState::BlkOffFailed => {
                self.session
                    .msgbox("custom-error", "Unblock user input", "Failed");
            }
            _ => {}
        }
    }

    #[inline(always)]
    fn update_privacy_mode(&mut self, on: bool) {
        let mut config = self.session.load_config();
        config.privacy_mode = on;
        self.session.save_config(&config);
        self.session.lc.write().unwrap().get_config().privacy_mode = on;
        self.session.push_event("update_privacy_mode", [].into());
    }

    async fn handle_back_msg_privacy_mode(
        &mut self,
        state: back_notification::PrivacyModeState,
    ) -> bool {
        match state {
            back_notification::PrivacyModeState::PrvOnByOther => {
                self.session.msgbox(
                    "error",
                    "Connecting...",
                    "Someone turns on privacy mode, exit",
                );
                return false;
            }
            back_notification::PrivacyModeState::PrvNotSupported => {
                self.session
                    .msgbox("custom-error", "Privacy mode", "Unsupported");
                self.update_privacy_mode(false);
            }
            back_notification::PrivacyModeState::PrvOnSucceeded => {
                self.session
                    .msgbox("custom-nocancel", "Privacy mode", "In privacy mode");
                self.update_privacy_mode(true);
            }
            back_notification::PrivacyModeState::PrvOnFailedDenied => {
                self.session
                    .msgbox("custom-error", "Privacy mode", "Peer denied");
                self.update_privacy_mode(false);
            }
            back_notification::PrivacyModeState::PrvOnFailedPlugin => {
                self.session
                    .msgbox("custom-error", "Privacy mode", "Please install plugins");
                self.update_privacy_mode(false);
            }
            back_notification::PrivacyModeState::PrvOnFailed => {
                self.session
                    .msgbox("custom-error", "Privacy mode", "Failed");
                self.update_privacy_mode(false);
            }
            back_notification::PrivacyModeState::PrvOffSucceeded => {
                self.session
                    .msgbox("custom-nocancel", "Privacy mode", "Out privacy mode");
                self.update_privacy_mode(false);
            }
            back_notification::PrivacyModeState::PrvOffByPeer => {
                self.session
                    .msgbox("custom-error", "Privacy mode", "Peer exit");
                self.update_privacy_mode(false);
            }
            back_notification::PrivacyModeState::PrvOffFailed => {
                self.session
                    .msgbox("custom-error", "Privacy mode", "Failed to turn off");
            }
            back_notification::PrivacyModeState::PrvOffUnknown => {
                self.session
                    .msgbox("custom-error", "Privacy mode", "Turned off");
                // log::error!("Privacy mode is turned off with unknown reason");
                self.update_privacy_mode(false);
            }
            _ => {}
        }
        true
    }

    async fn handle_msg_from_ui(&mut self, data: Data, peer: &mut Stream) -> bool {
        match data {
            Data::Close => {
                self.sync_jobs_status_to_local().await;
                return false;
            }
            Data::Login((password, remember)) => {
                self.session
                    .handle_login_from_ui(password, remember, peer)
                    .await;
            }
            Data::Message(msg) => {
                allow_err!(peer.send(&msg).await);
            }
            Data::SendFiles((id, path, to, file_num, include_hidden, is_remote)) => {
                let od = can_enable_overwrite_detection(self.session.lc.read().unwrap().version);
                if is_remote {
                    log::debug!("New job {}, write to {} from remote {}", id, to, path);
                    self.write_jobs.push(fs::TransferJob::new_write(
                        id,
                        path.clone(),
                        to,
                        file_num,
                        include_hidden,
                        is_remote,
                        Vec::new(),
                        od,
                    ));
                    allow_err!(
                        peer.send(&fs::new_send(id, path, file_num, include_hidden))
                            .await
                    );
                } else {
                    match fs::TransferJob::new_read(
                        id,
                        to.clone(),
                        path.clone(),
                        file_num,
                        include_hidden,
                        is_remote,
                        od,
                    ) {
                        Err(err) => {
                            self.handle_job_status(id, -1, Some(err.to_string()));
                        }
                        Ok(job) => {
                            log::debug!(
                                "New job {}, read {} to remote {}, {} files",
                                id,
                                path,
                                to,
                                job.files().len()
                            );
                            let m = make_fd_flutter(id, job.files(), true);
                            self.session
                                .push_event("update_folder_files", vec![("info", &m)]);
                            let files = job.files().clone();
                            self.read_jobs.push(job);
                            self.timer = time::interval(MILLI1);
                            allow_err!(peer.send(&fs::new_receive(id, to, file_num, files)).await);
                        }
                    }
                }
            }
            Data::RemoveDirAll((id, path, is_remote, include_hidden)) => {
                if is_remote {
                    let mut msg_out = Message::new();
                    let mut file_action = FileAction::new();
                    file_action.set_all_files(ReadAllFiles {
                        id,
                        path: path.clone(),
                        include_hidden,
                        ..Default::default()
                    });
                    msg_out.set_file_action(file_action);
                    allow_err!(peer.send(&msg_out).await);
                } else {
                    match fs::get_recursive_files(&path, include_hidden) {
                        Ok(entries) => {
                            let mut fd = FileDirectory::new();
                            fd.id = id;
                            fd.path = path;
                            fd.entries = entries;
                            self.session.push_event(
                                "file_dir",
                                vec![("value", &make_fd_to_json(fd)), ("is_local", "true")],
                            );
                        }
                        Err(err) => {
                            self.handle_job_status(id, -1, Some(err.to_string()));
                        }
                    }
                }
            }
            Data::CancelJob(id) => {
                let mut msg_out = Message::new();
                let mut file_action = FileAction::new();
                file_action.set_cancel(FileTransferCancel {
                    id: id,
                    ..Default::default()
                });
                msg_out.set_file_action(file_action);
                allow_err!(peer.send(&msg_out).await);
                if let Some(job) = fs::get_job(id, &mut self.write_jobs) {
                    job.remove_download_file();
                    fs::remove_job(id, &mut self.write_jobs);
                }
                fs::remove_job(id, &mut self.read_jobs);
            }
            Data::RemoveDir((id, path)) => {
                let mut msg_out = Message::new();
                let mut file_action = FileAction::new();
                file_action.set_remove_dir(FileRemoveDir {
                    id,
                    path,
                    recursive: true,
                    ..Default::default()
                });
                msg_out.set_file_action(file_action);
                allow_err!(peer.send(&msg_out).await);
            }
            Data::RemoveFile((id, path, file_num, is_remote)) => {
                if is_remote {
                    let mut msg_out = Message::new();
                    let mut file_action = FileAction::new();
                    file_action.set_remove_file(FileRemoveFile {
                        id,
                        path,
                        file_num,
                        ..Default::default()
                    });
                    msg_out.set_file_action(file_action);
                    allow_err!(peer.send(&msg_out).await);
                } else {
                    match fs::remove_file(&path) {
                        Err(err) => {
                            self.handle_job_status(id, file_num, Some(err.to_string()));
                        }
                        Ok(()) => {
                            self.handle_job_status(id, file_num, None);
                        }
                    }
                }
            }
            Data::CreateDir((id, path, is_remote)) => {
                if is_remote {
                    let mut msg_out = Message::new();
                    let mut file_action = FileAction::new();
                    file_action.set_create(FileDirCreate {
                        id,
                        path,
                        ..Default::default()
                    });
                    msg_out.set_file_action(file_action);
                    allow_err!(peer.send(&msg_out).await);
                } else {
                    match fs::create_dir(&path) {
                        Err(err) => {
                            self.handle_job_status(id, -1, Some(err.to_string()));
                        }
                        Ok(()) => {
                            self.handle_job_status(id, -1, None);
                        }
                    }
                }
            }
            Data::SetConfirmOverrideFile((id, file_num, need_override, remember, is_upload)) => {
                if is_upload {
                    if let Some(job) = fs::get_job(id, &mut self.read_jobs) {
                        if remember {
                            job.set_overwrite_strategy(Some(need_override));
                        }
                        job.confirm(&FileTransferSendConfirmRequest {
                            id,
                            file_num,
                            union: if need_override {
                                Some(file_transfer_send_confirm_request::Union::OffsetBlk(0))
                            } else {
                                Some(file_transfer_send_confirm_request::Union::Skip(true))
                            },
                            ..Default::default()
                        });
                    }
                } else {
                    if let Some(job) = fs::get_job(id, &mut self.write_jobs) {
                        if remember {
                            job.set_overwrite_strategy(Some(need_override));
                        }
                        let mut msg = Message::new();
                        let mut file_action = FileAction::new();
                        file_action.set_send_confirm(FileTransferSendConfirmRequest {
                            id,
                            file_num,
                            union: if need_override {
                                Some(file_transfer_send_confirm_request::Union::OffsetBlk(0))
                            } else {
                                Some(file_transfer_send_confirm_request::Union::Skip(true))
                            },
                            ..Default::default()
                        });
                        msg.set_file_action(file_action);
                        self.session.send_msg(msg);
                    }
                }
            }
            Data::AddJob((id, path, to, file_num, include_hidden, is_remote)) => {
                let od = can_enable_overwrite_detection(self.session.lc.read().unwrap().version);
                if is_remote {
                    log::debug!(
                        "new write waiting job {}, write to {} from remote {}",
                        id,
                        to,
                        path
                    );
                    let mut job = fs::TransferJob::new_write(
                        id,
                        path.clone(),
                        to,
                        file_num,
                        include_hidden,
                        is_remote,
                        Vec::new(),
                        od,
                    );
                    job.is_last_job = true;
                    self.write_jobs.push(job);
                } else {
                    match fs::TransferJob::new_read(
                        id,
                        to.clone(),
                        path.clone(),
                        file_num,
                        include_hidden,
                        is_remote,
                        od,
                    ) {
                        Err(err) => {
                            self.handle_job_status(id, -1, Some(err.to_string()));
                        }
                        Ok(mut job) => {
                            log::debug!(
                                "new read waiting job {}, read {} to remote {}, {} files",
                                id,
                                path,
                                to,
                                job.files().len()
                            );
                            let m = make_fd_flutter(job.id(), job.files(), true);
                            self.session
                                .push_event("update_folder_files", vec![("info", &m)]);
                            job.is_last_job = true;
                            self.read_jobs.push(job);
                            self.timer = time::interval(MILLI1);
                        }
                    }
                }
            }
            Data::ResumeJob((id, is_remote)) => {
                if is_remote {
                    if let Some(job) = get_job(id, &mut self.write_jobs) {
                        job.is_last_job = false;
                        allow_err!(
                            peer.send(&fs::new_send(
                                id,
                                job.remote.clone(),
                                job.file_num,
                                job.show_hidden
                            ))
                            .await
                        );
                    }
                } else {
                    if let Some(job) = get_job(id, &mut self.read_jobs) {
                        job.is_last_job = false;
                        allow_err!(
                            peer.send(&fs::new_receive(
                                id,
                                job.path.to_string_lossy().to_string(),
                                job.file_num,
                                job.files.clone()
                            ))
                            .await
                        );
                    }
                }
            }
            _ => {}
        }
        true
    }

    #[inline]
    fn update_job_status(
        job: &fs::TransferJob,
        elapsed: i32,
        last_update_jobs_status: &mut (Instant, HashMap<i32, u64>),
        session: &Session,
    ) {
        if elapsed <= 0 {
            return;
        }
        let transferred = job.transferred();
        let last_transferred = {
            if let Some(v) = last_update_jobs_status.1.get(&job.id()) {
                v.to_owned()
            } else {
                0
            }
        };
        last_update_jobs_status.1.insert(job.id(), transferred);
        let speed = (transferred - last_transferred) as f64 / (elapsed as f64 / 1000.);
        let file_num = job.file_num() - 1;
        session.push_event(
            "job_progress",
            vec![
                ("id", &job.id().to_string()),
                ("file_num", &file_num.to_string()),
                ("speed", &speed.to_string()),
                ("finished_size", &job.finished_size().to_string()),
            ],
        );
    }

    fn update_jobs_status(&mut self) {
        let elapsed = self.last_update_jobs_status.0.elapsed().as_millis() as i32;
        if elapsed >= 1000 {
            for job in self.read_jobs.iter() {
                Self::update_job_status(
                    job,
                    elapsed,
                    &mut self.last_update_jobs_status,
                    &self.session,
                );
            }
            for job in self.write_jobs.iter() {
                Self::update_job_status(
                    job,
                    elapsed,
                    &mut self.last_update_jobs_status,
                    &self.session,
                );
            }
            self.last_update_jobs_status.0 = Instant::now();
        }
    }

    fn handle_job_status(&mut self, id: i32, file_num: i32, err: Option<String>) {
        if let Some(err) = err {
            self.session
                .push_event("job_error", vec![("id", &id.to_string()), ("err", &err)]);
        } else {
            self.session.push_event(
                "job_done",
                vec![("id", &id.to_string()), ("file_num", &file_num.to_string())],
            );
        }
    }

    fn handle_override_file_confirm(
        &mut self,
        id: i32,
        file_num: i32,
        read_path: String,
        is_upload: bool,
    ) {
        self.session.push_event(
            "override_file_confirm",
            vec![
                ("id", &id.to_string()),
                ("file_num", &file_num.to_string()),
                ("read_path", &read_path),
                ("is_upload", &is_upload.to_string()),
            ],
        );
    }

    async fn sync_jobs_status_to_local(&mut self) -> bool {
        log::info!("sync transfer job status");
        let mut config: PeerConfig = self.session.load_config();
        let mut transfer_metas = TransferSerde::default();
        for job in self.read_jobs.iter() {
            let json_str = serde_json::to_string(&job.gen_meta()).unwrap();
            transfer_metas.read_jobs.push(json_str);
        }
        for job in self.write_jobs.iter() {
            let json_str = serde_json::to_string(&job.gen_meta()).unwrap();
            transfer_metas.write_jobs.push(json_str);
        }
        log::info!("meta: {:?}", transfer_metas);
        config.transfer = transfer_metas;
        self.session.save_config(&config);
        true
    }
}

// Server Side
// TODO connection_manager need use struct and trait,impl default method
#[cfg(not(any(target_os = "ios")))]
pub mod connection_manager {
    use std::{
        collections::HashMap,
        iter::FromIterator,
        sync::{
            atomic::{AtomicI64, Ordering},
            RwLock,
        },
    };

    use serde_derive::Serialize;

    use hbb_common::{
        allow_err,
        config::Config,
        fs::is_write_need_confirmation,
        fs::{self, get_string, new_send_confirm, DigestCheckResult},
        log,
        message_proto::*,
        protobuf::Message as _,
        tokio::{
            self,
            sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
            task::spawn_blocking,
        },
    };
    #[cfg(any(target_os = "android"))]
    use scrap::android::call_main_service_set_by_name;

    use crate::ipc::Data;
    use crate::ipc::{self, new_listener, Connection};

    use super::GLOBAL_EVENT_STREAM;

    #[derive(Debug, Serialize, Clone)]
    struct Client {
        id: i32,
        pub authorized: bool,
        is_file_transfer: bool,
        name: String,
        peer_id: String,
        keyboard: bool,
        clipboard: bool,
        audio: bool,
        file: bool,
        restart: bool,
        #[serde(skip)]
        tx: UnboundedSender<Data>,
    }

    lazy_static::lazy_static! {
        static ref CLIENTS: RwLock<HashMap<i32,Client>> = Default::default();
    }

    static CLICK_TIME: AtomicI64 = AtomicI64::new(0);

    // // TODO clipboard_file
    // enum ClipboardFileData {
    //     #[cfg(windows)]
    //     Clip((i32, ipc::ClipbaordFile)),
    //     Enable((i32, bool)),
    // }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub fn start_listen_ipc_thread() {
        std::thread::spawn(move || start_ipc());
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    #[tokio::main(flavor = "current_thread")]
    async fn start_ipc() {
        // TODO clipboard_file
        // let (tx_file, _rx_file) = mpsc::unbounded_channel::<ClipboardFileData>();
        // #[cfg(windows)]
        // let cm_clip = cm.clone();
        // #[cfg(windows)]
        // std::thread::spawn(move || start_clipboard_file(cm_clip, _rx_file));

        #[cfg(windows)]
        std::thread::spawn(move || {
            log::info!("try create privacy mode window");
            #[cfg(windows)]
            {
                if let Err(e) = crate::platform::windows::check_update_broker_process() {
                    log::warn!(
                        "Failed to check update broker process. Privacy mode may not work properly. {}",
                        e
                    );
                }
            }
            allow_err!(crate::ui::win_privacy::start());
        });

        match new_listener("_cm").await {
            Ok(mut incoming) => {
                while let Some(result) = incoming.next().await {
                    match result {
                        Ok(stream) => {
                            log::debug!("Got new connection");
                            let mut stream = Connection::new(stream);
                            // let tx_file = tx_file.clone();
                            tokio::spawn(async move {
                                // for tmp use, without real conn id
                                let conn_id_tmp = -1;
                                let mut conn_id: i32 = 0;
                                let (tx, mut rx) = mpsc::unbounded_channel::<Data>();
                                let mut write_jobs: Vec<fs::TransferJob> = Vec::new();
                                loop {
                                    tokio::select! {
                                        res = stream.next() => {
                                            match res {
                                                Err(err) => {
                                                    log::info!("cm ipc connection closed: {}", err);
                                                    break;
                                                }
                                                Ok(Some(data)) => {
                                                    match data {
                                                        Data::Login{id, is_file_transfer, port_forward, peer_id, name, authorized, keyboard, clipboard, audio, file, file_transfer_enabled, restart} => {
                                                            log::debug!("conn_id: {}", id);
                                                            conn_id = id;
                                                            // tx_file.send(ClipboardFileData::Enable((id, file_transfer_enabled))).ok();
                                                            on_login(id, is_file_transfer, port_forward, peer_id, name, authorized, keyboard, clipboard, audio, file, restart, tx.clone());
                                                        }
                                                        Data::Close => {
                                                            // tx_file.send(ClipboardFileData::Enable((conn_id, false))).ok();
                                                            log::info!("cm ipc connection closed from connection request");
                                                            break;
                                                        }
                                                        Data::PrivacyModeState((_, _)) => {
                                                            conn_id = conn_id_tmp;
                                                            allow_err!(tx.send(data));
                                                        }
                                                        Data::ClickTime(ms) => {
                                                            CLICK_TIME.store(ms, Ordering::SeqCst);
                                                        }
                                                        Data::ChatMessage { text } => {
                                                            handle_chat(conn_id, text);
                                                        }
                                                        Data::FS(fs) => {
                                                            handle_fs(fs, &mut write_jobs, &tx).await;
                                                        }
                                                        // TODO ClipbaordFile
                                                        // #[cfg(windows)]
                                                        // Data::ClipbaordFile(_clip) => {
                                                        //     tx_file
                                                        //         .send(ClipboardFileData::Clip((id, _clip)))
                                                        //         .ok();
                                                        // }
                                                        // #[cfg(windows)]
                                                        // Data::ClipboardFileEnabled(enabled) => {
                                                        //     tx_file
                                                        //         .send(ClipboardFileData::Enable((id, enabled)))
                                                        //         .ok();
                                                        // }
                                                        _ => {}
                                                    }
                                                }
                                                _ => {}
                                            }
                                        }
                                        Some(data) = rx.recv() => {
                                            if stream.send(&data).await.is_err() {
                                                break;
                                            }
                                        }
                                    }
                                }
                                if conn_id != conn_id_tmp {
                                    remove_connection(conn_id);
                                }
                            });
                        }
                        Err(err) => {
                            log::error!("Couldn't get cm client: {:?}", err);
                        }
                    }
                }
            }
            Err(err) => {
                log::error!("Failed to start cm ipc server: {}", err);
            }
        }
        // crate::platform::quit_gui();
        // TODO flutter quit_gui
    }

    #[cfg(target_os = "android")]
    pub fn start_channel(rx: UnboundedReceiver<Data>, tx: UnboundedSender<Data>) {
        std::thread::spawn(move || start_listen(rx, tx));
    }

    #[cfg(target_os = "android")]
    #[tokio::main(flavor = "current_thread")]
    async fn start_listen(mut rx: UnboundedReceiver<Data>, tx: UnboundedSender<Data>) {
        let mut current_id = 0;
        let mut write_jobs: Vec<fs::TransferJob> = Vec::new();
        loop {
            match rx.recv().await {
                Some(Data::Login {
                    id,
                    is_file_transfer,
                    port_forward,
                    peer_id,
                    name,
                    authorized,
                    keyboard,
                    clipboard,
                    audio,
                    file,
                    restart,
                    ..
                }) => {
                    current_id = id;
                    on_login(
                        id,
                        is_file_transfer,
                        port_forward,
                        peer_id,
                        name,
                        authorized,
                        keyboard,
                        clipboard,
                        audio,
                        file,
                        restart,
                        tx.clone(),
                    );
                }
                Some(Data::ChatMessage { text }) => {
                    handle_chat(current_id, text);
                }
                Some(Data::FS(fs)) => {
                    handle_fs(fs, &mut write_jobs, &tx).await;
                }
                Some(Data::Close) => {
                    break;
                }
                None => {
                    break;
                }
                _ => {}
            }
        }
        remove_connection(current_id);
    }

    fn on_login(
        id: i32,
        is_file_transfer: bool,
        _port_forward: String,
        peer_id: String,
        name: String,
        authorized: bool,
        keyboard: bool,
        clipboard: bool,
        audio: bool,
        file: bool,
        restart: bool,
        tx: mpsc::UnboundedSender<Data>,
    ) {
        let mut client = Client {
            id,
            authorized,
            is_file_transfer,
            name: name.clone(),
            peer_id: peer_id.clone(),
            keyboard,
            clipboard,
            audio,
            file,
            restart,
            tx,
        };
        if authorized {
            client.authorized = true;
            let client_json = serde_json::to_string(&client).unwrap_or("".into());
            // send to Android service, active notification no matter UI is shown or not.
            #[cfg(any(target_os = "android"))]
            if let Err(e) =
                call_main_service_set_by_name("on_client_authorized", Some(&client_json), None)
            {
                log::debug!("call_service_set_by_name fail,{}", e);
            }
            // send to UI, refresh widget
            push_event("on_client_authorized", vec![("client", &client_json)]);
        } else {
            let client_json = serde_json::to_string(&client).unwrap_or("".into());
            // send to Android service, active notification no matter UI is shown or not.
            #[cfg(any(target_os = "android"))]
            if let Err(e) =
                call_main_service_set_by_name("try_start_without_auth", Some(&client_json), None)
            {
                log::debug!("call_service_set_by_name fail,{}", e);
            }
            // send to UI, refresh widget
            push_event("try_start_without_auth", vec![("client", &client_json)]);
        }
        CLIENTS.write().unwrap().insert(id, client);
    }

    fn push_event(name: &str, event: Vec<(&str, &str)>) {
        let mut h: HashMap<&str, &str> = event.iter().cloned().collect();
        assert!(h.get("name").is_none());
        h.insert("name", name);

        if let Some(s) = GLOBAL_EVENT_STREAM
            .read()
            .unwrap()
            .get(super::APP_TYPE_MAIN)
        {
            s.add(serde_json::ser::to_string(&h).unwrap_or("".to_owned()));
        };
    }

    pub fn get_click_time() -> i64 {
        CLICK_TIME.load(Ordering::SeqCst)
    }

    pub fn check_click_time(id: i32) {
        if let Some(client) = CLIENTS.read().unwrap().get(&id) {
            allow_err!(client.tx.send(Data::ClickTime(0)));
        };
    }

    pub fn switch_permission(id: i32, name: String, enabled: bool) {
        if let Some(client) = CLIENTS.read().unwrap().get(&id) {
            allow_err!(client.tx.send(Data::SwitchPermission { name, enabled }));
        };
    }

    pub fn get_clients_state() -> String {
        let clients = CLIENTS.read().unwrap();
        let res = Vec::from_iter(clients.values().cloned());
        serde_json::to_string(&res).unwrap_or("".into())
    }

    pub fn get_clients_length() -> usize {
        let clients = CLIENTS.read().unwrap();
        clients.len()
    }

    pub fn close_conn(id: i32) {
        if let Some(client) = CLIENTS.read().unwrap().get(&id) {
            allow_err!(client.tx.send(Data::Close));
        };
    }

    pub fn on_login_res(id: i32, res: bool) {
        if let Some(client) = CLIENTS.write().unwrap().get_mut(&id) {
            if res {
                allow_err!(client.tx.send(Data::Authorize));
                client.authorized = true;
            } else {
                allow_err!(client.tx.send(Data::Close));
            }
        };
    }

    fn remove_connection(id: i32) {
        let mut clients = CLIENTS.write().unwrap();
        clients.remove(&id);

        if clients
            .iter()
            .filter(|(_k, v)| !v.is_file_transfer)
            .next()
            .is_none()
        {
            #[cfg(any(target_os = "android"))]
            if let Err(e) = call_main_service_set_by_name("stop_capture", None, None) {
                log::debug!("stop_capture err:{}", e);
            }
        }

        push_event("on_client_remove", vec![("id", &id.to_string())]);
    }

    // server mode handle chat from other peers
    fn handle_chat(id: i32, text: String) {
        push_event(
            "chat_server_mode",
            vec![("id", &id.to_string()), ("text", &text)],
        );
    }

    // server mode send chat to peer
    pub fn send_chat(id: i32, text: String) {
        let clients = CLIENTS.read().unwrap();
        if let Some(client) = clients.get(&id) {
            allow_err!(client.tx.send(Data::ChatMessage { text }));
        }
    }

    // handle FS server
    async fn handle_fs(
        fs: ipc::FS,
        write_jobs: &mut Vec<fs::TransferJob>,
        tx: &UnboundedSender<Data>,
    ) {
        match fs {
            ipc::FS::ReadDir {
                dir,
                include_hidden,
            } => {
                read_dir(&dir, include_hidden, tx).await;
            }
            ipc::FS::RemoveDir {
                path,
                id,
                recursive,
            } => {
                remove_dir(path, id, recursive, tx).await;
            }
            ipc::FS::RemoveFile { path, id, file_num } => {
                remove_file(path, id, file_num, tx).await;
            }
            ipc::FS::CreateDir { path, id } => {
                create_dir(path, id, tx).await;
            }
            ipc::FS::NewWrite {
                path,
                id,
                file_num,
                mut files,
                overwrite_detection,
            } => {
                write_jobs.push(fs::TransferJob::new_write(
                    id,
                    "".to_string(),
                    path,
                    file_num,
                    false,
                    false,
                    files
                        .drain(..)
                        .map(|f| FileEntry {
                            name: f.0,
                            modified_time: f.1,
                            ..Default::default()
                        })
                        .collect(),
                    overwrite_detection,
                ));
            }
            ipc::FS::CancelWrite { id } => {
                if let Some(job) = fs::get_job(id, write_jobs) {
                    job.remove_download_file();
                    fs::remove_job(id, write_jobs);
                }
            }
            ipc::FS::WriteDone { id, file_num } => {
                if let Some(job) = fs::get_job(id, write_jobs) {
                    job.modify_time();
                    send_raw(fs::new_done(id, file_num), tx);
                    fs::remove_job(id, write_jobs);
                }
            }
            ipc::FS::WriteBlock {
                id,
                file_num,
                data,
                compressed,
            } => {
                if let Some(job) = fs::get_job(id, write_jobs) {
                    if let Err(err) = job
                        .write(
                            FileTransferBlock {
                                id,
                                file_num,
                                data,
                                compressed,
                                ..Default::default()
                            },
                            None,
                        )
                        .await
                    {
                        send_raw(fs::new_error(id, err, file_num), &tx);
                    }
                }
            }
            ipc::FS::CheckDigest {
                id,
                file_num,
                file_size,
                last_modified,
                is_upload,
            } => {
                if let Some(job) = fs::get_job(id, write_jobs) {
                    let mut req = FileTransferSendConfirmRequest {
                        id,
                        file_num,
                        union: Some(file_transfer_send_confirm_request::Union::OffsetBlk(0)),
                        ..Default::default()
                    };
                    let digest = FileTransferDigest {
                        id,
                        file_num,
                        last_modified,
                        file_size,
                        ..Default::default()
                    };
                    if let Some(file) = job.files().get(file_num as usize) {
                        let path = get_string(&job.join(&file.name));
                        match is_write_need_confirmation(&path, &digest) {
                            Ok(digest_result) => {
                                match digest_result {
                                    DigestCheckResult::IsSame => {
                                        req.set_skip(true);
                                        let msg_out = new_send_confirm(req);
                                        send_raw(msg_out, &tx);
                                    }
                                    DigestCheckResult::NeedConfirm(mut digest) => {
                                        // upload to server, but server has the same file, request
                                        digest.is_upload = is_upload;
                                        let mut msg_out = Message::new();
                                        let mut fr = FileResponse::new();
                                        fr.set_digest(digest);
                                        msg_out.set_file_response(fr);
                                        send_raw(msg_out, &tx);
                                    }
                                    DigestCheckResult::NoSuchFile => {
                                        let msg_out = new_send_confirm(req);
                                        send_raw(msg_out, &tx);
                                    }
                                }
                            }
                            Err(err) => {
                                send_raw(fs::new_error(id, err, file_num), &tx);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    async fn read_dir(dir: &str, include_hidden: bool, tx: &UnboundedSender<Data>) {
        let path = {
            if dir.is_empty() {
                Config::get_home()
            } else {
                fs::get_path(dir)
            }
        };
        if let Ok(Ok(fd)) = spawn_blocking(move || fs::read_dir(&path, include_hidden)).await {
            let mut msg_out = Message::new();
            let mut file_response = FileResponse::new();
            file_response.set_dir(fd);
            msg_out.set_file_response(file_response);
            send_raw(msg_out, tx);
        }
    }

    async fn handle_result<F: std::fmt::Display, S: std::fmt::Display>(
        res: std::result::Result<std::result::Result<(), F>, S>,
        id: i32,
        file_num: i32,
        tx: &UnboundedSender<Data>,
    ) {
        match res {
            Err(err) => {
                send_raw(fs::new_error(id, err, file_num), tx);
            }
            Ok(Err(err)) => {
                send_raw(fs::new_error(id, err, file_num), tx);
            }
            Ok(Ok(())) => {
                send_raw(fs::new_done(id, file_num), tx);
            }
        }
    }

    async fn remove_file(path: String, id: i32, file_num: i32, tx: &UnboundedSender<Data>) {
        handle_result(
            spawn_blocking(move || fs::remove_file(&path)).await,
            id,
            file_num,
            tx,
        )
        .await;
    }

    async fn create_dir(path: String, id: i32, tx: &UnboundedSender<Data>) {
        handle_result(
            spawn_blocking(move || fs::create_dir(&path)).await,
            id,
            0,
            tx,
        )
        .await;
    }

    async fn remove_dir(path: String, id: i32, recursive: bool, tx: &UnboundedSender<Data>) {
        let path = fs::get_path(&path);
        handle_result(
            spawn_blocking(move || {
                if recursive {
                    fs::remove_all_empty_dir(&path)
                } else {
                    std::fs::remove_dir(&path).map_err(|err| err.into())
                }
            })
            .await,
            id,
            0,
            tx,
        )
        .await;
    }

    fn send_raw(msg: Message, tx: &UnboundedSender<Data>) {
        match msg.write_to_bytes() {
            Ok(bytes) => {
                allow_err!(tx.send(Data::RawMessage(bytes)));
            }
            err => allow_err!(err),
        }
    }
}

#[inline]
pub fn get_session_id(id: String) -> String {
    return if let Some(index) = id.find('_') {
        id[index + 1..].to_string()
    } else {
        id
    };
}

async fn start_one_port_forward(
    handler: Session,
    port: i32,
    remote_host: String,
    remote_port: i32,
    receiver: mpsc::UnboundedReceiver<Data>,
    key: &str,
    token: &str,
) {
    if let Err(err) = crate::port_forward::listen(
        handler.id.clone(),
        String::new(), // TODO
        port,
        handler.clone(),
        receiver,
        key,
        token,
        handler.lc.clone(),
        remote_host,
        remote_port,
    )
    .await
    {
        handler.on_error(&format!("Failed to listen on {}: {}", port, err));
    }
    log::info!("port forward (:{}) exit", port);
}
