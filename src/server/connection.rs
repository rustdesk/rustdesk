use super::input_service::*;
use super::*;
use crate::common::update_clipboard;
use crate::ipc;
use hbb_common::{
    config::Config,
    fs,
    futures::{SinkExt, StreamExt},
    sleep, timeout,
    tokio::{
        net::TcpStream,
        sync::mpsc,
        time::{self, Duration, Instant, Interval},
    },
    tokio_util::codec::{BytesCodec, Framed},
};
use sha2::{Digest, Sha256};

pub type Sender = mpsc::UnboundedSender<(Instant, Arc<Message>)>;

lazy_static::lazy_static! {
    static ref LOGIN_FAILURES: Arc::<Mutex<HashMap<String, (i32, i32, i32)>>> = Default::default();
}

#[derive(Clone, Default)]
pub struct ConnInner {
    id: i32,
    tx: Option<Sender>,
}

pub struct Connection {
    inner: ConnInner,
    stream: super::Stream,
    server: super::ServerPtrWeak,
    hash: Hash,
    read_jobs: Vec<fs::TransferJob>,
    timer: Interval,
    file_transfer: Option<(String, bool)>,
    port_forward_socket: Option<Framed<TcpStream, BytesCodec>>,
    port_forward_address: String,
    tx_to_cm: mpsc::UnboundedSender<ipc::Data>,
    authorized: bool,
    keyboard: bool,
    clipboard: bool,
    audio: bool,
    last_test_delay: i64,
    image_quality: i32,
    lock_after_session_end: bool,
    show_remote_cursor: bool, // by peer
    privacy_mode: bool,
    ip: String,
    disable_clipboard: bool, // by peer
    disable_audio: bool,     // by peer
}

impl Subscriber for ConnInner {
    #[inline]
    fn id(&self) -> i32 {
        self.id
    }

    #[inline]
    fn send(&mut self, msg: Arc<Message>) {
        self.tx.as_mut().map(|tx| {
            allow_err!(tx.send((Instant::now(), msg)));
        });
    }
}

const TEST_DELAY_TIMEOUT: Duration = Duration::from_secs(3);
const SEC30: Duration = Duration::from_secs(30);
const H1: Duration = Duration::from_secs(3600);
const MILLI1: Duration = Duration::from_millis(1);

impl Connection {
    pub async fn start(
        addr: SocketAddr,
        stream: super::Stream,
        id: i32,
        server: super::ServerPtrWeak,
    ) {
        let hash = Hash {
            salt: Config::get_salt(),
            challenge: Config::get_auto_password(),
            ..Default::default()
        };
        let (tx_from_cm, mut rx_from_cm) = mpsc::unbounded_channel::<ipc::Data>();
        let (tx_to_cm, rx_to_cm) = mpsc::unbounded_channel::<ipc::Data>();
        let (tx, mut rx) = mpsc::unbounded_channel::<(Instant, Arc<Message>)>();
        let mut conn = Self {
            inner: ConnInner { id, tx: Some(tx) },
            stream,
            server,
            hash,
            read_jobs: Vec::new(),
            timer: time::interval(SEC30),
            file_transfer: None,
            port_forward_socket: None,
            port_forward_address: "".to_owned(),
            tx_to_cm,
            authorized: false,
            keyboard: Config::get_option("enable-keyboard").is_empty(),
            clipboard: Config::get_option("enable-clipboard").is_empty(),
            audio: Config::get_option("audio-input") != "Mute",
            last_test_delay: 0,
            image_quality: ImageQuality::Balanced.value(),
            lock_after_session_end: false,
            show_remote_cursor: false,
            privacy_mode: false,
            ip: "".to_owned(),
            disable_audio: false,
            disable_clipboard: false,
        };
        tokio::spawn(async move {
            if let Err(err) = start_ipc(rx_to_cm, tx_from_cm).await {
                log::error!("ipc to connection manager exit: {}", err);
            }
        });
        if !conn.on_open(addr).await {
            return;
        }
        if !conn.keyboard {
            conn.send_permisssion(Permission::Keyboard, false).await;
        }
        if !conn.clipboard {
            conn.send_permisssion(Permission::Clipboard, false).await;
        }
        if !conn.audio {
            conn.send_permisssion(Permission::Audio, false).await;
        }
        let mut test_delay_timer =
            time::interval_at(Instant::now() + TEST_DELAY_TIMEOUT, TEST_DELAY_TIMEOUT);
        let mut last_recv_time = Instant::now();
        loop {
            tokio::select! {
                Some(data) = rx_from_cm.recv() => {
                    match data {
                        ipc::Data::Authorize => {
                            conn.send_logon_response().await;
                            if conn.port_forward_socket.is_some() {
                                break;
                            }
                        }
                        ipc::Data::Close => {
                            let mut misc = Misc::new();
                            misc.set_close_reason("Closed manually by the peer".into());
                            let mut msg_out = Message::new();
                            msg_out.set_misc(misc);
                            conn.send(msg_out).await;
                            conn.on_close("Close requested from connection manager", false);
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
                        }
                        ipc::Data::SwitchPermission{name, enabled} => {
                            log::info!("Change permission {} -> {}", name, enabled);
                            if &name == "keyboard" {
                                conn.keyboard = enabled;
                                conn.send_permisssion(Permission::Keyboard, enabled).await;
                                if let Some(s) = conn.server.upgrade() {
                                    s.write().unwrap().subscribe(
                                        NAME_CURSOR,
                                        conn.inner.clone(), enabled || conn.show_remote_cursor);
                                }
                            } else if &name == "clipboard" {
                                conn.clipboard = enabled;
                                conn.send_permisssion(Permission::Clipboard, enabled).await;
                                if let Some(s) = conn.server.upgrade() {
                                    s.write().unwrap().subscribe(
                                        super::clipboard_service::NAME,
                                        conn.inner.clone(), conn.clipboard_enabled() && conn.keyboard);
                                }
                            } else if &name == "audio" {
                                conn.audio = enabled;
                                conn.send_permisssion(Permission::Audio, enabled).await;
                                if let Some(s) = conn.server.upgrade() {
                                    s.write().unwrap().subscribe(
                                        super::audio_service::NAME,
                                        conn.inner.clone(), conn.audio_enabled());
                                }
                            }
                        }
                        ipc::Data::RawMessage(bytes) => {
                            allow_err!(conn.stream.send_raw(bytes).await);
                        }
                        _ => {}
                    }
                }
                Some((instant, value)) = rx.recv() => {
                    let latency = instant.elapsed().as_millis() as i64;
                    super::video_service::update_internal_latency(id, latency);
                    let msg: &Message = &value;
                    if latency > 1000 {
                        match &msg.union {
                            Some(message::Union::video_frame(_)) => {
                                continue;
                            }
                            Some(message::Union::audio_frame(_)) => {
                                continue;
                            }
                            _ => {}
                        }
                    }
                    if let Err(err) = conn.stream.send(msg).await {
                        conn.on_close(&err.to_string(), false);
                        break;
                    }
                },
                res = conn.stream.next() => {
                    if let Some(res) = res {
                        match res {
                            Err(err) => {
                                conn.on_close(&err.to_string(), true);
                                break;
                            },
                            Ok(bytes) => {
                                last_recv_time = Instant::now();
                                if let Ok(msg_in) = Message::parse_from_bytes(&bytes) {
                                    if !conn.on_message(msg_in).await {
                                        break;
                                    }
                                }
                            }
                        }
                    } else {
                        conn.on_close("Reset by the peer", true);
                        break;
                    }
                },
                _ = conn.timer.tick() => {
                    if !conn.read_jobs.is_empty() {
                        if let Err(err) = fs::handle_read_jobs(&mut conn.read_jobs, &mut conn.stream).await {
                            conn.on_close(&err.to_string(), false);
                            break;
                        }
                    } else {
                        conn.timer = time::interval_at(Instant::now() + SEC30, SEC30);
                    }
                }
                _ = test_delay_timer.tick() => {
                    if last_recv_time.elapsed() >= SEC30 {
                        conn.on_close("Timeout", true);
                        break;
                    }
                    let time = crate::get_time();
                    if time > 0 && conn.last_test_delay == 0 {
                        conn.last_test_delay = time;
                        let mut msg_out = Message::new();
                        msg_out.set_test_delay(TestDelay{
                            time,
                            ..Default::default()
                        });
                        conn.inner.send(msg_out.into());
                    }
                }
            }
        }
        super::video_service::update_internal_latency(id, 0);
        super::video_service::update_test_latency(id, 0);
        super::video_service::update_image_quality(id, None);
        if let Some(forward) = conn.port_forward_socket.as_mut() {
            log::info!("Running port forwarding loop");
            conn.stream.set_raw();
            loop {
                tokio::select! {
                    Some(data) = rx_from_cm.recv() => {
                        match data {
                            ipc::Data::Close => {
                                conn.on_close("Close requested from connection manager", false);
                                break;
                            }
                            _ => {}
                        }
                    }
                    res = forward.next() => {
                        if let Some(res) = res {
                            match res {
                                Err(err) => {
                                    conn.on_close(&err.to_string(), false);
                                    break;
                                },
                                Ok(bytes) => {
                                    last_recv_time = Instant::now();
                                    if let Err(err) = conn.stream.send_bytes(bytes.into()).await {
                                        conn.on_close(&err.to_string(), false);
                                        break;
                                    }
                                }
                            }
                        } else {
                            conn.on_close("Forward reset by the peer", false);
                            break;
                        }
                    },
                    res = conn.stream.next() => {
                        if let Some(res) = res {
                            match res {
                                Err(err) => {
                                    conn.on_close(&err.to_string(), false);
                                    break;
                                },
                                Ok(bytes) => {
                                    last_recv_time = Instant::now();
                                    if let Err(err) = forward.send(bytes.into()).await {
                                        conn.on_close(&err.to_string(), false);
                                        break;
                                    }
                                }
                            }
                        } else {
                            conn.on_close("Stream reset by the peer", false);
                            break;
                        }
                    },
                    _ = conn.timer.tick() => {
                        if last_recv_time.elapsed() >= H1 {
                            conn.on_close("Timeout", false);
                            break;
                        }
                    }
                }
            }
        }
    }

    async fn send_permisssion(&mut self, permission: Permission, enabled: bool) {
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
                .filter(|x| x.parse() == Ok(addr.ip()))
                .next()
                .is_none()
        {
            self.send_login_error("Your ip is blocked by the peer")
                .await;
            sleep(1.).await;
            return false;
        }
        self.ip = addr.ip().to_string();
        let mut msg_out = Message::new();
        msg_out.set_hash(self.hash.clone());
        self.send(msg_out).await;
        true
    }

    async fn send_logon_response(&mut self) {
        if self.authorized {
            return;
        }
        #[allow(unused_mut)]
        let mut username = crate::platform::get_active_username();
        let mut res = LoginResponse::new();
        if self.port_forward_socket.is_some() {
            let mut msg_out = Message::new();
            res.set_peer_info(PeerInfo {
                hostname: whoami::hostname(),
                username,
                platform: whoami::platform().to_string(),
                version: crate::VERSION.to_owned(),
                ..Default::default()
            });
            msg_out.set_login_response(res);
            self.send(msg_out).await;
            return;
        }
        #[cfg(target_os = "linux")]
        if !self.file_transfer.is_some() {
            let dtype = crate::platform::linux::get_display_server();
            if dtype != "x11" {
                res.set_error(format!(
                    "Unsupported display server type {}, x11 expected",
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
        if self.file_transfer.is_some() {
            if crate::platform::is_prelogin() || self.tx_to_cm.send(ipc::Data::Test).is_err() {
                username = "".to_owned();
            }
        }
        self.authorized = true;
        let mut pi = PeerInfo {
            hostname: whoami::hostname(),
            username,
            platform: whoami::platform().to_string(),
            version: crate::VERSION.to_owned(),
            sas_enabled,
            ..Default::default()
        };
        let mut sub_service = false;
        if self.file_transfer.is_some() {
            res.set_peer_info(pi);
        } else {
            try_activate_screen();
            match super::video_service::get_displays() {
                Err(err) => {
                    res.set_error(format!("X11 error: {}", err));
                }
                Ok((current, displays)) => {
                    pi.displays = displays.into();
                    pi.current_display = current as _;
                    res.set_peer_info(pi);
                    sub_service = true;
                }
            }
        }
        let mut msg_out = Message::new();
        msg_out.set_login_response(res);
        self.send(msg_out).await;
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
                if !self.keyboard && !self.show_remote_cursor {
                    noperms.push(NAME_CURSOR);
                }
                if !self.show_remote_cursor {
                    noperms.push(NAME_POS);
                }
                if !self.clipboard_enabled() || !self.keyboard {
                    noperms.push(super::clipboard_service::NAME);
                }
                if !self.audio_enabled() {
                    noperms.push(super::audio_service::NAME);
                }
                s.write()
                    .unwrap()
                    .add_connection(self.inner.clone(), &noperms);
            }
        }
    }

    fn clipboard_enabled(&self) -> bool {
        self.clipboard && !self.disable_clipboard
    }

    fn audio_enabled(&self) -> bool {
        self.audio && !self.disable_audio
    }

    async fn try_start_cm(&mut self, peer_id: String, name: String, authorized: bool) {
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

    async fn on_message(&mut self, msg: Message) -> bool {
        if let Some(message::Union::login_request(lr)) = msg.union {
            if let Some(o) = lr.option.as_ref() {
                self.update_option(o);
            }
            if self.authorized {
                return true;
            }
            match lr.union {
                Some(login_request::Union::file_transfer(ft)) => {
                    if !Config::get_option("enable-file-transfer").is_empty() {
                        self.send_login_error("No permission of file transfer")
                            .await;
                        sleep(1.).await;
                        return false;
                    }
                    self.file_transfer = Some((ft.dir, ft.show_hidden));
                }
                Some(login_request::Union::port_forward(mut pf)) => {
                    if !Config::get_option("enable-tunnel").is_empty() {
                        self.send_login_error("No permission of IP tunneling").await;
                        sleep(1.).await;
                        return false;
                    }
                    let mut is_rdp = false;
                    if pf.host == "RDP" && pf.port == 0 {
                        pf.host = "localhost".to_owned();
                        pf.port = 3389;
                        is_rdp = true;
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
                        }
                    }
                }
                _ => {}
            }
            if lr.username != Config::get_id() {
                self.send_login_error("Offline").await;
            } else if lr.password.is_empty() {
                self.try_start_cm(lr.my_id, lr.my_name, false).await;
            } else {
                let mut hasher = Sha256::new();
                hasher.update(&Config::get_password());
                hasher.update(&self.hash.salt);
                let mut hasher2 = Sha256::new();
                hasher2.update(&hasher.finalize()[..]);
                hasher2.update(&self.hash.challenge);
                let mut failure = LOGIN_FAILURES
                    .lock()
                    .unwrap()
                    .get(&self.ip)
                    .map(|x| x.clone())
                    .unwrap_or((0, 0, 0));
                let time = (crate::get_time() / 60_000) as i32;
                if failure.2 > 30 {
                    self.send_login_error("Too many wrong password attempts")
                        .await;
                } else if time == failure.0 && failure.1 > 6 {
                    self.send_login_error("Please try 1 minute later").await;
                } else if hasher2.finalize()[..] != lr.password[..] {
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
                    self.send_login_error("Wrong Password").await;
                    self.try_start_cm(lr.my_id, lr.my_name, false).await;
                } else {
                    if failure.0 != 0 {
                        LOGIN_FAILURES.lock().unwrap().remove(&self.ip);
                    }
                    self.send_logon_response().await;
                    self.try_start_cm(lr.my_id, lr.my_name, true).await;
                    if self.port_forward_socket.is_some() {
                        return false;
                    }
                }
            }
        } else if let Some(message::Union::test_delay(t)) = msg.union {
            if t.from_client {
                let mut msg_out = Message::new();
                msg_out.set_test_delay(t);
                self.inner.send(msg_out.into());
            } else {
                self.last_test_delay = 0;
                let latency = crate::get_time() - t.time;
                if latency > 0 {
                    super::video_service::update_test_latency(self.inner.id(), latency);
                }
            }
        } else if self.authorized {
            match msg.union {
                Some(message::Union::mouse_event(me)) => {
                    if self.keyboard {
                        handle_mouse(&me, self.inner.id());
                    }
                }
                Some(message::Union::key_event(mut me)) => {
                    if self.keyboard {
                        // handle all down as press
                        // fix unexpected repeating key on remote linux, seems also fix abnormal alt/shift, which
                        // make sure all key are released
                        let is_press = if cfg!(target_os = "linux") {
                            (me.press || me.down) && !crate::is_modifier(&me)
                        } else {
                            me.press
                        };
                        if is_press {
                            if let Some(key_event::Union::unicode(_)) = me.union {
                                handle_key(&me);
                            } else if let Some(key_event::Union::seq(_)) = me.union {
                                handle_key(&me);
                            } else {
                                me.down = true;
                                handle_key(&me);
                                me.down = false;
                                handle_key(&me);
                            }
                        } else {
                            handle_key(&me);
                        }
                    }
                }
                Some(message::Union::clipboard(cb)) => {
                    if self.clipboard {
                        update_clipboard(cb, None);
                    }
                }
                Some(message::Union::file_action(fa)) => {
                    if self.file_transfer.is_some() {
                        match fa.union {
                            Some(file_action::Union::read_dir(rd)) => {
                                self.read_dir(&rd.path, rd.include_hidden);
                            }
                            Some(file_action::Union::all_files(f)) => {
                                match fs::get_recursive_files(&f.path, f.include_hidden) {
                                    Err(err) => {
                                        self.send(fs::new_error(f.id, err, -1)).await;
                                    }
                                    Ok(files) => {
                                        self.send(fs::new_dir(f.id, files)).await;
                                    }
                                }
                            }
                            Some(file_action::Union::send(s)) => {
                                let id = s.id;
                                match fs::TransferJob::new_read(id, s.path, s.include_hidden) {
                                    Err(err) => {
                                        self.send(fs::new_error(id, err, 0)).await;
                                    }
                                    Ok(job) => {
                                        self.send(fs::new_dir(id, job.files().to_vec())).await;
                                        self.read_jobs.push(job);
                                        self.timer = time::interval(MILLI1);
                                    }
                                }
                            }
                            Some(file_action::Union::receive(r)) => {
                                self.send_fs(ipc::FS::NewWrite {
                                    path: r.path,
                                    id: r.id,
                                    files: r
                                        .files
                                        .to_vec()
                                        .drain(..)
                                        .map(|f| (f.name, f.modified_time))
                                        .collect(),
                                });
                            }
                            Some(file_action::Union::remove_dir(d)) => {
                                self.send_fs(ipc::FS::RemoveDir {
                                    path: d.path,
                                    id: d.id,
                                    recursive: d.recursive,
                                });
                            }
                            Some(file_action::Union::remove_file(f)) => {
                                self.send_fs(ipc::FS::RemoveFile {
                                    path: f.path,
                                    id: f.id,
                                    file_num: f.file_num,
                                });
                            }
                            Some(file_action::Union::create(c)) => {
                                self.send_fs(ipc::FS::CreateDir {
                                    path: c.path,
                                    id: c.id,
                                });
                            }
                            Some(file_action::Union::cancel(c)) => {
                                self.send_fs(ipc::FS::CancelWrite { id: c.id });
                                fs::remove_job(c.id, &mut self.read_jobs);
                            }
                            _ => {}
                        }
                    }
                }
                Some(message::Union::file_response(fr)) => match fr.union {
                    Some(file_response::Union::block(block)) => {
                        self.send_fs(ipc::FS::WriteBlock {
                            id: block.id,
                            file_num: block.file_num,
                            data: block.data,
                            compressed: block.compressed,
                        });
                    }
                    Some(file_response::Union::done(d)) => {
                        self.send_fs(ipc::FS::WriteDone {
                            id: d.id,
                            file_num: d.file_num,
                        });
                    }
                    _ => {}
                },
                Some(message::Union::misc(misc)) => match misc.union {
                    Some(misc::Union::switch_display(s)) => {
                        super::video_service::switch_display(s.display);
                    }
                    Some(misc::Union::chat_message(c)) => {
                        self.send_to_cm(ipc::Data::ChatMessage { text: c.text });
                    }
                    Some(misc::Union::option(o)) => {
                        self.update_option(&o);
                    }
                    Some(misc::Union::refresh_video(r)) => {
                        if r {
                            super::video_service::refresh();
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        true
    }

    fn update_option(&mut self, o: &OptionMessage) {
        log::info!("Option update: {:?}", o);
        if let Ok(q) = o.image_quality.enum_value() {
            self.image_quality = q.value();
            super::video_service::update_image_quality(self.inner.id(), Some(q.value()));
        }
        let q = o.custom_image_quality;
        if q > 0 {
            self.image_quality = q;
            super::video_service::update_image_quality(self.inner.id(), Some(q));
        }
        if let Ok(q) = o.lock_after_session_end.enum_value() {
            if q != BoolOption::NotSet {
                self.lock_after_session_end = q == BoolOption::Yes;
            }
        }
        if let Ok(q) = o.show_remote_cursor.enum_value() {
            if q != BoolOption::NotSet {
                self.show_remote_cursor = q == BoolOption::Yes;
                if let Some(s) = self.server.upgrade() {
                    s.write().unwrap().subscribe(
                        NAME_CURSOR,
                        self.inner.clone(),
                        self.keyboard || self.show_remote_cursor,
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
        if let Ok(q) = o.disable_clipboard.enum_value() {
            if q != BoolOption::NotSet {
                self.disable_clipboard = q == BoolOption::Yes;
                if let Some(s) = self.server.upgrade() {
                    s.write().unwrap().subscribe(
                        super::clipboard_service::NAME,
                        self.inner.clone(),
                        self.clipboard_enabled() && self.keyboard,
                    );
                }
            }
        }
        if let Ok(q) = o.privacy_mode.enum_value() {
            if q != BoolOption::NotSet {
                self.privacy_mode = q == BoolOption::Yes;
                if self.privacy_mode && self.keyboard {
                    crate::platform::toggle_privacy_mode(true);
                }
            }
        }
        if self.keyboard {
            if let Ok(q) = o.block_input.enum_value() {
                if q != BoolOption::NotSet {
                    crate::platform::block_input(q == BoolOption::Yes);
                }
            }
        }
    }

    fn on_close(&mut self, reason: &str, lock: bool) {
        if let Some(s) = self.server.upgrade() {
            s.write().unwrap().remove_connection(&self.inner);
        }
        log::info!("#{} Connection closed: {}", self.inner.id(), reason);
        if lock && self.lock_after_session_end && self.keyboard {
            crate::platform::lock_screen();
            super::video_service::switch_to_primary();
        }
        if self.privacy_mode {
            crate::platform::toggle_privacy_mode(false);
        }
        self.port_forward_socket.take();
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
}

async fn start_ipc(
    mut rx_to_cm: mpsc::UnboundedReceiver<ipc::Data>,
    tx_from_cm: mpsc::UnboundedSender<ipc::Data>,
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
        let run_done;
        if crate::platform::is_root() {
            let mut res = Ok(None);
            for _ in 0..10 {
                res = crate::platform::run_as_user("--cm");
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
            super::CHILD_PROCESS
                .lock()
                .unwrap()
                .push(crate::run_me(vec!["--cm"])?);
        }
        for _ in 0..10 {
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
    let mut stream = stream.unwrap();
    loop {
        tokio::select! {
            res = stream.next() => {
                match res {
                    Err(err) => {
                        return Err(err.into());
                    }
                    Ok(Some(data)) => {
                        tx_from_cm.send(data)?;
                    }
                    _ => {}
                }
            }
            res = rx_to_cm.recv() => {
                match res {
                    Some(data) => {
                        stream.send(&data).await?;
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
