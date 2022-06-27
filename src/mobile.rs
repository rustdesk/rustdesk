use crate::client::*;
use crate::common::{make_fd_to_json};
use flutter_rust_bridge::{StreamSink, ZeroCopyBuffer};
use hbb_common::{
    allow_err,
    compress::decompress,
    config::{Config, LocalConfig},
    fs, log,
    fs::{can_enable_overwrite_detection, new_send_confirm, DigestCheckResult, get_string, transform_windows_path},
    message_proto::*,
    protobuf::Message as _,
    rendezvous_proto::ConnType,
    tokio::{
        self,
        sync::mpsc,
        time::{self, Duration, Instant, Interval},
    },
    Stream,
};
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex, RwLock},
};

lazy_static::lazy_static! {
    static ref SESSION: Arc<RwLock<Option<Session>>> = Default::default();
    pub static ref EVENT_STREAM: RwLock<Option<StreamSink<String>>> = Default::default(); // rust to dart event channel
    pub static ref RGBA_STREAM: RwLock<Option<StreamSink<ZeroCopyBuffer<Vec<u8>>>>> = Default::default(); // rust to dart rgba (big u8 list) channel
}

#[derive(Clone, Default)]
pub struct Session {
    id: String,
    sender: Arc<RwLock<Option<mpsc::UnboundedSender<Data>>>>,
    lc: Arc<RwLock<LoginConfigHandler>>,
    events2ui: Arc<RwLock<VecDeque<String>>>,
}

impl Session {
    pub fn start(id: &str, is_file_transfer: bool) {
        LocalConfig::set_remote_id(id);
        Self::close();
        let mut session = Session::default();
        session
            .lc
            .write()
            .unwrap()
            .initialize(id.to_owned(), false, false);
        session.id = id.to_owned();
        *SESSION.write().unwrap() = Some(session.clone());
        std::thread::spawn(move || {
            Connection::start(session, is_file_transfer);
        });
    }

    pub fn get() -> Arc<RwLock<Option<Session>>> {
        SESSION.clone()
    }

    pub fn get_option(name: &str) -> String {
        if let Some(session) = SESSION.read().unwrap().as_ref() {
            if name == "remote_dir" {
                return session.lc.read().unwrap().get_remote_dir();
            }
            return session.lc.read().unwrap().get_option(name);
        }
        "".to_owned()
    }

    pub fn set_option(name: String, value: String) {
        if let Some(session) = SESSION.read().unwrap().as_ref() {
            let mut value = value;
            if name == "remote_dir" {
                value = session.lc.write().unwrap().get_all_remote_dir(value);
            }
            return session.lc.write().unwrap().set_option(name, value);
        }
    }

    pub fn input_os_password(pass: String, activate: bool) {
        if let Some(session) = SESSION.read().unwrap().as_ref() {
            input_os_password(pass, activate, session.clone());
        }
    }

    fn send(data: Data) {
        if let Some(session) = SESSION.read().unwrap().as_ref() {
            session.send(data);
        }
    }

    pub fn pop_event() -> Option<String> {
        if let Some(session) = SESSION.read().unwrap().as_ref() {
            session.events2ui.write().unwrap().pop_front()
        } else {
            None
        }
    }

    pub fn toggle_option(name: &str) {
        if let Some(session) = SESSION.read().unwrap().as_ref() {
            let msg = session.lc.write().unwrap().toggle_option(name.to_owned());
            if let Some(msg) = msg {
                session.send_msg(msg);
            }
        }
    }

    pub fn refresh() {
        Self::send(Data::Message(LoginConfigHandler::refresh()));
    }

    pub fn get_image_quality() -> String {
        if let Some(session) = SESSION.read().unwrap().as_ref() {
            session.lc.read().unwrap().image_quality.clone()
        } else {
            "".to_owned()
        }
    }

    pub fn set_image_quality(value: &str) {
        if let Some(session) = SESSION.read().unwrap().as_ref() {
            let msg = session
                .lc
                .write()
                .unwrap()
                .save_image_quality(value.to_owned());
            if let Some(msg) = msg {
                session.send_msg(msg);
            }
        }
    }

    pub fn get_toggle_option(name: &str) -> Option<bool> {
        if let Some(session) = SESSION.read().unwrap().as_ref() {
            Some(session.lc.write().unwrap().get_toggle_option(name))
        } else {
            None
        }
    }

    pub fn login(password: &str, remember: bool) {
        Session::send(Data::Login((password.to_owned(), remember)));
    }

    pub fn close() {
        Session::send(Data::Close);
        SESSION.write().unwrap().take();
    }

    pub fn reconnect() {
        if let Some(session) = SESSION.read().unwrap().as_ref() {
            if let Some(sender) = session.sender.read().unwrap().as_ref() {
                sender.send(Data::Close).ok();
            }
            let session = session.clone();
            std::thread::spawn(move || {
                Connection::start(session, false);
            });
        }
    }

    pub fn get_remember() -> bool {
        if let Some(session) = SESSION.read().unwrap().as_ref() {
            session.lc.read().unwrap().remember
        } else {
            false
        }
    }

    #[inline]
    pub fn send_msg(&self, msg: Message) {
        if let Some(sender) = self.sender.read().unwrap().as_ref() {
            sender.send(Data::Message(msg)).ok();
        }
    }

    pub fn send_chat(text: String) {
        let mut misc = Misc::new();
        misc.set_chat_message(ChatMessage {
            text,
            ..Default::default()
        });
        let mut msg_out = Message::new();
        msg_out.set_misc(misc);
        Self::send_msg_static(msg_out);
    }

    pub fn send_files(
        id: i32,
        path: String,
        to: String,
        file_num: i32,
        include_hidden: bool,
        is_remote: bool,
    ) {
        if let Some(session) = SESSION.write().unwrap().as_mut() {
            session.send_files(id, path, to, file_num, include_hidden, is_remote);
        }
    }

    pub fn set_confirm_override_file(
        id: i32,
        file_num: i32,
        need_override: bool,
        remember: bool,
        is_upload: bool,
    ) {
        if let Some(session) = SESSION.read().unwrap().as_ref() {
            if let Some(sender) = session.sender.read().unwrap().as_ref() {
                log::info!(
                    "confirm file transfer, job: {}, need_override: {}",
                    id,
                    need_override
                );
                sender
                    .send(Data::SetConfirmOverrideFile((
                        id,
                        file_num,
                        need_override,
                        remember,
                        is_upload,
                    )))
                    .ok();
            }
        }
    }

    #[inline]
    pub fn send_msg_static(msg: Message) {
        if let Some(session) = SESSION.read().unwrap().as_ref() {
            session.send_msg(msg);
        }
    }

    fn push_event(&self, name: &str, event: Vec<(&str, &str)>) {
        let mut h: HashMap<&str, &str> = event.iter().cloned().collect();
        assert!(h.get("name").is_none());
        h.insert("name", name);

        if let Some(s) = EVENT_STREAM.read().unwrap().as_ref() {
            s.add(serde_json::ser::to_string(&h).unwrap_or("".to_owned()));
        };
    }

    #[inline]
    fn peer_platform(&self) -> String {
        self.lc.read().unwrap().info.platform.clone()
    }

    pub fn ctrl_alt_del() {
        if let Some(session) = SESSION.read().unwrap().as_ref() {
            if session.peer_platform() == "Windows" {
                let k = Key::ControlKey(ControlKey::CtrlAltDel);
                session.key_down_or_up(1, k, false, false, false, false);
            } else {
                let k = Key::ControlKey(ControlKey::Delete);
                session.key_down_or_up(3, k, true, true, false, false);
            }
        }
    }

    pub fn switch_display(display: i32) {
        let mut misc = Misc::new();
        misc.set_switch_display(SwitchDisplay {
            display,
            ..Default::default()
        });
        let mut msg_out = Message::new();
        msg_out.set_misc(misc);
        Self::send_msg_static(msg_out);
    }

    pub fn lock_screen() {
        if let Some(session) = SESSION.read().unwrap().as_ref() {
            let k = Key::ControlKey(ControlKey::LockScreen);
            session.key_down_or_up(1, k, false, false, false, false);
        }
    }

    pub fn input_key(
        name: &str,
        down: bool,
        press: bool,
        alt: bool,
        ctrl: bool,
        shift: bool,
        command: bool,
    ) {
        if let Some(session) = SESSION.read().unwrap().as_ref() {
            let chars: Vec<char> = name.chars().collect();
            if chars.len() == 1 {
                let key = Key::_Raw(chars[0] as _);
                session._input_key(key, down, press, alt, ctrl, shift, command);
            } else {
                if let Some(key) = KEY_MAP.get(name) {
                    session._input_key(key.clone(), down, press, alt, ctrl, shift, command);
                }
            }
        }
    }

    pub fn input_string(value: &str) {
        let mut key_event = KeyEvent::new();
        key_event.set_seq(value.to_owned());
        let mut msg_out = Message::new();
        msg_out.set_key_event(key_event);
        Self::send_msg_static(msg_out);
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
        mask: i32,
        x: i32,
        y: i32,
        alt: bool,
        ctrl: bool,
        shift: bool,
        command: bool,
    ) {
        if let Some(session) = SESSION.read().unwrap().as_ref() {
            send_mouse(mask, x, y, alt, ctrl, shift, command, session);
        }
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
            _ => {}
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
}

impl FileManager for Session {}

#[async_trait]
impl Interface for Session {
    fn send(&self, data: Data) {
        if let Some(sender) = self.sender.read().unwrap().as_ref() {
            sender.send(data).ok();
        }
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
                self.msgbox("error", "Error", "No active console user logged on, please connect and logon first.");
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

    async fn handle_hash(&mut self, hash: Hash, peer: &mut Stream) {
        handle_hash(self.lc.clone(), hash, self, peer).await;
    }

    async fn handle_login_from_ui(&mut self, password: String, remember: bool, peer: &mut Stream) {
        handle_login_from_ui(self.lc.clone(), password, remember, peer).await;
    }

    async fn handle_test_delay(&mut self, t: TestDelay, peer: &mut Stream) {
        handle_test_delay(t, peer).await;
    }
}

const MILLI1: Duration = Duration::from_millis(1);

struct Connection {
    video_handler: VideoHandler,
    audio_handler: AudioHandler,
    session: Session,
    first_frame: bool,
    read_jobs: Vec<fs::TransferJob>,
    write_jobs: Vec<fs::TransferJob>,
    timer: Interval,
    last_update_jobs_status: (Instant, HashMap<i32, u64>),
}

impl Connection {
    #[tokio::main(flavor = "current_thread")]
    async fn start(session: Session, is_file_transfer: bool) {
        let mut last_recv_time = Instant::now();
        let (sender, mut receiver) = mpsc::unbounded_channel::<Data>();
        *session.sender.write().unwrap() = Some(sender);
        let conn_type = if is_file_transfer {
            session.lc.write().unwrap().is_file_transfer = true;
            ConnType::FILE_TRANSFER
        } else {
            ConnType::DEFAULT_CONN
        };
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
        };
        let key = Config::get_option("key");
        let token = Config::get_option("access_token");

        match Client::start(&session.id, &key, &token, conn_type).await {
            Ok((mut peer, direct)) => {
                session.push_event(
                    "connection_ready",
                    vec![
                        ("secure", &peer.is_secured().to_string()),
                        ("direct", &direct.to_string()),
                    ],
                );
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
                                        if !conn.handle_msg_from_peer(bytes, &mut peer).await {
                                            break
                                        }
                                    }
                                }
                            } else {
                                log::info!("Reset by the peer");
                                session.msgbox("error", "Connection Error", "Reset by the peer");
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
                                    log::debug!("Connection Error");
                                    break;
                                }
                                conn.update_jobs_status();
                            } else {
                                conn.timer = time::interval_at(Instant::now() + SEC30, SEC30);
                            }
                        }
                    }
                }
                log::debug!("Exit io_loop of id={}", session.id);
            }
            Err(err) => {
                crate::common::test_rendezvous_server();
                session.msgbox("error", "Connection Error", &err.to_string());
            }
        }
    }

    async fn handle_msg_from_peer(&mut self, data: &[u8], peer: &mut Stream) -> bool {
        if let Ok(msg_in) = Message::parse_from_bytes(&data) {
            match msg_in.union {
                Some(message::Union::video_frame(vf)) => {
                    if !self.first_frame {
                        self.first_frame = true;
                    }
                    if let (Ok(true), Some(s)) = (
                        self.video_handler.handle_frame(vf),
                        RGBA_STREAM.read().unwrap().as_ref(),
                    ) {
                        s.add(ZeroCopyBuffer(self.video_handler.rgb.clone()));
                    }
                }
                Some(message::Union::hash(hash)) => {
                    self.session.handle_hash(hash, peer).await;
                }
                Some(message::Union::login_response(lr)) => match lr.union {
                    Some(login_response::Union::error(err)) => {
                        if !self.session.handle_login_error(&err) {
                            return false;
                        }
                    }
                    Some(login_response::Union::peer_info(pi)) => {
                        self.session.handle_peer_info(pi);
                    }
                    _ => {}
                },
                Some(message::Union::clipboard(cb)) => {
                    if !self.session.lc.read().unwrap().disable_clipboard {
                        let content = if cb.compress {
                            decompress(&cb.content)
                        } else {
                            cb.content
                        };
                        if let Ok(content) = String::from_utf8(content) {
                            self.session
                                .push_event("clipboard", vec![("content", &content)]);
                        }
                    }
                }
                Some(message::Union::cursor_data(cd)) => {
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
                Some(message::Union::cursor_id(id)) => {
                    self.session
                        .push_event("cursor_id", vec![("id", &id.to_string())]);
                }
                Some(message::Union::cursor_position(cp)) => {
                    self.session.push_event(
                        "cursor_position",
                        vec![("x", &cp.x.to_string()), ("y", &cp.y.to_string())],
                    );
                }
                Some(message::Union::file_response(fr)) => match fr.union {
                    Some(file_response::Union::dir(fd)) => {
                        let mut entries = fd.entries.to_vec();
                        if self.session.peer_platform() == "Windows" {
                            fs::transform_windows_path(&mut entries);
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
                    Some(file_response::Union::block(block)) => {
                        if let Some(job) = fs::get_job(block.id, &mut self.write_jobs) {
                            if let Err(_err) = job.write(block, None).await {
                                // to-do: add "skip" for writing job
                            }
                            self.update_jobs_status();
                        }
                    }
                    Some(file_response::Union::done(d)) => {
                        if let Some(job) = fs::get_job(d.id, &mut self.write_jobs) {
                            job.modify_time();
                            fs::remove_job(d.id, &mut self.write_jobs);
                        }
                        self.handle_job_status(d.id, d.file_num, None);
                    }
                    Some(file_response::Union::error(e)) => {
                        self.handle_job_status(e.id, e.file_num, Some(e.error));
                    }
                    Some(file_response::Union::digest(digest)) => {
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
                                                file_transfer_send_confirm_request::Union::offset_blk(0)
                                            } else {
                                                file_transfer_send_confirm_request::Union::skip(
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
                                                    union: Some(file_transfer_send_confirm_request::Union::skip(true)),
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
                                                                file_transfer_send_confirm_request::Union::offset_blk(0)
                                                            } else {
                                                                file_transfer_send_confirm_request::Union::skip(true)
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
                                                        union: Some(file_transfer_send_confirm_request::Union::offset_blk(0)),
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
                },
                Some(message::Union::misc(misc)) => match misc.union {
                    Some(misc::Union::audio_format(f)) => {
                        self.audio_handler.handle_format(f); //
                    }
                    Some(misc::Union::chat_message(c)) => {
                        self.session
                            .push_event("chat_client_mode", vec![("text", &c.text)]);
                    }
                    Some(misc::Union::permission_info(p)) => {
                        log::info!("Change permission {:?} -> {}", p.permission, p.enabled);
                        use permission_info::Permission;
                        self.session.push_event(
                            "permission",
                            vec![(
                                match p.permission.enum_value_or_default() {
                                    Permission::Keyboard => "keyboard",
                                    Permission::Clipboard => "clipboard",
                                    Permission::Audio => "audio",
                                    _ => "",
                                },
                                &p.enabled.to_string(),
                            )],
                        );
                    }
                    Some(misc::Union::switch_display(s)) => {
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
                    Some(misc::Union::close_reason(c)) => {
                        self.session.msgbox("error", "Connection Error", &c);
                        return false;
                    }
                    _ => {}
                },
                Some(message::Union::test_delay(t)) => {
                    self.session.handle_test_delay(t, peer).await;
                }
                Some(message::Union::audio_frame(frame)) => {
                    if !self.session.lc.read().unwrap().disable_audio {
                        self.audio_handler.handle_frame(frame);
                    }
                }
                Some(message::Union::file_action(action)) => match action.union {
                    Some(file_action::Union::send_confirm(c)) => {
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

    async fn handle_msg_from_ui(&mut self, data: Data, peer: &mut Stream) -> bool {
        match data {
            Data::Close => {
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
                // in mobile, can_enable_override_detection is always true
                let od = true;
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
                        true,
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
                        true,
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
                            let files = job.files().clone();
                            self.read_jobs.push(job);
                            self.timer = time::interval(MILLI1);
                            allow_err!(peer.send(&fs::new_receive(id, to, file_num, files)).await);
                        }
                    }
                }
            }
            Data::RemoveDirAll((id, path, is_remote)) => {
                if is_remote {
                    let mut msg_out = Message::new();
                    let mut file_action = FileAction::new();
                    file_action.set_all_files(ReadAllFiles {
                        id,
                        path: path.clone(),
                        include_hidden: true,
                        ..Default::default()
                    });
                    msg_out.set_file_action(file_action);
                    allow_err!(peer.send(&msg_out).await);
                } else {
                    match fs::get_recursive_files(&path, true) {
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
                                Some(file_transfer_send_confirm_request::Union::offset_blk(0))
                            } else {
                                Some(file_transfer_send_confirm_request::Union::skip(true))
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
                                Some(file_transfer_send_confirm_request::Union::offset_blk(0))
                            } else {
                                Some(file_transfer_send_confirm_request::Union::skip(true))
                            },
                            ..Default::default()
                        });
                        msg.set_file_action(file_action);
                        self.session.send_msg(msg);
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
}

// Server Side
// TODO connection_manager need use struct and trait,impl default method
#[cfg(target_os = "android")]
pub mod connection_manager {
    use std::{
        collections::HashMap,
        iter::FromIterator,
        rc::{Rc, Weak},
        sync::{Mutex, RwLock},
    };

    use crate::ipc;
    use crate::ipc::Data;
    use crate::server::Connection as Conn;
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
            sync::mpsc::{UnboundedReceiver, UnboundedSender},
            task::spawn_blocking,
        },
    };
    use scrap::android::call_main_service_set_by_name;
    use serde_derive::Serialize;

    use super::EVENT_STREAM;

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
        #[serde(skip)]
        tx: UnboundedSender<Data>,
    }

    lazy_static::lazy_static! {
        static ref CLIENTS: RwLock<HashMap<i32,Client>> = Default::default();
        static ref WRITE_JOBS: Mutex<Vec<fs::TransferJob>> = Mutex::new(Vec::new());
    }

    pub fn start_channel(rx: UnboundedReceiver<Data>, tx: UnboundedSender<Data>) {
        std::thread::spawn(move || start_listen(rx, tx));
    }

    #[tokio::main(flavor = "current_thread")]
    async fn start_listen(mut rx: UnboundedReceiver<Data>, tx: UnboundedSender<Data>) {
        let mut current_id = 0;
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
                    file_transfer_enabled,
                }) => {
                    current_id = id;
                    let mut client = Client {
                        id,
                        authorized,
                        is_file_transfer,
                        name: name.clone(),
                        peer_id: peer_id.clone(),
                        keyboard,
                        clipboard,
                        audio,
                        tx: tx.clone(),
                    };
                    if authorized {
                        client.authorized = true;
                        let client_json = serde_json::to_string(&client).unwrap_or("".into());
                        // send to Android service,active notification no matter UI is shown or not.
                        if let Err(e) = call_main_service_set_by_name(
                            "on_client_authorized",
                            Some(&client_json),
                            None,
                        ) {
                            log::debug!("call_service_set_by_name fail,{}", e);
                        }
                        // send to UI,refresh widget
                        push_event("on_client_authorized", vec![("client", &client_json)]);
                    } else {
                        let client_json = serde_json::to_string(&client).unwrap_or("".into());
                        // send to Android service,active notification no matter UI is shown or not.
                        if let Err(e) = call_main_service_set_by_name(
                            "try_start_without_auth",
                            Some(&client_json),
                            None,
                        ) {
                            log::debug!("call_service_set_by_name fail,{}", e);
                        }
                        // send to UI,refresh widget
                        push_event("try_start_without_auth", vec![("client", &client_json)]);
                    }
                    CLIENTS.write().unwrap().insert(id, client);
                }
                Some(Data::ChatMessage { text }) => {
                    handle_chat(current_id, text);
                }
                Some(Data::FS(fs)) => {
                    handle_fs(fs, &tx).await;
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

    fn push_event(name: &str, event: Vec<(&str, &str)>) {
        let mut h: HashMap<&str, &str> = event.iter().cloned().collect();
        assert!(h.get("name").is_none());
        h.insert("name", name);

        if let Some(s) = EVENT_STREAM.read().unwrap().as_ref() {
            s.add(serde_json::ser::to_string(&h).unwrap_or("".to_owned()));
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
        if let Some(client) = CLIENTS.write().unwrap().get(&id) {
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
            .filter(|(k, v)| !v.is_file_transfer)
            .next()
            .is_none()
        {
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
        let mut clients = CLIENTS.read().unwrap();
        if let Some(client) = clients.get(&id) {
            allow_err!(client.tx.send(Data::ChatMessage { text }));
        }
    }

    // handle FS server
    async fn handle_fs(fs: ipc::FS, tx: &UnboundedSender<Data>) {
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
            } => {
                // in mobile, can_enable_override_detection is always true
                let od = true;
                WRITE_JOBS.lock().unwrap().push(fs::TransferJob::new_write(
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
                    true,
                ));
            }
            ipc::FS::CancelWrite { id } => {
                let write_jobs = &mut *WRITE_JOBS.lock().unwrap();
                if let Some(job) = fs::get_job(id, write_jobs) {
                    job.remove_download_file();
                    fs::remove_job(id, write_jobs);
                }
            }
            ipc::FS::WriteDone { id, file_num } => {
                let write_jobs = &mut *WRITE_JOBS.lock().unwrap();
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
                if let Some(job) = fs::get_job(id, &mut *WRITE_JOBS.lock().unwrap()) {
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
                if let Some(job) = fs::get_job(id, &mut *WRITE_JOBS.lock().unwrap()) {
                    let mut req = FileTransferSendConfirmRequest {
                        id,
                        file_num,
                        union: Some(file_transfer_send_confirm_request::Union::offset_blk(0)),
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
