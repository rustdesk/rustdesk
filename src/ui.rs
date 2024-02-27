use std::{
    collections::HashMap,
    iter::FromIterator,
    sync::{Arc, Mutex},
};

use sciter::Value;

use hbb_common::{
    allow_err,
    config::{LocalConfig, PeerConfig},
    log,
};

#[cfg(not(any(feature = "flutter", feature = "cli")))]
use crate::ui_session_interface::Session;
use crate::{common::get_app_name, ipc, ui_interface::*};

mod cm;
#[cfg(feature = "inline")]
pub mod inline;
pub mod remote;

#[allow(dead_code)]
type Status = (i32, bool, i64, String);

lazy_static::lazy_static! {
    // stupid workaround for https://sciter.com/forums/topic/crash-on-latest-tis-mac-sdk-sometimes/
    static ref STUPID_VALUES: Mutex<Vec<Arc<Vec<Value>>>> = Default::default();
}

#[cfg(not(any(feature = "flutter", feature = "cli")))]
lazy_static::lazy_static! {
    pub static ref CUR_SESSION: Arc<Mutex<Option<Session<remote::SciterHandler>>>> = Default::default();
}

struct UIHostHandler;

pub fn start(args: &mut [String]) {
    #[cfg(target_os = "macos")]
    crate::platform::delegate::show_dock();
    #[cfg(all(target_os = "linux", feature = "inline"))]
    {
        #[cfg(feature = "appimage")]
        let prefix = std::env::var("APPDIR").unwrap_or("".to_string());
        #[cfg(not(feature = "appimage"))]
        let prefix = "".to_string();
        #[cfg(feature = "flatpak")]
        let dir = "/app";
        #[cfg(not(feature = "flatpak"))]
        let dir = "/usr";
        sciter::set_library(&(prefix + dir + "/lib/rustdesk/libsciter-gtk.so")).ok();
    }
    #[cfg(windows)]
    // Check if there is a sciter.dll nearby.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let sciter_dll_path = parent.join("sciter.dll");
            if sciter_dll_path.exists() {
                // Try to set the sciter dll.
                let p = sciter_dll_path.to_string_lossy().to_string();
                log::debug!("Found dll:{}, \n {:?}", p, sciter::set_library(&p));
            }
        }
    }
    // https://github.com/c-smile/sciter-sdk/blob/master/include/sciter-x-types.h
    // https://github.com/rustdesk/rustdesk/issues/132#issuecomment-886069737
    #[cfg(windows)]
    allow_err!(sciter::set_options(sciter::RuntimeOptions::GfxLayer(
        sciter::GFX_LAYER::WARP
    )));
    use sciter::SCRIPT_RUNTIME_FEATURES::*;
    allow_err!(sciter::set_options(sciter::RuntimeOptions::ScriptFeatures(
        ALLOW_FILE_IO as u8 | ALLOW_SOCKET_IO as u8 | ALLOW_EVAL as u8 | ALLOW_SYSINFO as u8
    )));
    let mut frame = sciter::WindowBuilder::main_window().create();
    #[cfg(windows)]
    allow_err!(sciter::set_options(sciter::RuntimeOptions::UxTheming(true)));
    frame.set_title(&crate::get_app_name());
    #[cfg(target_os = "macos")]
    crate::platform::delegate::make_menubar(frame.get_host(), args.is_empty());
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
        std::thread::spawn(move || check_zombie());
        crate::common::check_software_update();
        frame.event_handler(UI {});
        frame.sciter_handler(UIHostHandler {});
        page = "index.html";
        // Start pulse audio local server.
        #[cfg(target_os = "linux")]
        std::thread::spawn(crate::ipc::start_pa);
    } else if args[0] == "--install" {
        frame.event_handler(UI {});
        frame.sciter_handler(UIHostHandler {});
        page = "install.html";
    } else if args[0] == "--cm" {
        frame.register_behavior("connection-manager", move || {
            Box::new(cm::SciterConnectionManager::new())
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
        let Some(cmd) = iter.next() else {
            log::error!("Failed to get cmd arg");
            return;
        };
        let cmd = cmd.to_owned();
        let Some(id) = iter.next() else {
            log::error!("Failed to get id arg");
            return;
        };
        let id = id.to_owned();
        let pass = iter.next().unwrap_or(&"".to_owned()).clone();
        let args: Vec<String> = iter.map(|x| x.clone()).collect();
        frame.set_title(&id);
        frame.register_behavior("native-remote", move || {
            let handler =
                remote::SciterSession::new(cmd.clone(), id.clone(), pass.clone(), args.clone());
            #[cfg(not(any(feature = "flutter", feature = "cli")))]
            {
                *CUR_SESSION.lock().unwrap() = Some(handler.inner());
            }
            Box::new(handler)
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

struct UI {}

impl UI {
    fn recent_sessions_updated(&self) -> bool {
        recent_sessions_updated()
    }

    fn get_id(&self) -> String {
        ipc::get_id()
    }

    fn temporary_password(&mut self) -> String {
        temporary_password()
    }

    fn update_temporary_password(&self) {
        update_temporary_password()
    }

    fn permanent_password(&self) -> String {
        permanent_password()
    }

    fn set_permanent_password(&self, password: String) {
        set_permanent_password(password);
    }

    fn get_remote_id(&mut self) -> String {
        LocalConfig::get_remote_id()
    }

    fn set_remote_id(&mut self, id: String) {
        LocalConfig::set_remote_id(&id);
    }

    fn goto_install(&mut self) {
        goto_install();
    }

    fn install_me(&mut self, _options: String, _path: String) {
        install_me(_options, _path, false, false);
    }

    fn update_me(&self, _path: String) {
        update_me(_path);
    }

    fn run_without_install(&self) {
        run_without_install();
    }

    fn show_run_without_install(&self) -> bool {
        show_run_without_install()
    }

    fn get_license(&self) -> String {
        get_license()
    }

    fn get_option(&self, key: String) -> String {
        get_option(key)
    }

    fn get_local_option(&self, key: String) -> String {
        get_local_option(key)
    }

    fn set_local_option(&self, key: String, value: String) {
        set_local_option(key, value);
    }

    fn peer_has_password(&self, id: String) -> bool {
        peer_has_password(id)
    }

    fn forget_password(&self, id: String) {
        forget_password(id)
    }

    fn get_peer_option(&self, id: String, name: String) -> String {
        get_peer_option(id, name)
    }

    fn set_peer_option(&self, id: String, name: String, value: String) {
        set_peer_option(id, name, value)
    }

    fn using_public_server(&self) -> bool {
        crate::using_public_server()
    }

    fn get_options(&self) -> Value {
        let hashmap: HashMap<String, String> =
            serde_json::from_str(&get_options()).unwrap_or_default();
        let mut m = Value::map();
        for (k, v) in hashmap {
            m.set_item(k, v);
        }
        m
    }

    fn test_if_valid_server(&self, host: String) -> String {
        test_if_valid_server(host)
    }

    fn get_sound_inputs(&self) -> Value {
        Value::from_iter(get_sound_inputs())
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
        set_options(m);
    }

    fn set_option(&self, key: String, value: String) {
        set_option(key, value);
    }

    fn install_path(&mut self) -> String {
        install_path()
    }

    fn get_socks(&self) -> Value {
        Value::from_iter(get_socks())
    }

    fn set_socks(&self, proxy: String, username: String, password: String) {
        set_socks(proxy, username, password)
    }

    fn is_installed(&self) -> bool {
        is_installed()
    }

    fn is_root(&self) -> bool {
        is_root()
    }

    fn is_release(&self) -> bool {
        #[cfg(not(debug_assertions))]
        return true;
        #[cfg(debug_assertions)]
        return false;
    }

    fn is_rdp_service_open(&self) -> bool {
        is_rdp_service_open()
    }

    fn is_share_rdp(&self) -> bool {
        is_share_rdp()
    }

    fn set_share_rdp(&self, _enable: bool) {
        set_share_rdp(_enable);
    }

    fn is_installed_lower_version(&self) -> bool {
        is_installed_lower_version()
    }

    fn closing(&mut self, x: i32, y: i32, w: i32, h: i32) {
        crate::server::input_service::fix_key_down_timeout_at_exit();
        LocalConfig::set_size(x, y, w, h);
    }

    fn get_size(&mut self) -> Value {
        let s = LocalConfig::get_size();
        let mut v = Vec::new();
        v.push(s.0);
        v.push(s.1);
        v.push(s.2);
        v.push(s.3);
        Value::from_iter(v)
    }

    fn get_mouse_time(&self) -> f64 {
        get_mouse_time()
    }

    fn check_mouse_time(&self) {
        check_mouse_time()
    }

    fn get_connect_status(&mut self) -> Value {
        let mut v = Value::array(0);
        let x = get_connect_status();
        v.push(x.status_num);
        v.push(x.key_confirmed);
        v.push(x.id);
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
        let c = get_peer(id.clone());
        Self::get_peer_value(id, c)
    }

    fn get_fav(&self) -> Value {
        Value::from_iter(get_fav())
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
        store_fav(tmp);
    }

    fn get_recent_sessions(&mut self) -> Value {
        // to-do: limit number of recent sessions, and remove old peer file
        let peers: Vec<Value> = PeerConfig::peers(None)
            .drain(..)
            .map(|p| Self::get_peer_value(p.0, p.2))
            .collect();
        Value::from_iter(peers)
    }

    fn get_icon(&mut self) -> String {
        get_icon()
    }

    fn remove_peer(&mut self, id: String) {
        PeerConfig::remove(&id);
    }

    fn remove_discovered(&mut self, id: String) {
        remove_discovered(id);
    }

    fn send_wol(&mut self, id: String) {
        crate::lan::send_wol(id)
    }

    fn new_remote(&mut self, id: String, remote_type: String, force_relay: bool) {
        new_remote(id, remote_type, force_relay)
    }

    fn is_process_trusted(&mut self, _prompt: bool) -> bool {
        is_process_trusted(_prompt)
    }

    fn is_can_screen_recording(&mut self, _prompt: bool) -> bool {
        is_can_screen_recording(_prompt)
    }

    fn is_installed_daemon(&mut self, _prompt: bool) -> bool {
        is_installed_daemon(_prompt)
    }

    fn get_error(&mut self) -> String {
        get_error()
    }

    fn is_login_wayland(&mut self) -> bool {
        is_login_wayland()
    }

    fn current_is_wayland(&mut self) -> bool {
        current_is_wayland()
    }

    fn get_software_update_url(&self) -> String {
        crate::SOFTWARE_UPDATE_URL.lock().unwrap().clone()
    }

    fn get_new_version(&self) -> String {
        get_new_version()
    }

    fn get_version(&self) -> String {
        get_version()
    }

    fn get_fingerprint(&self) -> String {
        get_fingerprint()
    }

    fn get_app_name(&self) -> String {
        get_app_name()
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
        let name = crate::SOFTWARE_UPDATE_URL
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
        create_shortcut(_id)
    }

    fn discover(&self) {
        std::thread::spawn(move || {
            allow_err!(crate::lan::discover());
        });
    }

    fn get_lan_peers(&self) -> String {
        // let peers = get_lan_peers()
        //     .into_iter()
        //     .map(|mut peer| {
        //         (
        //             peer.remove("id").unwrap_or_default(),
        //             peer.remove("username").unwrap_or_default(),
        //             peer.remove("hostname").unwrap_or_default(),
        //             peer.remove("platform").unwrap_or_default(),
        //         )
        //     })
        //     .collect::<Vec<(String, String, String, String)>>();
        serde_json::to_string(&get_lan_peers()).unwrap_or_default()
    }

    fn get_uuid(&self) -> String {
        get_uuid()
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
        reset_async_job_status();
        let old_id = self.get_id();
        change_id_shared(id, old_id);
    }

    fn post_request(&self, url: String, body: String, header: String) {
        post_request(url, body, header)
    }

    fn is_ok_change_id(&self) -> bool {
        hbb_common::machine_uid::get().is_ok()
    }

    fn get_async_job_status(&self) -> String {
        get_async_job_status()
    }

    fn t(&self, name: String) -> String {
        crate::client::translate(name)
    }

    fn is_xfce(&self) -> bool {
        crate::platform::is_xfce()
    }

    fn get_api_server(&self) -> String {
        get_api_server()
    }

    fn has_hwcodec(&self) -> bool {
        has_hwcodec()
    }

    fn has_gpucodec(&self) -> bool {
        has_gpucodec()
    }

    fn get_langs(&self) -> String {
        get_langs()
    }

    fn default_video_save_directory(&self) -> String {
        default_video_save_directory()
    }

    fn handle_relay_id(&self, id: String) -> String {
        handle_relay_id(&id).to_owned()
    }

    fn get_login_device_info(&self) -> String {
        get_login_device_info_json()
    }

    fn support_remove_wallpaper(&self) -> bool {
        support_remove_wallpaper()
    }

    fn has_valid_2fa(&self) -> bool {
        has_valid_2fa()
    }

    fn generate2fa(&self) -> String {
        generate2fa()
    }

    pub fn verify2fa(&self, code: String) -> bool {
        verify2fa(code)
    }

    fn generate_2fa_img_src(&self, data: String) -> String {
        let v = qrcode_generator::to_png_to_vec(data, qrcode_generator::QrCodeEcc::Low, 128)
            .unwrap_or_default();
        let s = hbb_common::sodiumoxide::base64::encode(
            v,
            hbb_common::sodiumoxide::base64::Variant::Original,
        );
        format!("data:image/png;base64,{s}")
    }
}

impl sciter::EventHandler for UI {
    sciter::dispatch_script_call! {
        fn t(String);
        fn get_api_server();
        fn is_xfce();
        fn using_public_server();
        fn get_id();
        fn temporary_password();
        fn update_temporary_password();
        fn permanent_password();
        fn set_permanent_password(String);
        fn get_remote_id();
        fn set_remote_id(String);
        fn closing(i32, i32, i32, i32);
        fn get_size();
        fn new_remote(String, String, bool);
        fn send_wol(String);
        fn remove_peer(String);
        fn remove_discovered(String);
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
        fn is_root();
        fn is_release();
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
        fn current_is_wayland();
        fn get_options();
        fn get_option(String);
        fn get_local_option(String);
        fn set_local_option(String, String);
        fn get_peer_option(String, String);
        fn peer_has_password(String);
        fn forget_password(String);
        fn set_peer_option(String, String, String);
        fn get_license();
        fn test_if_valid_server(String);
        fn get_sound_inputs();
        fn set_options(Value);
        fn set_option(String, String);
        fn get_software_update_url();
        fn get_new_version();
        fn get_version();
        fn get_fingerprint();
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
        fn has_gpucodec();
        fn get_langs();
        fn default_video_save_directory();
        fn handle_relay_id(String);
        fn get_login_device_info();
        fn support_remove_wallpaper();
        fn has_valid_2fa();
        fn generate2fa();
        fn generate_2fa_img_src(String);
        fn verify2fa(String);
    }
}

impl sciter::host::HostHandler for UIHostHandler {
    fn on_graphics_critical_failure(&mut self) {
        log::error!("Critical rendering error: e.g. DirectX gfx driver error. Most probably bad gfx drivers.");
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

// sacrifice some memory
pub fn value_crash_workaround(values: &[Value]) -> Arc<Vec<Value>> {
    let persist = Arc::new(values.to_vec());
    STUPID_VALUES.lock().unwrap().push(persist.clone());
    persist
}

pub fn get_icon() -> String {
    // 128x128
    #[cfg(target_os = "macos")]
    // 128x128 on 160x160 canvas, then shrink to 128, mac looks better with padding
    {
        "data:image/png;base64,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".into()
    }
    #[cfg(not(target_os = "macos"))] // 128x128 no padding
    {
        "data:image/png;base64,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".into()
    }
}
