use std::{
    collections::HashMap,
    ops::Deref,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex, RwLock,
    },
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

#[cfg(windows)]
use clipboard::{
    cliprdr::CliprdrClientContext, create_cliprdr_context as create_clipboard_file_context,
    get_rx_clip_client, server_clip_file,
};
use enigo::{self, Enigo, KeyboardControllable};
use hbb_common::fs::{
    can_enable_overwrite_detection, get_job, get_string, new_send_confirm, DigestCheckResult,
    RemoveJobMeta,
};
use hbb_common::{
    allow_err,
    config::{Config, LocalConfig, PeerConfig},
    fs, log,
    message_proto::{permission_info::Permission, *},
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
use hbb_common::{config::TransferSerde, fs::TransferJobMeta};

#[cfg(windows)]
use crate::clipboard_file::*;
use crate::{
    client::*,
    common::{self, check_clipboard, update_clipboard, ClipboardContext, CLIPBOARD_INTERVAL},
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

static IS_IN: AtomicBool = AtomicBool::new(false);
static KEYBOARD_HOOKED: AtomicBool = AtomicBool::new(false);
static SERVER_KEYBOARD_ENABLED: AtomicBool = AtomicBool::new(true);
static SERVER_FILE_TRANSFER_ENABLED: AtomicBool = AtomicBool::new(true);
static SERVER_CLIPBOARD_ENABLED: AtomicBool = AtomicBool::new(true);
#[cfg(windows)]
static mut IS_ALT_GR: bool = false;

#[derive(Default)]
pub struct HandlerInner {
    element: Option<Element>,
    sender: Option<mpsc::UnboundedSender<Data>>,
    thread: Option<std::thread::JoinHandle<()>>,
    close_state: HashMap<String, String>,
}

#[derive(Clone, Default)]
pub struct Handler {
    inner: Arc<RwLock<HandlerInner>>,
    cmd: String,
    id: String,
    args: Vec<String>,
    lc: Arc<RwLock<LoginConfigHandler>>,
}

impl Deref for Handler {
    type Target = Arc<RwLock<HandlerInner>>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl FileManager for Handler {}

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
        fn get_audit_server();
        fn send_note(String);
        fn is_xfce();
        fn get_id();
        fn get_default_pi();
        fn get_option(String);
        fn t(String);
        fn set_option(String, String);
        fn input_os_password(String, bool);
        fn save_close_state(String, String);
        fn is_file_transfer();
        fn is_port_forward();
        fn is_rdp();
        fn login(String, bool);
        fn new_rdp();
        fn send_mouse(i32, i32, i32, bool, bool, bool, bool);
        fn enter();
        fn leave();
        fn ctrl_alt_del();
        fn transfer_file();
        fn tunnel();
        fn lock_screen();
        fn reconnect();
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
        fn send_files(i32, String, String, i32, bool, bool);
        fn add_job(i32, String, String, i32, bool, bool);
        fn resume_job(i32, bool);
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
        fn save_custom_image_quality(i32);
        fn refresh_video();
        fn get_toggle_option(String);
        fn is_privacy_mode_supported();
        fn toggle_option(String);
        fn get_remember();
        fn peer_platform();
        fn set_write_override(i32, i32, bool, bool, bool);
        fn has_hwcodec();
        fn supported_hwcodec();
        fn change_prefer_codec();
    }
}

#[derive(Debug, Default)]
struct QualityStatus {
    speed: Option<String>,
    fps: Option<i32>,
    delay: Option<i32>,
    target_bitrate: Option<i32>,
    codec_format: Option<CodecFormat>,
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

    fn update_quality_status(&self, status: QualityStatus) {
        self.call2(
            "updateQualityStatus",
            &make_args!(
                status.speed.map_or(Value::null(), |it| it.into()),
                status.fps.map_or(Value::null(), |it| it.into()),
                status.delay.map_or(Value::null(), |it| it.into()),
                status.target_bitrate.map_or(Value::null(), |it| it.into()),
                status
                    .codec_format
                    .map_or(Value::null(), |it| it.to_string().into())
            ),
        );
    }

    fn start_keyboard_hook(&self) {
        if self.is_port_forward() || self.is_file_transfer() {
            return;
        }
        if KEYBOARD_HOOKED.swap(true, Ordering::SeqCst) {
            return;
        }
        log::info!("keyboard hooked");
        let mut me = self.clone();
        let peer = self.peer_platform();
        let is_win = peer == "Windows";
        #[cfg(windows)]
        crate::platform::windows::enable_lowlevel_keyboard(std::ptr::null_mut() as _);
        std::thread::spawn(move || {
            // This will block.
            std::env::set_var("KEYBOARD_ONLY", "y"); // pass to rdev
            use rdev::{EventType::*, *};
            let func = move |evt: Event| {
                if !IS_IN.load(Ordering::SeqCst) || !SERVER_KEYBOARD_ENABLED.load(Ordering::SeqCst)
                {
                    return;
                }
                let (key, down) = match evt.event_type {
                    KeyPress(k) => (k, 1),
                    KeyRelease(k) => (k, 0),
                    _ => return,
                };
                let alt = get_key_state(enigo::Key::Alt);
                #[cfg(windows)]
                let ctrl = {
                    let mut tmp = get_key_state(enigo::Key::Control);
                    unsafe {
                        if IS_ALT_GR {
                            if alt || key == Key::AltGr {
                                if tmp {
                                    tmp = false;
                                }
                            } else {
                                IS_ALT_GR = false;
                            }
                        }
                    }
                    tmp
                };
                #[cfg(not(windows))]
                let ctrl = get_key_state(enigo::Key::Control);
                let shift = get_key_state(enigo::Key::Shift);
                #[cfg(windows)]
                let command = crate::platform::windows::get_win_key_state();
                #[cfg(not(windows))]
                let command = get_key_state(enigo::Key::Meta);
                let control_key = match key {
                    Key::Alt => Some(ControlKey::Alt),
                    Key::AltGr => Some(ControlKey::RAlt),
                    Key::Backspace => Some(ControlKey::Backspace),
                    Key::ControlLeft => {
                        // when pressing AltGr, an extra VK_LCONTROL with a special
                        // scancode with bit 9 set is sent, let's ignore this.
                        #[cfg(windows)]
                        if evt.scan_code & 0x200 != 0 {
                            unsafe {
                                IS_ALT_GR = true;
                            }
                            return;
                        }
                        Some(ControlKey::Control)
                    }
                    Key::ControlRight => Some(ControlKey::RControl),
                    Key::DownArrow => Some(ControlKey::DownArrow),
                    Key::Escape => Some(ControlKey::Escape),
                    Key::F1 => Some(ControlKey::F1),
                    Key::F10 => Some(ControlKey::F10),
                    Key::F11 => Some(ControlKey::F11),
                    Key::F12 => Some(ControlKey::F12),
                    Key::F2 => Some(ControlKey::F2),
                    Key::F3 => Some(ControlKey::F3),
                    Key::F4 => Some(ControlKey::F4),
                    Key::F5 => Some(ControlKey::F5),
                    Key::F6 => Some(ControlKey::F6),
                    Key::F7 => Some(ControlKey::F7),
                    Key::F8 => Some(ControlKey::F8),
                    Key::F9 => Some(ControlKey::F9),
                    Key::LeftArrow => Some(ControlKey::LeftArrow),
                    Key::MetaLeft => Some(ControlKey::Meta),
                    Key::MetaRight => Some(ControlKey::RWin),
                    Key::Return => Some(ControlKey::Return),
                    Key::RightArrow => Some(ControlKey::RightArrow),
                    Key::ShiftLeft => Some(ControlKey::Shift),
                    Key::ShiftRight => Some(ControlKey::RShift),
                    Key::Space => Some(ControlKey::Space),
                    Key::Tab => Some(ControlKey::Tab),
                    Key::UpArrow => Some(ControlKey::UpArrow),
                    Key::Delete => {
                        if is_win && ctrl && alt {
                            me.ctrl_alt_del();
                            return;
                        }
                        Some(ControlKey::Delete)
                    }
                    Key::Apps => Some(ControlKey::Apps),
                    Key::Cancel => Some(ControlKey::Cancel),
                    Key::Clear => Some(ControlKey::Clear),
                    Key::Kana => Some(ControlKey::Kana),
                    Key::Hangul => Some(ControlKey::Hangul),
                    Key::Junja => Some(ControlKey::Junja),
                    Key::Final => Some(ControlKey::Final),
                    Key::Hanja => Some(ControlKey::Hanja),
                    Key::Hanji => Some(ControlKey::Hanja),
                    Key::Convert => Some(ControlKey::Convert),
                    Key::Print => Some(ControlKey::Print),
                    Key::Select => Some(ControlKey::Select),
                    Key::Execute => Some(ControlKey::Execute),
                    Key::PrintScreen => Some(ControlKey::Snapshot),
                    Key::Help => Some(ControlKey::Help),
                    Key::Sleep => Some(ControlKey::Sleep),
                    Key::Separator => Some(ControlKey::Separator),
                    Key::KpReturn => Some(ControlKey::NumpadEnter),
                    Key::Kp0 => Some(ControlKey::Numpad0),
                    Key::Kp1 => Some(ControlKey::Numpad1),
                    Key::Kp2 => Some(ControlKey::Numpad2),
                    Key::Kp3 => Some(ControlKey::Numpad3),
                    Key::Kp4 => Some(ControlKey::Numpad4),
                    Key::Kp5 => Some(ControlKey::Numpad5),
                    Key::Kp6 => Some(ControlKey::Numpad6),
                    Key::Kp7 => Some(ControlKey::Numpad7),
                    Key::Kp8 => Some(ControlKey::Numpad8),
                    Key::Kp9 => Some(ControlKey::Numpad9),
                    Key::KpDivide => Some(ControlKey::Divide),
                    Key::KpMultiply => Some(ControlKey::Multiply),
                    Key::KpDecimal => Some(ControlKey::Decimal),
                    Key::KpMinus => Some(ControlKey::Subtract),
                    Key::KpPlus => Some(ControlKey::Add),
                    Key::CapsLock | Key::NumLock | Key::ScrollLock => {
                        return;
                    }
                    Key::Home => Some(ControlKey::Home),
                    Key::End => Some(ControlKey::End),
                    Key::Insert => Some(ControlKey::Insert),
                    Key::PageUp => Some(ControlKey::PageUp),
                    Key::PageDown => Some(ControlKey::PageDown),
                    Key::Pause => Some(ControlKey::Pause),
                    _ => None,
                };
                let mut key_event = KeyEvent::new();
                if let Some(k) = control_key {
                    key_event.set_control_key(k);
                } else {
                    let mut chr = match evt.name {
                        Some(ref s) => {
                            if s.len() <= 2 {
                                // exclude chinese characters
                                s.chars().next().unwrap_or('\0')
                            } else {
                                '\0'
                            }
                        }
                        _ => '\0',
                    };
                    if chr == '·' {
                        // special for Chinese
                        chr = '`';
                    }
                    if chr == '\0' {
                        chr = match key {
                            Key::Num1 => '1',
                            Key::Num2 => '2',
                            Key::Num3 => '3',
                            Key::Num4 => '4',
                            Key::Num5 => '5',
                            Key::Num6 => '6',
                            Key::Num7 => '7',
                            Key::Num8 => '8',
                            Key::Num9 => '9',
                            Key::Num0 => '0',
                            Key::KeyA => 'a',
                            Key::KeyB => 'b',
                            Key::KeyC => 'c',
                            Key::KeyD => 'd',
                            Key::KeyE => 'e',
                            Key::KeyF => 'f',
                            Key::KeyG => 'g',
                            Key::KeyH => 'h',
                            Key::KeyI => 'i',
                            Key::KeyJ => 'j',
                            Key::KeyK => 'k',
                            Key::KeyL => 'l',
                            Key::KeyM => 'm',
                            Key::KeyN => 'n',
                            Key::KeyO => 'o',
                            Key::KeyP => 'p',
                            Key::KeyQ => 'q',
                            Key::KeyR => 'r',
                            Key::KeyS => 's',
                            Key::KeyT => 't',
                            Key::KeyU => 'u',
                            Key::KeyV => 'v',
                            Key::KeyW => 'w',
                            Key::KeyX => 'x',
                            Key::KeyY => 'y',
                            Key::KeyZ => 'z',
                            Key::Comma => ',',
                            Key::Dot => '.',
                            Key::SemiColon => ';',
                            Key::Quote => '\'',
                            Key::LeftBracket => '[',
                            Key::RightBracket => ']',
                            Key::BackSlash => '\\',
                            Key::Minus => '-',
                            Key::Equal => '=',
                            Key::BackQuote => '`',
                            _ => '\0',
                        }
                    }
                    if chr != '\0' {
                        if chr == 'l' && is_win && command {
                            me.lock_screen();
                            return;
                        }
                        key_event.set_chr(chr as _);
                    } else {
                        log::error!("Unknown key {:?}", evt);
                        return;
                    }
                }
                me.key_down_or_up(down, key_event, alt, ctrl, shift, command);
            };
            if let Err(error) = rdev::listen(func) {
                log::error!("rdev: {:?}", error);
            }
        });
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
    pub(super) fn save_config(&self, config: PeerConfig) {
        self.lc.write().unwrap().save_config(config);
    }

    fn save_view_style(&mut self, value: String) {
        self.lc.write().unwrap().save_view_style(value);
    }

    #[inline]
    pub(super) fn load_config(&self) -> PeerConfig {
        load_config(&self.id)
    }

    fn toggle_option(&mut self, name: String) {
        let msg = self.lc.write().unwrap().toggle_option(name.clone());
        if name == "enable-file-transfer" {
            self.send(Data::ToggleClipboardFile);
        }
        if let Some(msg) = msg {
            self.send(Data::Message(msg));
        }
    }

    fn get_toggle_option(&mut self, name: String) -> bool {
        self.lc.read().unwrap().get_toggle_option(&name)
    }

    fn is_privacy_mode_supported(&self) -> bool {
        self.lc.read().unwrap().is_privacy_mode_supported()
    }

    fn refresh_video(&mut self) {
        self.send(Data::Message(LoginConfigHandler::refresh()));
    }

    fn save_custom_image_quality(&mut self, custom_image_quality: i32) {
        let msg = self
            .lc
            .write()
            .unwrap()
            .save_custom_image_quality(custom_image_quality);
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

    fn set_write_override(
        &mut self,
        job_id: i32,
        file_num: i32,
        is_override: bool,
        remember: bool,
        is_upload: bool,
    ) -> bool {
        self.send(Data::SetConfirmOverrideFile((
            job_id,
            file_num,
            is_override,
            remember,
            is_upload,
        )));
        true
    }

    fn has_hwcodec(&self) -> bool {
        #[cfg(not(feature = "hwcodec"))]
        return false;
        #[cfg(feature = "hwcodec")]
        return true;
    }

    fn supported_hwcodec(&self) -> Value {
        #[cfg(feature = "hwcodec")]
        {
            let mut v = Value::array(0);
            let decoder = scrap::codec::Decoder::video_codec_state(&self.id);
            let mut h264 = decoder.score_h264 > 0;
            let mut h265 = decoder.score_h265 > 0;
            if let Some((encoding_264, encoding_265)) = self.lc.read().unwrap().supported_encoding {
                h264 = h264 && encoding_264;
                h265 = h265 && encoding_265;
            }
            v.push(h264);
            v.push(h265);
            v
        }
        #[cfg(not(feature = "hwcodec"))]
        {
            let mut v = Value::array(0);
            v.push(false);
            v.push(false);
            v
        }
    }

    fn change_prefer_codec(&self) {
        let msg = self.lc.write().unwrap().change_prefer_codec();
        self.send(Data::Message(msg));
    }

    fn t(&self, name: String) -> String {
        crate::client::translate(name)
    }

    fn get_audit_server(&self) -> String {
        if self.lc.read().unwrap().conn_id <= 0
            || LocalConfig::get_option("access_token").is_empty()
        {
            return "".to_owned();
        }
        crate::get_audit_server(
            Config::get_option("api-server"),
            Config::get_option("custom-rendezvous-server"),
        )
    }

    fn send_note(&self, note: String) {
        let url = self.get_audit_server();
        let id = self.id.clone();
        let conn_id = self.lc.read().unwrap().conn_id;
        std::thread::spawn(move || {
            send_note(url, id, conn_id, note);
        });
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
            for (k, mut v) in close_state {
                if k == "remote_dir" {
                    v = self.lc.read().unwrap().get_all_remote_dir(v);
                }
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

    fn set_option(&self, k: String, v: String) {
        self.lc.write().unwrap().set_option(k, v);
    }

    fn input_os_password(&mut self, pass: String, activate: bool) {
        input_os_password(pass, activate, self.clone());
    }

    fn save_close_state(&self, k: String, v: String) {
        self.write().unwrap().close_state.insert(k, v);
    }

    fn get_chatbox(&mut self) -> String {
        #[cfg(feature = "inline")]
        return super::inline::get_chatbox();
        #[cfg(not(feature = "inline"))]
        return "".to_owned();
    }

    fn get_icon(&mut self) -> String {
        crate::get_icon()
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
        println!("reconnecting");
        let cloned = self.clone();
        let mut lock = self.write().unwrap();
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

    fn login(&mut self, password: String, remember: bool) {
        self.send(Data::Login((password, remember)));
    }

    fn new_rdp(&mut self) {
        self.send(Data::NewRDP);
    }

    fn enter(&mut self) {
        #[cfg(windows)]
        crate::platform::windows::stop_system_key_propagate(true);
        IS_IN.store(true, Ordering::SeqCst);
    }

    fn leave(&mut self) {
        #[cfg(windows)]
        crate::platform::windows::stop_system_key_propagate(false);
        IS_IN.store(false, Ordering::SeqCst);
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
        #[allow(unused_mut)]
        let mut command = command;
        #[cfg(windows)]
        {
            if !command && crate::platform::windows::get_win_key_state() {
                command = true;
            }
        }

        send_mouse(mask, x, y, alt, ctrl, shift, command, self);
        // on macos, ctrl + left button down = right button down, up won't emit, so we need to
        // emit up myself if peer is not macos
        // to-do: how about ctrl + left from win to macos
        if cfg!(target_os = "macos") {
            let buttons = mask >> 3;
            let evt_type = mask & 0x7;
            if buttons == 1 && evt_type == 1 && ctrl && self.peer_platform() != "Mac OS" {
                self.send_mouse((1 << 3 | 2) as _, x, y, alt, ctrl, shift, command);
            }
        }
    }

    fn set_cursor_data(&mut self, cd: CursorData) {
        let mut colors = hbb_common::compress::decompress(&cd.colors);
        if colors.iter().filter(|x| **x != 0).next().is_none() {
            log::info!("Fix transparent");
            // somehow all 0 images shows black rect, here is a workaround
            colors[3] = 1;
        }
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
               for <= 0xFF, best practice should only avoid those not on keyboard, but
               for now, we have no way to test, so avoid <= 0xFF totally
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
                    65515 => key_event.set_control_key(ControlKey::Meta),
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
                Some(key_event::Union::Chr(chr)) => {
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
            let mut key_event = KeyEvent::new();
            key_event.set_control_key(ControlKey::CtrlAltDel);
            self.key_down_or_up(1, key_event, false, false, false, false);
        } else {
            let mut key_event = KeyEvent::new();
            key_event.set_control_key(ControlKey::Delete);
            self.key_down_or_up(3, key_event, true, true, false, false);
        }
    }

    fn lock_screen(&mut self) {
        let mut key_event = KeyEvent::new();
        key_event.set_control_key(ControlKey::LockScreen);
        self.key_down_or_up(1, key_event, false, false, false, false);
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
        evt: KeyEvent,
        alt: bool,
        ctrl: bool,
        shift: bool,
        command: bool,
    ) {
        let mut key_event = evt;

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
        if get_key_state(enigo::Key::CapsLock) {
            key_event.modifiers.push(ControlKey::CapsLock.into());
        }
        if self.peer_platform() != "Mac OS" {
            if get_key_state(enigo::Key::NumLock) && common::valid_for_numlock(&key_event) {
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
    fn call2(&self, func: &str, args: &[Value]) {
        let r = self.read().unwrap();
        if let Some(ref e) = r.element {
            allow_err!(e.call_method(func, &super::value_crash_workaround(args)[..]));
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
    key: &str,
    token: &str,
) {
    handler.lc.write().unwrap().port_forward = (remote_host, remote_port);
    if let Err(err) = crate::port_forward::listen(
        handler.id.clone(),
        port,
        handler.clone(),
        receiver,
        key,
        token,
    )
    .await
    {
        handler.on_error(&format!("Failed to listen on {}: {}", port, err));
    }
    log::info!("port forward (:{}) exit", port);
}

#[tokio::main(flavor = "current_thread")]
async fn io_loop(handler: Handler) {
    let (sender, mut receiver) = mpsc::unbounded_channel::<Data>();
    handler.write().unwrap().sender = Some(sender.clone());
    let mut options = crate::ipc::get_options_async().await;
    let mut key = options.remove("key").unwrap_or("".to_owned());
    let token = LocalConfig::get_option("access_token");
    if key.is_empty() {
        key = crate::platform::get_license_key();
    }
    if handler.is_port_forward() {
        if handler.is_rdp() {
            let port = handler
                .get_option("rdp_port".to_owned())
                .parse::<i32>()
                .unwrap_or(3389);
            std::env::set_var(
                "rdp_username",
                handler.get_option("rdp_username".to_owned()),
            );
            std::env::set_var(
                "rdp_password",
                handler.get_option("rdp_password".to_owned()),
            );
            log::info!("Remote rdp port: {}", port);
            start_one_port_forward(handler, 0, "".to_owned(), port, receiver, &key, &token).await;
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
            let port = handler.args[0].parse::<i32>().unwrap_or(0);
            if handler.args.len() != 3
                || handler.args[2].parse::<i32>().unwrap_or(0) <= 0
                || port <= 0
            {
                handler.on_error("Invalid arguments, usage:<br><br> rustdesk --port-forward remote-id listen-port remote-host remote-port");
            }
            let remote_host = handler.args[1].clone();
            let remote_port = handler.args[2].parse::<i32>().unwrap_or(0);
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
        }
        return;
    }
    let frame_count = Arc::new(AtomicUsize::new(0));
    let frame_count_cl = frame_count.clone();
    let (video_sender, audio_sender) = start_video_audio_threads(move |data: &[u8]| {
        frame_count_cl.fetch_add(1, Ordering::Relaxed);
        VIDEO
            .lock()
            .unwrap()
            .as_mut()
            .map(|v| v.render_frame(data).ok());
    });

    let mut remote = Remote {
        handler,
        video_sender,
        audio_sender,
        receiver,
        sender,
        old_clipboard: Default::default(),
        read_jobs: Vec::new(),
        write_jobs: Vec::new(),
        remove_jobs: Default::default(),
        timer: time::interval(SEC30),
        last_update_jobs_status: (Instant::now(), Default::default()),
        first_frame: false,
        #[cfg(windows)]
        clipboard_file_context: None,
        data_count: Arc::new(AtomicUsize::new(0)),
        frame_count,
        video_format: CodecFormat::Unknown,
    };
    remote.io_loop(&key, &token).await;
    remote.sync_jobs_status_to_local().await;
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

    pub fn _gen_meta(&self) -> RemoveJobMeta {
        RemoveJobMeta {
            path: self.path.clone(),
            is_remote: self.is_remote,
            no_confirm: self.no_confirm,
        }
    }
}

struct Remote {
    handler: Handler,
    video_sender: MediaSender,
    audio_sender: MediaSender,
    receiver: mpsc::UnboundedReceiver<Data>,
    sender: mpsc::UnboundedSender<Data>,
    old_clipboard: Arc<Mutex<String>>,
    read_jobs: Vec<fs::TransferJob>,
    write_jobs: Vec<fs::TransferJob>,
    remove_jobs: HashMap<i32, RemoveJob>,
    timer: Interval,
    last_update_jobs_status: (Instant, HashMap<i32, u64>),
    first_frame: bool,
    #[cfg(windows)]
    clipboard_file_context: Option<Box<CliprdrClientContext>>,
    data_count: Arc<AtomicUsize>,
    frame_count: Arc<AtomicUsize>,
    video_format: CodecFormat,
}

impl Remote {
    async fn io_loop(&mut self, key: &str, token: &str) {
        let stop_clipboard = self.start_clipboard();
        let mut last_recv_time = Instant::now();
        let conn_type = if self.handler.is_file_transfer() {
            ConnType::FILE_TRANSFER
        } else {
            ConnType::default()
        };
        match Client::start(&self.handler.id, key, token, conn_type).await {
            Ok((mut peer, direct)) => {
                SERVER_KEYBOARD_ENABLED.store(true, Ordering::SeqCst);
                SERVER_CLIPBOARD_ENABLED.store(true, Ordering::SeqCst);
                SERVER_FILE_TRANSFER_ENABLED.store(true, Ordering::SeqCst);
                self.handler
                    .call("setConnectionType", &make_args!(peer.is_secured(), direct));

                // just build for now
                #[cfg(not(windows))]
                let (_tx_holder, mut rx_clip_client) = mpsc::unbounded_channel::<i32>();
                #[cfg(windows)]
                let mut rx_clip_client = get_rx_clip_client().lock().await;

                let mut status_timer = time::interval(Duration::new(1, 0));

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
                                        self.data_count.fetch_add(bytes.len(), Ordering::Relaxed);
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
                        _msg = rx_clip_client.recv() => {
                            #[cfg(windows)]
                            match _msg {
                                Some((_, clip)) => {
                                    allow_err!(peer.send(&clip_2_msg(clip)).await);
                                }
                                None => {
                                    // unreachable!()
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
                        _ = status_timer.tick() => {
                            let speed = self.data_count.swap(0, Ordering::Relaxed);
                            let speed = format!("{:.2}kB/s", speed as f32 / 1024 as f32);
                            let fps = self.frame_count.swap(0, Ordering::Relaxed) as _;
                            self.handler.update_quality_status(QualityStatus {
                                speed:Some(speed),
                                fps:Some(fps),
                                ..Default::default()
                            });
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
        SERVER_KEYBOARD_ENABLED.store(false, Ordering::SeqCst);
        SERVER_CLIPBOARD_ENABLED.store(false, Ordering::SeqCst);
        SERVER_FILE_TRANSFER_ENABLED.store(false, Ordering::SeqCst);
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

    async fn load_last_jobs(&mut self) {
        log::info!("start load last jobs");
        self.handler.call("clearAllJobs", &make_args!());
        let pc = self.handler.load_config();
        if pc.transfer.write_jobs.is_empty() && pc.transfer.read_jobs.is_empty() {
            // no last jobs
            return;
        }
        // TODO: can add a confirm dialog
        let mut cnt = 1;
        for job_str in pc.transfer.read_jobs.iter() {
            let job: Result<TransferJobMeta, serde_json::Error> = serde_json::from_str(&job_str);
            if let Ok(job) = job {
                self.handler.call(
                    "addJob",
                    &make_args!(
                        cnt,
                        job.to.clone(),
                        job.remote.clone(),
                        job.file_num,
                        job.show_hidden,
                        false
                    ),
                );
                cnt += 1;
                println!("restore read_job: {:?}", job);
            }
        }
        for job_str in pc.transfer.write_jobs.iter() {
            let job: Result<TransferJobMeta, serde_json::Error> = serde_json::from_str(&job_str);
            if let Ok(job) = job {
                self.handler.call(
                    "addJob",
                    &make_args!(
                        cnt,
                        job.remote.clone(),
                        job.to.clone(),
                        job.file_num,
                        job.show_hidden,
                        true
                    ),
                );
                cnt += 1;
                println!("restore write_job: {:?}", job);
            }
        }
        self.handler.call("updateTransferList", &make_args!());
    }

    async fn handle_msg_from_ui(&mut self, data: Data, peer: &mut Stream) -> bool {
        // log::info!("new msg from ui, {}",data);
        match data {
            Data::Close => {
                let mut misc = Misc::new();
                misc.set_close_reason("".to_owned());
                let mut msg = Message::new();
                msg.set_misc(misc);
                allow_err!(peer.send(&msg).await);
                return false;
            }
            Data::Login((password, remember)) => {
                self.handler
                    .handle_login_from_ui(password, remember, peer)
                    .await;
            }
            Data::ToggleClipboardFile => {
                self.check_clipboard_file_context();
            }
            Data::Message(msg) => {
                allow_err!(peer.send(&msg).await);
            }
            Data::SendFiles((id, path, to, file_num, include_hidden, is_remote)) => {
                log::info!("send files, is remote {}", is_remote);
                let od = can_enable_overwrite_detection(self.handler.lc.read().unwrap().version);
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
                            let m = make_fd(job.id(), job.files(), true);
                            self.handler.call("updateFolderFiles", &make_args!(m));
                            #[cfg(not(windows))]
                            let files = job.files().clone();
                            #[cfg(windows)]
                            let mut files = job.files().clone();
                            #[cfg(windows)]
                            if self.handler.peer_platform() != "Windows" {
                                // peer is not windows, need transform \ to /
                                fs::transform_windows_path(&mut files);
                            }
                            self.read_jobs.push(job);
                            self.timer = time::interval(MILLI1);
                            allow_err!(peer.send(&fs::new_receive(id, to, file_num, files)).await);
                        }
                    }
                }
            }
            Data::AddJob((id, path, to, file_num, include_hidden, is_remote)) => {
                let od = can_enable_overwrite_detection(self.handler.lc.read().unwrap().version);
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
                            let m = make_fd(job.id(), job.files(), true);
                            self.handler.call("updateFolderFiles", &make_args!(m));
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
                        allow_err!(peer.send(&msg).await);
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

    async fn sync_jobs_status_to_local(&mut self) -> bool {
        log::info!("sync transfer job status");
        let mut config: PeerConfig = self.handler.load_config();
        let mut transfer_metas = TransferSerde::default();
        for job in self.read_jobs.iter() {
            let json_str = serde_json::to_string(&job.gen_meta()).unwrap_or_default();
            transfer_metas.read_jobs.push(json_str);
        }
        for job in self.write_jobs.iter() {
            let json_str = serde_json::to_string(&job.gen_meta()).unwrap_or_default();
            transfer_metas.write_jobs.push(json_str);
        }
        log::info!("meta: {:?}", transfer_metas);
        config.transfer = transfer_metas;
        self.handler.save_config(config);
        true
    }

    async fn handle_msg_from_peer(&mut self, data: &[u8], peer: &mut Stream) -> bool {
        if let Ok(msg_in) = Message::parse_from_bytes(&data) {
            match msg_in.union {
                Some(message::Union::VideoFrame(vf)) => {
                    if !self.first_frame {
                        self.first_frame = true;
                        self.handler.call2("closeSuccess", &make_args!());
                        self.handler.call("adaptSize", &make_args!());
                    }
                    let incomming_format = CodecFormat::from(&vf);
                    if self.video_format != incomming_format {
                        self.video_format = incomming_format.clone();
                        self.handler.update_quality_status(QualityStatus {
                            codec_format: Some(incomming_format),
                            ..Default::default()
                        })
                    };
                    self.video_sender.send(MediaData::VideoFrame(vf)).ok();
                }
                Some(message::Union::Hash(hash)) => {
                    self.handler.handle_hash(hash, peer).await;
                }
                Some(message::Union::LoginResponse(lr)) => match lr.union {
                    Some(login_response::Union::Error(err)) => {
                        if !self.handler.handle_login_error(&err) {
                            return false;
                        }
                    }
                    Some(login_response::Union::PeerInfo(pi)) => {
                        self.handler.handle_peer_info(pi);
                        self.check_clipboard_file_context();
                        if !(self.handler.is_file_transfer()
                            || self.handler.is_port_forward()
                            || !SERVER_CLIPBOARD_ENABLED.load(Ordering::SeqCst)
                            || !SERVER_KEYBOARD_ENABLED.load(Ordering::SeqCst)
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

                        if self.handler.is_file_transfer() {
                            self.load_last_jobs().await;
                        }
                    }
                    _ => {}
                },
                Some(message::Union::CursorData(cd)) => {
                    self.handler.set_cursor_data(cd);
                }
                Some(message::Union::CursorId(id)) => {
                    self.handler.set_cursor_id(id.to_string());
                }
                Some(message::Union::CursorPosition(cp)) => {
                    self.handler.set_cursor_position(cp);
                }
                Some(message::Union::Clipboard(cb)) => {
                    if !self.handler.lc.read().unwrap().disable_clipboard {
                        update_clipboard(cb, Some(&self.old_clipboard));
                    }
                }
                #[cfg(windows)]
                Some(message::Union::Cliprdr(clip)) => {
                    if !self.handler.lc.read().unwrap().disable_clipboard {
                        if let Some(context) = &mut self.clipboard_file_context {
                            if let Some(clip) = msg_2_clip(clip) {
                                server_clip_file(context, 0, clip);
                            }
                        }
                    }
                }
                Some(message::Union::FileResponse(fr)) => {
                    match fr.union {
                        Some(file_response::Union::Dir(fd)) => {
                            #[cfg(windows)]
                            let entries = fd.entries.to_vec();
                            #[cfg(not(windows))]
                            let mut entries = fd.entries.to_vec();
                            #[cfg(not(windows))]
                            {
                                if self.handler.peer_platform() == "Windows" {
                                    fs::transform_windows_path(&mut entries);
                                }
                            }
                            let mut m = make_fd(fd.id, &entries, fd.id > 0);
                            if fd.id <= 0 {
                                m.set_item("path", fd.path);
                            }
                            self.handler.call("updateFolderFiles", &make_args!(m));
                            if let Some(job) = fs::get_job(fd.id, &mut self.write_jobs) {
                                log::info!("job set_files: {:?}", entries);
                                job.set_files(entries);
                            } else if let Some(job) = self.remove_jobs.get_mut(&fd.id) {
                                job.files = entries;
                            }
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
                                            self.handler.call(
                                                "overrideFileConfirm",
                                                &make_args!(
                                                    digest.id,
                                                    digest.file_num,
                                                    read_path,
                                                    true
                                                ),
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
                                                    allow_err!(peer.send(&msg).await);
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
                                                        allow_err!(peer.send(&msg).await);
                                                    } else {
                                                        self.handler.call(
                                                            "overrideFileConfirm",
                                                            &make_args!(
                                                                digest.id,
                                                                digest.file_num,
                                                                write_path,
                                                                false
                                                            ),
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
                                                    allow_err!(peer.send(&msg).await);
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
                        Some(file_response::Union::Block(block)) => {
                            log::info!(
                                "file response block, file id:{}, file num: {}",
                                block.id,
                                block.file_num
                            );
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
                        _ => {}
                    }
                }
                Some(message::Union::Misc(misc)) => match misc.union {
                    Some(misc::Union::AudioFormat(f)) => {
                        self.audio_sender.send(MediaData::AudioFormat(f)).ok();
                    }
                    Some(misc::Union::ChatMessage(c)) => {
                        self.handler.call("newMessage", &make_args!(c.text));
                    }
                    Some(misc::Union::PermissionInfo(p)) => {
                        log::info!("Change permission {:?} -> {}", p.permission, p.enabled);
                        match p.permission.enum_value_or_default() {
                            Permission::Keyboard => {
                                SERVER_KEYBOARD_ENABLED.store(p.enabled, Ordering::SeqCst);
                                self.handler
                                    .call2("setPermission", &make_args!("keyboard", p.enabled));
                            }
                            Permission::Clipboard => {
                                SERVER_CLIPBOARD_ENABLED.store(p.enabled, Ordering::SeqCst);
                                self.handler
                                    .call2("setPermission", &make_args!("clipboard", p.enabled));
                            }
                            Permission::Audio => {
                                self.handler
                                    .call2("setPermission", &make_args!("audio", p.enabled));
                            }
                            Permission::File => {
                                SERVER_FILE_TRANSFER_ENABLED.store(p.enabled, Ordering::SeqCst);
                                if !p.enabled && self.handler.is_file_transfer() {
                                    return true;
                                }
                                self.check_clipboard_file_context();
                                self.handler
                                    .call2("setPermission", &make_args!("file", p.enabled));
                            }
                        }
                    }
                    Some(misc::Union::SwitchDisplay(s)) => {
                        self.handler.call("switchDisplay", &make_args!(s.display));
                        self.video_sender.send(MediaData::Reset).ok();
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
                    Some(misc::Union::CloseReason(c)) => {
                        self.handler.msgbox("error", "Connection Error", &c);
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
                    self.handler.handle_test_delay(t, peer).await;
                }
                Some(message::Union::AudioFrame(frame)) => {
                    if !self.handler.lc.read().unwrap().disable_audio {
                        self.audio_sender.send(MediaData::AudioFrame(frame)).ok();
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
                    state.enum_value_or(back_notification::BlockInputState::StateUnknown),
                )
                .await;
            }
            Some(back_notification::Union::PrivacyModeState(state)) => {
                if !self
                    .handle_back_msg_privacy_mode(
                        state.enum_value_or(back_notification::PrivacyModeState::StateUnknown),
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
        self.handler.call("updateBlockInputState", &make_args!(on));
    }

    async fn handle_back_msg_block_input(&mut self, state: back_notification::BlockInputState) {
        match state {
            back_notification::BlockInputState::OnSucceeded => {
                self.update_block_input_state(true);
            }
            back_notification::BlockInputState::OnFailed => {
                self.handler
                    .msgbox("custom-error", "Block user input", "Failed");
                self.update_block_input_state(false);
            }
            back_notification::BlockInputState::OffSucceeded => {
                self.update_block_input_state(false);
            }
            back_notification::BlockInputState::OffFailed => {
                self.handler
                    .msgbox("custom-error", "Unblock user input", "Failed");
            }
            _ => {}
        }
    }

    #[inline(always)]
    fn update_privacy_mode(&mut self, on: bool) {
        let mut config = self.handler.load_config();
        config.privacy_mode = on;
        self.handler.save_config(config);

        self.handler.call("updatePrivacyMode", &[]);
    }

    async fn handle_back_msg_privacy_mode(
        &mut self,
        state: back_notification::PrivacyModeState,
    ) -> bool {
        match state {
            back_notification::PrivacyModeState::OnByOther => {
                self.handler.msgbox(
                    "error",
                    "Connecting...",
                    "Someone turns on privacy mode, exit",
                );
                return false;
            }
            back_notification::PrivacyModeState::NotSupported => {
                self.handler
                    .msgbox("custom-error", "Privacy mode", "Unsupported");
                self.update_privacy_mode(false);
            }
            back_notification::PrivacyModeState::OnSucceeded => {
                self.handler
                    .msgbox("custom-nocancel", "Privacy mode", "In privacy mode");
                self.update_privacy_mode(true);
            }
            back_notification::PrivacyModeState::OnFailedDenied => {
                self.handler
                    .msgbox("custom-error", "Privacy mode", "Peer denied");
                self.update_privacy_mode(false);
            }
            back_notification::PrivacyModeState::OnFailedPlugin => {
                self.handler
                    .msgbox("custom-error", "Privacy mode", "Please install plugins");
                self.update_privacy_mode(false);
            }
            back_notification::PrivacyModeState::OnFailed => {
                self.handler
                    .msgbox("custom-error", "Privacy mode", "Failed");
                self.update_privacy_mode(false);
            }
            back_notification::PrivacyModeState::OffSucceeded => {
                self.handler
                    .msgbox("custom-nocancel", "Privacy mode", "Out privacy mode");
                self.update_privacy_mode(false);
            }
            back_notification::PrivacyModeState::OffByPeer => {
                self.handler
                    .msgbox("custom-error", "Privacy mode", "Peer exit");
                self.update_privacy_mode(false);
            }
            back_notification::PrivacyModeState::OffFailed => {
                self.handler
                    .msgbox("custom-error", "Privacy mode", "Failed to turn off");
            }
            back_notification::PrivacyModeState::OffUnknown => {
                self.handler
                    .msgbox("custom-error", "Privacy mode", "Turned off");
                // log::error!("Privacy mode is turned off with unknown reason");
                self.update_privacy_mode(false);
            }
            _ => {}
        }
        true
    }

    fn check_clipboard_file_context(&mut self) {
        #[cfg(windows)]
        {
            let enabled = SERVER_FILE_TRANSFER_ENABLED.load(Ordering::SeqCst)
                && self.handler.lc.read().unwrap().enable_file_transfer;
            if enabled == self.clipboard_file_context.is_none() {
                self.clipboard_file_context = if enabled {
                    match create_clipboard_file_context(true, false) {
                        Ok(context) => {
                            log::info!("clipboard context for file transfer created.");
                            Some(context)
                        }
                        Err(err) => {
                            log::error!(
                                "Create clipboard context for file transfer: {}",
                                err.to_string()
                            );
                            None
                        }
                    }
                } else {
                    log::info!("clipboard context for file transfer destroyed.");
                    None
                };
            }
        }
    }
}

pub fn make_fd(id: i32, entries: &Vec<FileEntry>, only_count: bool) -> Value {
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
        let tmp = entry.entry_type.value();
        e.set_item("type", if tmp == 0 { 1 } else { tmp });
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
    fn send(&self, data: Data) {
        if let Some(ref sender) = self.read().unwrap().sender {
            sender.send(data).ok();
        }
    }

    fn msgbox(&self, msgtype: &str, title: &str, text: &str) {
        let retry = check_if_retry(msgtype, title, text);
        self.call2("msgbox_retry", &make_args!(msgtype, title, text, retry));
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
                self.call("updatePrivacyMode", &[]);
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
            let p = self.lc.read().unwrap().should_auto_login();
            if !p.is_empty() {
                input_os_password(p, true, self.clone());
            }
        }
        self.lc.write().unwrap().handle_peer_info(username, pi);
        self.call("updatePrivacyMode", &[]);
        self.call("updatePi", &make_args!(pi_sciter));
        if self.is_file_transfer() {
            self.call2("closeSuccess", &make_args!());
        } else if !self.is_port_forward() {
            self.msgbox("success", "Successful", "Connected, waiting for image...");
        }
        #[cfg(windows)]
        {
            let mut path = std::env::temp_dir();
            path.push(&self.id);
            let path = path.with_extension(crate::get_app_name().to_lowercase());
            std::fs::File::create(&path).ok();
            if let Some(path) = path.to_str() {
                crate::platform::windows::add_recent_document(&path);
            }
        }
        self.start_keyboard_hook();
    }

    async fn handle_hash(&mut self, hash: Hash, peer: &mut Stream) {
        handle_hash(self.lc.clone(), hash, self, peer).await;
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

impl Handler {
    fn on_error(&self, err: &str) {
        self.msgbox("error", "Error", err);
    }
}

#[tokio::main(flavor = "current_thread")]
async fn send_note(url: String, id: String, conn_id: i32, note: String) {
    let body = serde_json::json!({ "id": id, "Id": conn_id, "note": note });
    allow_err!(crate::post_request(url, body.to_string(), "").await);
}
