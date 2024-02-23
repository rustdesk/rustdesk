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
        "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAIAAAACACAYAAADDPmHLAAABhGlDQ1BJQ0MgcHJvZmlsZQAAeJx9kT1Iw0AYht+mSkUqHewg4pChOlkQFXHUVihChVArtOpgcukfNGlIUlwcBdeCgz+LVQcXZ10dXAVB8AfE1cVJ0UVK/C4ptIjxjuMe3vvel7vvAKFZZZrVMwFoum1mUgkxl18VQ68QEEKYZkRmljEvSWn4jq97BPh+F+dZ/nV/jgG1YDEgIBLPMcO0iTeIZzZtg/M+cZSVZZX4nHjcpAsSP3Jd8fiNc8llgWdGzWwmSRwlFktdrHQxK5sa8TRxTNV0yhdyHquctzhr1Tpr35O/MFzQV5a5TmsEKSxiCRJEKKijgipsxGnXSbGQofOEj3/Y9UvkUshVASPHAmrQILt+8D/43VurODXpJYUTQO+L43yMAqFdoNVwnO9jx2mdAMFn4Erv+GtNYPaT9EZHix0BkW3g4rqjKXvA5Q4w9GTIpuxKQVpCsQi8n9E35YHBW6B/zetb+xynD0CWepW+AQ4OgbESZa/7vLuvu2//1rT79wPpl3Jwc6WkiQAAE5pJREFUeAHtXQt0VNW5/s5kkskkEyCEZwgQSIAEg6CgYBGKiFolwQDRlWW5BatiqiIWiYV6l4uq10fN9fq4rahYwAILXNAlGlAUgV5oSXiqDRggQIBAgJAEwmQeycycu//JDAwQyJzHPpPTmW+tk8yc2fucs//v23v/+3mMiCCsYQz1A0QQWkQEEOaICCDMERFAmCMigDBHRABhjogAwhwRAYQ5IgIIc0QEEOaICCDMobkAhg8f3m/cuHHjR40adXtGRkZmampqX4vFksR+MrPDoPXzhAgedtitVmttVVXVibKysn0lJSU7tm3btrm0tPSIlg+iiQDS0tK6FBQUzMjPz/+PlJSUIeyUoMV92zFI6PFM+PEsE/Rhx+i8vLyZ7JzIBFG2cuXKZQsXLlx8+PDhGt4PwlUAjPjuRUVFL2ZnZz9uNBrNPO/1bwKBMsjcuXPfZMeCzz///BP2/1UmhDO8bshFACaTybBgwYJZ7OFfZsR34HGPMIA5Nzf3GZZ5fsUy0UvMnu87nU6P2jdRXQCDBg3quXr16hVZWVnj1L52OIIy0Lx5895hQshl1cQjBw4cqFb1+mpe7L777hvOyP+C1W3Jal43AoAy1C4GJoJJGzZs2K3WdVUTwNSpU8cw56U4UuTzA2Ws4uLiTcyZzl6zZs1WNa6pigAo50fI1wZkY7I1qxLGq1ESKBaAr87/IkK+diBbk81HMCj1CRQJgLx9cvj0Uue7RRFnmSNd3+xBg0tEk0f0no82CLAYBSRGG9A9xuD93t5BNifbMw3craR1oEgA1NRrj96+yIiuaHRje10z9l5oRlmDCxU2N6ocLriIcy+/Yst/P9dCy3eBHT1MBgyIN2KwxYhhCdEY1SkGWZZoRAntSxhke+Jg/vz578q9hmwBUCcPtfPlxlcbF1mu/vpME76sdmLj2SZUOzw+glty+RVke78LpJTLv4nePyQLb9xqZxP+r9556ffEaAHjk2IxsUssctjRJSZKq6TdEMTBokWLVsrtLJItAOrhC3W972EEfnu6GUsqHVh7ygG7vyD05WYvm95sLbbyGdcVQWtx65tFrDljZ4cNRgNwLxPDjJ7xyO1qDmmVQRwQF5MnT35WVnw5kahvn7p35cRVA42sHF98xIF3Dtpw2OoJKMbRJpFKROAP72K+w/pzDqyvdaAnqy5+08uCp1Ms6BwdmlKBuGCcvMxKgXNS48oSQEFBwa9D0bfvcIv480EH3txvY86ceLl4J0giUrkI/OGrmf/10pEG/PH4RTzb24LCPh3QyajtoCZxwTh5tLCw8C3JceXcMD8//5dy4skFOXWrjzfhhT02VDLn7nJdroRI9URAP1lZqfRaZQM+PGXFK/064slkCwwaOo2Mk2maCGDkyJH9fEO6muCY1Y0nSxqx4VSzj3hpxGgpAgpf2+TBUwfr8c8LTnyamcSCaCMC4oS4KS0tPSolnmQB0GQOaDCeT2ZdesiJ2TttaGgOLOohixgtRUA/LmPO4rQe8bivs2Y1pUDcMAF8IiWSZAGMGDHidqlxpKKREV7wTxuWHbncDFOLGC1F8E2dQ0sBEDe3sX98BZCRkTFYahwpOMa8+ge/teKHOneLYTkQo5UIojSe+CSHG8kCSE1N7SM1TrDYe86FBzY04rTdoxKpwYQHt3tNTIpVxzBBguZXSo0jWQC+CZyqY9tpFyZ+3eir79XM2W2F53Mv6hf4eaK2ApDDjZxmoOqV2ncnXZjEyLe5fIblSEzr4dW91xOM/PcGdVLTRMFCMjdyBKBqL0fJGRce/IrIB+c6vq3w6tzriV7xWJjZSdM+gABI5iakC0MqLniQs97OvP6AkzoWwRO9GfmDQ0a+LIRMAA1NInLW2XDO7qvz/d263q/6E8HMPnH4QGfkE0IiAOrafXSjA+V1/iFbXGt4HYlgJsv5H9zUUXfkE0IigA/KmvG3w662SVOJVBqkG5FkxPDORmR2jELfeAO6mgyIMwreYDa36O3CPW7z4IDVhT3nm7Gjvtl7vq17eXN+lj7JJ2gugEPnPSjc2hR8zpUpAjNL2eQ+MXiorwkTekTDEi2NICcjf2ttE9accuKzk3bUNQVUVb57FaTG409DOsgin0rB4loHNtU7QI+W08WMMZ20bTYSNBUAJXrmRids5PRdIhCqiqCbWcCcwWY8MdCEzib5DRZTlIAJ3Uze4+0hCVhVZcefjtrwk9WN9PgoPJcWh+m9zbIGe5weEY+U1eJvNXZfmkS8deIi5vROwH+nJ8p+ZjnQVAB//cmFLVVu3zeJdXgbv8cywl64ORaFWbGSc3tbMLNrz+gb5z2UgsjP+6EWxefs1/g/bzMRjOloQm5X5fcJFpoJwNosYv62Zh+ZkOfIXef3O7pHYcnYeAzs2D7m6V0PNKFlKiOfZhNdLy3PV5zH/UlmmDSaZqaZAN7b04xT1gD2VRLB80Ni8fptse1+KjeRP+X7WnxF5PvRSlqP2F1YeNKK2aw60AKaCIDa/EU7XQG5X7kIWKmMD8fG4rFBJi2SoAhE/uQ9tfj6nBPBjHC+cawBM5PjWdXDf2qZJgL46AcX6gOEr1QERP6K8WY8nBajxeMrgp3I312HDV7yEVRaTzs9WFzdiKdS+JcC3AXgZk7P+7tdrRbfckXw0Vj9kP/grjp8S+RLrPreOWFFQS/+8wq5C2DdEQ+ONwScUCiCwmEm/Dqj/ZNPxf6kHXXY6M/5EtN6yObCxjqnd/0BT3AXwJJ/tZb75YlgdM8ovDay/df5hJcPWrGxpkmR4JewakDXAjjvELGuwnOd3CzNMGbWtl9ytxnGdu7tE6jD66NKW/BO7XVEsLbGDqvbAwtHZ5CrAIj8JteNivTgDTP/1hikd9THLnK0LLHWGZgOyBIBTZD5mjUb87rz6xjiLAB3EPV624bpGS/g+Vvaf73vB/UcDk4wYv9Fl7TmbSt2+lKvAvAu3DzqS4lCETx/azTiVO7e5Y1Z/ePwm+/J+5XYx3FV+G+ZAKhK4bXAhJsAys+JONeIAA8YkCOCeJbxH78pmtdjcsO03rF4oewiLvo3JJApAlp7WGF3YUAcHxtwE0DJSX/ul9LMu9YwU9ON6GjSV+4nWIwGTEmOxdLjdskdXVeH336+SX8C2Hval1jJbf0rDfPwgPY9wHMjTOlpwtJjdskdXVeH39vQjF9x2oSHmwD2nQ1MKGSJIJZxP76PfgUwvlsMjLSfgBhsutGqncqsLm7PyE0Ah2p92V92r5+A23sYYDbqr/j3g6qBYR2N2FVPBMoXwaFGnQmAdtCovggo7f8f3l0f7f4b4ZZO0S0CUDD4VWV3e3c447FJFRcBnG2kQaCAEzJFkJmkfwEMshhl+kKXw9McqpomD3qY1K8OuQigjqa6icravxS+bwf9Fv9+9DYbrkqrPBHUNetIAFanKClx1zNGV7P+BZAU4yvFFIqgpT9BfXARQJN/3qdCEXBq+moKasm0XgVIE4F/V1O1wakVIAQk2vddhgj0n/8pmcINmsPBi4AP/ZwE4N1EU4WlXLZm6B5Wf1ewwmVoMXoaC0jwD9wpFEHLwlF9o8bpCaI53LadLJz6Q7gIIJG2KVDY9KHPJy7oXwCVVneQgr+xnWgncx7gIoBuFoAm7ngUiqC8Vv8C2H/B5xErEAFR3z1GRwKgaVsprA1//Lz0zp/A8Lur9S+AnbW+XkAFS9OTYw3cpsJxGwtI7wwmAGnt/qsNU3pSZE1K5gBF6bM9cKLRjcMXL21hLlsE6fH8Jm5xu3JWdwGbDouSO38Cw1ubgH+cEHFXqj4FsO6kkrWQlz/flKBDAQzrGZg4+SJYU+5mAtDnmMCqSqfCllDLZxpR5AVuV77Dv52kxM6fq8Ov3OdB0QQRsTobFj7U4Mbfz/iGcRWK4I7O/CbEchPAoK4CulsEnLFK6/y52jC1jSJWMRFMH6qviSHv/uSASNW/AEUtoSSTgMwEfmnnJgBKz4R0YPleKWr3nbwq/J936UsAVY0efHLQtx5Q4VrIu7uauK4P5LouICdTwPI9Pi9IgQjKzuqrOfife+xweDe+hCL/h37K7sl3KRxXAdw/CKzuRosxFIigfyf91P9bqpvxaUVTyxeF/g91/mX35LsghqsAOsQKmDQY+OxHMegirzXDzB6pj1bA+SYRj261+ZKkvOp7oEcMEjn1APrBfXXwjBFMAD9ApgcMFNwWhcduaf8CoJVQM/5uQ2XDVZtfKhDB9FT+28ZxF8C9AwX07wwcqZPuAT/Fcv7/TjRwWxalJn5X6sDayubW0yJDBL3MBuQk818PyV0AtLJ59p3sWCvN+Xmakf++Tsh/ebcDRT86L59QQQSzBmizFF6TPYIeGwm8+h1QYw1OBLPuEPCuDsinYr9wuwNv/+jbCKItkoMUQcdoAU+ma7NrqCYCiI8R8LtxIuYWo816b/ZoA/7HS74WTyYf9U4R07+z48tjzdKqtiB2RZ+TYUYnzs6fH5rtE/jUaOD9bcCx87iuCJ4bLeBtHZC/8YQLj2224ziHfQ97xBrw2wzt3jSmmQBoi5e3ckQ8/ClaNcScMQKKFJBPxTGNHiaw0oaXgI4xD//3251YcShgqZeMzp0bieDVYXFI0HAvBE33Cs67WcC88SLe3OyzjUhkiXjxbgEv3yuPOIdLxB+2uPHhHo93L8L+icAztxswY2gUEmPVMeT+Wg/e+b4JS8td3vkJavTwtSaC0V2j8GiatptgaSoAssHrEwXk3yLim4Mtaf9FhoCsHvKIsjWLmLTCje+O+iZdsMscqWelyQY3XtzsRs5AA6YMMmBCfwOSJCwyIZ4qznuw/qgbqw66sP20+9L1LxMMVUVA6wc+/pm27xsmhOSFEUOTBXYouwaRn7PcjU1HxFY9cHuTiM/2efDZfo/358FdgVuY0AYlGZCSICApDt53ChAfVubH1dhFbxG/v1bEzjMenGz1tfS+LxzeVPL6rXHel1lojZC+NEoubPS+oeUeH/lo09D0d99ZdtQQqZdLi0se+TWfA26mRvHe1oBPSgyezQzN/oe6E4CX/GU+8pV64FeE55Oz2wqf3sGAT8fGheyVM7oSgJf8v3p8cw3BgRhtRZBoMuCLeyze/6GCbgTQyMiftJRyPjgTo40IzKy6//yeeGR2Cu1EFzkCoEpUU8kS+TlLRGw+EnBSxyKgae6rJ8RhbE/V85+n7SBXQs4T0PYP8TLiyQJtN5O7lJFfgVa9fb2JgFoeq++NwwN9uKx9t0uNIFkAVqu11mKxaCaAFXuAjQfBzQPXUgSJMQLW3h+HMcl8al7iRmocyU9SWVl5PCsrq0/bIdXBxkPg5oEHF16dew3oyBy+iWZkJPKr8xk3x6TGkSyA8vLy/UwAd0qNJxdGv7ehYxHk9DNi6T1m5u0LqtmlNRA3UuNIFsCuXbt25OXlzZQaTy5yBgOLd4ADqVLDS49rZtX86z+LwbNDozWZ21BSUrJDahzJAtiyZcsmtCSRf4oYcrMETB8hYuku6EoEdyYb8PGEWFbka9ZgErdt27ZJaiTJAigtLT1aVVX1r5SUlJulxpUDsvHifAETBoqYtw44STuwt2MR9Igz4LU7ozF9sFHT3j3ihHFTKTWeLHd05cqVy+bOnftHOXHlgOw4bbiAKUNEvLcNeGsLUGdrXyLoZALmjDDit7dGwxKjHfF+ECdy4skSwMKFCxc/99xzfzAajdpNXWGIi6H5BMDTo0V8XAK89w8Bx+pDK4LeCQJm3WrEzKGh29be5XLZiBM5cWUJ4PDhw+eKi4sX5ebmzpITXykSmKHn/ByYPUbEV+UCFjP/YF25CKfCFUjBho8xinggzYAZQ4yYmMZv945gwbj4hDiRE1d2jwSrAv4rOzt7OisFOsi9hlJEMcNns1YCHQ0OZohyYP1PIr6pEFDTqK4I6IXe4/sJyEmPwgPpBtVmGykFy/0NxIXc+LIFwBR3pqio6KV58+a9I/caaoKWoT0yDOwQvNyV14goOQ58Xy16F5dW1ArMgRTh9rdfrrchE/vXqwNtcWPATd0E7ySSkb0EZHYRQjZkeyMQB8SF3PiK+iQXLFjwPisFcrOyssYpuY7aIJ4yGXmZ3bzfLp2ncYWzVnjnDl50tmxpS3MSaREmVSu0vV23eIS8SA8WZWVlW4gDJddQJACn0+nJy8t7ZBeDxWLh9FIT9UDEJrPcnXxFpaUPsq+G1Wo9RbYnDpRcR/GoxIEDB6rZg+QwR2RzKP2BcALV+8zmk8j2Sq+lyrDUhg0b9uTn52eztmhxRAR8QeSTrZnNd6txPdXGJdesWbOV+QN3rV69+ks9VAd6hK/Yn6QW+QRVB6apJBjBwESwnDmGd6l57XAHOXxU56tR7AdC9ZkJ9IBMAxOYd/oMa5++EqkSlIGKfGrqkbev1OFrDVymptCDzp8//71FixateuONN36fm5v7OBMCvzcg/xuCEW+n3lbq5FHSzm8LXGcF04M/9NBDs9PS0l4pKCiYwZyXab5RRH22vfhDrKqqKqOBHerbZ/ar4X1DTaaFUz91YWFhER3Dhw9PHTdu3PhRo0bdnpGRMTg1NbUvcxqTWDAaWGr/mwGpAyrK7TSHj6bYlZeX7yspKdlJ4/k03K7lg2i+LmD37t2V7PgL+/gXre8dwbXQzcKQCPggIoAwR0QAYY6IAMIcEQGEOSICCHNEBBDmiAggzBERQJgjIoAwR0QAYY7/B1LDyJ6QBLUVAAAAAElFTkSuQmCC".into()
    }
    #[cfg(not(target_os = "macos"))] // 128x128 no padding
    {
        "data:image/png;base64,"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Smv5K3Vg5pP0WkuLHAF928DFdUtT9oDLHWDoSZcN2ZH8NIV8Hng/o2/KAP23QPea21tzH6cPQIq6StwAB4fAaIGy1z3e3dXe279nmv39AEs1cpfuQbuwAAANGmlUWHRYTUw6Y29tLmFkb2JlLnhtcAAAAAAAPD94cGFja2V0IGJlZ2luPSLvu78iIGlkPSJXNU0wTXBDZWhpSHpyZVN6TlRjemtjOWQiPz4KPHg6eG1wbWV0YSB4bWxuczp4PSJhZG9iZTpuczptZXRhLyIgeDp4bXB0az0iWE1QIENvcmUgNC40LjAtRXhpdjIiPgogPHJkZjpSREYgeG1sbnM6cmRmPSJodHRwOi8vd3d3LnczLm9yZy8xOTk5LzAyLzIyLXJkZi1zeW50YXgtbnMjIj4KICA8cmRmOkRlc2NyaXB0aW9uIHJkZjphYm91dD0iIgogICAgeG1sbnM6eG1wTU09Imh0dHA6Ly9ucy5hZG9iZS5jb20veGFwLzEuMC9tbS8iCiAgICB4bWxuczpzdEV2dD0iaHR0cDovL25zLmFkb2JlLmNvbS94YXAvMS4wL3NUeXBlL1Jlc291cmNlRXZlbnQjIgogICAgeG1sbnM6ZGM9Imh0dHA6Ly9wdXJsLm9yZy9kYy9lbGVtZW50cy8xLjEvIgogICAgeG1sbnM6R0lNUD0iaHR0cDovL3d3dy5naW1wLm9yZy94bXAvIgogICAgeG1sbnM6dGlmZj0iaHR0cDovL25zLmFkb2JlLmNvbS90aWZmLzEuMC8iCiAgICB4bWxuczp4bXA9Imh0dHA6Ly9ucy5hZG9iZS5jb20veGFwLzEuMC8iCiAgIHhtcE1NOkRvY3VtZW50SUQ9ImdpbXA6ZG9jaWQ6Z2ltcDo0MGQ2ZTQ4Zi1mNDRkLTQ0MGQtODFkMS1iNjNmYTJkMDgwMTIiCiAgIHhtcE1NOkluc3RhbmNlSUQ9InhtcC5paWQ6MmZhMmY5MmYtZmRhNC00OTJhLTkyZDMtODdiNDVkMjU3Yzk3IgogICB4bXBNTTpPcmlnaW5hbERvY3VtZW50SUQ9InhtcC5kaWQ6ZWM5ODMxNGMtMjI1Ny00ZjA4LWFlZTktYzRkZWJhMmM2NWQwIgogICBkYzpGb3JtYXQ9ImltYWdlL3BuZyIKICAgR0lNUDpBUEk9IjIuMCIKICAgR0lNUDpQbGF0Zm9ybT0iTGludXgiCiAgIEdJTVA6VGltZVN0YW1wPSIxNzA4Njk4MTU2ODgxNzcxIgogICBHSU1QOlZlcnNpb249IjIuMTAuMzAiCiAgIHRpZmY6T3JpZW50YXRpb249IjEiCiAgIHhtcDpDcmVhdG9yVG9vbD0iR0lNUCAyLjEwIj4KICAgPHhtcE1NOkhpc3Rvcnk+CiAgICA8cmRmOlNlcT4KICAgICA8cmRmOmxpCiAgICAgIHN0RXZ0OmFjdGlvbj0ic2F2ZWQiCiAgICAgIHN0RXZ0OmNoYW5nZWQ9Ii8iCiAgICAgIHN0RXZ0Omluc3RhbmNlSUQ9InhtcC5paWQ6ODFiMmNhZmEtMTU0OS00NjVkLTkzZDMtZmZiY2YyNzczNzQzIgogICAgICBzdEV2dDpzb2Z0d2FyZUFnZW50PSJHaW1wIDIuMTAgKExpbnV4KSIKICAgICAgc3RFdnQ6d2hlbj0iMjAyNC0wMi0yM1QxNToyMjozNiswMTowMCIvPgogICAgPC9yZGY6U2VxPgogICA8L3htcE1NOkhpc3Rvcnk+CiAgPC9yZGY6RGVzY3JpcHRpb24+CiA8L3JkZjpSREY+CjwveDp4bXBtZXRhPgogICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgCiAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAKICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgIAogICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgCiAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAKICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgIAogICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgCiAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAKICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgIAogICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgCiAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAKICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgIAogICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgCiAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAKICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgIAogICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgCiAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAKICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgIAogICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgCiAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAKICAgICAgICAgICAgICAgICAgICAgICAgICAgCjw/eHBhY2tldCBlbmQ9InciPz5gQlNaAAAABmJLR0QA/wD/AP+gvaeTAAAACXBIWXMAAEiuAABIrgHwmhA7AAAAB3RJTUUH6AIXDhYkq8hzBQAAFWRJREFUeNrtnXt4VNW1wH/rzOTFI4CQIC+BWvmUaNWCoq0KQcEr2Hq9CtJbQGx9tGqxPEPxNT7ABApa9Npii1aqrQZKaUFB26r4RFvUtgK2oGhVrCSB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.into()
    }
}
