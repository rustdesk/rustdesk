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

    fn has_vram(&self) -> bool {
        has_vram()
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
        fn has_vram();
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
