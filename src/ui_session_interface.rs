use crate::client::io_loop::Remote;
use crate::client::{
    check_if_retry, handle_hash, handle_login_error, handle_login_from_ui, handle_test_delay,
    input_os_password, load_config, send_mouse, start_video_audio_threads, FileManager, Key,
    LoginConfigHandler, QualityStatus, KEY_MAP,
};
use crate::common::GrabState;
use crate::keyboard;
use crate::{client::Data, client::Interface};
use async_trait::async_trait;
use hbb_common::config::{Config, LocalConfig, PeerConfig};
use hbb_common::rendezvous_proto::ConnType;
use hbb_common::tokio::{self, sync::mpsc};
use hbb_common::{allow_err, message_proto::*};
use hbb_common::{fs, get_version_number, log, Stream};
use rdev::{Event, EventType::*};
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};
pub static IS_IN: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Default)]
pub struct Session<T: InvokeUiSession> {
    pub id: String,
    pub password: String,
    pub args: Vec<String>,
    pub lc: Arc<RwLock<LoginConfigHandler>>,
    pub sender: Arc<RwLock<Option<mpsc::UnboundedSender<Data>>>>,
    pub thread: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
    pub ui_handler: T,
}

impl<T: InvokeUiSession> Session<T> {
    pub fn is_file_transfer(&self) -> bool {
        self.lc
            .read()
            .unwrap()
            .conn_type
            .eq(&ConnType::FILE_TRANSFER)
    }

    pub fn is_port_forward(&self) -> bool {
        self.lc
            .read()
            .unwrap()
            .conn_type
            .eq(&ConnType::PORT_FORWARD)
    }

    pub fn is_rdp(&self) -> bool {
        self.lc.read().unwrap().conn_type.eq(&ConnType::RDP)
    }

    pub fn set_connection_info(&mut self, direct: bool, received: bool) {
        let mut lc = self.lc.write().unwrap();
        lc.direct = Some(direct);
        lc.received = received;
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

    pub fn get_keyboard_mode(&self) -> String {
        self.lc.read().unwrap().keyboard_mode.clone()
    }

    pub fn save_keyboard_mode(&mut self, value: String) {
        self.lc.write().unwrap().save_keyboard_mode(value);
    }

    pub fn save_view_style(&mut self, value: String) {
        self.lc.write().unwrap().save_view_style(value);
    }

    pub fn save_scroll_style(&mut self, value: String) {
        self.lc.write().unwrap().save_scroll_style(value);
    }

    pub fn save_flutter_config(&mut self, k: String, v: String) {
        self.lc.write().unwrap().save_ui_flutter(k, v);
    }

    pub fn get_flutter_config(&self, k: String) -> String {
        self.lc.write().unwrap().get_ui_flutter(&k)
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

    pub fn record_screen(&self, start: bool, w: i32, h: i32) {
        self.send(Data::RecordScreen(start, w, h, self.id.clone()));
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

    pub fn set_custom_fps(&mut self, custom_fps: i32) {
        let msg = self.lc.write().unwrap().set_custom_fps(custom_fps);
        self.send(Data::Message(msg));
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

    pub fn supported_hwcodec(&self) -> (bool, bool) {
        #[cfg(any(feature = "hwcodec", feature = "mediacodec"))]
        {
            let decoder = scrap::codec::Decoder::video_codec_state(&self.id);
            let mut h264 = decoder.score_h264 > 0;
            let mut h265 = decoder.score_h265 > 0;
            let (encoding_264, encoding_265) = self
                .lc
                .read()
                .unwrap()
                .supported_encoding
                .unwrap_or_default();
            h264 = h264 && encoding_264;
            h265 = h265 && encoding_265;
            return (h264, h265);
        }
        #[allow(unreachable_code)]
        (false, false)
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

    pub fn get_audit_server(&self, typ: String) -> String {
        if self.lc.read().unwrap().conn_id <= 0
            || LocalConfig::get_option("access_token").is_empty()
        {
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
        return crate::ui::inline::get_chatbox();
        #[cfg(not(feature = "inline"))]
        return "".to_owned();
    }

    pub fn send_key_event(&self, evt: &KeyEvent) {
        // mode: legacy(0), map(1), translate(2), auto(3)
        let mut msg_out = Message::new();
        msg_out.set_key_event(evt.clone());
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

    pub fn enter(&self) {
        IS_IN.store(true, Ordering::SeqCst);
        keyboard::client::change_grab_status(GrabState::Run);
    }

    pub fn leave(&self) {
        IS_IN.store(false, Ordering::SeqCst);
        keyboard::client::change_grab_status(GrabState::Wait);
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

    pub fn handle_flutter_key_event(
        &self,
        name: &str,
        keycode: i32,
        scancode: i32,
        lock_modes: i32,
        down_or_up: bool,
    ) {
        if scancode < 0 || keycode < 0 {
            return;
        }
        let keycode: u32 = keycode as u32;
        let scancode: u32 = scancode as u32;

        #[cfg(not(target_os = "windows"))]
        let key = rdev::key_from_code(keycode) as rdev::Key;
        // Windows requires special handling
        #[cfg(target_os = "windows")]
        let key = rdev::get_win_key(keycode, scancode);

        let event_type = if down_or_up {
            KeyPress(key)
        } else {
            KeyRelease(key)
        };
        let event = Event {
            time: std::time::SystemTime::now(),
            name: Option::Some(name.to_owned()),
            code: keycode as _,
            scan_code: scancode as _,
            event_type: event_type,
        };
        keyboard::client::process_event(&event, Some(lock_modes));
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

        // #[cfg(not(any(target_os = "android", target_os = "ios")))]
        let (alt, ctrl, shift, command) =
            keyboard::client::get_modifiers_state(alt, ctrl, shift, command);

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
    fn set_display(&self, x: i32, y: i32, w: i32, h: i32, cursor_embedded: bool);
    fn switch_display(&self, display: &SwitchDisplay);
    fn set_peer_info(&self, peer_info: &PeerInfo); // flutter
    fn on_connected(&self, conn_type: ConnType);
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
    fn msgbox(&self, msgtype: &str, title: &str, text: &str, link: &str, retry: bool);
    #[cfg(any(target_os = "android", target_os = "ios"))]
    fn clipboard(&self, content: String);
    fn cancel_msgbox(&self, tag: &str);
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
    fn get_login_config_handler(&self) -> Arc<RwLock<LoginConfigHandler>> {
        return self.lc.clone();
    }

    fn send(&self, data: Data) {
        if let Some(sender) = self.sender.read().unwrap().as_ref() {
            sender.send(data).ok();
        }
    }

    fn msgbox(&self, msgtype: &str, title: &str, text: &str, link: &str) {
        let direct = self.lc.read().unwrap().direct.unwrap_or_default();
        let received = self.lc.read().unwrap().received;
        let retry_for_relay = direct && !received;
        let retry = check_if_retry(msgtype, title, text, retry_for_relay);
        self.ui_handler.msgbox(msgtype, title, text, link, retry);
    }

    fn handle_login_error(&mut self, err: &str) -> bool {
        handle_login_error(self.lc.clone(), err, self)
    }

    fn handle_peer_info(&mut self, mut pi: PeerInfo) {
        log::debug!("handle_peer_info :{:?}", pi);
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
                self.msgbox("error", "Remote Error", "No Display", "");
                return;
            }
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
            path.push(&self.id);
            let path = path.with_extension(crate::get_app_name().to_lowercase());
            std::fs::File::create(&path).ok();
            if let Some(path) = path.to_str() {
                crate::platform::windows::add_recent_document(&path);
            }
        }
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

impl<T: InvokeUiSession> Session<T> {
    pub fn lock_screen(&self) {
        crate::keyboard::client::lock_screen();
    }
    pub fn ctrl_alt_del(&self) {
        crate::keyboard::client::ctrl_alt_del();
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
