use crate::client::{
    self, check_if_retry, handle_hash, handle_login_from_ui, handle_test_delay, input_os_password,
    FileManager, LoginConfigHandler, QualityStatus, load_config,
};
use crate::{client::Data, client::Interface};
use async_trait::async_trait;
use hbb_common::config::PeerConfig;
use hbb_common::message_proto::{CursorData, Hash, PeerInfo, TestDelay, CursorPosition};
use hbb_common::tokio::{
    self,
    sync::mpsc,
    time::{self, Duration, Instant, Interval},
};
use hbb_common::{get_version_number, log, Stream};
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, RwLock};

#[derive(Clone, Default)]
pub struct Session<T: InvokeUi> {
    pub cmd: String,
    pub id: String,
    pub password: String,
    pub args: Vec<String>,
    pub lc: Arc<RwLock<LoginConfigHandler>>,
    pub sender: Arc<RwLock<Option<mpsc::UnboundedSender<Data>>>>,
    pub ui_handler: T,
}

impl<T: InvokeUi> Session<T> {
    pub fn get_option(&self, k: String) -> String {
        self.lc.read().unwrap().get_option(&k)
    }

    pub fn set_option(&self, k: String, v: String) {
        self.lc.write().unwrap().set_option(k, v);
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

    pub fn get_platform(&mut self, is_remote: bool) -> String {
        if is_remote {
            self.peer_platform()
        } else {
            whoami::platform().to_string()
        }
    }

    pub fn get_path_sep(&mut self, is_remote: bool) -> &'static str {
        let p = self.get_platform(is_remote);
        if &p == "Windows" {
            return "\\";
        } else {
            return "/";
        }
    }
}

pub trait InvokeUi: Send + Sync + Clone + 'static + Sized + Default {
    fn set_cursor_data(&self, cd: CursorData);
    fn set_cursor_id(&self, id: String);
    fn set_cursor_position(&self, cp:CursorPosition);
    fn set_display(&self, x: i32, y: i32, w: i32, h: i32);
    fn update_privacy_mode(&self);
    fn set_permission(&self, name: &str, value: bool);
    fn update_pi(&self, pi: PeerInfo);
    fn close_success(&self);
    fn update_quality_status(&self, qs: QualityStatus);
    fn set_connection_type(&self,is_secured: bool, direct: bool);
    fn job_error(&self,id:i32, err:String, file_num:i32);
    fn job_done(&self,id:i32, file_num:i32);
    fn clear_all_jobs(&self);
    fn add_job(&self, id:i32, path:String, to:String, file_num:i32, show_hidden:bool, is_remote:bool);
    fn update_transfer_list(&self);
    // fn update_folder_files(&self); // TODO
    fn confirm_delete_files(&self,id:i32, i:i32, name:String);
    fn override_file_confirm(&self, id:i32, file_num:i32, to:String, is_upload:bool);
    fn job_progress(&self, id:i32, file_num:i32, speed:f64, finished_size:f64);
    fn adapt_size(&self);
}


impl<T: InvokeUi> Deref for Session<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.ui_handler
    }
}

impl<T: InvokeUi> DerefMut for Session<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ui_handler
    }
}

impl<T: InvokeUi> FileManager for Session<T> {}

#[async_trait]
impl<T: InvokeUi> Interface for Session<T> {
    fn send(&self, data: Data) {
        if let Some(sender) = self.sender.read().unwrap().as_ref() {
            sender.send(data).ok();
        }
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

    fn msgbox(&self, msgtype: &str, title: &str, text: &str) {
        let retry = check_if_retry(msgtype, title, text);
        // self.call2("msgbox_retry", &make_args!(msgtype, title, text, retry));
    }

    fn handle_login_error(&mut self, err: &str) -> bool {
        self.lc.write().unwrap().handle_login_error(err, self)
    }

    fn handle_peer_info(&mut self, pi: PeerInfo) {
        // let mut pi_sciter = Value::map();
        let username = self.lc.read().unwrap().get_username(&pi);
        // pi_sciter.set_item("username", username.clone());
        // pi_sciter.set_item("hostname", pi.hostname.clone());
        // pi_sciter.set_item("platform", pi.platform.clone());
        // pi_sciter.set_item("sas_enabled", pi.sas_enabled);
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
                self.lc.write().unwrap().handle_peer_info(username, pi);
                self.update_privacy_mode();
                self.msgbox("error", "Remote Error", "No Display");
                return;
            }
            // let mut displays = Value::array(0);
            // for ref d in pi.displays.iter() {
            //     let mut display = Value::map();
            //     display.set_item("x", d.x);
            //     display.set_item("y", d.y);
            //     display.set_item("width", d.width);
            //     display.set_item("height", d.height);
            //     displays.push(display);
            // }
            // pi_sciter.set_item("displays", displays);
            let mut current = pi.current_display as usize;
            if current >= pi.displays.len() {
                current = 0;
            }
            // pi_sciter.set_item("current_display", current as i32);
            let current = &pi.displays[current];
            self.set_display(current.x, current.y, current.width, current.height);
            // https://sciter.com/forums/topic/color_spaceiyuv-crash
            // Nothing spectacular in decoder – done on CPU side.
            // So if you can do BGRA translation on your side – the better.
            // BGRA is used as internal image format so it will not require additional transformations.
            // VIDEO.lock().unwrap().as_mut().map(|v| {
            //     let ok = v.start_streaming(
            //         (current.width as _, current.height as _),
            //         COLOR_SPACE::Rgb32,
            //         None,
            //     );
            //     log::info!("[video] initialized: {:?}", ok);
            // });
            let p = self.lc.read().unwrap().should_auto_login();
            if !p.is_empty() {
                input_os_password(p, true, self.clone());
            }
        }
        self.lc.write().unwrap().handle_peer_info(username, pi);
        self.update_privacy_mode();
        // self.update_pi(pi);
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
        // self.start_keyboard_hook(); // TODO
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
