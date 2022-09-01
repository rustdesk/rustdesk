use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
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
use enigo::{self};
use hbb_common::{allow_err, log, message_proto::*};

#[cfg(windows)]
use crate::clipboard_file::*;
use crate::{
    client::*,
    ui_session_interface::{InvokeUi, Session},
};

type Video = AssetPtr<video_destination>;

lazy_static::lazy_static! {
    static ref VIDEO: Arc<Mutex<Option<Video>>> = Default::default();
}

static IS_IN: AtomicBool = AtomicBool::new(false);
static KEYBOARD_HOOKED: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
static mut IS_ALT_GR: bool = false;

/// SciterHandler
/// * element
/// * thread  TODO check if flutter need
/// * close_state  for file path when close
#[derive(Clone, Default)]
pub struct SciterHandler {
    element: Arc<Mutex<Option<Element>>>,
    close_state: HashMap<String, String>,
}

impl SciterHandler {
    #[inline]
    fn call(&self, func: &str, args: &[Value]) {
        if let Some(ref e) = self.element.lock().unwrap().as_ref() {
            allow_err!(e.call_method(func, args));
        }
    }

    #[inline]
    fn call2(&self, func: &str, args: &[Value]) {
        if let Some(ref e) = self.element.lock().unwrap().as_ref() {
            allow_err!(e.call_method(func, &super::value_crash_workaround(args)[..]));
        }
    }
}

impl InvokeUi for SciterHandler {
    fn set_cursor_data(&self, cd: CursorData) {
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

    fn set_display(&self, x: i32, y: i32, w: i32, h: i32) {
        self.call("setDisplay", &make_args!(x, y, w, h));
        VIDEO.lock().unwrap().as_mut().map(|v| {
            v.stop_streaming().ok();
            let ok = v.start_streaming((w, h), COLOR_SPACE::Rgb32, None);
            log::info!("[video] reinitialized: {:?}", ok);
        });
    }

    fn update_privacy_mode(&self) {
        self.call("updatePrivacyMode", &[]);
    }

    fn set_permission(&self, name: &str, value: bool) {
        self.call2("setPermission", &make_args!(name, value));
    }

    fn update_pi(&self, pi: PeerInfo) {} // TODO dup flutter

    fn close_success(&self) {
        self.call2("closeSuccess", &make_args!());
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

    fn set_cursor_id(&self, id: String) {
        self.call("setCursorId", &make_args!(id));
    }

    fn set_cursor_position(&self, cp: CursorPosition) {
        self.call("setCursorPosition", &make_args!(cp.x, cp.y));
    }

    fn set_connection_type(&self, is_secured: bool, direct: bool) {
        self.call("setConnectionType", &make_args!(is_secured, direct));
    }

    fn job_error(&self, id: i32, err: String, file_num: i32) {
        self.call("jobError", &make_args!(id, err, file_num));
    }

    fn job_done(&self, id: i32, file_num: i32) {
        self.call("jobDone", &make_args!(id, file_num));
    }

    fn clear_all_jobs(&self) {
        self.call("clearAllJobs", &make_args!());
    }

    fn add_job(
        &self,
        id: i32,
        path: String,
        to: String,
        file_num: i32,
        show_hidden: bool,
        is_remote: bool,
    ) {
        todo!()
    }

    fn update_transfer_list(&self) {
        self.call("updateTransferList", &make_args!());
    }

    fn confirm_delete_files(&self, id: i32, i: i32, name: String) {
        self.call("confirmDeleteFiles", &make_args!(id, i, name));
    }

    fn override_file_confirm(&self, id: i32, file_num: i32, to: String, is_upload: bool) {
        self.call(
            "overrideFileConfirm",
            &make_args!(id, file_num, to, is_upload),
        );
    }

    fn job_progress(&self, id: i32, file_num: i32, speed: f64, finished_size: f64) {
        self.call(
            "jobProgress",
            &make_args!(id, file_num, speed, finished_size),
        );
    }

    fn adapt_size(&self) {
        self.call("adaptSize", &make_args!());
    }

    fn on_rgba(&self, data: &[u8]) {
        VIDEO
            .lock()
            .unwrap()
            .as_mut()
            .map(|v| v.render_frame(data).ok());
    }

    fn set_peer_info(
        &self,
        username: &str,
        hostname: &str,
        platform: &str,
        sas_enabled: bool,
        displays: &Vec<HashMap<&str, i32>>,
        version: &str,
        current_display: usize,
        is_file_transfer: bool,
    ) {
    }

    fn msgbox(&self, msgtype: &str, title: &str, text: &str, retry: bool) {
        self.call2("msgbox_retry", &make_args!(msgtype, title, text, retry));
    }

    fn new_message(&self, msg: String) {
        self.call("newMessage", &make_args!(msg));
    }

    fn switch_display(&self, display: &SwitchDisplay) {
        self.call("switchDisplay", &make_args!(display.display));
    }

    fn update_block_input_state(&self, on: bool) {
        self.call("updateBlockInputState", &make_args!(on));
    }
}

pub struct SciterSession(Session<SciterHandler>);

impl Deref for SciterSession {
    type Target = Session<SciterHandler>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for SciterSession {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl sciter::EventHandler for SciterSession {
    fn get_subscription(&mut self) -> Option<EVENT_GROUPS> {
        Some(EVENT_GROUPS::HANDLE_BEHAVIOR_EVENT)
    }

    fn attached(&mut self, root: HELEMENT) {
        *self.element.lock().unwrap() = Some(Element::from(root));
    }

    fn detached(&mut self, _root: HELEMENT) {
        *self.element.lock().unwrap() = None;
        self.sender.write().unwrap().take().map(|sender| {
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
        fn remove_dir_all(i32, String, bool, bool);
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
        fn restart_remote_device();
    }
}

impl SciterSession {
    pub fn new(cmd: String, id: String, password: String, args: Vec<String>) -> Self {
        let session: Session<SciterHandler> = Session {
            cmd,
            id: id.clone(),
            password: password.clone(),
            args,
            ..Default::default()
        };
        session.lc.write().unwrap().initialize(
            id,
            session.is_file_transfer(),
            session.is_port_forward(),
        );

        Self(session)
    }

    // TODO
    fn start_keyboard_hook(&'static self) {
        if self.is_port_forward() || self.is_file_transfer() {
            return;
        }
        if KEYBOARD_HOOKED.swap(true, Ordering::SeqCst) {
            return;
        }
        log::info!("keyboard hooked");
        let me = self.clone();
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
                            // me.ctrl_alt_del(); // TODO
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
                            // me.lock_screen(); // TODO
                            return;
                        }
                        key_event.set_chr(chr as _);
                    } else {
                        log::error!("Unknown key {:?}", evt);
                        return;
                    }
                }
                // me.key_down_or_up(down, key_event, alt, ctrl, shift, command); // TODO
            };
            if let Err(error) = rdev::listen(func) {
                log::error!("rdev: {:?}", error);
            }
        });
    }

    // TODO
    fn get_custom_image_quality(&mut self) -> Value {
        let mut v = Value::array(0);
        for x in self.lc.read().unwrap().custom_image_quality.iter() {
            v.push(x);
        }
        v
    }

    // TODO
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

    // TODO
    fn save_size(&mut self, x: i32, y: i32, w: i32, h: i32) {
        let size = (x, y, w, h);
        let mut config = self.load_config();
        if self.is_file_transfer() {
            let close_state = self.close_state.clone();
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

    fn get_default_pi(&mut self) -> Value {
        let mut pi = Value::map();
        let info = self.lc.read().unwrap().info.clone();
        pi.set_item("username", info.username.clone());
        pi.set_item("hostname", info.hostname.clone());
        pi.set_item("platform", info.platform.clone());
        pi
    }

    // close_state sciter only
    fn save_close_state(&mut self, k: String, v: String) {
        self.close_state.insert(k, v);
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

    fn transfer_file(&mut self) {
        let id = self.get_id();
        let args = vec!["--file-transfer", &id, &self.password];
        if let Err(err) = crate::run_me(args) {
            log::error!("Failed to spawn file transfer: {}", err);
        }
    }

    fn tunnel(&mut self) {
        let id = self.get_id();
        let args = vec!["--port-forward", &id, &self.password];
        if let Err(err) = crate::run_me(args) {
            log::error!("Failed to spawn IP tunneling: {}", err);
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
