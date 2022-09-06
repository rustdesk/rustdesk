#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::client::get_key_state;
use crate::client::io_loop::Remote;
use crate::client::{
    check_if_retry, handle_hash, handle_login_from_ui, handle_test_delay, input_os_password,
    load_config, send_mouse, start_video_audio_threads, FileManager, Key, LoginConfigHandler,
    QualityStatus, KEY_MAP, SERVER_KEYBOARD_ENABLED,
};
use crate::common;
use crate::{client::Data, client::Interface};
use async_trait::async_trait;

use hbb_common::config::{Config, LocalConfig, PeerConfig};

use hbb_common::rendezvous_proto::ConnType;
use hbb_common::tokio::{self, sync::mpsc};

use hbb_common::{allow_err, message_proto::*};
use hbb_common::{fs, get_version_number, log, Stream};
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};

/// IS_IN KEYBOARD_HOOKED sciter only
pub static IS_IN: AtomicBool = AtomicBool::new(false);
static KEYBOARD_HOOKED: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
static mut IS_ALT_GR: bool = false;

#[derive(Clone, Default)]
pub struct Session<T: InvokeUiSession> {
    pub cmd: String,
    pub id: String,
    pub password: String,
    pub args: Vec<String>,
    pub lc: Arc<RwLock<LoginConfigHandler>>,
    pub sender: Arc<RwLock<Option<mpsc::UnboundedSender<Data>>>>,
    pub thread: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
    pub ui_handler: T,
}

impl<T: InvokeUiSession> Session<T> {
    pub fn get_view_style(&self) -> String {
        self.lc.read().unwrap().view_style.clone()
    }

    pub fn get_image_quality(&self) -> String {
        self.lc.read().unwrap().image_quality.clone()
    }

    /// Get custom image quality.
    pub fn get_custom_image_quality(&self) -> Vec<i32> {
        self.lc.read().unwrap().custom_image_quality.clone()
    }

    pub fn save_view_style(&mut self, value: String) {
        self.lc.write().unwrap().save_view_style(value);
    }

    pub fn toggle_option(&mut self, name: String) {
        let msg = self.lc.write().unwrap().toggle_option(name.clone());
        if name == "enable-file-transfer" {
            self.send(Data::ToggleClipboardFile);
        }
        if let Some(msg) = msg {
            self.send(Data::Message(msg));
        }
    }

    pub fn get_toggle_option(&self, name: String) -> bool {
        self.lc.read().unwrap().get_toggle_option(&name)
    }

    pub fn is_privacy_mode_supported(&self) -> bool {
        self.lc.read().unwrap().is_privacy_mode_supported()
    }

    pub fn refresh_video(&self) {
        self.send(Data::Message(LoginConfigHandler::refresh()));
    }

    pub fn save_custom_image_quality(&mut self, custom_image_quality: i32) {
        let msg = self
            .lc
            .write()
            .unwrap()
            .save_custom_image_quality(custom_image_quality);
        self.send(Data::Message(msg));
    }

    pub fn save_image_quality(&mut self, value: String) {
        let msg = self.lc.write().unwrap().save_image_quality(value);
        if let Some(msg) = msg {
            self.send(Data::Message(msg));
        }
    }

    pub fn get_remember(&self) -> bool {
        self.lc.read().unwrap().remember
    }

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

    pub fn has_hwcodec(&self) -> bool {
        #[cfg(not(feature = "hwcodec"))]
        return false;
        #[cfg(feature = "hwcodec")]
        return true;
    }

    pub fn change_prefer_codec(&self) {
        let msg = self.lc.write().unwrap().change_prefer_codec();
        self.send(Data::Message(msg));
    }

    pub fn restart_remote_device(&self) {
        let mut lc = self.lc.write().unwrap();
        lc.restarting_remote_device = true;
        let msg = lc.restart_remote_device();
        self.send(Data::Message(msg));
    }

    pub fn get_audit_server(&self) -> String {
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

    pub fn send_note(&self, note: String) {
        let url = self.get_audit_server();
        let id = self.id.clone();
        let conn_id = self.lc.read().unwrap().conn_id;
        std::thread::spawn(move || {
            send_note(url, id, conn_id, note);
        });
    }

    pub fn is_xfce(&self) -> bool {
        crate::platform::is_xfce()
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
        self.save_config(config);
        self.send(Data::AddPortForward(pf));
    }

    pub fn get_id(&self) -> String {
        self.id.clone()
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
        load_config(&self.id)
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

    pub fn ctrl_alt_del(&self) {
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

    pub fn key_down_or_up(
        &self,
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
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        if get_key_state(enigo::Key::CapsLock) {
            key_event.modifiers.push(ControlKey::CapsLock.into());
        }
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
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

    pub fn get_platform(&self, is_remote: bool) -> String {
        if is_remote {
            self.peer_platform()
        } else {
            whoami::platform().to_string()
        }
    }

    pub fn get_path_sep(&self, is_remote: bool) -> &'static str {
        let p = self.get_platform(is_remote);
        if &p == "Windows" {
            return "\\";
        } else {
            return "/";
        }
    }

    pub fn input_os_password(&self, pass: String, activate: bool) {
        input_os_password(pass, activate, self.clone());
    }

    pub fn get_chatbox(&self) -> String {
        #[cfg(feature = "inline")]
        return super::inline::get_chatbox();
        #[cfg(not(feature = "inline"))]
        return "".to_owned();
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

    pub fn switch_display(&self, display: i32) {
        let mut misc = Misc::new();
        misc.set_switch_display(SwitchDisplay {
            display,
            ..Default::default()
        });
        let mut msg_out = Message::new();
        msg_out.set_misc(misc);
        self.send(Data::Message(msg_out));
    }

    pub fn lock_screen(&self) {
        let mut key_event = KeyEvent::new();
        key_event.set_control_key(ControlKey::LockScreen);
        self.key_down_or_up(1, key_event, false, false, false, false);
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
                if raw > 'z' as u32 || raw < 'a' as u32 {
                    key_event.set_unicode(raw);
                    // TODO
                    // if down_or_up == 0 {
                    //     // ignore up, avoiding trigger twice
                    //     return;
                    // }
                    // down_or_up = 1; // if press, turn into down for avoiding trigger twice on server side
                } else {
                    // to make ctrl+c works on windows
                    key_event.set_chr(raw);
                }
            }
        }

        self.key_down_or_up(v, key_event, alt, ctrl, shift, command);
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

    pub fn reconnect(&self) {
        self.send(Data::Close);
        let cloned = self.clone();
        let mut lock = self.thread.lock().unwrap();
        lock.take().map(|t| t.join());
        *lock = Some(std::thread::spawn(move || {
            io_loop(cloned);
        }));
    }

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

    pub fn login(&self, password: String, remember: bool) {
        self.send(Data::Login((password, remember)));
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
}

pub trait InvokeUiSession: Send + Sync + Clone + 'static + Sized + Default {
    fn set_cursor_data(&self, cd: CursorData);
    fn set_cursor_id(&self, id: String);
    fn set_cursor_position(&self, cp: CursorPosition);
    fn set_display(&self, x: i32, y: i32, w: i32, h: i32);
    fn switch_display(&self, display: &SwitchDisplay);
    fn set_peer_info(&self, peer_info: &PeerInfo); // flutter
    fn update_privacy_mode(&self);
    fn set_permission(&self, name: &str, value: bool);
    fn close_success(&self);
    fn update_quality_status(&self, qs: QualityStatus);
    fn set_connection_type(&self, is_secured: bool, direct: bool);
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
    fn override_file_confirm(&self, id: i32, file_num: i32, to: String, is_upload: bool);
    fn update_block_input_state(&self, on: bool);
    fn job_progress(&self, id: i32, file_num: i32, speed: f64, finished_size: f64);
    fn adapt_size(&self);
    fn on_rgba(&self, data: &[u8]);
    fn msgbox(&self, msgtype: &str, title: &str, text: &str, retry: bool);
    #[cfg(any(target_os = "android", target_os = "ios"))]
    fn clipboard(&self, content: String);
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
    fn send(&self, data: Data) {
        if let Some(sender) = self.sender.read().unwrap().as_ref() {
            sender.send(data).ok();
        }
    }

    fn is_file_transfer(&self) -> bool {
        self.lc
            .read()
            .unwrap()
            .conn_type
            .eq(&ConnType::FILE_TRANSFER)
    }

    fn is_port_forward(&self) -> bool {
        self.lc
            .read()
            .unwrap()
            .conn_type
            .eq(&ConnType::PORT_FORWARD)
    }

    fn is_rdp(&self) -> bool {
        self.lc.read().unwrap().conn_type.eq(&ConnType::RDP)
    }

    fn msgbox(&self, msgtype: &str, title: &str, text: &str) {
        let retry = check_if_retry(msgtype, title, text);
        self.ui_handler.msgbox(msgtype, title, text, retry);
    }

    fn handle_login_error(&mut self, err: &str) -> bool {
        self.lc.write().unwrap().handle_login_error(err, self)
    }

    fn handle_peer_info(&mut self, mut pi: PeerInfo) {
        pi.username = self.lc.read().unwrap().get_username(&pi);
        if pi.current_display as usize >= pi.displays.len() {
            pi.current_display = 0;
        }
        if get_version_number(&pi.version) < get_version_number("1.1.10") {
            self.set_permission("restart", false);
        }
        if self.is_file_transfer() {
            if pi.username.is_empty() {
                self.on_error("No active console user logged on, please connect and logon first.");
                return;
            }
        } else if !self.is_port_forward() {
            if pi.displays.is_empty() {
                self.lc.write().unwrap().handle_peer_info(&pi);
                self.update_privacy_mode();
                self.msgbox("error", "Remote Error", "No Display");
                return;
            }
            let p = self.lc.read().unwrap().should_auto_login();
            if !p.is_empty() {
                input_os_password(p, true, self.clone());
            }
            let current = &pi.displays[pi.current_display as usize];
            self.set_display(current.x, current.y, current.width, current.height);
        }
        self.update_privacy_mode();
        // Save recent peers, then push event to flutter. So flutter can refresh peer page.
        self.lc.write().unwrap().handle_peer_info(&pi);
        self.set_peer_info(&pi);
        if self.is_file_transfer() {
            self.close_success();
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
        // TODO use event callbcak
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        self.start_keyboard_hook();
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
}

// TODO use event callbcak
// sciter only
#[cfg(not(any(target_os = "android", target_os = "ios")))]
impl<T: InvokeUiSession> Session<T> {
    fn start_keyboard_hook(&self) {
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
                me.key_down_or_up(down, key_event, alt, ctrl, shift, command); // TODO
            };
            if let Err(error) = rdev::listen(func) {
                log::error!("rdev: {:?}", error);
            }
        });
    }
}

#[tokio::main(flavor = "current_thread")]
pub async fn io_loop<T: InvokeUiSession>(handler: Session<T>) {
    let (sender, mut receiver) = mpsc::unbounded_channel::<Data>();
    *handler.sender.write().unwrap() = Some(sender.clone());
    let mut options = crate::ipc::get_options_async().await;
    let mut key = options.remove("key").unwrap_or("".to_owned());
    let token = LocalConfig::get_option("access_token");
    if key.is_empty() {
        key = crate::platform::get_license_key();
    }
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
    let frame_count = Arc::new(AtomicUsize::new(0));
    let frame_count_cl = frame_count.clone();
    let ui_handler = handler.ui_handler.clone();
    let (video_sender, audio_sender) = start_video_audio_threads(move |data: &[u8]| {
        frame_count_cl.fetch_add(1, Ordering::Relaxed);
        ui_handler.on_rgba(data);
    });

    let mut remote = Remote::new(
        handler,
        video_sender,
        audio_sender,
        receiver,
        sender,
        frame_count,
    );
    remote.io_loop(&key, &token).await;
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
        handler.id.clone(),
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
async fn send_note(url: String, id: String, conn_id: i32, note: String) {
    let body = serde_json::json!({ "id": id, "Id": conn_id, "note": note });
    allow_err!(crate::post_request(url, body.to_string(), "").await);
}
