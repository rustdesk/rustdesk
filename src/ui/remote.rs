use crate::client::*;
use crate::common::{
    self, check_clipboard, update_clipboard, ClipboardContext, CLIPBOARD_INTERVAL,
};
use enigo::{self, Enigo, KeyboardControllable};
use hbb_common::{
    allow_err,
    config::{self, Config, PeerConfig},
    fs, log,
    message_proto::*,
    protobuf::Message as _,
    rendezvous_proto::ConnType,
    sleep,
    tokio::{
        self,
        sync::mpsc,
        time::{self, Duration, Instant, Interval},
    },
    Stream,
};
use sciter::{
    dom::{
        event::{EventReason, BEHAVIOR_EVENTS, EVENT_GROUPS, PHASE_MASK},
        Element, HELEMENT,
    },
    make_args,
    video::{video_destination, AssetPtr, COLOR_SPACE},
    Value,
};
use std::{
    collections::HashMap,
    ops::Deref,
    sync::{Arc, Mutex, RwLock},
};

type Video = AssetPtr<video_destination>;

lazy_static::lazy_static! {
    static ref ENIGO: Arc<Mutex<Enigo>> = Arc::new(Mutex::new(Enigo::new()));
    static ref VIDEO: Arc<Mutex<Option<Video>>> = Default::default();
}

fn get_key_state(key: enigo::Key) -> bool {
    #[cfg(target_os = "macos")]
    if key == enigo::Key::NumLock {
        return true;
    }
    ENIGO.lock().unwrap().get_key_state(key)
}

#[derive(Default)]
pub struct HandlerInner {
    element: Option<Element>,
    sender: Option<mpsc::UnboundedSender<Data>>,
    thread: Option<std::thread::JoinHandle<()>>,
    close_state: HashMap<String, String>,
    last_down_key: Option<(String, i32, bool)>,
}

#[derive(Clone, Default)]
pub struct Handler {
    inner: Arc<RwLock<HandlerInner>>,
    cmd: String,
    id: String,
    args: Vec<String>,
    lc: Arc<RwLock<LoginConfigHandler>>,
    super_on: bool,
}

impl Deref for Handler {
    type Target = Arc<RwLock<HandlerInner>>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl sciter::EventHandler for Handler {
    fn get_subscription(&mut self) -> Option<EVENT_GROUPS> {
        Some(EVENT_GROUPS::HANDLE_BEHAVIOR_EVENT)
    }

    fn attached(&mut self, root: HELEMENT) {
        self.write().unwrap().element = Some(Element::from(root));
    }

    fn detached(&mut self, _root: HELEMENT) {
        self.write().unwrap().element = None;
        self.write().unwrap().sender.take().map(|sender| {
            sender.send(Data::Close).ok();
        });
    }

    // https://github.com/sciter-sdk/rust-sciter/blob/master/examples/video.rs
    fn on_event(
        &mut self,
        _root: HELEMENT,
        source: HELEMENT,
        _target: HELEMENT,
        code: BEHAVIOR_EVENTS,
        phase: PHASE_MASK,
        reason: EventReason,
    ) -> bool {
        if phase != PHASE_MASK::BUBBLING {
            return false;
        }
        match code {
            BEHAVIOR_EVENTS::VIDEO_BIND_RQ => {
                let source = Element::from(source);
                log::debug!("[video] {:?} {} ({:?})", code, source, reason);
                if let EventReason::VideoBind(ptr) = reason {
                    if ptr.is_null() {
                        return true;
                    }
                    let site = AssetPtr::adopt(ptr as *mut video_destination);
                    log::debug!("[video] start video");
                    *VIDEO.lock().unwrap() = Some(site);
                    self.reconnect();
                }
            }
            BEHAVIOR_EVENTS::VIDEO_INITIALIZED => {
                log::debug!("[video] {:?}", code);
            }
            BEHAVIOR_EVENTS::VIDEO_STARTED => {
                log::debug!("[video] {:?}", code);
                let source = Element::from(source);
                use sciter::dom::ELEMENT_AREAS;
                let flags = ELEMENT_AREAS::CONTENT_BOX as u32 | ELEMENT_AREAS::SELF_RELATIVE as u32;
                let rc = source.get_location(flags).unwrap();
                log::debug!(
                    "[video] start video thread on <{}> which is about {:?} pixels",
                    source,
                    rc.size()
                );
            }
            BEHAVIOR_EVENTS::VIDEO_STOPPED => {
                log::debug!("[video] {:?}", code);
            }
            _ => return false,
        };
        return true;
    }

    sciter::dispatch_script_call! {
        fn is_xfce();
        fn get_id();
        fn get_default_pi();
        fn get_option(String);
        fn save_close_state(String, String);
        fn is_file_transfer();
        fn is_port_forward();
        fn is_rdp();
        fn login(String, bool);
        fn new_rdp();
        fn send_mouse(i32, i32, i32, bool, bool, bool, bool);
        fn key_down_or_up(bool, String, i32, bool, bool, bool, bool, bool);
        fn ctrl_alt_del();
        fn ctrl_space();
        fn alt_tab();
        fn super_x();
        fn transfer_file();
        fn tunnel();
        fn lock_screen();
        fn reconnect();
        fn get_msgbox();
        fn get_chatbox();
        fn get_icon();
        fn get_home_dir();
        fn read_dir(String, bool);
        fn remove_dir(i32, String, bool);
        fn create_dir(i32, String, bool);
        fn remove_file(i32, String, i32, bool);
        fn read_remote_dir(String, bool);
        fn send_chat(String);
        fn switch_display(i32);
        fn remove_dir_all(i32, String, bool);
        fn confirm_delete_files(i32, i32);
        fn set_no_confirm(i32);
        fn cancel_job(i32);
        fn send_files(i32, String, String, bool, bool);
        fn get_platform(bool);
        fn get_path_sep(bool);
        fn get_icon_path(i32, String);
        fn get_char(String, i32);
        fn get_size();
        fn get_port_forwards();
        fn remove_port_forward(i32);
        fn get_args();
        fn add_port_forward(i32, String, i32);
        fn save_size(i32, i32, i32, i32);
        fn get_view_style();
        fn get_image_quality();
        fn get_custom_image_quality();
        fn save_view_style(String);
        fn save_image_quality(String);
        fn save_custom_image_quality(i32, i32);
        fn refresh_video();
        fn support_refresh();
        fn get_toggle_option(String);
        fn toggle_option(String);
        fn get_remember();
    }
}

impl Handler {
    pub fn new(cmd: String, id: String, args: Vec<String>) -> Self {
        let me = Self {
            cmd,
            id: id.clone(),
            args,
            ..Default::default()
        };
        me.lc
            .write()
            .unwrap()
            .initialize(id, me.is_file_transfer(), me.is_port_forward());
        me
    }

    fn get_view_style(&mut self) -> String {
        return self.lc.read().unwrap().view_style.clone();
    }

    fn get_image_quality(&mut self) -> String {
        return self.lc.read().unwrap().image_quality.clone();
    }

    fn get_custom_image_quality(&mut self) -> Value {
        let mut v = Value::array(0);
        for x in self.lc.read().unwrap().custom_image_quality.iter() {
            v.push(x);
        }
        v
    }

    #[inline]
    fn save_config(&self, config: PeerConfig) {
        self.lc.write().unwrap().save_config(config);
    }

    fn save_view_style(&mut self, value: String) {
        self.lc.write().unwrap().save_view_style(value);
    }

    #[inline]
    fn load_config(&self) -> PeerConfig {
        load_config(&self.id)
    }

    fn toggle_option(&mut self, name: String) {
        let msg = self.lc.write().unwrap().toggle_option(name);
        if let Some(msg) = msg {
            self.send(Data::Message(msg));
        }
    }

    fn get_toggle_option(&mut self, name: String) -> bool {
        self.lc.read().unwrap().get_toggle_option(&name)
    }

    fn refresh_video(&mut self) {
        self.send(Data::Message(LoginConfigHandler::refresh()));
    }

    fn support_refresh(&self) -> bool {
        self.lc.read().unwrap().support_refresh
    }

    fn save_custom_image_quality(&mut self, bitrate: i32, quantizer: i32) {
        let msg = self
            .lc
            .write()
            .unwrap()
            .save_custom_image_quality(bitrate, quantizer);
        self.send(Data::Message(msg));
    }

    fn save_image_quality(&mut self, value: String) {
        let msg = self.lc.write().unwrap().save_image_quality(value);
        if let Some(msg) = msg {
            self.send(Data::Message(msg));
        }
    }

    fn get_remember(&mut self) -> bool {
        self.lc.read().unwrap().remember
    }

    fn is_xfce(&self) -> bool {
        crate::platform::is_xfce()
    }

    fn save_size(&mut self, x: i32, y: i32, w: i32, h: i32) {
        let size = (x, y, w, h);
        let mut config = self.load_config();
        if self.is_file_transfer() {
            let close_state = self.read().unwrap().close_state.clone();
            let mut has_change = false;
            for (k, v) in close_state {
                let v2 = if v.is_empty() { None } else { Some(&v) };
                if v2 != config.options.get(&k) {
                    has_change = true;
                    if v2.is_none() {
                        config.options.remove(&k);
                    } else {
                        config.options.insert(k, v);
                    }
                }
            }
            if size == config.size_ft && !has_change {
                return;
            }
            config.size_ft = size;
        } else if self.is_port_forward() {
            if size == config.size_pf {
                return;
            }
            config.size_pf = size;
        } else {
            if size == config.size {
                return;
            }
            config.size = size;
        }
        self.save_config(config);
        log::info!("size saved");
    }

    fn get_port_forwards(&mut self) -> Value {
        let port_forwards = self.lc.read().unwrap().port_forwards.clone();
        let mut v = Value::array(0);
        for (port, remote_host, remote_port) in port_forwards {
            let mut v2 = Value::array(0);
            v2.push(port);
            v2.push(remote_host);
            v2.push(remote_port);
            v.push(v2);
        }
        v
    }

    fn get_args(&mut self) -> Value {
        let mut v = Value::array(0);
        for x in self.args.iter() {
            v.push(x);
        }
        v
    }

    fn remove_port_forward(&mut self, port: i32) {
        let mut config = self.load_config();
        config.port_forwards = config
            .port_forwards
            .drain(..)
            .filter(|x| x.0 != port)
            .collect();
        self.save_config(config);
        self.send(Data::RemovePortForward(port));
    }

    fn add_port_forward(&mut self, port: i32, remote_host: String, remote_port: i32) {
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
        self.save_config(config);
        self.send(Data::AddPortForward(pf));
    }

    fn get_size(&mut self) -> Value {
        let s = if self.is_file_transfer() {
            self.lc.read().unwrap().size_ft
        } else if self.is_port_forward() {
            self.lc.read().unwrap().size_pf
        } else {
            self.lc.read().unwrap().size
        };
        let mut v = Value::array(0);
        v.push(s.0);
        v.push(s.1);
        v.push(s.2);
        v.push(s.3);
        v
    }

    fn get_id(&mut self) -> String {
        self.id.clone()
    }

    fn get_default_pi(&mut self) -> Value {
        let mut pi = Value::map();
        let info = self.lc.read().unwrap().info.clone();
        pi.set_item("username", info.username.clone());
        pi.set_item("hostname", info.hostname.clone());
        pi.set_item("platform", info.platform.clone());
        pi
    }

    fn get_option(&self, k: String) -> String {
        self.lc.read().unwrap().get_option(&k)
    }

    fn save_close_state(&self, k: String, v: String) {
        self.write().unwrap().close_state.insert(k, v);
    }

    fn get_msgbox(&mut self) -> String {
        super::get_msgbox()
    }

    fn get_chatbox(&mut self) -> String {
        #[cfg(feature = "inline")]
        return super::inline::get_chatbox();
        #[cfg(not(feature = "inline"))]
        return "".to_owned();
    }

    fn get_icon(&mut self) -> String {
        config::ICON.to_owned()
    }

    fn get_home_dir(&mut self) -> String {
        fs::get_home_as_string()
    }

    fn read_dir(&mut self, path: String, include_hidden: bool) -> Value {
        match fs::read_dir(&fs::get_path(&path), include_hidden) {
            Err(_) => Value::null(),
            Ok(fd) => {
                let mut m = make_fd(0, &fd.entries.to_vec(), false);
                m.set_item("path", path);
                m
            }
        }
    }

    fn cancel_job(&mut self, id: i32) {
        self.send(Data::CancelJob(id));
    }

    fn read_remote_dir(&mut self, path: String, include_hidden: bool) {
        let mut msg_out = Message::new();
        let mut file_action = FileAction::new();
        file_action.set_read_dir(ReadDir {
            path,
            include_hidden,
            ..Default::default()
        });
        msg_out.set_file_action(file_action);
        self.send(Data::Message(msg_out));
    }

    fn send_chat(&mut self, text: String) {
        let mut misc = Misc::new();
        misc.set_chat_message(ChatMessage {
            text,
            ..Default::default()
        });
        let mut msg_out = Message::new();
        msg_out.set_misc(misc);
        self.send(Data::Message(msg_out));
    }

    fn switch_display(&mut self, display: i32) {
        let mut misc = Misc::new();
        misc.set_switch_display(SwitchDisplay {
            display,
            ..Default::default()
        });
        let mut msg_out = Message::new();
        msg_out.set_misc(misc);
        self.send(Data::Message(msg_out));
    }

    fn remove_file(&mut self, id: i32, path: String, file_num: i32, is_remote: bool) {
        self.send(Data::RemoveFile((id, path, file_num, is_remote)));
    }

    fn remove_dir_all(&mut self, id: i32, path: String, is_remote: bool) {
        self.send(Data::RemoveDirAll((id, path, is_remote)));
    }

    fn confirm_delete_files(&mut self, id: i32, file_num: i32) {
        self.send(Data::ConfirmDeleteFiles((id, file_num)));
    }

    fn set_no_confirm(&mut self, id: i32) {
        self.send(Data::SetNoConfirm(id));
    }

    fn remove_dir(&mut self, id: i32, path: String, is_remote: bool) {
        if is_remote {
            self.send(Data::RemoveDir((id, path)));
        } else {
            fs::remove_all_empty_dir(&fs::get_path(&path)).ok();
        }
    }

    fn create_dir(&mut self, id: i32, path: String, is_remote: bool) {
        self.send(Data::CreateDir((id, path, is_remote)));
    }

    fn send_files(
        &mut self,
        id: i32,
        path: String,
        to: String,
        include_hidden: bool,
        is_remote: bool,
    ) {
        self.send(Data::SendFiles((id, path, to, include_hidden, is_remote)));
    }

    fn is_file_transfer(&self) -> bool {
        self.cmd == "--file-transfer"
    }

    fn is_port_forward(&self) -> bool {
        self.cmd == "--port-forward" || self.is_rdp()
    }

    fn is_rdp(&self) -> bool {
        self.cmd == "--rdp"
    }

    fn reconnect(&mut self) {
        let cloned = self.clone();
        let mut lock = self.write().unwrap();
        lock.last_down_key.take();
        lock.thread.take().map(|t| t.join());
        lock.thread = Some(std::thread::spawn(move || {
            io_loop(cloned);
        }));
    }

    #[inline]
    fn peer_platform(&self) -> String {
        self.lc.read().unwrap().info.platform.clone()
    }

    fn get_platform(&mut self, is_remote: bool) -> String {
        if is_remote {
            self.peer_platform()
        } else {
            whoami::platform().to_string()
        }
    }

    fn get_path_sep(&mut self, is_remote: bool) -> &'static str {
        let p = self.get_platform(is_remote);
        if &p == "Windows" {
            return "\\";
        } else {
            return "/";
        }
    }

    fn get_icon_path(&mut self, file_type: i32, ext: String) -> String {
        let mut path = Config::icon_path();
        if file_type == FileType::DirLink as i32 {
            let new_path = path.join("dir_link");
            if !std::fs::metadata(&new_path).is_ok() {
                #[cfg(windows)]
                allow_err!(std::os::windows::fs::symlink_file(&path, &new_path));
                #[cfg(not(windows))]
                allow_err!(std::os::unix::fs::symlink(&path, &new_path));
            }
            path = new_path;
        } else if file_type == FileType::File as i32 {
            if !ext.is_empty() {
                path = path.join(format!("file.{}", ext));
            } else {
                path = path.join("file");
            }
            if !std::fs::metadata(&path).is_ok() {
                allow_err!(std::fs::File::create(&path));
            }
        } else if file_type == FileType::FileLink as i32 {
            let new_path = path.join("file_link");
            if !std::fs::metadata(&new_path).is_ok() {
                path = path.join("file");
                if !std::fs::metadata(&path).is_ok() {
                    allow_err!(std::fs::File::create(&path));
                }
                #[cfg(windows)]
                allow_err!(std::os::windows::fs::symlink_file(&path, &new_path));
                #[cfg(not(windows))]
                allow_err!(std::os::unix::fs::symlink(&path, &new_path));
            }
            path = new_path;
        } else if file_type == FileType::DirDrive as i32 {
            if cfg!(windows) {
                path = fs::get_path("C:");
            } else if cfg!(target_os = "macos") {
                if let Ok(entries) = fs::get_path("/Volumes/").read_dir() {
                    for entry in entries {
                        if let Ok(entry) = entry {
                            path = entry.path();
                            break;
                        }
                    }
                }
            }
        }
        fs::get_string(&path)
    }

    #[inline]
    fn send(&mut self, data: Data) {
        if let Some(ref sender) = self.read().unwrap().sender {
            sender.send(data).ok();
        }
    }

    fn login(&mut self, password: String, remember: bool) {
        self.send(Data::Login((password, remember)));
    }

    fn new_rdp(&mut self) {
        self.send(Data::NewRDP);
    }

    fn send_mouse(
        &mut self,
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
        self.send(Data::Message(msg_out));
        // on macos, ctrl + left = right, up wont emit, so we need to
        // emit up myself if peer is not macos
        // to-do: how about ctrl + left from win to macos
        if cfg!(target_os = "macos") {
            let buttons = mask >> 3;
            let evt_type = mask & 0x7;
            if buttons == 1 && evt_type == 1 && ctrl && self.peer_platform() != "Mac OS" {
                self.send_mouse((1 << 3 | 2) as _, x, y, alt, ctrl, shift, command);
                return;
            }
        }
    }

    fn set_cursor_data(&mut self, cd: CursorData) {
        let colors = hbb_common::compress::decompress(&cd.colors);
        let mut png = Vec::new();
        if let Ok(()) = repng::encode(&mut png, cd.width as _, cd.height as _, &colors) {
            self.call(
                "setCursorData",
                &make_args!(
                    cd.id.to_string(),
                    cd.hotx,
                    cd.hoty,
                    cd.width,
                    cd.height,
                    &png[..]
                ),
            );
        }
    }

    fn get_key_event(&self, down_or_up: i32, name: &str, code: i32) -> Option<KeyEvent> {
        let mut key_event = KeyEvent::new();
        if down_or_up == 2 {
            /* windows send both keyup/keydown and keychar, so here we avoid keychar
               for <= 0xFF, best practise should only avoid those not on keyboard, but
               for now, we have no way to test, so avoid <= 0xFF totaly
            */
            if code <= 0xFF {
                return None;
            }
            key_event.set_unicode(code.clone() as _);
        } else if let Some(key) = KEY_MAP.get(name) {
            match key {
                Key::Chr(chr) => {
                    key_event.set_chr(chr.clone());
                }
                Key::ControlKey(key) => {
                    key_event.set_control_key(key.clone());
                }
                _ => {}
            }
        } else {
            if cfg!(target_os = "macos") {
                match code {
                    0x4C => key_event.set_control_key(ControlKey::NumpadEnter), // numpad enter
                    0x69 => key_event.set_control_key(ControlKey::Snapshot),
                    0x72 => key_event.set_control_key(ControlKey::Help),
                    0x6E => key_event.set_control_key(ControlKey::Apps),
                    0x47 => {
                        key_event.set_control_key(if self.peer_platform() == "Mac OS" {
                            ControlKey::Clear
                        } else {
                            ControlKey::NumLock
                        });
                    }
                    0x51 => key_event.set_control_key(ControlKey::Equals),
                    0x2F => key_event.set_chr('.' as _),
                    0x32 => key_event.set_chr('`' as _),
                    _ => {
                        log::error!("Unknown key code {}", code);
                        return None;
                    }
                }
            } else if cfg!(windows) {
                match code {
                    0x2C => key_event.set_control_key(ControlKey::Snapshot),
                    0x91 => key_event.set_control_key(ControlKey::Scroll),
                    0x90 => key_event.set_control_key(ControlKey::NumLock),
                    0x5C => key_event.set_control_key(ControlKey::RWin),
                    0x5B => key_event.set_control_key(ControlKey::Meta),
                    0x5D => key_event.set_control_key(ControlKey::Apps),
                    0xBE => key_event.set_chr('.' as _),
                    0xC0 => key_event.set_chr('`' as _),
                    _ => {
                        log::error!("Unknown key code {}", code);
                        return None;
                    }
                }
            } else if cfg!(target_os = "linux") {
                match code {
                    65300 => key_event.set_control_key(ControlKey::Scroll),
                    65421 => key_event.set_control_key(ControlKey::NumpadEnter), // numpad enter
                    65407 => key_event.set_control_key(ControlKey::NumLock),
                    65516 => key_event.set_control_key(ControlKey::RWin),
                    65513 => key_event.set_control_key(ControlKey::Alt),
                    65514 => key_event.set_control_key(ControlKey::RAlt),
                    65508 => key_event.set_control_key(ControlKey::RControl),
                    65506 => key_event.set_control_key(ControlKey::RShift),
                    96 => key_event.set_chr('`' as _),
                    46 => key_event.set_chr('.' as _),
                    126 => key_event.set_chr('`' as _),
                    33 => key_event.set_chr('1' as _),
                    64 => key_event.set_chr('2' as _),
                    35 => key_event.set_chr('3' as _),
                    36 => key_event.set_chr('4' as _),
                    37 => key_event.set_chr('5' as _),
                    94 => key_event.set_chr('6' as _),
                    38 => key_event.set_chr('7' as _),
                    42 => key_event.set_chr('8' as _),
                    40 => key_event.set_chr('9' as _),
                    41 => key_event.set_chr('0' as _),
                    95 => key_event.set_chr('-' as _),
                    43 => key_event.set_chr('=' as _),
                    123 => key_event.set_chr('[' as _),
                    125 => key_event.set_chr(']' as _),
                    124 => key_event.set_chr('\\' as _),
                    58 => key_event.set_chr(';' as _),
                    34 => key_event.set_chr('\'' as _),
                    60 => key_event.set_chr(',' as _),
                    62 => key_event.set_chr('.' as _),
                    63 => key_event.set_chr('/' as _),
                    _ => {
                        log::error!("Unknown key code {}", code);
                        return None;
                    }
                }
            } else {
                log::error!("Unknown key code {}", code);
                return None;
            }
        }
        Some(key_event)
    }

    fn get_char(&mut self, name: String, code: i32) -> String {
        if let Some(key_event) = self.get_key_event(1, &name, code) {
            match key_event.union {
                Some(key_event::Union::chr(chr)) => {
                    if let Some(chr) = std::char::from_u32(chr as _) {
                        return chr.to_string();
                    }
                }
                _ => {}
            }
        }
        "".to_owned()
    }

    fn ctrl_alt_del(&mut self) {
        if self.peer_platform() == "Windows" {
            let del = "CTRL_ALT_DEL".to_owned();
            self.key_down_or_up(1, del, 0, false, false, false, false, false);
        } else {
            let del = "VK_DELETE".to_owned();
            self.key_down_or_up(1, del.clone(), 0, true, true, false, false, false);
            self.key_down_or_up(0, del, 0, true, true, false, false, false);
        }
    }

    fn super_x(&mut self) {
        self.super_on = true;
    }

    fn ctrl_space(&mut self) {
        let key = "VK_SPACE".to_owned();
        self.key_down_or_up(3, key, 0, false, true, false, false, false);
    }

    fn alt_tab(&mut self) {
        let key = "VK_TAB".to_owned();
        self.key_down_or_up(3, key, 0, true, false, false, false, false);
    }

    fn lock_screen(&mut self) {
        let lock = "LOCK_SCREEN".to_owned();
        self.key_down_or_up(1, lock, 0, false, false, false, false, false);
    }

    fn transfer_file(&mut self) {
        let id = self.get_id();
        let args = vec!["--file-transfer", &id];
        if let Err(err) = crate::run_me(args) {
            log::error!("Failed to spawn file transfer: {}", err);
        }
    }

    fn tunnel(&mut self) {
        let id = self.get_id();
        let args = vec!["--port-forward", &id];
        if let Err(err) = crate::run_me(args) {
            log::error!("Failed to spawn IP tunneling: {}", err);
        }
    }

    fn key_down_or_up(
        &mut self,
        down_or_up: i32,
        name: String,
        code: i32,
        alt: bool,
        ctrl: bool,
        shift: bool,
        command: bool,
        extended: bool,
    ) {
        // extended: e.g. ctrl key on right side, https://docs.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-keybd_event
        // not found api of osx and xdo
        log::debug!(
            "{:?} {} {} {} {} {} {} {} {}",
            std::time::SystemTime::now(),
            down_or_up,
            name,
            code,
            alt,
            ctrl,
            shift,
            command,
            extended,
        );

        let mut command = command;
        if self.super_on {
            command = true;
        }

        if down_or_up == 0 {
            self.super_on = false;
        }

        let mut name = name;
        #[cfg(target_os = "linux")]
        if code == 65383 { // VK_MENU
            name = "Apps".to_owned();
        }

        if extended {
            match name.as_ref() {
                "VK_CONTROL" => name = "RControl".to_owned(),
                "VK_MENU" => name = "RAlt".to_owned(),
                "VK_SHIFT" => name = "RShift".to_owned(),
                _ => {}
            }
        }

        if let Some(mut key_event) = self.get_key_event(down_or_up, &name, code) {
            if alt
                && !crate::is_control_key(&key_event, &ControlKey::Alt)
                && !crate::is_control_key(&key_event, &ControlKey::RAlt)
            {
                key_event.modifiers.push(ControlKey::Alt.into());
            }
            if shift
                && !crate::is_control_key(&key_event, &ControlKey::Shift)
                && !crate::is_control_key(&key_event, &ControlKey::RShift)
            {
                key_event.modifiers.push(ControlKey::Shift.into());
            }
            if ctrl
                && !crate::is_control_key(&key_event, &ControlKey::Control)
                && !crate::is_control_key(&key_event, &ControlKey::RControl)
            {
                key_event.modifiers.push(ControlKey::Control.into());
            }
            if command
                && !crate::is_control_key(&key_event, &ControlKey::Meta)
                && !crate::is_control_key(&key_event, &ControlKey::RWin)
            {
                key_event.modifiers.push(ControlKey::Meta.into());
            }
            if crate::is_control_key(&key_event, &ControlKey::CapsLock) {
                return;
            } else if get_key_state(enigo::Key::CapsLock) && common::valid_for_capslock(&key_event)
            {
                key_event.modifiers.push(ControlKey::CapsLock.into());
            }
            if self.peer_platform() != "Mac OS" {
                if crate::is_control_key(&key_event, &ControlKey::NumLock) {
                    return;
                } else if get_key_state(enigo::Key::NumLock)
                    && common::valid_for_numlock(&key_event)
                {
                    key_event.modifiers.push(ControlKey::NumLock.into());
                }
            }
            if down_or_up == 1 {
                key_event.down = true;
            } else if down_or_up == 3 {
                key_event.press = true;
            }
            let mut msg_out = Message::new();
            msg_out.set_key_event(key_event);
            log::debug!("{:?}", msg_out);
            self.send(Data::Message(msg_out));
        }
    }

    #[inline]
    fn set_cursor_id(&mut self, id: String) {
        self.call("setCursorId", &make_args!(id));
    }

    #[inline]
    fn set_cursor_position(&mut self, cd: CursorPosition) {
        self.call("setCursorPosition", &make_args!(cd.x, cd.y));
    }

    #[inline]
    fn call(&self, func: &str, args: &[Value]) {
        let r = self.read().unwrap();
        if let Some(ref e) = r.element {
            allow_err!(e.call_method(func, args));
        }
    }

    #[inline]
    fn set_display(&self, x: i32, y: i32, w: i32, h: i32) {
        self.call("setDisplay", &make_args!(x, y, w, h));
    }
}

const MILLI1: Duration = Duration::from_millis(1);

async fn start_one_port_forward(
    handler: Handler,
    port: i32,
    remote_host: String,
    remote_port: i32,
    receiver: mpsc::UnboundedReceiver<Data>,
) {
    handler.lc.write().unwrap().port_forward = (remote_host, remote_port);
    if let Err(err) =
        crate::port_forward::listen(handler.id.clone(), port, handler.clone(), receiver).await
    {
        handler.on_error(&format!("Failed to listen on {}: {}", port, err));
    }
    log::info!("port forward (:{}) exit", port);
}

#[tokio::main(flavor = "current_thread")]
async fn io_loop(handler: Handler) {
    let (sender, mut receiver) = mpsc::unbounded_channel::<Data>();
    handler.write().unwrap().sender = Some(sender.clone());
    if handler.is_port_forward() {
        if handler.is_rdp() {
            start_one_port_forward(handler, 0, "".to_owned(), 3389, receiver).await;
        } else if handler.args.len() == 0 {
            let pfs = handler.lc.read().unwrap().port_forwards.clone();
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
                        let handler = handler.clone();
                        tokio::spawn(async move {
                            start_one_port_forward(
                                handler,
                                port,
                                remote_host,
                                remote_port,
                                receiver,
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
            let port = handler.args[0].parse::<i32>().unwrap_or(0);
            if handler.args.len() != 3
                || handler.args[2].parse::<i32>().unwrap_or(0) <= 0
                || port <= 0
            {
                handler.on_error("Invalid arguments, usage:<br><br> rustdesk --port-forward remote-id listen-port remote-host remote-port");
            }
            let remote_host = handler.args[1].clone();
            let remote_port = handler.args[2].parse::<i32>().unwrap_or(0);
            start_one_port_forward(handler, port, remote_host, remote_port, receiver).await;
        }
        return;
    }
    let mut remote = Remote {
        handler,
        video_handler: VideoHandler::new(),
        audio_handler: Default::default(),
        receiver,
        sender,
        old_clipboard: Default::default(),
        read_jobs: Vec::new(),
        write_jobs: Vec::new(),
        remove_jobs: Default::default(),
        timer: time::interval(SEC30),
        last_update_jobs_status: (Instant::now(), Default::default()),
        clipboard: Arc::new(RwLock::new(true)),
        keyboard: Arc::new(RwLock::new(true)),
        first_frame: false,
    };
    remote.io_loop().await;
}

struct RemoveJob {
    files: Vec<FileEntry>,
    path: String,
    sep: &'static str,
    is_remote: bool,
    no_confirm: bool,
    last_update_job_status: Instant,
}

impl RemoveJob {
    fn new(files: Vec<FileEntry>, path: String, sep: &'static str, is_remote: bool) -> Self {
        Self {
            files,
            path,
            sep,
            is_remote,
            no_confirm: false,
            last_update_job_status: Instant::now(),
        }
    }
}

struct Remote {
    handler: Handler,
    audio_handler: AudioHandler,
    video_handler: VideoHandler,
    receiver: mpsc::UnboundedReceiver<Data>,
    sender: mpsc::UnboundedSender<Data>,
    old_clipboard: Arc<Mutex<String>>,
    read_jobs: Vec<fs::TransferJob>,
    write_jobs: Vec<fs::TransferJob>,
    remove_jobs: HashMap<i32, RemoveJob>,
    timer: Interval,
    last_update_jobs_status: (Instant, HashMap<i32, u64>),
    clipboard: Arc<RwLock<bool>>,
    keyboard: Arc<RwLock<bool>>,
    first_frame: bool,
}

impl Remote {
    async fn io_loop(&mut self) {
        let stop_clipboard = self.start_clipboard();
        let mut last_recv_time = Instant::now();
        let conn_type = if self.handler.is_file_transfer() {
            ConnType::FILE_TRANSFER
        } else {
            ConnType::default()
        };
        match Client::start(&self.handler.id, conn_type).await {
            Ok((mut peer, direct)) => {
                self.handler
                    .call("setConnectionType", &make_args!(peer.is_secured(), direct));
                loop {
                    tokio::select! {
                        res = peer.next() => {
                            if let Some(res) = res {
                                match res {
                                    Err(err) => {
                                        log::error!("Connection closed: {}", err);
                                        self.handler.msgbox("error", "Connection Error", &err.to_string());
                                        break;
                                    }
                                    Ok(ref bytes) => {
                                        last_recv_time = Instant::now();
                                        if !self.handle_msg_from_peer(bytes, &mut peer).await {
                                            break
                                        }
                                    }
                                }
                            } else {
                                log::info!("Reset by the peer");
                                self.handler.msgbox("error", "Connection Error", "Reset by the peer");
                                break;
                            }
                        }
                        d = self.receiver.recv() => {
                            if let Some(d) = d {
                                if !self.handle_msg_from_ui(d, &mut peer).await {
                                    break;
                                }
                            }
                        }
                        _ = self.timer.tick() => {
                            if last_recv_time.elapsed() >= SEC30 {
                                self.handler.msgbox("error", "Connection Error", "Timeout");
                                break;
                            }
                            if !self.read_jobs.is_empty() {
                                if let Err(err) = fs::handle_read_jobs(&mut self.read_jobs, &mut peer).await {
                                    self.handler.msgbox("error", "Connection Error", &err.to_string());
                                    break;
                                }
                                self.update_jobs_status();
                            } else {
                                self.timer = time::interval_at(Instant::now() + SEC30, SEC30);
                            }
                        }
                    }
                }
                log::debug!("Exit io_loop of id={}", self.handler.id);
            }
            Err(err) => {
                self.handler
                    .msgbox("error", "Connection Error", &err.to_string());
            }
        }
        if let Some(stop) = stop_clipboard {
            stop.send(()).ok();
        }
    }

    fn handle_job_status(&mut self, id: i32, file_num: i32, err: Option<String>) {
        if let Some(job) = self.remove_jobs.get_mut(&id) {
            if job.no_confirm {
                let file_num = (file_num + 1) as usize;
                if file_num < job.files.len() {
                    let path = format!("{}{}{}", job.path, job.sep, job.files[file_num].name);
                    self.sender
                        .send(Data::RemoveFile((id, path, file_num as i32, job.is_remote)))
                        .ok();
                    let elapsed = job.last_update_job_status.elapsed().as_millis() as i32;
                    if elapsed >= 1000 {
                        job.last_update_job_status = Instant::now();
                    } else {
                        return;
                    }
                } else {
                    self.remove_jobs.remove(&id);
                }
            }
        }
        if let Some(err) = err {
            self.handler
                .call("jobError", &make_args!(id, err, file_num));
        } else {
            self.handler.call("jobDone", &make_args!(id, file_num));
        }
    }

    fn start_clipboard(&mut self) -> Option<std::sync::mpsc::Sender<()>> {
        if self.handler.is_file_transfer() || self.handler.is_port_forward() {
            return None;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let old_clipboard = self.old_clipboard.clone();
        let tx_protobuf = self.sender.clone();
        let clipboard = self.clipboard.clone();
        let keyboard = self.keyboard.clone();
        let lc = self.handler.lc.clone();
        match ClipboardContext::new() {
            Ok(mut ctx) => {
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
                    if !*clipboard.read().unwrap()
                        || !*keyboard.read().unwrap()
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

    async fn handle_msg_from_ui(&mut self, data: Data, peer: &mut Stream) -> bool {
        match data {
            Data::Close => {
                return false;
            }
            Data::Login((password, remember)) => {
                self.handler
                    .handle_login_from_ui(password, remember, peer)
                    .await;
            }
            Data::Message(msg) => {
                allow_err!(peer.send(&msg).await);
            }
            Data::SendFiles((id, path, to, include_hidden, is_remote)) => {
                if is_remote {
                    log::debug!("New job {}, write to {} from remote {}", id, to, path);
                    self.write_jobs
                        .push(fs::TransferJob::new_write(id, to, Vec::new()));
                    allow_err!(peer.send(&fs::new_send(id, path, include_hidden)).await);
                } else {
                    match fs::TransferJob::new_read(id, path.clone(), include_hidden) {
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
                            let m = make_fd(job.id(), job.files(), true);
                            self.handler.call("updateFolderFiles", &make_args!(m));
                            let files = job.files().clone();
                            self.read_jobs.push(job);
                            self.timer = time::interval(MILLI1);
                            allow_err!(peer.send(&fs::new_receive(id, to, files)).await);
                        }
                    }
                }
            }
            Data::SetNoConfirm(id) => {
                if let Some(job) = self.remove_jobs.get_mut(&id) {
                    job.no_confirm = true;
                }
            }
            Data::ConfirmDeleteFiles((id, file_num)) => {
                if let Some(job) = self.remove_jobs.get_mut(&id) {
                    let i = file_num as usize;
                    if i < job.files.len() {
                        self.handler.call(
                            "confirmDeleteFiles",
                            &make_args!(id, file_num, job.files[i].name.clone()),
                        );
                    }
                }
            }
            Data::RemoveDirAll((id, path, is_remote)) => {
                let sep = self.handler.get_path_sep(is_remote);
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
                    self.remove_jobs
                        .insert(id, RemoveJob::new(Vec::new(), path, sep, is_remote));
                } else {
                    match fs::get_recursive_files(&path, true) {
                        Ok(entries) => {
                            let m = make_fd(id, &entries, true);
                            self.handler.call("updateFolderFiles", &make_args!(m));
                            self.remove_jobs
                                .insert(id, RemoveJob::new(entries, path, sep, is_remote));
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
                self.remove_jobs.remove(&id);
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
            _ => {}
        }
        true
    }

    #[inline]
    fn update_job_status(
        job: &fs::TransferJob,
        elapsed: i32,
        last_update_jobs_status: &mut (Instant, HashMap<i32, u64>),
        handler: &mut Handler,
    ) {
        if elapsed <= 0 {
            return;
        }
        let transfered = job.transfered();
        let last_transfered = {
            if let Some(v) = last_update_jobs_status.1.get(&job.id()) {
                v.to_owned()
            } else {
                0
            }
        };
        last_update_jobs_status.1.insert(job.id(), transfered);
        let speed = (transfered - last_transfered) as f64 / (elapsed as f64 / 1000.);
        let file_num = job.file_num() - 1;
        handler.call(
            "jobProgress",
            &make_args!(job.id(), file_num, speed, job.finished_size() as f64),
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
                    &mut self.handler,
                );
            }
            for job in self.write_jobs.iter() {
                Self::update_job_status(
                    job,
                    elapsed,
                    &mut self.last_update_jobs_status,
                    &mut self.handler,
                );
            }
            self.last_update_jobs_status.0 = Instant::now();
        }
    }

    async fn handle_msg_from_peer(&mut self, data: &[u8], peer: &mut Stream) -> bool {
        if let Ok(msg_in) = Message::parse_from_bytes(&data) {
            match msg_in.union {
                Some(message::Union::video_frame(vf)) => {
                    if !self.first_frame {
                        self.first_frame = true;
                        self.handler.call("closeSuccess", &make_args!());
                        self.handler.call("adaptSize", &make_args!());
                    }
                    if let Some(video_frame::Union::vp9s(vp9s)) = &vf.union {
                        if let Ok(true) = self.video_handler.handle_vp9s(vp9s) {
                            VIDEO
                                .lock()
                                .unwrap()
                                .as_mut()
                                .map(|v| v.render_frame(&self.video_handler.rgb).ok());
                        }
                    }
                }
                Some(message::Union::hash(hash)) => {
                    self.handler.handle_hash(hash, peer).await;
                }
                Some(message::Union::login_response(lr)) => match lr.union {
                    Some(login_response::Union::error(err)) => {
                        if !self.handler.handle_login_error(&err) {
                            return false;
                        }
                    }
                    Some(login_response::Union::peer_info(pi)) => {
                        self.handler.handle_peer_info(pi);
                        if !(self.handler.is_file_transfer()
                            || self.handler.is_port_forward()
                            || !*self.clipboard.read().unwrap()
                            || !*self.keyboard.read().unwrap()
                            || self.handler.lc.read().unwrap().disable_clipboard)
                        {
                            let txt = self.old_clipboard.lock().unwrap().clone();
                            if !txt.is_empty() {
                                let msg_out = crate::create_clipboard_msg(txt);
                                let sender = self.sender.clone();
                                tokio::spawn(async move {
                                    // due to clipboard service interval time
                                    sleep(common::CLIPBOARD_INTERVAL as f32 / 1_000.).await;
                                    sender.send(Data::Message(msg_out)).ok();
                                });
                            }
                        }
                    }
                    _ => {}
                },
                Some(message::Union::cursor_data(cd)) => {
                    self.handler.set_cursor_data(cd);
                }
                Some(message::Union::cursor_id(id)) => {
                    self.handler.set_cursor_id(id.to_string());
                }
                Some(message::Union::cursor_position(cp)) => {
                    self.handler.set_cursor_position(cp);
                }
                Some(message::Union::clipboard(cb)) => {
                    if !self.handler.lc.read().unwrap().disable_clipboard {
                        update_clipboard(cb, Some(&self.old_clipboard));
                    }
                }
                Some(message::Union::file_response(fr)) => match fr.union {
                    Some(file_response::Union::dir(fd)) => {
                        let entries = fd.entries.to_vec();
                        let mut m = make_fd(fd.id, &entries, fd.id > 0);
                        if fd.id <= 0 {
                            m.set_item("path", fd.path);
                        }
                        self.handler.call("updateFolderFiles", &make_args!(m));
                        if let Some(job) = fs::get_job(fd.id, &mut self.write_jobs) {
                            job.set_files(entries);
                        } else if let Some(job) = self.remove_jobs.get_mut(&fd.id) {
                            job.files = entries;
                        }
                    }
                    Some(file_response::Union::block(block)) => {
                        if let Some(job) = fs::get_job(block.id, &mut self.write_jobs) {
                            if let Err(_err) = job.write(block).await {
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
                    _ => {}
                },
                Some(message::Union::misc(misc)) => match misc.union {
                    Some(misc::Union::audio_format(f)) => {
                        self.audio_handler.handle_format(f);
                    }
                    Some(misc::Union::chat_message(c)) => {
                        self.handler.call("newMessage", &make_args!(c.text));
                    }
                    Some(misc::Union::permission_info(p)) => {
                        log::info!("Change permission {:?} -> {}", p.permission, p.enabled);
                        match p.permission.enum_value_or_default() {
                            Permission::Keyboard => {
                                self.handler
                                    .call("setPermission", &make_args!("keyboard", p.enabled));
                            }
                            Permission::Clipboard => {
                                *self.clipboard.write().unwrap() = p.enabled;
                                self.handler
                                    .call("setPermission", &make_args!("clipboard", p.enabled));
                            }
                            Permission::Audio => {
                                self.handler
                                    .call("setPermission", &make_args!("audio", p.enabled));
                            }
                        }
                    }
                    Some(misc::Union::switch_display(s)) => {
                        self.handler.call("switchDisplay", &make_args!(s.display));
                        self.video_handler.reset();
                        if s.width > 0 && s.height > 0 {
                            VIDEO.lock().unwrap().as_mut().map(|v| {
                                v.stop_streaming().ok();
                                let ok = v.start_streaming(
                                    (s.width, s.height),
                                    COLOR_SPACE::Rgb32,
                                    None,
                                );
                                log::info!("[video] reinitialized: {:?}", ok);
                            });
                            self.handler.set_display(s.x, s.y, s.width, s.height);
                        }
                    }
                    Some(misc::Union::close_reason(c)) => {
                        self.handler.msgbox("error", "Connection Error", &c);
                        return false;
                    }
                    _ => {}
                },
                Some(message::Union::test_delay(t)) => {
                    self.handler.handle_test_delay(t, peer).await;
                }
                Some(message::Union::audio_frame(frame)) => {
                    self.audio_handler
                        .handle_frame(frame, !self.handler.lc.read().unwrap().disable_audio);
                }
                _ => {}
            }
        }
        true
    }
}

fn make_fd(id: i32, entries: &Vec<FileEntry>, only_count: bool) -> Value {
    let mut m = Value::map();
    m.set_item("id", id);
    let mut a = Value::array(0);
    let mut n: u64 = 0;
    for entry in entries {
        n += entry.size;
        if only_count {
            continue;
        }
        let mut e = Value::map();
        e.set_item("name", entry.name.to_owned());
        e.set_item("type", entry.entry_type.value());
        e.set_item("time", entry.modified_time as f64);
        e.set_item("size", entry.size as f64);
        a.push(e);
    }
    if only_count {
        m.set_item("num_entries", entries.len() as i32);
    } else {
        m.set_item("entries", a);
    }
    m.set_item("total_size", n as f64);
    m
}

#[async_trait]
impl Interface for Handler {
    fn msgbox(&self, msgtype: &str, title: &str, text: &str) {
        let retry = check_if_retry(msgtype, title, text);
        self.call("msgbox_retry", &make_args!(msgtype, title, text, retry));
    }

    fn handle_login_error(&mut self, err: &str) -> bool {
        self.lc.write().unwrap().handle_login_error(err, self)
    }

    fn handle_peer_info(&mut self, pi: PeerInfo) {
        let mut pi_sciter = Value::map();
        let username = self.lc.read().unwrap().get_username(&pi);
        pi_sciter.set_item("username", username.clone());
        pi_sciter.set_item("hostname", pi.hostname.clone());
        pi_sciter.set_item("platform", pi.platform.clone());
        pi_sciter.set_item("sas_enabled", pi.sas_enabled);
        if self.is_file_transfer() {
            if pi.username.is_empty() {
                self.on_error("No active console user logged on, please connect and logon first.");
                return;
            }
        } else if !self.is_port_forward() {
            if pi.displays.is_empty() {
                self.lc.write().unwrap().handle_peer_info(username, pi);
                self.msgbox("error", "Remote Error", "No Display");
                return;
            }
            let mut displays = Value::array(0);
            for ref d in pi.displays.iter() {
                let mut display = Value::map();
                display.set_item("x", d.x);
                display.set_item("y", d.y);
                display.set_item("width", d.width);
                display.set_item("height", d.height);
                displays.push(display);
            }
            pi_sciter.set_item("displays", displays);
            let mut current = pi.current_display as usize;
            if current >= pi.displays.len() {
                current = 0;
            }
            pi_sciter.set_item("current_display", current as i32);
            let current = &pi.displays[current];
            self.set_display(current.x, current.y, current.width, current.height);
            // https://sciter.com/forums/topic/color_spaceiyuv-crash
            // Nothing spectacular in decoder – done on CPU side.
            // So if you can do BGRA translation on your side – the better.
            // BGRA is used as internal image format so it will not require additional transformations.
            VIDEO.lock().unwrap().as_mut().map(|v| {
                let ok = v.start_streaming(
                    (current.width as _, current.height as _),
                    COLOR_SPACE::Rgb32,
                    None,
                );
                log::info!("[video] initialized: {:?}", ok);
            });
        }
        self.lc.write().unwrap().handle_peer_info(username, pi);
        self.call("updatePi", &make_args!(pi_sciter));
        if self.is_file_transfer() {
            self.call("closeSuccess", &make_args!());
        } else if !self.is_port_forward() {
            self.msgbox("success", "Successful", "Connected, waiting for image...");
        }
        #[cfg(windows)]
        {
            let mut path = std::env::temp_dir();
            path.push(&self.id);
            let path = path.with_extension(config::APP_NAME.to_lowercase());
            std::fs::File::create(&path).ok();
            if let Some(path) = path.to_str() {
                crate::platform::windows::add_recent_document(&path);
            }
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

impl Handler {
    fn on_error(&self, err: &str) {
        self.msgbox("error", "Error", err);
    }
}
