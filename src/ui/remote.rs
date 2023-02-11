use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    sync::{Arc, Mutex},
};

use sciter::{
    dom::{
        Element,
        event::{BEHAVIOR_EVENTS, EVENT_GROUPS, EventReason, PHASE_MASK}, HELEMENT,
    },
    make_args,
    Value,
    video::{AssetPtr, COLOR_SPACE, video_destination},
};

use hbb_common::{
    allow_err, fs::TransferJobMeta, log, message_proto::*, rendezvous_proto::ConnType,
};

use crate::{
    client::*,
    ui_interface::has_hwcodec,
    ui_session_interface::{InvokeUiSession, Session},
};

type Video = AssetPtr<video_destination>;

lazy_static::lazy_static! {
    static ref VIDEO: Arc<Mutex<Option<Video>>> = Default::default();
}

/// SciterHandler
/// * element
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

impl InvokeUiSession for SciterHandler {
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

    fn set_display(&self, x: i32, y: i32, w: i32, h: i32, cursor_embedded: bool) {
        self.call("setDisplay", &make_args!(x, y, w, h, cursor_embedded));
        // https://sciter.com/forums/topic/color_spaceiyuv-crash
        // Nothing spectacular in decoder – done on CPU side.
        // So if you can do BGRA translation on your side – the better.
        // BGRA is used as internal image format so it will not require additional transformations.
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

    fn load_last_job(&self, cnt: i32, job_json: &str) {
        let job: Result<TransferJobMeta, serde_json::Error> = serde_json::from_str(job_json);
        if let Ok(job) = job {
            let path;
            let to;
            if job.is_remote {
                path = job.remote.clone();
                to = job.to.clone();
            } else {
                path = job.to.clone();
                to = job.remote.clone();
            }
            self.call(
                "addJob",
                &make_args!(cnt, path, to, job.file_num, job.show_hidden, job.is_remote),
            );
        }
    }

    fn update_folder_files(
        &self,
        id: i32,
        entries: &Vec<FileEntry>,
        path: String,
        _is_local: bool,
        only_count: bool,
    ) {
        let mut m = make_fd(id, entries, only_count);
        m.set_item("path", path);
        self.call("updateFolderFiles", &make_args!(m));
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

    fn on_rgba(&self, data: Vec<u8>) {
        VIDEO
            .lock()
            .unwrap()
            .as_mut()
            .map(|v| v.render_frame(&data).ok());
    }

    fn set_peer_info(&self, pi: &PeerInfo) {
        let mut pi_sciter = Value::map();
        pi_sciter.set_item("username", pi.username.clone());
        pi_sciter.set_item("hostname", pi.hostname.clone());
        pi_sciter.set_item("platform", pi.platform.clone());
        pi_sciter.set_item("sas_enabled", pi.sas_enabled);

        let mut displays = Value::array(0);
        for ref d in pi.displays.iter() {
            let mut display = Value::map();
            display.set_item("x", d.x);
            display.set_item("y", d.y);
            display.set_item("width", d.width);
            display.set_item("height", d.height);
            display.set_item("cursor_embedded", d.cursor_embedded);
            displays.push(display);
        }
        pi_sciter.set_item("displays", displays);
        pi_sciter.set_item("current_display", pi.current_display);
        self.call("updatePi", &make_args!(pi_sciter));
    }

    fn on_connected(&self, conn_type: ConnType) {
        match conn_type {
            ConnType::RDP => {}
            ConnType::PORT_FORWARD => {}
            ConnType::FILE_TRANSFER => {}
            ConnType::DEFAULT_CONN => {
                crate::keyboard::client::start_grab_loop();
            }
        }
    }

    fn msgbox(&self, msgtype: &str, title: &str, text: &str, link: &str, retry: bool) {
        self.call2(
            "msgbox_retry",
            &make_args!(msgtype, title, text, link, retry),
        );
    }

    fn cancel_msgbox(&self, tag: &str) {
        self.call("cancel_msgbox", &make_args!(tag));
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

    fn switch_back(&self, _id: &str) {}

    fn on_voice_call_started(&self) {
        self.call("onVoiceCallStart", &make_args!());
    }

    fn on_voice_call_closed(&self, reason: &str) {
        self.call("onVoiceCallClosed", &make_args!(reason));
    }

    fn on_voice_call_waiting(&self) {
        self.call("onVoiceCallWaiting", &make_args!());
    }

    fn on_voice_call_incoming(&self) {
        self.call("onVoiceCallIncoming", &make_args!());
    }

    /// RGBA is directly rendered by [on_rgba]. No need to store the rgba for the sciter ui.
    fn get_rgba(&self) -> Option<Vec<u8>> {
        None
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
        fn get_audit_server(String);
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
        fn record_screen(bool, i32, i32);
        fn get_toggle_option(String);
        fn is_privacy_mode_supported();
        fn toggle_option(String);
        fn get_remember();
        fn peer_platform();
        fn set_write_override(i32, i32, bool, bool, bool);
        fn get_keyboard_mode();
        fn save_keyboard_mode(String);
        fn has_hwcodec();
        fn supported_hwcodec();
        fn change_prefer_codec();
        fn restart_remote_device();
        fn request_voice_call();
        fn close_voice_call();
    }
}

impl SciterSession {
    pub fn new(cmd: String, id: String, password: String, args: Vec<String>) -> Self {
        let session: Session<SciterHandler> = Session {
            id: id.clone(),
            password: password.clone(),
            args,
            ..Default::default()
        };

        let conn_type = if cmd.eq("--file-transfer") {
            ConnType::FILE_TRANSFER
        } else if cmd.eq("--port-forward") {
            ConnType::PORT_FORWARD
        } else if cmd.eq("--rdp") {
            ConnType::RDP
        } else {
            ConnType::DEFAULT_CONN
        };

        session.lc.write().unwrap().initialize(id, conn_type, None);

        Self(session)
    }

    pub fn inner(&self) -> Session<SciterHandler> {
        self.0.clone()
    }

    fn get_custom_image_quality(&mut self) -> Value {
        let mut v = Value::array(0);
        for x in self.lc.read().unwrap().custom_image_quality.iter() {
            v.push(x);
        }
        v
    }

    fn has_hwcodec(&self) -> bool {
        has_hwcodec()
    }

    pub fn t(&self, name: String) -> String {
        crate::client::translate(name)
    }

    pub fn get_icon(&self) -> String {
        super::get_icon()
    }

    fn supported_hwcodec(&self) -> Value {
        let (h264, h265) = self.0.supported_hwcodec();
        let mut v = Value::array(0);
        v.push(h264);
        v.push(h265);
        v
    }

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

    fn save_close_state(&mut self, k: String, v: String) {
        self.close_state.insert(k, v);
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
    if !only_count {
        m.set_item("entries", a);
    }
    m.set_item("num_entries", entries.len() as i32);
    m.set_item("total_size", n as f64);
    m
}
