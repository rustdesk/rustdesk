use std::{
    collections::HashMap,
    process::Child,
    sync::{Arc, Mutex},
};

#[cfg(any(target_os = "android", target_os = "ios"))]
use hbb_common::password_security;
use hbb_common::{
    allow_err,
    config::{self, Config, LocalConfig, PeerConfig},
    directories_next, log, sleep,
    tokio::{self, sync::mpsc, time},
};

use hbb_common::{
    config::{RENDEZVOUS_PORT, RENDEZVOUS_TIMEOUT},
    futures::future::join_all,
    protobuf::Message as _,
    rendezvous_proto::*,
};

#[cfg(feature = "flutter")]
use crate::hbbs_http::account;
use crate::{common::SOFTWARE_UPDATE_URL, ipc};

type Message = RendezvousMessage;

pub type Children = Arc<Mutex<(bool, HashMap<(String, String), Child>)>>;
type Status = (i32, bool, i64, String); // (status_num, key_confirmed, mouse_time, id)

lazy_static::lazy_static! {
    static ref UI_STATUS : Arc<Mutex<Status>> = Arc::new(Mutex::new((0, false, 0, "".to_owned())));
    static ref OPTIONS : Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(Config::get_options()));
    static ref ASYNC_JOB_STATUS : Arc<Mutex<String>> = Default::default();
    static ref TEMPORARY_PASSWD : Arc<Mutex<String>> = Arc::new(Mutex::new("".to_owned()));
    pub static ref OPTION_SYNCED : Arc<Mutex<bool>> = Default::default();
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
lazy_static::lazy_static! {
    pub static ref SENDER : Mutex<mpsc::UnboundedSender<ipc::Data>> = Mutex::new(check_connect_status(true));
}

#[cfg(any(target_os = "android", target_os = "ios", feature = "flutter"))]
#[inline]
pub fn get_id() -> String {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    return Config::get_id();
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    return ipc::get_id();
}

#[inline]
pub fn goto_install() {
    allow_err!(crate::run_me(vec!["--install"]));
}

#[inline]
pub fn install_me(_options: String, _path: String, _silent: bool, _debug: bool) {
    #[cfg(windows)]
    std::thread::spawn(move || {
        allow_err!(crate::platform::windows::install_me(
            &_options, _path, _silent, _debug
        ));
        std::process::exit(0);
    });
}

#[inline]
pub fn update_me(_path: String) {
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

#[inline]
pub fn run_without_install() {
    crate::run_me(vec!["--noinstall"]).ok();
    std::process::exit(0);
}

#[inline]
pub fn show_run_without_install() -> bool {
    let mut it = std::env::args();
    if let Some(tmp) = it.next() {
        if crate::is_setup(&tmp) {
            return it.next() == None;
        }
    }
    false
}

#[inline]
pub fn get_license() -> String {
    #[cfg(windows)]
    if let Some(lic) = crate::platform::windows::get_license() {
        #[cfg(feature = "flutter")]
        return format!("Key: {}\nHost: {}\nApi: {}", lic.key, lic.host, lic.api);
        // default license format is html formed (sciter)
        #[cfg(not(feature = "flutter"))]
        return format!(
            "<br /> Key: {} <br /> Host: {} Api: {}",
            lic.key, lic.host, lic.api
        );
    }
    Default::default()
}

#[inline]
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub fn get_option_opt(key: &str) -> Option<String> {
    OPTIONS.lock().unwrap().get(key).map(|x| x.clone())
}

#[inline]
pub fn get_option(key: String) -> String {
    get_option_(&key)
}

#[inline]
fn get_option_(key: &str) -> String {
    let map = OPTIONS.lock().unwrap();
    if let Some(v) = map.get(key) {
        v.to_owned()
    } else {
        "".to_owned()
    }
}

#[inline]
pub fn get_local_option(key: String) -> String {
    LocalConfig::get_option(&key)
}

#[inline]
pub fn set_local_option(key: String, value: String) {
    LocalConfig::set_option(key, value);
}

#[cfg(any(target_os = "android", target_os = "ios", feature = "flutter"))]
#[inline]
pub fn get_local_flutter_config(key: String) -> String {
    LocalConfig::get_flutter_config(&key)
}

#[cfg(any(target_os = "android", target_os = "ios", feature = "flutter"))]
#[inline]
pub fn set_local_flutter_config(key: String, value: String) {
    LocalConfig::set_flutter_config(key, value);
}

#[cfg(feature = "flutter")]
#[inline]
pub fn get_kb_layout_type() -> String {
    LocalConfig::get_kb_layout_type()
}

#[cfg(feature = "flutter")]
#[inline]
pub fn set_kb_layout_type(kb_layout_type: String) {
    LocalConfig::set_kb_layout_type(kb_layout_type);
}

#[inline]
pub fn peer_has_password(id: String) -> bool {
    !PeerConfig::load(&id).password.is_empty()
}

#[inline]
pub fn forget_password(id: String) {
    let mut c = PeerConfig::load(&id);
    c.password.clear();
    c.store(&id);
}

#[inline]
pub fn get_peer_option(id: String, name: String) -> String {
    let c = PeerConfig::load(&id);
    c.options.get(&name).unwrap_or(&"".to_owned()).to_owned()
}

#[inline]
pub fn set_peer_option(id: String, name: String, value: String) {
    let mut c = PeerConfig::load(&id);
    if value.is_empty() {
        c.options.remove(&name);
    } else {
        c.options.insert(name, value);
    }
    c.store(&id);
}

#[inline]
pub fn using_public_server() -> bool {
    option_env!("RENDEZVOUS_SERVER").unwrap_or("").is_empty()
        && crate::get_custom_rendezvous_server(get_option_("custom-rendezvous-server")).is_empty()
}

#[inline]
pub fn get_options() -> String {
    let options = OPTIONS.lock().unwrap();
    let mut m = serde_json::Map::new();
    for (k, v) in options.iter() {
        m.insert(k.into(), v.to_owned().into());
    }
    serde_json::to_string(&m).unwrap()
}

#[inline]
pub fn test_if_valid_server(host: String) -> String {
    hbb_common::socket_client::test_if_valid_server(&host)
}

#[inline]
#[cfg(feature = "flutter")]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn get_sound_inputs() -> Vec<String> {
    let mut a = Vec::new();
    #[cfg(not(target_os = "linux"))]
    {
        fn get_sound_inputs_() -> Vec<String> {
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

        let inputs = Arc::new(Mutex::new(Vec::new()));
        let cloned = inputs.clone();
        // can not call below in UI thread, because conflict with sciter sound com initialization
        std::thread::spawn(move || *cloned.lock().unwrap() = get_sound_inputs_())
            .join()
            .ok();
        for name in inputs.lock().unwrap().drain(..) {
            a.push(name);
        }
    }
    #[cfg(target_os = "linux")]
    {
        let inputs: Vec<String> = crate::platform::linux::get_pa_sources()
            .drain(..)
            .map(|x| x.1)
            .collect();

        for name in inputs {
            a.push(name);
        }
    }
    a
}

#[inline]
pub fn set_options(m: HashMap<String, String>) {
    *OPTIONS.lock().unwrap() = m.clone();
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    ipc::set_options(m).ok();
    #[cfg(any(target_os = "android", target_os = "ios"))]
    Config::set_options(m);
}

#[inline]
pub fn set_option(key: String, value: String) {
    let mut options = OPTIONS.lock().unwrap();
    #[cfg(target_os = "macos")]
    if &key == "stop-service" {
        let is_stop = value == "Y";
        if is_stop && crate::platform::macos::uninstall() {
            return;
        }
    }
    if value.is_empty() {
        options.remove(&key);
    } else {
        options.insert(key.clone(), value.clone());
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    ipc::set_options(options.clone()).ok();
    #[cfg(any(target_os = "android", target_os = "ios"))]
    Config::set_option(key, value);
}

#[inline]
pub fn install_path() -> String {
    #[cfg(windows)]
    return crate::platform::windows::get_install_info().1;
    #[cfg(not(windows))]
    return "".to_owned();
}

#[inline]
pub fn get_socks() -> Vec<String> {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    return Vec::new();
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let s = ipc::get_socks();
        match s {
            None => Vec::new(),
            Some(s) => {
                let mut v = Vec::new();
                v.push(s.proxy);
                v.push(s.username);
                v.push(s.password);
                v
            }
        }
    }
}

#[inline]
pub fn set_socks(proxy: String, username: String, password: String) {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    ipc::set_socks(config::Socks5Server {
        proxy,
        username,
        password,
    })
    .ok();
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[inline]
pub fn is_installed() -> bool {
    crate::platform::is_installed()
}

#[cfg(any(target_os = "android", target_os = "ios"))]
#[inline]
pub fn is_installed() -> bool {
    false
}

#[inline]
pub fn is_rdp_service_open() -> bool {
    #[cfg(windows)]
    return is_installed() && crate::platform::windows::is_rdp_service_open();
    #[cfg(not(windows))]
    return false;
}

#[inline]
pub fn is_share_rdp() -> bool {
    #[cfg(windows)]
    return crate::platform::windows::is_share_rdp();
    #[cfg(not(windows))]
    return false;
}

#[inline]
pub fn set_share_rdp(_enable: bool) {
    #[cfg(windows)]
    crate::platform::windows::set_share_rdp(_enable);
}

#[inline]
pub fn is_installed_lower_version() -> bool {
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

#[inline]
pub fn get_mouse_time() -> f64 {
    let ui_status = UI_STATUS.lock().unwrap();
    let res = ui_status.2 as f64;
    return res;
}

#[inline]
pub fn check_mouse_time() {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let sender = SENDER.lock().unwrap();
        allow_err!(sender.send(ipc::Data::MouseMoveTime(0)));
    }
}

#[inline]
pub fn get_connect_status() -> Status {
    let ui_statue = UI_STATUS.lock().unwrap();
    let res = ui_statue.clone();
    res
}

#[inline]
pub fn temporary_password() -> String {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    return password_security::temporary_password();
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    return TEMPORARY_PASSWD.lock().unwrap().clone();
}

#[inline]
pub fn update_temporary_password() {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    password_security::update_temporary_password();
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    allow_err!(ipc::update_temporary_password());
}

#[inline]
pub fn permanent_password() -> String {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    return Config::get_permanent_password();
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    return ipc::get_permanent_password();
}

#[inline]
pub fn set_permanent_password(password: String) {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    Config::set_permanent_password(&password);
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    allow_err!(ipc::set_permanent_password(password));
}

#[inline]
pub fn get_peer(id: String) -> PeerConfig {
    PeerConfig::load(&id)
}

#[inline]
pub fn get_fav() -> Vec<String> {
    LocalConfig::get_fav()
}

#[inline]
pub fn store_fav(fav: Vec<String>) {
    LocalConfig::set_fav(fav);
}

#[inline]
pub fn is_process_trusted(_prompt: bool) -> bool {
    #[cfg(target_os = "macos")]
    return crate::platform::macos::is_process_trusted(_prompt);
    #[cfg(not(target_os = "macos"))]
    return true;
}

#[inline]
pub fn is_can_screen_recording(_prompt: bool) -> bool {
    #[cfg(target_os = "macos")]
    return crate::platform::macos::is_can_screen_recording(_prompt);
    #[cfg(not(target_os = "macos"))]
    return true;
}

#[inline]
pub fn is_installed_daemon(_prompt: bool) -> bool {
    #[cfg(target_os = "macos")]
    return crate::platform::macos::is_installed_daemon(_prompt);
    #[cfg(not(target_os = "macos"))]
    return true;
}

#[inline]
#[cfg(feature = "flutter")]
pub fn is_can_input_monitoring(_prompt: bool) -> bool {
    #[cfg(target_os = "macos")]
    return crate::platform::macos::is_can_input_monitoring(_prompt);
    #[cfg(not(target_os = "macos"))]
    return true;
}

#[inline]
pub fn get_error() -> String {
    #[cfg(not(any(feature = "cli")))]
    #[cfg(target_os = "linux")]
    {
        let dtype = crate::platform::linux::get_display_server();
        if "wayland" == dtype {
            return crate::server::wayland::common_get_error();
        }
        if dtype != "x11" {
            return format!(
                "{} {}, {}",
                t("Unsupported display server ".to_owned()),
                dtype,
                t("x11 expected".to_owned()),
            );
        }
    }
    return "".to_owned();
}

#[inline]
pub fn is_login_wayland() -> bool {
    #[cfg(target_os = "linux")]
    return crate::platform::linux::is_login_wayland();
    #[cfg(not(target_os = "linux"))]
    return false;
}

#[inline]
pub fn current_is_wayland() -> bool {
    #[cfg(target_os = "linux")]
    return crate::platform::linux::current_is_wayland();
    #[cfg(not(target_os = "linux"))]
    return false;
}

#[inline]
pub fn get_new_version() -> String {
    hbb_common::get_version_from_url(&*SOFTWARE_UPDATE_URL.lock().unwrap())
}

#[inline]
pub fn get_version() -> String {
    crate::VERSION.to_owned()
}

#[cfg(any(target_os = "android", target_os = "ios", feature = "flutter"))]
#[inline]
pub fn get_app_name() -> String {
    crate::get_app_name()
}

#[cfg(windows)]
#[inline]
pub fn create_shortcut(_id: String) {
    crate::platform::windows::create_shortcut(&_id).ok();
}

#[cfg(any(target_os = "android", target_os = "ios", feature = "flutter"))]
#[inline]
pub fn discover() {
    std::thread::spawn(move || {
        allow_err!(crate::lan::discover());
    });
}

#[cfg(feature = "flutter")]
pub fn peer_to_map(id: String, p: PeerConfig) -> HashMap<&'static str, String> {
    HashMap::<&str, String>::from_iter([
        ("id", id),
        ("username", p.info.username.clone()),
        ("hostname", p.info.hostname.clone()),
        ("platform", p.info.platform.clone()),
        (
            "alias",
            p.options.get("alias").unwrap_or(&"".to_owned()).to_owned(),
        ),
    ])
}

#[inline]
pub fn get_lan_peers() -> Vec<HashMap<&'static str, String>> {
    config::LanPeers::load()
        .peers
        .iter()
        .map(|peer| {
            HashMap::<&str, String>::from_iter([
                ("id", peer.id.clone()),
                ("username", peer.username.clone()),
                ("hostname", peer.hostname.clone()),
                ("platform", peer.platform.clone()),
            ])
        })
        .collect()
}

#[inline]
pub fn get_uuid() -> String {
    base64::encode(hbb_common::get_uuid())
}

#[cfg(any(target_os = "android", target_os = "ios", feature = "flutter"))]
#[inline]
pub fn change_id(id: String) {
    *ASYNC_JOB_STATUS.lock().unwrap() = " ".to_owned();
    let old_id = get_id();
    std::thread::spawn(move || {
        *ASYNC_JOB_STATUS.lock().unwrap() = change_id_shared(id, old_id).to_owned();
    });
}

#[inline]
pub fn post_request(url: String, body: String, header: String) {
    *ASYNC_JOB_STATUS.lock().unwrap() = " ".to_owned();
    std::thread::spawn(move || {
        *ASYNC_JOB_STATUS.lock().unwrap() = match crate::post_request_sync(url, body, &header) {
            Err(err) => err.to_string(),
            Ok(text) => text,
        };
    });
}

#[inline]
pub fn get_async_job_status() -> String {
    ASYNC_JOB_STATUS.lock().unwrap().clone()
}

#[inline]
pub fn get_langs() -> String {
    crate::lang::LANGS.to_string()
}

#[inline]
pub fn default_video_save_directory() -> String {
    let appname = crate::get_app_name();

    #[cfg(any(target_os = "android", target_os = "ios"))]
    if let Ok(home) = config::APP_HOME_DIR.read() {
        let mut path = home.to_owned();
        path.push_str("/RustDesk/ScreenRecord");
        return path;
    }

    if let Some(user) = directories_next::UserDirs::new() {
        if let Some(video_dir) = user.video_dir() {
            return video_dir.join(appname).to_string_lossy().to_string();
        }
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    if let Some(home) = crate::platform::get_active_user_home() {
        let name = if cfg!(target_os = "macos") {
            "Movies"
        } else {
            "Videos"
        };
        return home.join(name).join(appname).to_string_lossy().to_string();
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            return dir.join("videos").to_string_lossy().to_string();
        }
    }
    "".to_owned()
}

#[inline]
pub fn get_api_server() -> String {
    crate::get_api_server(
        get_option_("api-server"),
        get_option_("custom-rendezvous-server"),
    )
}

#[inline]
pub fn has_hwcodec() -> bool {
    #[cfg(not(any(feature = "hwcodec", feature = "mediacodec")))]
    return false;
    #[cfg(any(feature = "hwcodec", feature = "mediacodec"))]
    return true;
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[inline]
pub fn is_root() -> bool {
    crate::platform::is_root()
}

#[cfg(any(target_os = "android", target_os = "ios"))]
#[inline]
pub fn is_root() -> bool {
    false
}

#[cfg(any(target_os = "android", target_os = "ios", feature = "flutter"))]
#[inline]
pub fn check_super_user_permission() -> bool {
    #[cfg(feature = "flatpak")]
    return true;
    #[cfg(any(windows, target_os = "linux"))]
    return crate::platform::check_super_user_permission().unwrap_or(false);
    #[cfg(not(any(windows, target_os = "linux")))]
    true
}

#[allow(dead_code)]
pub fn check_zombie(children: Children) {
    let mut deads = Vec::new();
    loop {
        let mut lock = children.lock().unwrap();
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

pub fn start_option_status_sync() {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let _sender = SENDER.lock().unwrap();
    }
}

// not call directly
fn check_connect_status(reconnect: bool) -> mpsc::UnboundedSender<ipc::Data> {
    let (tx, rx) = mpsc::unbounded_channel::<ipc::Data>();
    std::thread::spawn(move || check_connect_status_(reconnect, rx));
    tx
}

#[cfg(feature = "flutter")]
pub fn account_auth(op: String, id: String, uuid: String) {
    account::OidcSession::account_auth(op, id, uuid);
}

#[cfg(feature = "flutter")]
pub fn account_auth_cancel() {
    account::OidcSession::auth_cancel();
}

#[cfg(feature = "flutter")]
pub fn account_auth_result() -> String {
    serde_json::to_string(&account::OidcSession::get_result()).unwrap_or_default()
}

#[cfg(feature = "flutter")]
pub fn set_user_default_option(key: String, value: String) {
    use hbb_common::config::UserDefaultConfig;
    UserDefaultConfig::load().set(key, value);
}

#[cfg(feature = "flutter")]
pub fn get_user_default_option(key: String) -> String {
    use hbb_common::config::UserDefaultConfig;
    UserDefaultConfig::load().get(&key)
}

// notice: avoiding create ipc connection repeatedly,
// because windows named pipe has serious memory leak issue.
#[tokio::main(flavor = "current_thread")]
async fn check_connect_status_(reconnect: bool, rx: mpsc::UnboundedReceiver<ipc::Data>) {
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
                                UI_STATUS.lock().unwrap().2 = v;
                            }
                            Ok(Some(ipc::Data::Options(Some(v)))) => {
                                *OPTIONS.lock().unwrap() = v;
                                *OPTION_SYNCED.lock().unwrap() = true;
                            }
                            Ok(Some(ipc::Data::Config((name, Some(value))))) => {
                                if name == "id" {
                                    id = value;
                                } else if name == "temporary-password" {
                                    *TEMPORARY_PASSWD.lock().unwrap() = value;
                                }
                            }
                            Ok(Some(ipc::Data::OnlineStatus(Some((mut x, c))))) => {
                                if x > 0 {
                                    x = 1
                                }
                                key_confirmed = c;
                                *UI_STATUS.lock().unwrap() = (x as _, key_confirmed, mouse_time, id.clone());
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
                        c.send(&ipc::Data::Config(("temporary-password".to_owned(), None))).await.ok();
                    }
                }
            }
        }
        if !reconnect {
            OPTIONS
                .lock()
                .unwrap()
                .insert("ipc-closed".to_owned(), "Y".to_owned());
            break;
        }
        *UI_STATUS.lock().unwrap() = (-1, key_confirmed, mouse_time, id.clone());
        sleep(1.).await;
    }
}

#[allow(dead_code)]
pub fn option_synced() -> bool {
    OPTION_SYNCED.lock().unwrap().clone()
}

#[cfg(any(target_os = "android", target_os = "ios", feature = "flutter"))]
#[tokio::main(flavor = "current_thread")]
pub(crate) async fn send_to_cm(data: &ipc::Data) {
    if let Ok(mut c) = ipc::connect(1000, "_cm").await {
        c.send(data).await.ok();
    }
}

const INVALID_FORMAT: &'static str = "Invalid format";
const UNKNOWN_ERROR: &'static str = "Unknown error";

#[tokio::main(flavor = "current_thread")]
pub async fn change_id_shared(id: String, old_id: String) -> &'static str {
    if !hbb_common::is_valid_custom_id(&id) {
        return INVALID_FORMAT;
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let uuid = machine_uid::get().unwrap_or("".to_owned());
    #[cfg(any(target_os = "android", target_os = "ios"))]
    let uuid = base64::encode(hbb_common::get_uuid());

    if uuid.is_empty() {
        log::error!("Failed to change id, uuid is_empty");
        return UNKNOWN_ERROR;
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let rendezvous_servers = crate::ipc::get_rendezvous_servers(1_000).await;
    #[cfg(any(target_os = "android", target_os = "ios"))]
    let rendezvous_servers = Config::get_rendezvous_servers();

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
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        crate::ipc::set_config_async("id", id.to_owned()).await.ok();
        #[cfg(any(target_os = "android", target_os = "ios"))]
        {
            Config::set_key_confirmed(false);
            Config::set_id(&id);
        }
    }
    err
}

async fn check_id(
    rendezvous_server: String,
    old_id: String,
    id: String,
    uuid: String,
) -> &'static str {
    if let Ok(mut socket) = hbb_common::socket_client::connect_tcp(
        crate::check_port(rendezvous_server, RENDEZVOUS_PORT),
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
                        Some(rendezvous_message::Union::RegisterPkResponse(rpr)) => {
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
                                register_pk_response::Result::SERVER_ERROR => {
                                    return "Server error";
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
