use crate::{
    common::{get_supported_keyboard_modes, is_keyboard_mode_supported},
    input::{MOUSE_BUTTON_LEFT, MOUSE_TYPE_DOWN, MOUSE_TYPE_UP, MOUSE_TYPE_WHEEL},
    ui_interface::use_texture_render,
};
use async_trait::async_trait;
use bytes::Bytes;
use rdev::{Event, EventType::*, KeyCode};
use std::{
    collections::HashMap,
    ffi::c_void,
    ops::{Deref, DerefMut},
    str::FromStr,
    sync::{Arc, Mutex, RwLock},
    time::SystemTime,
};
use uuid::Uuid;

#[cfg(not(feature = "flutter"))]
use hbb_common::fs;
use hbb_common::{
    allow_err,
    config::{Config, LocalConfig, PeerConfig},
    get_version_number, log,
    message_proto::*,
    rendezvous_proto::ConnType,
    tokio::{
        self,
        sync::mpsc,
        time::{Duration as TokioDuration, Instant},
    },
    Stream,
};

use crate::client::io_loop::Remote;
use crate::client::{
    check_if_retry, handle_hash, handle_login_error, handle_login_from_ui, handle_test_delay,
    input_os_password, send_mouse, send_pointer_device_event, start_video_audio_threads,
    FileManager, Key, LoginConfigHandler, QualityStatus, KEY_MAP,
};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::common::GrabState;
use crate::keyboard;
use crate::{client::Data, client::Interface};

const CHANGE_RESOLUTION_VALID_TIMEOUT_SECS: u64 = 15;

#[derive(Clone, Default)]
pub struct Session<T: InvokeUiSession> {
    pub password: String,
    pub args: Vec<String>,
    pub lc: Arc<RwLock<LoginConfigHandler>>,
    pub sender: Arc<RwLock<Option<mpsc::UnboundedSender<Data>>>>,
    pub thread: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
    pub ui_handler: T,
    pub server_keyboard_enabled: Arc<RwLock<bool>>,
    pub server_file_transfer_enabled: Arc<RwLock<bool>>,
    pub server_clipboard_enabled: Arc<RwLock<bool>>,
    pub last_change_display: Arc<Mutex<ChangeDisplayRecord>>,
    pub connection_round_state: Arc<Mutex<ConnectionRoundState>>,
}

#[derive(Clone)]
pub struct SessionPermissionConfig {
    pub lc: Arc<RwLock<LoginConfigHandler>>,
    pub server_keyboard_enabled: Arc<RwLock<bool>>,
    pub server_file_transfer_enabled: Arc<RwLock<bool>>,
    pub server_clipboard_enabled: Arc<RwLock<bool>>,
}

pub struct ChangeDisplayRecord {
    time: Instant,
    display: i32,
    width: i32,
    height: i32,
}

enum ConnectionState {
    Connecting,
    Connected,
    Disconnected,
}

/// ConnectionRoundState is used to control the reconnecting logic.
pub struct ConnectionRoundState {
    round: u32,
    state: ConnectionState,
}

impl ConnectionRoundState {
    pub fn new_round(&mut self) -> u32 {
        self.round += 1;
        self.state = ConnectionState::Connecting;
        self.round
    }

    pub fn set_connected(&mut self) {
        self.state = ConnectionState::Connected;
    }

    pub fn is_round_gt(&self, round: u32) -> bool {
        if round == u32::MAX && self.round == 0 {
            true
        } else {
            round < self.round
        }
    }

    pub fn set_disconnected(&mut self, round: u32) -> bool {
        if self.is_round_gt(round) {
            false
        } else {
            self.state = ConnectionState::Disconnected;
            true
        }
    }
}

impl Default for ConnectionRoundState {
    fn default() -> Self {
        Self {
            round: 0,
            state: ConnectionState::Connecting,
        }
    }
}

impl Default for ChangeDisplayRecord {
    fn default() -> Self {
        Self {
            time: Instant::now()
                - TokioDuration::from_secs(CHANGE_RESOLUTION_VALID_TIMEOUT_SECS + 1),
            display: 0,
            width: 0,
            height: 0,
        }
    }
}

impl ChangeDisplayRecord {
    fn new(display: i32, width: i32, height: i32) -> Self {
        Self {
            time: Instant::now(),
            display,
            width,
            height,
        }
    }

    pub fn is_the_same_record(&self, display: i32, width: i32, height: i32) -> bool {
        self.time.elapsed().as_secs() < CHANGE_RESOLUTION_VALID_TIMEOUT_SECS
            && self.display == display
            && self.width == width
            && self.height == height
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
impl SessionPermissionConfig {
    pub fn is_text_clipboard_required(&self) -> bool {
        *self.server_clipboard_enabled.read().unwrap()
            && *self.server_keyboard_enabled.read().unwrap()
            && !self.lc.read().unwrap().disable_clipboard.v
    }
}

impl<T: InvokeUiSession> Session<T> {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub fn get_permission_config(&self) -> SessionPermissionConfig {
        SessionPermissionConfig {
            lc: self.lc.clone(),
            server_keyboard_enabled: self.server_keyboard_enabled.clone(),
            server_file_transfer_enabled: self.server_file_transfer_enabled.clone(),
            server_clipboard_enabled: self.server_clipboard_enabled.clone(),
        }
    }

    pub fn is_file_transfer(&self) -> bool {
        self.lc
            .read()
            .unwrap()
            .conn_type
            .eq(&ConnType::FILE_TRANSFER)
    }

    pub fn is_port_forward(&self) -> bool {
        let conn_type = self.lc.read().unwrap().conn_type;
        conn_type == ConnType::PORT_FORWARD || conn_type == ConnType::RDP
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub fn is_rdp(&self) -> bool {
        self.lc.read().unwrap().conn_type.eq(&ConnType::RDP)
    }

    #[cfg(feature = "flutter")]
    pub fn is_multi_ui_session(&self) -> bool {
        self.ui_handler.is_multi_ui_session()
    }

    pub fn get_view_style(&self) -> String {
        self.lc.read().unwrap().view_style.clone()
    }

    pub fn get_scroll_style(&self) -> String {
        self.lc.read().unwrap().scroll_style.clone()
    }

    pub fn get_image_quality(&self) -> String {
        self.lc.read().unwrap().image_quality.clone()
    }

    pub fn get_custom_image_quality(&self) -> Vec<i32> {
        self.lc.read().unwrap().custom_image_quality.clone()
    }

    pub fn get_peer_version(&self) -> i64 {
        self.lc.read().unwrap().version.clone()
    }

    pub fn fallback_keyboard_mode(&self) -> String {
        let peer_version = self.get_peer_version();
        let platform = self.peer_platform();

        let supported_modes = get_supported_keyboard_modes(peer_version, &platform);
        if let Some(mode) = supported_modes.first() {
            return mode.to_string();
        } else {
            if self.get_peer_version() >= get_version_number("1.2.0") {
                return KeyboardMode::Map.to_string();
            } else {
                return KeyboardMode::Legacy.to_string();
            }
        }
    }

    // Caution: This function must be called after peer info is received.
    pub fn get_keyboard_mode(&self) -> String {
        let mode = self.lc.read().unwrap().keyboard_mode.clone();
        let keyboard_mode = KeyboardMode::from_str(&mode);

        // Note: peer_version is 0 before peer info is received.
        let peer_version = self.get_peer_version();
        let platform = self.peer_platform();

        // Saved keyboard mode still exists in this version.
        if let Ok(mode) = keyboard_mode {
            if is_keyboard_mode_supported(&mode, peer_version, &platform) {
                return mode.to_string();
            }
        }
        self.fallback_keyboard_mode()
    }

    pub fn save_keyboard_mode(&self, value: String) {
        self.lc.write().unwrap().save_keyboard_mode(value);
    }

    pub fn get_reverse_mouse_wheel(&self) -> String {
        self.lc.read().unwrap().reverse_mouse_wheel.clone()
    }

    pub fn get_displays_as_individual_windows(&self) -> String {
        self.lc
            .read()
            .unwrap()
            .displays_as_individual_windows
            .clone()
    }

    pub fn get_use_all_my_displays_for_the_remote_session(&self) -> String {
        self.lc
            .read()
            .unwrap()
            .use_all_my_displays_for_the_remote_session
            .clone()
    }

    pub fn save_reverse_mouse_wheel(&self, value: String) {
        self.lc.write().unwrap().save_reverse_mouse_wheel(value);
    }

    pub fn save_displays_as_individual_windows(&self, value: String) {
        self.lc
            .write()
            .unwrap()
            .save_displays_as_individual_windows(value);
    }

    pub fn save_use_all_my_displays_for_the_remote_session(&self, value: String) {
        self.lc
            .write()
            .unwrap()
            .save_use_all_my_displays_for_the_remote_session(value);
    }

    pub fn save_view_style(&self, value: String) {
        self.lc.write().unwrap().save_view_style(value);
    }

    pub fn save_scroll_style(&self, value: String) {
        self.lc.write().unwrap().save_scroll_style(value);
    }

    pub fn save_flutter_option(&self, k: String, v: String) {
        self.lc.write().unwrap().save_ui_flutter(k, v);
    }

    pub fn get_flutter_option(&self, k: String) -> String {
        self.lc.read().unwrap().get_ui_flutter(&k)
    }

    pub fn toggle_option(&self, name: String) {
        let msg = self.lc.write().unwrap().toggle_option(name.clone());
        #[cfg(not(feature = "flutter"))]
        if name == hbb_common::config::keys::OPTION_ENABLE_FILE_COPY_PASTE {
            self.send(Data::ToggleClipboardFile);
        }
        if let Some(msg) = msg {
            self.send(Data::Message(msg));
        }
    }

    pub fn toggle_privacy_mode(&self, impl_key: String, on: bool) {
        let mut misc = Misc::new();
        misc.set_toggle_privacy_mode(TogglePrivacyMode {
            impl_key,
            on,
            ..Default::default()
        });
        let mut msg_out = Message::new();
        msg_out.set_misc(misc);
        self.send(Data::Message(msg_out));
    }

    pub fn get_toggle_option(&self, name: String) -> bool {
        self.lc.read().unwrap().get_toggle_option(&name)
    }

    #[cfg(not(feature = "flutter"))]
    pub fn is_privacy_mode_supported(&self) -> bool {
        self.lc.read().unwrap().is_privacy_mode_supported()
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub fn is_text_clipboard_required(&self) -> bool {
        *self.server_clipboard_enabled.read().unwrap()
            && *self.server_keyboard_enabled.read().unwrap()
            && !self.lc.read().unwrap().disable_clipboard.v
    }

    #[cfg(feature = "flutter")]
    pub fn refresh_video(&self, display: i32) {
        if crate::common::is_support_multi_ui_session_num(self.lc.read().unwrap().version) {
            self.send(Data::Message(LoginConfigHandler::refresh_display(
                display as _,
            )));
        } else {
            self.send(Data::Message(LoginConfigHandler::refresh()));
        }
    }

    pub fn toggle_virtual_display(&self, index: i32, on: bool) {
        let mut misc = Misc::new();
        misc.set_toggle_virtual_display(ToggleVirtualDisplay {
            display: index,
            on,
            ..Default::default()
        });
        let mut msg_out = Message::new();
        msg_out.set_misc(misc);
        self.send(Data::Message(msg_out));
    }

    #[cfg(not(feature = "flutter"))]
    pub fn refresh_video(&self, _display: i32) {
        self.send(Data::Message(LoginConfigHandler::refresh()));
    }

    pub fn record_screen(&self, start: bool, display: i32, w: i32, h: i32) {
        self.send(Data::RecordScreen(
            start,
            display as usize,
            w,
            h,
            self.get_id(),
        ));
    }

    pub fn record_status(&self, status: bool) {
        let mut misc = Misc::new();
        misc.set_client_record_status(status);
        let mut msg = Message::new();
        msg.set_misc(misc);
        self.send(Data::Message(msg));
    }

    pub fn save_custom_image_quality(&self, custom_image_quality: i32) {
        let msg = self
            .lc
            .write()
            .unwrap()
            .save_custom_image_quality(custom_image_quality);
        self.send(Data::Message(msg));
    }

    pub fn save_image_quality(&self, value: String) {
        let msg = self.lc.write().unwrap().save_image_quality(value.clone());
        if let Some(msg) = msg {
            self.send(Data::Message(msg));
        }
        if value != "custom" {
            // non custom quality use 30 fps
            let msg = self.lc.write().unwrap().set_custom_fps(30, false);
            self.send(Data::Message(msg));
        }
    }

    pub fn set_custom_fps(&self, custom_fps: i32) {
        let msg = self.lc.write().unwrap().set_custom_fps(custom_fps, true);
        self.send(Data::Message(msg));
    }

    pub fn get_remember(&self) -> bool {
        self.lc.read().unwrap().remember
    }

    #[cfg(not(feature = "flutter"))]
    pub fn set_write_override(
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

    pub fn alternative_codecs(&self) -> (bool, bool, bool, bool) {
        let luid = self.lc.read().unwrap().adapter_luid;
        let mark_unsupported = self.lc.read().unwrap().mark_unsupported.clone();
        let decoder = scrap::codec::Decoder::supported_decodings(
            None,
            use_texture_render(),
            luid,
            &mark_unsupported,
        );
        let mut vp8 = decoder.ability_vp8 > 0;
        let mut av1 = decoder.ability_av1 > 0;
        let mut h264 = decoder.ability_h264 > 0;
        let mut h265 = decoder.ability_h265 > 0;
        let enc = &self.lc.read().unwrap().supported_encoding;
        vp8 = vp8 && enc.vp8;
        av1 = av1 && enc.av1;
        h264 = h264 && enc.h264;
        h265 = h265 && enc.h265;
        (vp8, av1, h264, h265)
    }

    pub fn change_prefer_codec(&self) {
        let msg = self.lc.write().unwrap().update_supported_decodings();
        self.send(Data::Message(msg));
    }

    pub fn use_texture_render_changed(&self) {
        self.send(Data::ResetDecoder(None));
        self.change_prefer_codec();
        self.send(Data::Message(LoginConfigHandler::refresh()));
    }

    pub fn restart_remote_device(&self) {
        let mut lc = self.lc.write().unwrap();
        lc.restarting_remote_device = true;
        let msg = lc.restart_remote_device();
        self.send(Data::Message(msg));
    }

    #[cfg(all(feature = "flutter", feature = "plugin_framework"))]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub fn send_plugin_request(&self, request: PluginRequest) {
        let mut misc = Misc::new();
        misc.set_plugin_request(request);
        let mut msg_out = Message::new();
        msg_out.set_misc(misc);
        self.send(Data::Message(msg_out));
    }

    pub fn get_audit_server(&self, typ: String) -> String {
        if LocalConfig::get_option("access_token").is_empty() {
            return "".to_owned();
        }
        crate::get_audit_server(
            Config::get_option("api-server"),
            Config::get_option("custom-rendezvous-server"),
            typ,
        )
    }

    pub fn send_note(&self, note: String) {
        let url = self.get_audit_server("conn".to_string());
        let id = self.get_id();
        let session_id = self.lc.read().unwrap().session_id;
        std::thread::spawn(move || {
            send_note(url, id, session_id, note);
        });
    }

    #[cfg(not(feature = "flutter"))]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub fn is_xfce(&self) -> bool {
        #[cfg(not(any(target_os = "ios")))]
        return crate::platform::is_xfce();
        #[cfg(any(target_os = "ios"))]
        false
    }

    pub fn remove_port_forward(&self, port: i32) {
        let mut config = self.load_config();
        config.port_forwards = config
            .port_forwards
            .drain(..)
            .filter(|x| x.0 != port)
            .collect();
        self.save_config(config);
        self.send(Data::RemovePortForward(port));
    }

    pub fn add_port_forward(&self, port: i32, remote_host: String, remote_port: i32) {
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

    pub fn get_option(&self, k: String) -> String {
        if k.eq("remote_dir") {
            return self.lc.read().unwrap().get_remote_dir();
        }
        self.lc.read().unwrap().get_option(&k)
    }

    pub fn set_option(&self, k: String, mut v: String) {
        let mut lc = self.lc.write().unwrap();
        if k.eq("remote_dir") {
            v = lc.get_all_remote_dir(v);
        }
        lc.set_option(k, v);
    }

    #[inline]
    pub fn load_config(&self) -> PeerConfig {
        self.lc.read().unwrap().load_config()
    }

    #[inline]
    pub(super) fn save_config(&self, config: PeerConfig) {
        self.lc.write().unwrap().save_config(config);
    }

    pub fn is_restarting_remote_device(&self) -> bool {
        self.lc.read().unwrap().restarting_remote_device
    }

    #[inline]
    pub fn peer_platform(&self) -> String {
        self.lc.read().unwrap().info.platform.clone()
    }

    pub fn get_platform(&self, is_remote: bool) -> String {
        if is_remote {
            self.peer_platform()
        } else {
            whoami::platform().to_string()
        }
    }

    pub fn get_path_sep(&self, is_remote: bool) -> &'static str {
        let p = self.get_platform(is_remote);
        if &p == crate::PLATFORM_WINDOWS {
            return "\\";
        } else {
            return "/";
        }
    }

    pub fn input_os_password(&self, pass: String, activate: bool) {
        input_os_password(pass, activate, self.clone());
    }

    #[cfg(not(feature = "flutter"))]
    pub fn get_chatbox(&self) -> String {
        #[cfg(feature = "inline")]
        return crate::ui::inline::get_chatbox();
        #[cfg(not(feature = "inline"))]
        return "".to_owned();
    }

    pub fn swap_modifier_key(&self, msg: &mut KeyEvent) {
        let allow_swap_key = self.get_toggle_option("allow_swap_key".to_string());
        if allow_swap_key {
            if let Some(key_event::Union::ControlKey(ck)) = msg.union {
                let ck = ck.enum_value_or_default();
                let ck = match ck {
                    ControlKey::Control => ControlKey::Meta,
                    ControlKey::Meta => ControlKey::Control,
                    ControlKey::RControl => ControlKey::Meta,
                    ControlKey::RWin => ControlKey::Control,
                    _ => ck,
                };
                msg.set_control_key(ck);
            }
            msg.modifiers = msg
                .modifiers
                .iter()
                .map(|ck| {
                    let ck = ck.enum_value_or_default();
                    let ck = match ck {
                        ControlKey::Control => ControlKey::Meta,
                        ControlKey::Meta => ControlKey::Control,
                        ControlKey::RControl => ControlKey::Meta,
                        ControlKey::RWin => ControlKey::Control,
                        _ => ck,
                    };
                    hbb_common::protobuf::EnumOrUnknown::new(ck)
                })
                .collect();

            let code = msg.chr();
            if code != 0 {
                let mut peer = self.peer_platform().to_lowercase();
                peer.retain(|c| !c.is_whitespace());

                let key = match peer.as_str() {
                    "windows" => {
                        let key = rdev::win_key_from_scancode(code);
                        let key = match key {
                            rdev::Key::ControlLeft => rdev::Key::MetaLeft,
                            rdev::Key::MetaLeft => rdev::Key::ControlLeft,
                            rdev::Key::ControlRight => rdev::Key::MetaLeft,
                            rdev::Key::MetaRight => rdev::Key::ControlLeft,
                            _ => key,
                        };
                        rdev::win_scancode_from_key(key).unwrap_or_default()
                    }
                    "macos" => {
                        let key = rdev::macos_key_from_code(code as _);
                        let key = match key {
                            rdev::Key::ControlLeft => rdev::Key::MetaLeft,
                            rdev::Key::MetaLeft => rdev::Key::ControlLeft,
                            rdev::Key::ControlRight => rdev::Key::MetaLeft,
                            rdev::Key::MetaRight => rdev::Key::ControlLeft,
                            _ => key,
                        };
                        rdev::macos_keycode_from_key(key).unwrap_or_default() as _
                    }
                    _ => {
                        let key = rdev::linux_key_from_code(code);
                        let key = match key {
                            rdev::Key::ControlLeft => rdev::Key::MetaLeft,
                            rdev::Key::MetaLeft => rdev::Key::ControlLeft,
                            rdev::Key::ControlRight => rdev::Key::MetaLeft,
                            rdev::Key::MetaRight => rdev::Key::ControlLeft,
                            _ => key,
                        };
                        rdev::linux_keycode_from_key(key).unwrap_or_default()
                    }
                };
                msg.set_chr(key);
            }
        }
    }

    pub fn send_key_event(&self, evt: &KeyEvent) {
        // mode: legacy(0), map(1), translate(2), auto(3)

        let mut msg = evt.clone();
        self.swap_modifier_key(&mut msg);
        let mut msg_out = Message::new();
        msg_out.set_key_event(msg);
        self.send(Data::Message(msg_out));
    }

    pub fn send_chat(&self, text: String) {
        let mut misc = Misc::new();
        misc.set_chat_message(ChatMessage {
            text,
            ..Default::default()
        });
        let mut msg_out = Message::new();
        msg_out.set_misc(misc);
        self.send(Data::Message(msg_out));
    }

    pub fn capture_displays(&self, add: Vec<i32>, sub: Vec<i32>, set: Vec<i32>) {
        let mut misc = Misc::new();
        misc.set_capture_displays(CaptureDisplays {
            add,
            sub,
            set,
            ..Default::default()
        });
        let mut msg_out = Message::new();
        msg_out.set_misc(misc);
        self.send(Data::Message(msg_out));
    }

    pub fn switch_display(&self, display: i32) {
        let (w, h) = match self.lc.read().unwrap().get_custom_resolution(display) {
            Some((w, h)) => (w, h),
            None => (0, 0),
        };

        let mut misc = Misc::new();
        misc.set_switch_display(SwitchDisplay {
            display,
            width: w,
            height: h,
            ..Default::default()
        });
        let mut msg_out = Message::new();
        msg_out.set_misc(misc);
        self.send(Data::Message(msg_out));

        if !use_texture_render() {
            self.capture_displays(vec![], vec![], vec![display]);
        }
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub fn enter(&self, keyboard_mode: String) {
        keyboard::client::change_grab_status(GrabState::Run, &keyboard_mode);
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub fn leave(&self, keyboard_mode: String) {
        keyboard::client::change_grab_status(GrabState::Wait, &keyboard_mode);
    }

    // flutter only TODO new input
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

    // flutter only TODO new input
    pub fn input_string(&self, value: &str) {
        let mut key_event = KeyEvent::new();
        key_event.set_seq(value.to_owned());
        let mut msg_out = Message::new();
        msg_out.set_key_event(key_event);
        self.send(Data::Message(msg_out));
    }

    #[cfg(any(target_os = "ios"))]
    pub fn handle_flutter_key_event(
        &self,
        _keyboard_mode: &str,
        _name: &str,
        _platform_code: i32,
        _position_code: i32,
        _lock_modes: i32,
        _down_or_up: bool,
    ) {
    }

    #[cfg(not(any(target_os = "ios")))]
    pub fn handle_flutter_key_event(
        &self,
        keyboard_mode: &str,
        _name: &str,
        platform_code: i32,
        position_code: i32,
        lock_modes: i32,
        down_or_up: bool,
    ) {
        if position_code < 0 || platform_code < 0 {
            return;
        }
        let platform_code: u32 = platform_code as _;
        let position_code: KeyCode = position_code as _;

        #[cfg(not(target_os = "windows"))]
        let key = rdev::key_from_code(position_code) as rdev::Key;
        // Windows requires special handling
        #[cfg(target_os = "windows")]
        let key = rdev::get_win_key(platform_code, position_code);

        let event_type = if down_or_up {
            KeyPress(key)
        } else {
            KeyRelease(key)
        };
        let event = Event {
            time: SystemTime::now(),
            unicode: None,
            platform_code,
            position_code: position_code as _,
            event_type,
            #[cfg(any(target_os = "windows", target_os = "macos"))]
            extra_data: 0,
        };
        keyboard::client::process_event(keyboard_mode, &event, Some(lock_modes));
    }

    // flutter only TODO new input
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
        let mut key_event = KeyEvent::new();
        match key {
            Key::Chr(chr) => {
                key_event.set_chr(chr);
            }
            Key::ControlKey(key) => {
                key_event.set_control_key(key.clone());
            }
            Key::_Raw(raw) => {
                key_event.set_chr(raw);
            }
        }

        if v == 1 {
            key_event.down = true;
        } else if v == 3 {
            key_event.press = true;
        }
        keyboard::client::legacy_modifiers(&mut key_event, alt, ctrl, shift, command);
        key_event.mode = KeyboardMode::Legacy.into();

        self.send_key_event(&key_event);
    }

    pub fn send_touch_scale(&self, scale: i32, alt: bool, ctrl: bool, shift: bool, command: bool) {
        let scale_evt = TouchScaleUpdate {
            scale,
            ..Default::default()
        };
        let mut touch_evt = TouchEvent::new();
        touch_evt.set_scale_update(scale_evt);
        let mut evt = PointerDeviceEvent::new();
        evt.set_touch_event(touch_evt);
        send_pointer_device_event(evt, alt, ctrl, shift, command, self);
    }

    pub fn send_touch_pan_event(
        &self,
        event: &str,
        x: i32,
        y: i32,
        alt: bool,
        ctrl: bool,
        shift: bool,
        command: bool,
    ) {
        let mut touch_evt = TouchEvent::new();
        match event {
            "pan_start" => {
                touch_evt.set_pan_start(TouchPanStart {
                    x,
                    y,
                    ..Default::default()
                });
            }
            "pan_update" => {
                let (x, y) = self.get_scroll_xy((x, y));
                touch_evt.set_pan_update(TouchPanUpdate {
                    x,
                    y,
                    ..Default::default()
                });
            }
            "pan_end" => {
                touch_evt.set_pan_end(TouchPanEnd {
                    x,
                    y,
                    ..Default::default()
                });
            }
            _ => {
                log::warn!("unknown touch pan event: {}", event);
                return;
            }
        };
        let mut evt = PointerDeviceEvent::new();
        evt.set_touch_event(touch_evt);
        send_pointer_device_event(evt, alt, ctrl, shift, command, self);
    }

    #[inline]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    fn is_scroll_reverse_mode(&self) -> bool {
        self.lc.read().unwrap().reverse_mouse_wheel.eq("Y")
    }

    #[inline]
    fn get_scroll_xy(&self, xy: (i32, i32)) -> (i32, i32) {
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        if self.is_scroll_reverse_mode() {
            return (-xy.0, -xy.1);
        }
        xy
    }

    pub fn send_mouse(
        &self,
        mut mask: i32,
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

        let (x, y) = if mask == MOUSE_TYPE_WHEEL {
            self.get_scroll_xy((x, y))
        } else {
            (x, y)
        };

        // #[cfg(not(any(target_os = "android", target_os = "ios")))]
        let (alt, ctrl, shift, command) =
            keyboard::client::get_modifiers_state(alt, ctrl, shift, command);

        use crate::input::*;
        let is_left = (mask & (MOUSE_BUTTON_LEFT << 3)) > 0;
        let is_right = (mask & (MOUSE_BUTTON_RIGHT << 3)) > 0;
        if is_left ^ is_right {
            let swap_lr = self.get_toggle_option("swap-left-right-mouse".to_string());
            if swap_lr {
                if is_left {
                    mask = (mask & (!(MOUSE_BUTTON_LEFT << 3))) | (MOUSE_BUTTON_RIGHT << 3);
                } else {
                    mask = (mask & (!(MOUSE_BUTTON_RIGHT << 3))) | (MOUSE_BUTTON_LEFT << 3);
                }
            }
        }

        send_mouse(mask, x, y, alt, ctrl, shift, command, self);
        // on macos, ctrl + left button down = right button down, up won't emit, so we need to
        // emit up myself if peer is not macos
        // to-do: how about ctrl + left from win to macos
        if cfg!(target_os = "macos") {
            let buttons = mask >> 3;
            let evt_type = mask & 0x7;
            if buttons == MOUSE_BUTTON_LEFT
                && evt_type == MOUSE_TYPE_DOWN
                && ctrl
                && self.peer_platform() != "Mac OS"
            {
                self.send_mouse(
                    (MOUSE_BUTTON_LEFT << 3 | MOUSE_TYPE_UP) as _,
                    x,
                    y,
                    alt,
                    ctrl,
                    shift,
                    command,
                );
            }
        }
    }

    pub fn reconnect(&self, force_relay: bool) {
        // 1. If current session is connecting, do not reconnect.
        // 2. If the connection is established, send `Data::Close`.
        // 3. If the connection is disconnected, do nothing.
        let mut connection_round_state_lock = self.connection_round_state.lock().unwrap();
        if self.thread.lock().unwrap().is_some() {
            match connection_round_state_lock.state {
                ConnectionState::Connecting => return,
                ConnectionState::Connected => self.send(Data::Close),
                ConnectionState::Disconnected => {}
            }
        }
        let round = connection_round_state_lock.new_round();
        drop(connection_round_state_lock);

        let cloned = self.clone();
        // override only if true
        if true == force_relay {
            self.lc.write().unwrap().force_relay = true;
        }
        self.lc.write().unwrap().peer_info = None;
        let mut lock = self.thread.lock().unwrap();
        // No need to join the previous thread, because it will exit automatically.
        // And the previous thread will not change important states.
        *lock = Some(std::thread::spawn(move || {
            io_loop(cloned, round);
        }));
    }

    #[cfg(not(feature = "flutter"))]
    pub fn get_icon_path(&self, file_type: i32, ext: String) -> String {
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

    pub fn login(
        &self,
        os_username: String,
        os_password: String,
        password: String,
        remember: bool,
    ) {
        self.send(Data::Login((os_username, os_password, password, remember)));
    }

    pub fn send2fa(&self, code: String) {
        let mut msg_out = Message::new();
        msg_out.set_auth_2fa(Auth2FA {
            code,
            ..Default::default()
        });
        self.send(Data::Message(msg_out));
    }

    pub fn new_rdp(&self) {
        self.send(Data::NewRDP);
    }

    pub fn close(&self) {
        self.send(Data::Close);
    }

    pub fn load_last_jobs(&self) {
        self.clear_all_jobs();
        let pc = self.load_config();
        if pc.transfer.write_jobs.is_empty() && pc.transfer.read_jobs.is_empty() {
            // no last jobs
            return;
        }
        // TODO: can add a confirm dialog
        let mut cnt = 1;
        for job_str in pc.transfer.read_jobs.iter() {
            if !job_str.is_empty() {
                self.load_last_job(cnt, job_str);
                cnt += 1;
                log::info!("restore read_job: {:?}", job_str);
            }
        }
        for job_str in pc.transfer.write_jobs.iter() {
            if !job_str.is_empty() {
                self.load_last_job(cnt, job_str);
                cnt += 1;
                log::info!("restore write_job: {:?}", job_str);
            }
        }
        self.update_transfer_list();
    }

    pub fn elevate_direct(&self) {
        self.send(Data::ElevateDirect);
    }

    pub fn elevate_with_logon(&self, username: String, password: String) {
        self.send(Data::ElevateWithLogon(username, password));
    }

    #[cfg(any(target_os = "ios"))]
    pub fn switch_sides(&self) {}

    #[cfg(not(any(target_os = "ios")))]
    #[tokio::main(flavor = "current_thread")]
    pub async fn switch_sides(&self) {
        match crate::ipc::connect(1000, "").await {
            Ok(mut conn) => {
                if conn
                    .send(&crate::ipc::Data::SwitchSidesRequest(self.get_id()))
                    .await
                    .is_ok()
                {
                    if let Ok(Some(data)) = conn.next_timeout(1000).await {
                        match data {
                            crate::ipc::Data::SwitchSidesRequest(str_uuid) => {
                                if let Ok(uuid) = Uuid::from_str(&str_uuid) {
                                    let mut misc = Misc::new();
                                    misc.set_switch_sides_request(SwitchSidesRequest {
                                        uuid: Bytes::from(uuid.as_bytes().to_vec()),
                                        ..Default::default()
                                    });
                                    let mut msg_out = Message::new();
                                    msg_out.set_misc(misc);
                                    self.send(Data::Message(msg_out));
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            Err(err) => {
                log::info!("server not started (will try to start): {}", err);
            }
        }
    }

    fn set_custom_resolution(&self, display: &SwitchDisplay) {
        if display.width == display.original_resolution.width
            && display.height == display.original_resolution.height
        {
            self.lc
                .write()
                .unwrap()
                .set_custom_resolution(display.display, None);
        } else {
            let last_change_display = self.last_change_display.lock().unwrap();
            if last_change_display.display == display.display {
                let wh = if last_change_display.is_the_same_record(
                    display.display,
                    display.width,
                    display.height,
                ) {
                    Some((display.width, display.height))
                } else {
                    // display origin is changed, or some other events.
                    None
                };
                self.lc
                    .write()
                    .unwrap()
                    .set_custom_resolution(display.display, wh);
            }
        }
    }

    #[inline]
    pub fn handle_peer_switch_display(&self, display: &SwitchDisplay) {
        self.ui_handler.switch_display(display);
        self.set_custom_resolution(display);
    }

    #[inline]
    pub fn change_resolution(&self, display: i32, width: i32, height: i32) {
        *self.last_change_display.lock().unwrap() =
            ChangeDisplayRecord::new(display, width, height);
        self.do_change_resolution(display, width, height);
    }

    #[inline]
    fn try_change_init_resolution(&self, display: i32) {
        if let Some((w, h)) = self.lc.read().unwrap().get_custom_resolution(display) {
            self.change_resolution(display, w, h);
        }
    }

    fn do_change_resolution(&self, display: i32, width: i32, height: i32) {
        let mut misc = Misc::new();
        let resolution = Resolution {
            width,
            height,
            ..Default::default()
        };
        if crate::common::is_support_multi_ui_session_num(self.lc.read().unwrap().version) {
            misc.set_change_display_resolution(DisplayResolution {
                display,
                resolution: Some(resolution).into(),
                ..Default::default()
            });
        } else {
            misc.set_change_resolution(resolution);
        }
        let mut msg = Message::new();
        msg.set_misc(misc);
        self.send(Data::Message(msg));
    }

    #[inline]
    pub fn request_voice_call(&self) {
        #[cfg(target_os = "linux")]
        std::thread::spawn(crate::ipc::start_pa);
        self.send(Data::NewVoiceCall);
    }

    #[inline]
    pub fn close_voice_call(&self) {
        self.send(Data::CloseVoiceCall);
    }

    pub fn send_selected_session_id(&self, sid: String) {
        if let Ok(sid) = sid.parse::<u32>() {
            self.lc.write().unwrap().selected_windows_session_id = Some(sid);
            let mut misc = Misc::new();
            misc.set_selected_sid(sid);
            let mut msg = Message::new();
            msg.set_misc(misc);
            self.send(Data::Message(msg));
            let pi = self.lc.read().unwrap().peer_info.clone();
            if let Some(pi) = pi {
                if pi.windows_sessions.current_sid == sid {
                    if self.is_file_transfer() {
                        if pi.username.is_empty() {
                            self.on_error(
                                "No active console user logged on, please connect and logon first.",
                            );
                        } else {
                            #[cfg(not(feature = "flutter"))]
                            {
                                let remote_dir = self.get_option("remote_dir".to_string());
                                let show_hidden =
                                    !self.get_option("remote_show_hidden".to_string()).is_empty();
                                self.read_remote_dir(remote_dir, show_hidden);
                            }
                        }
                    } else {
                        self.msgbox(
                            "success",
                            "Successful",
                            "Connected, waiting for image...",
                            "",
                        );
                    }
                }
            }
        } else {
            log::error!("selected invalid sid: {}", sid);
        }
    }

    #[inline]
    pub fn request_init_msgs(&self, display: usize) {
        self.send_message_query(display);
    }

    fn send_message_query(&self, display: usize) {
        let mut misc = Misc::new();
        misc.set_message_query(MessageQuery {
            switch_display: display as _,
            ..Default::default()
        });
        let mut msg = Message::new();
        msg.set_misc(misc);
        self.send(Data::Message(msg));
    }
}

pub trait InvokeUiSession: Send + Sync + Clone + 'static + Sized + Default {
    fn set_cursor_data(&self, cd: CursorData);
    fn set_cursor_id(&self, id: String);
    fn set_cursor_position(&self, cp: CursorPosition);
    fn set_display(&self, x: i32, y: i32, w: i32, h: i32, cursor_embedded: bool);
    fn switch_display(&self, display: &SwitchDisplay);
    fn set_peer_info(&self, peer_info: &PeerInfo); // flutter
    fn set_displays(&self, displays: &Vec<DisplayInfo>);
    fn set_platform_additions(&self, data: &str);
    fn on_connected(&self, conn_type: ConnType);
    fn update_privacy_mode(&self);
    fn set_permission(&self, name: &str, value: bool);
    fn close_success(&self);
    fn update_quality_status(&self, qs: QualityStatus);
    fn set_connection_type(&self, is_secured: bool, direct: bool);
    fn set_fingerprint(&self, fingerprint: String);
    fn job_error(&self, id: i32, err: String, file_num: i32);
    fn job_done(&self, id: i32, file_num: i32);
    fn clear_all_jobs(&self);
    fn new_message(&self, msg: String);
    fn update_transfer_list(&self);
    fn load_last_job(&self, cnt: i32, job_json: &str);
    fn update_folder_files(
        &self,
        id: i32,
        entries: &Vec<FileEntry>,
        path: String,
        is_local: bool,
        only_count: bool,
    );
    fn confirm_delete_files(&self, id: i32, i: i32, name: String);
    fn override_file_confirm(
        &self,
        id: i32,
        file_num: i32,
        to: String,
        is_upload: bool,
        is_identical: bool,
    );
    fn update_block_input_state(&self, on: bool);
    fn job_progress(&self, id: i32, file_num: i32, speed: f64, finished_size: f64);
    fn adapt_size(&self);
    fn on_rgba(&self, display: usize, rgba: &mut scrap::ImageRgb);
    fn msgbox(&self, msgtype: &str, title: &str, text: &str, link: &str, retry: bool);
    #[cfg(any(target_os = "android", target_os = "ios"))]
    fn clipboard(&self, content: String);
    fn cancel_msgbox(&self, tag: &str);
    fn switch_back(&self, id: &str);
    fn portable_service_running(&self, running: bool);
    fn on_voice_call_started(&self);
    fn on_voice_call_closed(&self, reason: &str);
    fn on_voice_call_waiting(&self);
    fn on_voice_call_incoming(&self);
    fn get_rgba(&self, display: usize) -> *const u8;
    fn next_rgba(&self, display: usize);
    #[cfg(all(feature = "vram", feature = "flutter"))]
    fn on_texture(&self, display: usize, texture: *mut c_void);
    fn set_multiple_windows_session(&self, sessions: Vec<WindowsSession>);
    fn set_current_display(&self, disp_idx: i32);
    #[cfg(feature = "flutter")]
    fn is_multi_ui_session(&self) -> bool;
}

impl<T: InvokeUiSession> Deref for Session<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.ui_handler
    }
}

impl<T: InvokeUiSession> DerefMut for Session<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ui_handler
    }
}

impl<T: InvokeUiSession> FileManager for Session<T> {}

#[async_trait]
impl<T: InvokeUiSession> Interface for Session<T> {
    fn get_lch(&self) -> Arc<RwLock<LoginConfigHandler>> {
        return self.lc.clone();
    }

    fn send(&self, data: Data) {
        if let Some(sender) = self.sender.read().unwrap().as_ref() {
            sender.send(data).ok();
        }
    }

    fn msgbox(&self, msgtype: &str, title: &str, text: &str, link: &str) {
        let direct = self.lc.read().unwrap().direct;
        let received = self.lc.read().unwrap().received;
        let retry_for_relay = direct == Some(true) && !received;
        let retry = check_if_retry(msgtype, title, text, retry_for_relay);
        self.ui_handler.msgbox(msgtype, title, text, link, retry);
    }

    fn handle_login_error(&self, err: &str) -> bool {
        handle_login_error(self.lc.clone(), err, self)
    }

    fn set_multiple_windows_session(&self, sessions: Vec<WindowsSession>) {
        self.ui_handler.set_multiple_windows_session(sessions);
    }

    fn handle_peer_info(&self, mut pi: PeerInfo) {
        log::debug!("handle_peer_info :{:?}", pi);
        self.lc.write().unwrap().peer_info = Some(pi.clone());
        if pi.current_display as usize >= pi.displays.len() {
            pi.current_display = 0;
        }
        if get_version_number(&pi.version) < get_version_number("1.1.10") {
            self.set_permission("restart", false);
        }
        if self.is_file_transfer() {
            if pi.username.is_empty() && pi.windows_sessions.sessions.is_empty() {
                self.on_error("No active console user logged on, please connect and logon first.");
                return;
            }
        } else if !self.is_port_forward() {
            if pi.displays.is_empty() {
                self.lc.write().unwrap().handle_peer_info(&pi);
                self.update_privacy_mode();
                self.msgbox("error", "Remote Error", "No Displays", "");
                return;
            }
            self.try_change_init_resolution(pi.current_display);
            let p = self.lc.read().unwrap().should_auto_login();
            if !p.is_empty() {
                input_os_password(p, true, self.clone());
            }
            let current = &pi.displays[pi.current_display as usize];
            self.set_display(
                current.x,
                current.y,
                current.width,
                current.height,
                current.cursor_embedded,
            );
        }
        self.update_privacy_mode();
        // Save recent peers, then push event to flutter. So flutter can refresh peer page.
        self.lc.write().unwrap().handle_peer_info(&pi);
        self.set_peer_info(&pi);
        if self.is_file_transfer() {
            self.close_success();
        } else if !self.is_port_forward() {
            self.msgbox(
                "success",
                "Successful",
                "Connected, waiting for image...",
                "",
            );
        }
        self.on_connected(self.lc.read().unwrap().conn_type);
        #[cfg(windows)]
        {
            let mut path = std::env::temp_dir();
            path.push(self.get_id());
            let path = path.with_extension(crate::get_app_name().to_lowercase());
            std::fs::File::create(&path).ok();
            if let Some(path) = path.to_str() {
                crate::platform::windows::add_recent_document(&path);
            }
        }
        if !pi.windows_sessions.sessions.is_empty() {
            let selected = self
                .lc
                .read()
                .unwrap()
                .selected_windows_session_id
                .to_owned();
            if selected == Some(pi.windows_sessions.current_sid) {
                self.send_selected_session_id(pi.windows_sessions.current_sid.to_string());
            } else {
                self.set_multiple_windows_session(pi.windows_sessions.sessions.clone());
            }
        }
    }

    async fn handle_hash(&self, pass: &str, hash: Hash, peer: &mut Stream) {
        handle_hash(self.lc.clone(), pass, hash, self, peer).await;
    }

    async fn handle_login_from_ui(
        &self,
        os_username: String,
        os_password: String,
        password: String,
        remember: bool,
        peer: &mut Stream,
    ) {
        handle_login_from_ui(
            self.lc.clone(),
            os_username,
            os_password,
            password,
            remember,
            peer,
        )
        .await;
    }

    async fn handle_test_delay(&self, t: TestDelay, peer: &mut Stream) {
        if !t.from_client {
            self.update_quality_status(QualityStatus {
                delay: Some(t.last_delay as _),
                target_bitrate: Some(t.target_bitrate as _),
                ..Default::default()
            });
            handle_test_delay(t, peer).await;
        }
    }

    fn swap_modifier_mouse(&self, msg: &mut hbb_common::protos::message::MouseEvent) {
        let allow_swap_key = self.get_toggle_option("allow_swap_key".to_string());
        if allow_swap_key {
            msg.modifiers = msg
                .modifiers
                .iter()
                .map(|ck| {
                    let ck = ck.enum_value_or_default();
                    let ck = match ck {
                        ControlKey::Control => ControlKey::Meta,
                        ControlKey::Meta => ControlKey::Control,
                        ControlKey::RControl => ControlKey::Meta,
                        ControlKey::RWin => ControlKey::Control,
                        _ => ck,
                    };
                    hbb_common::protobuf::EnumOrUnknown::new(ck)
                })
                .collect();
        };
    }
}

impl<T: InvokeUiSession> Session<T> {
    pub fn lock_screen(&self) {
        self.send_key_event(&crate::keyboard::client::event_lock_screen());
    }
    pub fn ctrl_alt_del(&self) {
        self.send_key_event(&crate::keyboard::client::event_ctrl_alt_del());
    }
}

#[tokio::main(flavor = "current_thread")]
pub async fn io_loop<T: InvokeUiSession>(handler: Session<T>, round: u32) {
    // It is ok to call this function multiple times.
    #[cfg(any(
        target_os = "windows",
        all(
            any(target_os = "linux", target_os = "macos"),
            feature = "unix-file-copy-paste"
        )
    ))]
    if !handler.is_file_transfer() && !handler.is_port_forward() {
        clipboard::ContextSend::enable(true);
    }

    #[cfg(any(target_os = "android", target_os = "ios"))]
    let (sender, receiver) = mpsc::unbounded_channel::<Data>();
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let (sender, mut receiver) = mpsc::unbounded_channel::<Data>();
    *handler.sender.write().unwrap() = Some(sender.clone());
    let token = LocalConfig::get_option("access_token");
    let key = crate::get_key(false).await;
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
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
    let frame_count_map: Arc<RwLock<HashMap<usize, usize>>> = Default::default();
    let frame_count_map_cl = frame_count_map.clone();
    let ui_handler = handler.ui_handler.clone();
    let (video_sender, audio_sender, video_queue_map, decode_fps_map, chroma) =
        start_video_audio_threads(
            handler.clone(),
            move |display: usize,
                  data: &mut scrap::ImageRgb,
                  _texture: *mut c_void,
                  pixelbuffer: bool| {
                let mut write_lock = frame_count_map_cl.write().unwrap();
                let count = write_lock.get(&display).unwrap_or(&0) + 1;
                write_lock.insert(display, count);
                drop(write_lock);
                if pixelbuffer {
                    ui_handler.on_rgba(display, data);
                } else {
                    #[cfg(all(feature = "vram", feature = "flutter"))]
                    ui_handler.on_texture(display, _texture);
                }
            },
        );

    let mut remote = Remote::new(
        handler,
        video_queue_map,
        video_sender,
        audio_sender,
        receiver,
        sender,
        frame_count_map,
        decode_fps_map,
        chroma,
    );
    remote.io_loop(&key, &token, round).await;
    remote.sync_jobs_status_to_local().await;
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
async fn start_one_port_forward<T: InvokeUiSession>(
    handler: Session<T>,
    port: i32,
    remote_host: String,
    remote_port: i32,
    receiver: mpsc::UnboundedReceiver<Data>,
    key: &str,
    token: &str,
) {
    if let Err(err) = crate::port_forward::listen(
        handler.get_id(),
        handler.password.clone(),
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

#[tokio::main(flavor = "current_thread")]
async fn send_note(url: String, id: String, sid: u64, note: String) {
    let body = serde_json::json!({ "id": id, "session_id": sid, "note": note });
    allow_err!(crate::post_request(url, body.to_string(), "").await);
}
