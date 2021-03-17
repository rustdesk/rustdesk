use crate::client::*;
use hbb_common::{
    allow_err,
    compress::{compress as compress_func, decompress},
    config::{self, Config, PeerConfig},
    fs, log,
    message_proto::*,
    protobuf::Message as _,
    protobuf::ProtobufEnum,
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
}

#[derive(Clone, Default)]
pub struct Session {
    id: String,
    sender: Arc<RwLock<Option<mpsc::UnboundedSender<Data>>>>,
    lc: Arc<RwLock<LoginConfigHandler>>,
    events2ui: Arc<RwLock<VecDeque<String>>>,
    rgba: Arc<RwLock<Option<Vec<u8>>>>,
}

impl Session {
    pub fn start(id: &str) {
        Config::set_remote_id(id);
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
            Connection::start(session);
        });
    }

    fn send(data: Data) {
        if let Some(session) = SESSION.read().unwrap().as_ref() {
            if let Some(sender) = session.sender.read().unwrap().as_ref() {
                sender.send(data).ok();
            }
        }
    }

    pub fn rgba() -> Option<Vec<u8>> {
        if let Some(session) = SESSION.read().unwrap().as_ref() {
            session.rgba.write().unwrap().take()
        } else {
            None
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
                Connection::start(session);
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

    fn push_event(&self, name: &str, event: Vec<(&str, &str)>) {
        let mut h: HashMap<&str, &str> = event.iter().cloned().collect();
        assert!(h.get("name").is_none());
        h.insert("name", name);
        self.events2ui
            .write()
            .unwrap()
            .push_back(serde_json::ser::to_string(&h).unwrap_or("".to_owned()));
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
                session._input_key(k, true, true, false, false);
            }
        }
    }

    pub fn switch_display(display: i32) {
        if let Some(session) = SESSION.read().unwrap().as_ref() {
            let mut misc = Misc::new();
            misc.set_switch_display(SwitchDisplay {
                display,
                ..Default::default()
            });
            let mut msg_out = Message::new();
            msg_out.set_misc(misc);
            session.send_msg(msg_out);
        }
    }

    pub fn lock_screen() {
        if let Some(session) = SESSION.read().unwrap().as_ref() {
            let k = Key::ControlKey(ControlKey::LockScreen);
            session._input_key(k, false, false, false, false);
        }
    }

    pub fn input_key(name: &str, alt: bool, ctrl: bool, shift: bool, command: bool) {
        if let Some(session) = SESSION.read().unwrap().as_ref() {
            let chars: Vec<char> = name.chars().collect();
            if chars.len() == 1 {
                let key = Key::_Raw(chars[0] as _);
                session._input_key(key, alt, ctrl, shift, command);
            } else {
                if let Some(key) = KEY_MAP.get(name) {
                    session._input_key(key.clone(), alt, ctrl, shift, command);
                }
            }
        }
    }

    pub fn input_string(value: &str) {
        if let Some(session) = SESSION.read().unwrap().as_ref() {
            let support_string = session.lc.read().unwrap().support_press;
            if support_string {
                let mut key_event = KeyEvent::new();
                key_event.set_seq(value.to_owned());
                let mut msg_out = Message::new();
                msg_out.set_key_event(key_event);
                log::debug!("{:?}", msg_out);
                session.send_msg(msg_out);
            } else {
                value.chars().for_each(|x| {
                    session._input_key(Key::_Raw(x as _), false, false, false, false)
                });
            }
        }
    }

    fn _input_key(&self, key: Key, alt: bool, ctrl: bool, shift: bool, command: bool) {
        if self.lc.read().unwrap().support_press {
            self.key_down_or_up(3, key, alt, ctrl, shift, command);
        } else {
            self.key_down_or_up(1, key.clone(), alt, ctrl, shift, command);
            self.key_down_or_up(0, key, alt, ctrl, shift, command);
        }
    }

    // mask = buttons << 3 | type
    // type, 1: down, 2: up, 3: wheel
    // buttons, 1: left, 2: right, 4: middle
    pub fn send_mouse(
        mask: i32,
        x: i32,
        y: i32,
        alt: bool,
        ctrl: bool,
        shift: bool,
        command: bool,
    ) {
        let mut msg_out = Message::new();
        let mut mouse_event = MouseEvent {
            mask,
            x,
            y,
            ..Default::default()
        };
        if alt {
            mouse_event.modifiers.push(ControlKey::Alt.into());
        }
        if shift {
            mouse_event.modifiers.push(ControlKey::Shift.into());
        }
        if ctrl {
            mouse_event.modifiers.push(ControlKey::Control.into());
        }
        if command {
            mouse_event.modifiers.push(ControlKey::Meta.into());
        }
        msg_out.set_mouse_event(mouse_event);
        Self::send(Data::Message(msg_out));
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

#[async_trait]
impl Interface for Session {
    fn msgbox(&self, msgtype: &str, title: &str, text: &str) {
        self.push_event(
            "msgbox",
            vec![("type", msgtype), ("title", title), ("text", text)],
        );
    }

    fn handle_login_error(&mut self, err: &str) -> bool {
        self.lc.write().unwrap().handle_login_error(err, self)
    }

    fn handle_peer_info(&mut self, pi: PeerInfo) {
        if pi.displays.is_empty() {
            self.msgbox("error", "Remote Error", "No Display");
        }
        let username = self.lc.read().unwrap().get_username(&pi);
        let mut displays = Vec::new();
        for ref d in pi.displays.iter() {
            let mut h: HashMap<&str, i32> = Default::default();
            h.insert("x", d.x);
            h.insert("y", d.y);
            h.insert("width", d.width);
            h.insert("height", d.height);
            displays.push(h);
        }
        let displays = serde_json::ser::to_string(&displays).unwrap_or("".to_owned());
        let mut current = pi.current_display as usize;
        if current >= pi.displays.len() {
            current = 0;
        }
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
            ],
        );
        self.lc.write().unwrap().handle_peer_info(username, pi);
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

struct Connection {
    video_handler: VideoHandler,
    audio_handler: AudioHandler,
    session: Session,
}

impl Connection {
    #[tokio::main(basic_scheduler)]
    async fn start(session: Session) {
        let mut last_recv_time = Instant::now();
        let mut timer = time::interval(SEC30);
        let (sender, mut receiver) = mpsc::unbounded_channel::<Data>();
        *session.sender.write().unwrap() = Some(sender);
        let mut conn = Connection {
            video_handler: VideoHandler::new(),
            audio_handler: Default::default(),
            session: session.clone(),
        };
        match Client::start(&session.id).await {
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
                        _ = timer.tick() => {
                            if last_recv_time.elapsed() >= SEC30 {
                                session.msgbox("error", "Connection Error", "Timeout");
                                break;
                            }
                        }
                    }
                }
                log::debug!("Exit io_loop of id={}", session.id);
            }
            Err(err) => {
                session.msgbox("error", "Connection Error", &err.to_string());
            }
        }
    }

    async fn handle_msg_from_peer(&mut self, data: &[u8], peer: &mut Stream) -> bool {
        if let Ok(msg_in) = Message::parse_from_bytes(&data) {
            match msg_in.union {
                Some(message::Union::video_frame(vf)) => {
                    if let Some(video_frame::Union::vp9s(vp9s)) = &vf.union {
                        if let Ok(true) = self.video_handler.handle_vp9s(vp9s) {
                            *self.session.rgba.write().unwrap() =
                                Some(self.video_handler.rgb.clone());
                        }
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
                Some(message::Union::misc(misc)) => match misc.union {
                    Some(misc::Union::audio_format(f)) => {
                        self.audio_handler.handle_format(f);
                    }
                    Some(misc::Union::permission_info(p)) => {
                        log::info!("Change permission {:?} -> {}", p.permission, p.enabled);
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
                    self.audio_handler
                        .handle_frame(frame, !self.session.lc.read().unwrap().disable_audio);
                }
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
            _ => {}
        }
        true
    }
}
