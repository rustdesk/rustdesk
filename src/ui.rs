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
