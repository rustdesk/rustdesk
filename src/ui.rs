mod cm;
#[cfg(feature = "inline")]
mod inline;
#[cfg(target_os = "macos")]
mod macos;
pub mod remote;
#[cfg(target_os = "windows")]
pub mod win_privacy;
use crate::common::SOFTWARE_UPDATE_URL;
use crate::ipc;
use hbb_common::{
    allow_err,
    config::{self, Config, LocalConfig, PeerConfig, RENDEZVOUS_PORT, RENDEZVOUS_TIMEOUT},
    futures::future::join_all,
    log,
    protobuf::Message as _,
    rendezvous_proto::*,
    sleep,
    tcp::FramedStream,
    tokio::{self, sync::mpsc, time},
};
use sciter::Value;
use std::{
    collections::HashMap,
    iter::FromIterator,
    process::Child,
    sync::{Arc, Mutex},
};

type Message = RendezvousMessage;

pub type Childs = Arc<Mutex<(bool, HashMap<(String, String), Child>)>>;
type Status = (i32, bool, i64, String);

lazy_static::lazy_static! {
    // stupid workaround for https://sciter.com/forums/topic/crash-on-latest-tis-mac-sdk-sometimes/
    static ref STUPID_VALUES: Mutex<Vec<Arc<Vec<Value>>>> = Default::default();
}

struct UI(
    Childs,
    Arc<Mutex<Status>>,
    Arc<Mutex<HashMap<String, String>>>,
    Arc<Mutex<String>>,
    mpsc::UnboundedSender<ipc::Data>,
);

struct UIHostHandler;

pub fn start(args: &mut [String]) {
    #[cfg(target_os = "macos")]
    if args.len() == 1 && args[0] == "--server" {
        macos::make_tray();
        return;
    } else {
        macos::show_dock();
    }
    #[cfg(all(target_os = "linux", feature = "inline"))]
    {
        #[cfg(feature = "appimage")]
        let prefix = std::env::var("APPDIR").unwrap_or("".to_string());
        #[cfg(not(feature = "appimage"))]
        let prefix = "".to_string();
        sciter::set_library(&(prefix + "/usr/lib/rustdesk/libsciter-gtk.so")).ok();
    }
    // https://github.com/c-smile/sciter-sdk/blob/master/include/sciter-x-types.h
    // https://github.com/rustdesk/rustdesk/issues/132#issuecomment-886069737
    #[cfg(windows)]
    allow_err!(sciter::set_options(sciter::RuntimeOptions::GfxLayer(
        sciter::GFX_LAYER::WARP
    )));
    #[cfg(all(windows, not(feature = "inline")))]
    unsafe {
        winapi::um::shellscalingapi::SetProcessDpiAwareness(2);
    }
    #[cfg(windows)]
    if args.len() > 0 && args[0] == "--tray" {
        let options = check_connect_status(false).1;
        crate::tray::start_tray(options);
        return;
    }
    use sciter::SCRIPT_RUNTIME_FEATURES::*;
    allow_err!(sciter::set_options(sciter::RuntimeOptions::ScriptFeatures(
        ALLOW_FILE_IO as u8 | ALLOW_SOCKET_IO as u8 | ALLOW_EVAL as u8 | ALLOW_SYSINFO as u8
    )));
    let mut frame = sciter::WindowBuilder::main_window().create();
    #[cfg(windows)]
    allow_err!(sciter::set_options(sciter::RuntimeOptions::UxTheming(true)));
    frame.set_title(&crate::get_app_name());
    #[cfg(target_os = "macos")]
    macos::make_menubar(frame.get_host(), args.is_empty());
    let page;
    if args.len() > 1 && args[0] == "--play" {
        args[0] = "--connect".to_owned();
        let path: std::path::PathBuf = (&args[1]).into();
        let id = path
            .file_stem()
            .map(|p| p.to_str().unwrap_or(""))
            .unwrap_or("")
            .to_owned();
        args[1] = id;
    }
    if args.is_empty() {
        let childs: Childs = Default::default();
        let cloned = childs.clone();
        std::thread::spawn(move || check_zombie(cloned));
        crate::common::check_software_update();
        frame.event_handler(UI::new(childs));
        frame.sciter_handler(UIHostHandler {});
        page = "index.html";
    } else if args[0] == "--install" {
        let childs: Childs = Default::default();
        frame.event_handler(UI::new(childs));
        frame.sciter_handler(UIHostHandler {});
        page = "install.html";
    } else if args[0] == "--cm" {
        frame.register_behavior("connection-manager", move || {
            Box::new(cm::ConnectionManager::new())
        });
        page = "cm.html";
    } else if (args[0] == "--connect"
        || args[0] == "--file-transfer"
        || args[0] == "--port-forward"
        || args[0] == "--rdp")
        && args.len() > 1
    {
        #[cfg(windows)]
        {
            let hw = frame.get_host().get_hwnd();
            crate::platform::windows::enable_lowlevel_keyboard(hw as _);
        }
        let mut iter = args.iter();
        let cmd = iter.next().unwrap().clone();
        let id = iter.next().unwrap().clone();
        let args: Vec<String> = iter.map(|x| x.clone()).collect();
        frame.set_title(&id);
        frame.register_behavior("native-remote", move || {
            Box::new(remote::Handler::new(cmd.clone(), id.clone(), args.clone()))
        });
        page = "remote.html";
    } else {
        log::error!("Wrong command: {:?}", args);
        return;
    }
    #[cfg(feature = "inline")]
    {
        let html = if page == "index.html" {
            inline::get_index()
        } else if page == "cm.html" {
            inline::get_cm()
        } else if page == "install.html" {
            inline::get_install()
        } else {
            inline::get_remote()
        };
        frame.load_html(html.as_bytes(), Some(page));
    }
    #[cfg(not(feature = "inline"))]
    frame.load_file(&format!(
        "file://{}/src/ui/{}",
        std::env::current_dir()
            .map(|c| c.display().to_string())
            .unwrap_or("".to_owned()),
        page
    ));
    frame.run_app();
}

impl UI {
    fn new(childs: Childs) -> Self {
        let res = check_connect_status(true);
        Self(childs, res.0, res.1, Default::default(), res.2)
    }

    fn recent_sessions_updated(&mut self) -> bool {
        let mut lock = self.0.lock().unwrap();
        if lock.0 {
            lock.0 = false;
            true
        } else {
            false
        }
    }

    fn get_id(&self) -> String {
        ipc::get_id()
    }

    fn get_password(&mut self) -> String {
        ipc::get_password()
    }

    fn update_password(&mut self, password: String) {
        if password.is_empty() {
            allow_err!(ipc::set_password(Config::get_auto_password()));
        } else {
            allow_err!(ipc::set_password(password));
        }
    }

    fn get_remote_id(&mut self) -> String {
        LocalConfig::get_remote_id()
    }

    fn set_remote_id(&mut self, id: String) {
        LocalConfig::set_remote_id(&id);
    }

    fn goto_install(&mut self) {
        allow_err!(crate::run_me(vec!["--install"]));
    }

    fn install_me(&mut self, _options: String, _path: String) {
        #[cfg(windows)]
        std::thread::spawn(move || {
            allow_err!(crate::platform::windows::install_me(
                &_options, _path, false, false
            ));
            std::process::exit(0);
        });
    }

    fn update_me(&self, _path: String) {
        #[cfg(target_os = "linux")]
        {
            std::process::Command::new("pkexec")
                .args(&["apt", "install", "-f", &_path])
                .spawn()
                .ok();
            std::fs::remove_file(&_path).ok();
            crate::run_me(Vec::<&str>::new()).ok();
        }
        #[cfg(windows)]
        {
            let mut path = _path;
            if path.is_empty() {
                if let Ok(tmp) = std::env::current_exe() {
                    path = tmp.to_string_lossy().to_string();
                }
            }
            std::process::Command::new(path)
                .arg("--update")
                .spawn()
                .ok();
            std::process::exit(0);
        }
    }

    fn run_without_install(&self) {
        crate::run_me(vec!["--noinstall"]).ok();
        std::process::exit(0);
    }

    fn show_run_without_install(&self) -> bool {
        let mut it = std::env::args();
        if let Some(tmp) = it.next() {
            if crate::is_setup(&tmp) {
                return it.next() == None;
            }
        }
        false
    }

    fn has_rendezvous_service(&self) -> bool {
        #[cfg(all(windows, feature = "hbbs"))]
        return crate::platform::is_win_server()
            && crate::platform::windows::get_license().is_some();
        return false;
    }

    fn get_license(&self) -> String {
        #[cfg(windows)]
        if let Some(lic) = crate::platform::windows::get_license() {
            return format!(
                "<br /> Key: {} <br /> Host: {} Api: {}",
                lic.key, lic.host, lic.api
            );
        }
        Default::default()
    }

    fn get_option(&self, key: String) -> String {
        self.get_option_(&key)
    }

    fn get_option_(&self, key: &str) -> String {
        if let Some(v) = self.2.lock().unwrap().get(key) {
            v.to_owned()
        } else {
            "".to_owned()
        }
    }

    fn get_local_option(&self, key: String) -> String {
        LocalConfig::get_option(&key)
    }

    fn set_local_option(&self, key: String, value: String) {
        LocalConfig::set_option(key, value);
    }

    fn peer_has_password(&self, id: String) -> bool {
        !PeerConfig::load(&id).password.is_empty()
    }

    fn forget_password(&self, id: String) {
        let mut c = PeerConfig::load(&id);
        c.password.clear();
        c.store(&id);
    }

    fn get_peer_option(&self, id: String, name: String) -> String {
        let c = PeerConfig::load(&id);
        c.options.get(&name).unwrap_or(&"".to_owned()).to_owned()
    }

    fn set_peer_option(&self, id: String, name: String, value: String) {
        let mut c = PeerConfig::load(&id);
        if value.is_empty() {
            c.options.remove(&name);
        } else {
            c.options.insert(name, value);
        }
        c.store(&id);
    }

    fn using_public_server(&self) -> bool {
        crate::get_custom_rendezvous_server(self.get_option_("custom-rendezvous-server")).is_empty()
    }

    fn get_options(&self) -> Value {
        let mut m = Value::map();
        for (k, v) in self.2.lock().unwrap().iter() {
            m.set_item(k, v);
        }
        m
    }

    fn test_if_valid_server(&self, host: String) -> String {
        hbb_common::socket_client::test_if_valid_server(&host)
    }

    fn get_sound_inputs(&self) -> Value {
        let mut a = Value::array(0);
        #[cfg(windows)]
        {
            let inputs = Arc::new(Mutex::new(Vec::new()));
            let cloned = inputs.clone();
            // can not call below in UI thread, because conflict with sciter sound com initialization
            std::thread::spawn(move || *cloned.lock().unwrap() = get_sound_inputs())
                .join()
                .ok();
            for name in inputs.lock().unwrap().drain(..) {
                a.push(name);
            }
        }
        #[cfg(not(windows))]
        for name in get_sound_inputs() {
            a.push(name);
        }
        a
    }

    fn set_options(&self, v: Value) {
        let mut m = HashMap::new();
        for (k, v) in v.items() {
            if let Some(k) = k.as_string() {
                if let Some(v) = v.as_string() {
                    if !v.is_empty() {
                        m.insert(k, v);
                    }
                }
            }
        }

        *self.2.lock().unwrap() = m.clone();
        ipc::set_options(m).ok();
    }

    fn set_option(&self, key: String, value: String) {
        #[cfg(target_os = "macos")]
        if &key == "stop-service" {
            let is_stop = value == "Y";
            if is_stop && crate::platform::macos::uninstall() {
                return;
            }
        }
        let mut options = self.2.lock().unwrap();
        if value.is_empty() {
            options.remove(&key);
        } else {
            options.insert(key.clone(), value.clone());
        }
        ipc::set_options(options.clone()).ok();
    }

    fn install_path(&mut self) -> String {
        #[cfg(windows)]
        return crate::platform::windows::get_install_info().1;
        #[cfg(not(windows))]
        return "".to_owned();
    }

    fn get_socks(&self) -> Value {
        let s = ipc::get_socks();
        match s {
            None => Value::null(),
            Some(s) => {
                let mut v = Value::array(0);
                v.push(s.proxy);
                v.push(s.username);
                v.push(s.password);
                v
            }
        }
    }

    fn set_socks(&self, proxy: String, username: String, password: String) {
        ipc::set_socks(config::Socks5Server {
            proxy,
            username,
            password,
        })
        .ok();
    }

    fn is_installed(&self) -> bool {
        crate::platform::is_installed()
    }

    fn is_rdp_service_open(&self) -> bool {
        #[cfg(windows)]
        return self.is_installed() && crate::platform::windows::is_rdp_service_open();
        #[cfg(not(windows))]
        return false;
    }

    fn is_share_rdp(&self) -> bool {
        #[cfg(windows)]
        return crate::platform::windows::is_share_rdp();
        #[cfg(not(windows))]
        return false;
    }

    fn set_share_rdp(&self, _enable: bool) {
        #[cfg(windows)]
        crate::platform::windows::set_share_rdp(_enable);
    }

    fn is_installed_lower_version(&self) -> bool {
        #[cfg(not(windows))]
        return false;
        #[cfg(windows)]
        {
            let installed_version = crate::platform::windows::get_installed_version();
            let a = hbb_common::get_version_number(crate::VERSION);
            let b = hbb_common::get_version_number(&installed_version);
            return a > b;
        }
    }

    fn closing(&mut self, x: i32, y: i32, w: i32, h: i32) {
        crate::server::input_service::fix_key_down_timeout_at_exit();
        LocalConfig::set_size(x, y, w, h);
    }

    fn get_size(&mut self) -> Value {
        let s = LocalConfig::get_size();
        let mut v = Value::array(0);
        v.push(s.0);
        v.push(s.1);
        v.push(s.2);
        v.push(s.3);
        v
    }

    fn get_mouse_time(&self) -> f64 {
        self.1.lock().unwrap().2 as _
    }

    fn check_mouse_time(&self) {
        allow_err!(self.4.send(ipc::Data::MouseMoveTime(0)));
    }

    fn get_connect_status(&mut self) -> Value {
        let mut v = Value::array(0);
        let x = self.1.lock().unwrap().clone();
        v.push(x.0);
        v.push(x.1);
        v.push(x.3);
        v
    }

    #[inline]
    fn get_peer_value(id: String, p: PeerConfig) -> Value {
        let values = vec![
            id,
            p.info.username.clone(),
            p.info.hostname.clone(),
            p.info.platform.clone(),
            p.options.get("alias").unwrap_or(&"".to_owned()).to_owned(),
        ];
        Value::from_iter(values)
    }

    fn get_peer(&self, id: String) -> Value {
        let c = PeerConfig::load(&id);
        Self::get_peer_value(id, c)
    }

    fn get_fav(&self) -> Value {
        Value::from_iter(LocalConfig::get_fav())
    }

    fn store_fav(&self, fav: Value) {
        let mut tmp = vec![];
        fav.values().for_each(|v| {
            if let Some(v) = v.as_string() {
                if !v.is_empty() {
                    tmp.push(v);
                }
            }
        });
        LocalConfig::set_fav(tmp);
    }

    fn get_recent_sessions(&mut self) -> Value {
        // to-do: limit number of recent sessions, and remove old peer file
        let peers: Vec<Value> = PeerConfig::peers()
            .drain(..)
            .map(|p| Self::get_peer_value(p.0, p.2))
            .collect();
        Value::from_iter(peers)
    }

    fn get_icon(&mut self) -> String {
        crate::get_icon()
    }

    fn remove_peer(&mut self, id: String) {
        PeerConfig::remove(&id);
    }

    fn new_remote(&mut self, id: String, remote_type: String) {
        let mut lock = self.0.lock().unwrap();
        let args = vec![format!("--{}", remote_type), id.clone()];
        let key = (id.clone(), remote_type.clone());
        if let Some(c) = lock.1.get_mut(&key) {
            if let Ok(Some(_)) = c.try_wait() {
                lock.1.remove(&key);
            } else {
                if remote_type == "rdp" {
                    allow_err!(c.kill());
                    std::thread::sleep(std::time::Duration::from_millis(30));
                    c.try_wait().ok();
                    lock.1.remove(&key);
                } else {
                    return;
                }
            }
        }
        match crate::run_me(args) {
            Ok(child) => {
                lock.1.insert(key, child);
            }
            Err(err) => {
                log::error!("Failed to spawn remote: {}", err);
            }
        }
    }

    fn is_process_trusted(&mut self, _prompt: bool) -> bool {
        #[cfg(target_os = "macos")]
        return crate::platform::macos::is_process_trusted(_prompt);
        #[cfg(not(target_os = "macos"))]
        return true;
    }

    fn is_can_screen_recording(&mut self, _prompt: bool) -> bool {
        #[cfg(target_os = "macos")]
        return crate::platform::macos::is_can_screen_recording(_prompt);
        #[cfg(not(target_os = "macos"))]
        return true;
    }

    fn is_installed_daemon(&mut self, _prompt: bool) -> bool {
        #[cfg(target_os = "macos")]
        return crate::platform::macos::is_installed_daemon(_prompt);
        #[cfg(not(target_os = "macos"))]
        return true;
    }

    fn get_error(&mut self) -> String {
        #[cfg(target_os = "linux")]
        {
            let dtype = crate::platform::linux::get_display_server();
            if "wayland" == dtype {
                return "".to_owned();
            }
            if dtype != "x11" {
                return format!(
                    "{} {}, {}",
                    self.t("Unsupported display server ".to_owned()),
                    dtype,
                    self.t("x11 expected".to_owned()),
                );
            }
        }
        return "".to_owned();
    }

    fn is_login_wayland(&mut self) -> bool {
        #[cfg(target_os = "linux")]
        return crate::platform::linux::is_login_wayland();
        #[cfg(not(target_os = "linux"))]
        return false;
    }

    fn fix_login_wayland(&mut self) {
        /*
        #[cfg(target_os = "linux")]
        crate::platform::linux::fix_login_wayland();
        */
    }

    fn current_is_wayland(&mut self) -> bool {
        #[cfg(target_os = "linux")]
        return crate::platform::linux::current_is_wayland();
        #[cfg(not(target_os = "linux"))]
        return false;
    }

    fn modify_default_login(&mut self) -> String {
        /*
        #[cfg(target_os = "linux")]
        return crate::platform::linux::modify_default_login();
        #[cfg(not(target_os = "linux"))]
        */
        return "".to_owned();
    }

    fn get_software_update_url(&self) -> String {
        SOFTWARE_UPDATE_URL.lock().unwrap().clone()
    }

    fn get_new_version(&self) -> String {
        hbb_common::get_version_from_url(&*SOFTWARE_UPDATE_URL.lock().unwrap())
    }

    fn get_version(&self) -> String {
        crate::VERSION.to_owned()
    }

    fn get_app_name(&self) -> String {
        crate::get_app_name()
    }

    fn get_software_ext(&self) -> String {
        #[cfg(windows)]
        let p = "exe";
        #[cfg(target_os = "macos")]
        let p = "dmg";
        #[cfg(target_os = "linux")]
        let p = "deb";
        p.to_owned()
    }

    fn get_software_store_path(&self) -> String {
        let mut p = std::env::temp_dir();
        let name = SOFTWARE_UPDATE_URL
            .lock()
            .unwrap()
            .split("/")
            .last()
            .map(|x| x.to_owned())
            .unwrap_or(crate::get_app_name());
        p.push(name);
        format!("{}.{}", p.to_string_lossy(), self.get_software_ext())
    }

    fn create_shortcut(&self, _id: String) {
        #[cfg(windows)]
        crate::platform::windows::create_shortcut(&_id).ok();
    }

    fn discover(&self) {
        std::thread::spawn(move || {
            allow_err!(crate::rendezvous_mediator::discover());
        });
    }

    fn get_lan_peers(&self) -> String {
        config::LanPeers::load().peers
    }

    fn get_uuid(&self) -> String {
        base64::encode(crate::get_uuid())
    }

    fn open_url(&self, url: String) {
        #[cfg(windows)]
        let p = "explorer";
        #[cfg(target_os = "macos")]
        let p = "open";
        #[cfg(target_os = "linux")]
        let p = if std::path::Path::new("/usr/bin/firefox").exists() {
            "firefox"
        } else {
            "xdg-open"
        };
        allow_err!(std::process::Command::new(p).arg(url).spawn());
    }

    fn change_id(&self, id: String) {
        let status = self.3.clone();
        *status.lock().unwrap() = " ".to_owned();
        let old_id = self.get_id();
        std::thread::spawn(move || {
            *status.lock().unwrap() = change_id(id, old_id).to_owned();
        });
    }

    fn post_request(&self, url: String, body: String, header: String) {
        let status = self.3.clone();
        *status.lock().unwrap() = " ".to_owned();
        std::thread::spawn(move || {
            *status.lock().unwrap() = match crate::post_request_sync(url, body, &header) {
                Err(err) => err.to_string(),
                Ok(text) => text,
            };
        });
    }

    fn is_ok_change_id(&self) -> bool {
        machine_uid::get().is_ok()
    }

    fn get_async_job_status(&self) -> String {
        self.3.clone().lock().unwrap().clone()
    }

    fn t(&self, name: String) -> String {
        crate::client::translate(name)
    }

    fn is_xfce(&self) -> bool {
        crate::platform::is_xfce()
    }

    fn get_api_server(&self) -> String {
        crate::get_api_server(
            self.get_option_("api-server"),
            self.get_option_("custom-rendezvous-server"),
        )
    }

    fn has_hwcodec(&self) -> bool {
        #[cfg(not(feature = "hwcodec"))]
        return false;
        #[cfg(feature = "hwcodec")]
        return true;
    }

    fn get_langs(&self) -> String {
        crate::lang::LANGS.to_string()
    }
}

impl sciter::EventHandler for UI {
    sciter::dispatch_script_call! {
        fn t(String);
        fn get_api_server();
        fn is_xfce();
        fn using_public_server();
        fn get_id();
        fn get_password();
        fn update_password(String);
        fn get_remote_id();
        fn set_remote_id(String);
        fn closing(i32, i32, i32, i32);
        fn get_size();
        fn new_remote(String, bool);
        fn remove_peer(String);
        fn get_connect_status();
        fn get_mouse_time();
        fn check_mouse_time();
        fn get_recent_sessions();
        fn get_peer(String);
        fn get_fav();
        fn store_fav(Value);
        fn recent_sessions_updated();
        fn get_icon();
        fn install_me(String, String);
        fn is_installed();
        fn set_socks(String, String, String);
        fn get_socks();
        fn is_rdp_service_open();
        fn is_share_rdp();
        fn set_share_rdp(bool);
        fn is_installed_lower_version();
        fn install_path();
        fn goto_install();
        fn is_process_trusted(bool);
        fn is_can_screen_recording(bool);
        fn is_installed_daemon(bool);
        fn get_error();
        fn is_login_wayland();
        fn fix_login_wayland();
        fn current_is_wayland();
        fn modify_default_login();
        fn get_options();
        fn get_option(String);
        fn get_local_option(String);
        fn set_local_option(String, String);
        fn get_peer_option(String, String);
        fn peer_has_password(String);
        fn forget_password(String);
        fn set_peer_option(String, String, String);
        fn has_rendezvous_service();
        fn get_license();
        fn test_if_valid_server(String);
        fn get_sound_inputs();
        fn set_options(Value);
        fn set_option(String, String);
        fn get_software_update_url();
        fn get_new_version();
        fn get_version();
        fn update_me(String);
        fn show_run_without_install();
        fn run_without_install();
        fn get_app_name();
        fn get_software_store_path();
        fn get_software_ext();
        fn open_url(String);
        fn change_id(String);
        fn get_async_job_status();
        fn post_request(String, String, String);
        fn is_ok_change_id();
        fn create_shortcut(String);
        fn discover();
        fn get_lan_peers();
        fn get_uuid();
        fn has_hwcodec();
        fn get_langs();
    }
}

impl sciter::host::HostHandler for UIHostHandler {
    fn on_graphics_critical_failure(&mut self) {
        log::error!("Critical rendering error: e.g. DirectX gfx driver error. Most probably bad gfx drivers.");
    }
}

pub fn check_zombie(childs: Childs) {
    let mut deads = Vec::new();
    loop {
        let mut lock = childs.lock().unwrap();
        let mut n = 0;
        for (id, c) in lock.1.iter_mut() {
            if let Ok(Some(_)) = c.try_wait() {
                deads.push(id.clone());
                n += 1;
            }
        }
        for ref id in deads.drain(..) {
            lock.1.remove(id);
        }
        if n > 0 {
            lock.0 = true;
        }
        drop(lock);
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

// notice: avoiding create ipc connecton repeatly,
// because windows named pipe has serious memory leak issue.
#[tokio::main(flavor = "current_thread")]
async fn check_connect_status_(
    reconnect: bool,
    status: Arc<Mutex<Status>>,
    options: Arc<Mutex<HashMap<String, String>>>,
    rx: mpsc::UnboundedReceiver<ipc::Data>,
) {
    let mut key_confirmed = false;
    let mut rx = rx;
    let mut mouse_time = 0;
    let mut id = "".to_owned();
    loop {
        if let Ok(mut c) = ipc::connect(1000, "").await {
            let mut timer = time::interval(time::Duration::from_secs(1));
            loop {
                tokio::select! {
                    res = c.next() => {
                        match res {
                            Err(err) => {
                                log::error!("ipc connection closed: {}", err);
                                break;
                            }
                            Ok(Some(ipc::Data::MouseMoveTime(v))) => {
                                mouse_time = v;
                                status.lock().unwrap().2 = v;
                            }
                            Ok(Some(ipc::Data::Options(Some(v)))) => {
                                *options.lock().unwrap() = v
                            }
                            Ok(Some(ipc::Data::Config((name, Some(value))))) => {
                                if name == "id" {
                                    id = value;
                                }
                            }
                            Ok(Some(ipc::Data::OnlineStatus(Some((mut x, c))))) => {
                                if x > 0 {
                                    x = 1
                                }
                                key_confirmed = c;
                                *status.lock().unwrap() = (x as _, key_confirmed, mouse_time, id.clone());
                            }
                            _ => {}
                        }
                    }
                    Some(data) = rx.recv() => {
                        allow_err!(c.send(&data).await);
                    }
                    _ = timer.tick() => {
                        c.send(&ipc::Data::OnlineStatus(None)).await.ok();
                        c.send(&ipc::Data::Options(None)).await.ok();
                        c.send(&ipc::Data::Config(("id".to_owned(), None))).await.ok();
                    }
                }
            }
        }
        if !reconnect {
            options
                .lock()
                .unwrap()
                .insert("ipc-closed".to_owned(), "Y".to_owned());
            break;
        }
        *status.lock().unwrap() = (-1, key_confirmed, mouse_time, id.clone());
        sleep(1.).await;
    }
}

#[cfg(not(target_os = "linux"))]
fn get_sound_inputs() -> Vec<String> {
    let mut out = Vec::new();
    use cpal::traits::{DeviceTrait, HostTrait};
    let host = cpal::default_host();
    if let Ok(devices) = host.devices() {
        for device in devices {
            if device.default_input_config().is_err() {
                continue;
            }
            if let Ok(name) = device.name() {
                out.push(name);
            }
        }
    }
    out
}

#[cfg(target_os = "linux")]
fn get_sound_inputs() -> Vec<String> {
    crate::platform::linux::get_pa_sources()
        .drain(..)
        .map(|x| x.1)
        .collect()
}

fn check_connect_status(
    reconnect: bool,
) -> (
    Arc<Mutex<Status>>,
    Arc<Mutex<HashMap<String, String>>>,
    mpsc::UnboundedSender<ipc::Data>,
) {
    let status = Arc::new(Mutex::new((0, false, 0, "".to_owned())));
    let options = Arc::new(Mutex::new(Config::get_options()));
    let cloned = status.clone();
    let cloned_options = options.clone();
    let (tx, rx) = mpsc::unbounded_channel::<ipc::Data>();
    std::thread::spawn(move || check_connect_status_(reconnect, cloned, cloned_options, rx));
    (status, options, tx)
}

const INVALID_FORMAT: &'static str = "Invalid format";
const UNKNOWN_ERROR: &'static str = "Unknown error";

#[tokio::main(flavor = "current_thread")]
async fn change_id(id: String, old_id: String) -> &'static str {
    if !hbb_common::is_valid_custom_id(&id) {
        return INVALID_FORMAT;
    }
    let uuid = machine_uid::get().unwrap_or("".to_owned());
    if uuid.is_empty() {
        return UNKNOWN_ERROR;
    }
    let rendezvous_servers = crate::ipc::get_rendezvous_servers(1_000).await;
    let mut futs = Vec::new();
    let err: Arc<Mutex<&str>> = Default::default();
    for rendezvous_server in rendezvous_servers {
        let err = err.clone();
        let id = id.to_owned();
        let uuid = uuid.clone();
        let old_id = old_id.clone();
        futs.push(tokio::spawn(async move {
            let tmp = check_id(rendezvous_server, old_id, id, uuid).await;
            if !tmp.is_empty() {
                *err.lock().unwrap() = tmp;
            }
        }));
    }
    join_all(futs).await;
    let err = *err.lock().unwrap();
    if err.is_empty() {
        crate::ipc::set_config_async("id", id.to_owned()).await.ok();
    }
    err
}

async fn check_id(
    rendezvous_server: String,
    old_id: String,
    id: String,
    uuid: String,
) -> &'static str {
    let any_addr = Config::get_any_listen_addr();
    if let Ok(mut socket) = FramedStream::new(
        crate::check_port(rendezvous_server, RENDEZVOUS_PORT),
        any_addr,
        RENDEZVOUS_TIMEOUT,
    )
    .await
    {
        let mut msg_out = Message::new();
        msg_out.set_register_pk(RegisterPk {
            old_id,
            id,
            uuid: uuid.into(),
            ..Default::default()
        });
        let mut ok = false;
        if socket.send(&msg_out).await.is_ok() {
            if let Some(Ok(bytes)) = socket.next_timeout(3_000).await {
                if let Ok(msg_in) = RendezvousMessage::parse_from_bytes(&bytes) {
                    match msg_in.union {
                        Some(rendezvous_message::Union::register_pk_response(rpr)) => {
                            match rpr.result.enum_value_or_default() {
                                register_pk_response::Result::OK => {
                                    ok = true;
                                }
                                register_pk_response::Result::ID_EXISTS => {
                                    return "Not available";
                                }
                                register_pk_response::Result::TOO_FREQUENT => {
                                    return "Too frequent";
                                }
                                register_pk_response::Result::NOT_SUPPORT => {
                                    return "server_not_support";
                                }
                                register_pk_response::Result::INVALID_ID_FORMAT => {
                                    return INVALID_FORMAT;
                                }
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        if !ok {
            return UNKNOWN_ERROR;
        }
    } else {
        return "Failed to connect to rendezvous server";
    }
    ""
}

// sacrifice some memory
pub fn value_crash_workaround(values: &[Value]) -> Arc<Vec<Value>> {
    let persist = Arc::new(values.to_vec());
    STUPID_VALUES.lock().unwrap().push(persist.clone());
    persist
}
