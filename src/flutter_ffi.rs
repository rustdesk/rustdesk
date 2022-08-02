use std::{
    collections::HashMap,
    ffi::{CStr, CString},
    os::raw::c_char,
};

use flutter_rust_bridge::{StreamSink, SyncReturn, ZeroCopyBuffer};
use serde_json::{json, Number, Value};

use hbb_common::{
    config::{self, Config, LocalConfig, PeerConfig, ONLINE},
    fs, log,
};
use hbb_common::{password_security, ResultType};

use crate::client::file_trait::FileManager;
use crate::common::make_fd_to_json;
use crate::flutter::connection_manager::{self, get_clients_length, get_clients_state};
use crate::flutter::{self, Session, SESSIONS};
use crate::start_server;
use crate::ui_interface;
use crate::ui_interface::{
    change_id, check_connect_status, discover, forget_password, get_api_server, get_app_name,
    get_async_job_status, get_connect_status, get_fav, get_id, get_lan_peers, get_license,
    get_local_option, get_options, get_peer, get_peer_option, get_socks, get_sound_inputs,
    get_uuid, get_version, has_rendezvous_service, is_ok_change_id, post_request, set_local_option,
    set_options, set_peer_option, set_socks, store_fav, temporary_password, test_if_valid_server,
    using_public_server,
};

fn initialize(app_dir: &str) {
    *config::APP_DIR.write().unwrap() = app_dir.to_owned();
    #[cfg(feature = "cli")]
    {
        #[cfg(any(target_os = "android", target_os = "ios"))]
        {
            crate::common::test_rendezvous_server();
            crate::common::test_nat_type();
        }
    }
    #[cfg(target_os = "android")]
    {
        android_logger::init_once(
            android_logger::Config::default()
                .with_min_level(log::Level::Debug) // limit log level
                .with_tag("ffi"), // logs will show under mytag tag
        );
    }
    #[cfg(target_os = "ios")]
    {
        use hbb_common::env_logger::*;
        init_from_env(Env::default().filter_or(DEFAULT_FILTER_ENV, "debug"));
    }
    #[cfg(target_os = "android")]
    {
        crate::common::check_software_update();
    }
    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
    {
        use hbb_common::env_logger::*;
        if let Err(e) = try_init_from_env(Env::default().filter_or(DEFAULT_FILTER_ENV, "debug")) {
            log::debug!("{}", e);
        }
    }
}

/// FFI for rustdesk core's main entry.
/// Return true if the app should continue running with UI(possibly Flutter), false if the app should exit.
#[no_mangle]
pub extern "C" fn rustdesk_core_main() -> bool {
    crate::core_main::core_main()
}

pub enum EventToUI {
    Event(String),
    Rgba(ZeroCopyBuffer<Vec<u8>>),
}

pub fn start_global_event_stream(s: StreamSink<String>) -> ResultType<()> {
    let _ = flutter::GLOBAL_EVENT_STREAM.write().unwrap().insert(s);
    Ok(())
}

pub fn host_stop_system_key_propagate(stopped: bool) {
    #[cfg(windows)]
    crate::platform::windows::stop_system_key_propagate(stopped);
}

pub fn session_connect(
    events2ui: StreamSink<EventToUI>,
    id: String,
    is_file_transfer: bool,
) -> ResultType<()> {
    Session::start(&id, is_file_transfer, events2ui);
    Ok(())
}

pub fn get_session_remember(id: String) -> Option<bool> {
    if let Some(session) = SESSIONS.read().unwrap().get(&id) {
        Some(session.get_remember())
    } else {
        None
    }
}

// TODO sync
pub fn get_session_toggle_option(id: String, arg: String) -> Option<bool> {
    if let Some(session) = SESSIONS.read().unwrap().get(&id) {
        Some(session.get_toggle_option(&arg))
    } else {
        None
    }
}

pub fn get_session_toggle_option_sync(id: String, arg: String) -> SyncReturn<bool> {
    let res = get_session_toggle_option(id, arg) == Some(true);
    SyncReturn(res)
}

pub fn get_session_image_quality(id: String) -> Option<String> {
    if let Some(session) = SESSIONS.read().unwrap().get(&id) {
        Some(session.get_image_quality())
    } else {
        None
    }
}

pub fn get_session_option(id: String, arg: String) -> Option<String> {
    if let Some(session) = SESSIONS.read().unwrap().get(&id) {
        Some(session.get_option(&arg))
    } else {
        None
    }
}

// void
pub fn session_login(id: String, password: String, remember: bool) {
    if let Some(session) = SESSIONS.read().unwrap().get(&id) {
        session.login(&password, remember);
    }
}

pub fn session_close(id: String) {
    if let Some(session) = SESSIONS.read().unwrap().get(&id) {
        session.close();
    }
    let _ = SESSIONS.write().unwrap().remove(&id);
}

pub fn session_refresh(id: String) {
    if let Some(session) = SESSIONS.read().unwrap().get(&id) {
        session.refresh();
    }
}

pub fn session_reconnect(id: String) {
    if let Some(session) = SESSIONS.read().unwrap().get(&id) {
        session.reconnect();
    }
}

pub fn session_toggle_option(id: String, value: String) {
    if let Some(session) = SESSIONS.read().unwrap().get(&id) {
        session.toggle_option(&value);
    }
}

pub fn session_set_image_quality(id: String, value: String) {
    if let Some(session) = SESSIONS.read().unwrap().get(&id) {
        session.set_image_quality(&value);
    }
}

pub fn session_lock_screen(id: String) {
    if let Some(session) = SESSIONS.read().unwrap().get(&id) {
        session.lock_screen();
    }
}

pub fn session_ctrl_alt_del(id: String) {
    if let Some(session) = SESSIONS.read().unwrap().get(&id) {
        session.ctrl_alt_del();
    }
}

pub fn session_switch_display(id: String, value: i32) {
    if let Some(session) = SESSIONS.read().unwrap().get(&id) {
        session.switch_display(value);
    }
}

pub fn session_input_key(
    id: String,
    name: String,
    down: bool,
    press: bool,
    alt: bool,
    ctrl: bool,
    shift: bool,
    command: bool,
) {
    if let Some(session) = SESSIONS.read().unwrap().get(&id) {
        session.input_key(&name, down, press, alt, ctrl, shift, command);
    }
}

pub fn session_input_string(id: String, value: String) {
    if let Some(session) = SESSIONS.read().unwrap().get(&id) {
        session.input_string(&value);
    }
}

// chat_client_mode
pub fn session_send_chat(id: String, text: String) {
    if let Some(session) = SESSIONS.read().unwrap().get(&id) {
        session.send_chat(text);
    }
}

// if let Some(_type) = m.get("type") {
//             mask = match _type.as_str() {
//                 "down" => 1,
//                 "up" => 2,
//                 "wheel" => 3,
//                 _ => 0,
//             };
//         }
// if let Some(buttons) = m.get("buttons") {
//             mask |= match buttons.as_str() {
//                 "left" => 1,
//                 "right" => 2,
//                 "wheel" => 4,
//                 _ => 0,
//             } << 3;
//         }
// TODO
pub fn session_send_mouse(
    id: String,
    mask: i32,
    x: i32,
    y: i32,
    alt: bool,
    ctrl: bool,
    shift: bool,
    command: bool,
) {
    if let Some(session) = SESSIONS.read().unwrap().get(&id) {
        session.send_mouse(mask, x, y, alt, ctrl, shift, command);
    }
}

pub fn session_peer_option(id: String, name: String, value: String) {
    if let Some(session) = SESSIONS.read().unwrap().get(&id) {
        session.set_option(name, value);
    }
}

pub fn session_get_peer_option(id: String, name: String) -> String {
    if let Some(session) = SESSIONS.read().unwrap().get(&id) {
        return session.get_option(&name);
    }
    "".to_string()
}

pub fn session_input_os_password(id: String, value: String) {
    if let Some(session) = SESSIONS.read().unwrap().get(&id) {
        session.input_os_password(value, true);
    }
}

// File Action
pub fn session_read_remote_dir(id: String, path: String, include_hidden: bool) {
    if let Some(session) = SESSIONS.read().unwrap().get(&id) {
        session.read_remote_dir(path, include_hidden);
    }
}

pub fn session_send_files(
    id: String,
    act_id: i32,
    path: String,
    to: String,
    file_num: i32,
    include_hidden: bool,
    is_remote: bool,
) {
    if let Some(session) = SESSIONS.read().unwrap().get(&id) {
        session.send_files(act_id, path, to, file_num, include_hidden, is_remote);
    }
}

pub fn session_set_confirm_override_file(
    id: String,
    act_id: i32,
    file_num: i32,
    need_override: bool,
    remember: bool,
    is_upload: bool,
) {
    if let Some(session) = SESSIONS.read().unwrap().get(&id) {
        session.set_confirm_override_file(act_id, file_num, need_override, remember, is_upload);
    }
}

pub fn session_remove_file(id: String, act_id: i32, path: String, file_num: i32, is_remote: bool) {
    if let Some(session) = SESSIONS.read().unwrap().get(&id) {
        session.remove_file(act_id, path, file_num, is_remote);
    }
}

pub fn session_read_dir_recursive(
    id: String,
    act_id: i32,
    path: String,
    is_remote: bool,
    show_hidden: bool,
) {
    if let Some(session) = SESSIONS.read().unwrap().get(&id) {
        session.remove_dir_all(act_id, path, is_remote, show_hidden);
    }
}

pub fn session_remove_all_empty_dirs(id: String, act_id: i32, path: String, is_remote: bool) {
    if let Some(session) = SESSIONS.read().unwrap().get(&id) {
        session.remove_dir(act_id, path, is_remote);
    }
}

pub fn session_cancel_job(id: String, act_id: i32) {
    if let Some(session) = SESSIONS.read().unwrap().get(&id) {
        session.cancel_job(act_id);
    }
}

pub fn session_create_dir(id: String, act_id: i32, path: String, is_remote: bool) {
    if let Some(session) = SESSIONS.read().unwrap().get(&id) {
        session.create_dir(act_id, path, is_remote);
    }
}

pub fn session_read_local_dir_sync(id: String, path: String, show_hidden: bool) -> String {
    if let Some(session) = SESSIONS.read().unwrap().get(&id) {
        if let Ok(fd) = fs::read_dir(&fs::get_path(&path), show_hidden) {
            return make_fd_to_json(fd);
        }
    }
    "".to_string()
}

pub fn session_get_platform(id: String, is_remote: bool) -> String {
    if let Some(session) = SESSIONS.read().unwrap().get(&id) {
        return session.get_platform(is_remote);
    }
    "".to_string()
}

pub fn session_load_last_transfer_jobs(id: String) {
    if let Some(session) = SESSIONS.read().unwrap().get(&id) {
        return session.load_last_jobs();
    } else {
        // a tip for flutter dev
        eprintln!(
            "cannot load last transfer job from non-existed session. Please ensure session \
        is connected before calling load last transfer jobs."
        );
    }
}

pub fn session_add_job(
    id: String,
    act_id: i32,
    path: String,
    to: String,
    file_num: i32,
    include_hidden: bool,
    is_remote: bool,
) {
    if let Some(session) = SESSIONS.read().unwrap().get(&id) {
        session.add_job(act_id, path, to, file_num, include_hidden, is_remote);
    }
}

pub fn session_resume_job(id: String, act_id: i32, is_remote: bool) {
    if let Some(session) = SESSIONS.read().unwrap().get(&id) {
        session.resume_job(act_id, is_remote);
    }
}

pub fn main_get_sound_inputs() -> Vec<String> {
    get_sound_inputs()
}

pub fn main_change_id(new_id: String) {
    change_id(new_id)
}

pub fn main_get_async_status() -> String {
    get_async_job_status()
}

pub fn main_get_options() -> String {
    get_options()
}

pub fn main_set_options(json: String) {
    let map: HashMap<String, String> = serde_json::from_str(&json).unwrap_or(HashMap::new());
    if !map.is_empty() {
        set_options(map)
    }
}

pub fn main_test_if_valid_server(server: String) -> String {
    test_if_valid_server(server)
}

pub fn main_set_socks(proxy: String, username: String, password: String) {
    set_socks(proxy, username, password)
}

pub fn main_get_socks() -> Vec<String> {
    get_socks()
}

pub fn main_get_app_name() -> String {
    get_app_name()
}

pub fn main_get_license() -> String {
    get_license()
}

pub fn main_get_version() -> String {
    get_version()
}

pub fn main_get_fav() -> Vec<String> {
    get_fav()
}

pub fn main_store_fav(favs: Vec<String>) {
    store_fav(favs)
}

pub fn main_get_peers(id: String) -> String {
    let conf = get_peer(id);
    serde_json::to_string(&conf).unwrap_or("".to_string())
}

pub fn main_get_lan_peers() -> String {
    get_lan_peers()
}

pub fn main_get_connect_status() -> String {
    let status = get_connect_status();
    // (status_num, key_confirmed, mouse_time, id)
    let mut m = serde_json::Map::new();
    m.insert("status_num".to_string(), json!(status.0));
    m.insert("key_confirmed".to_string(), json!(status.1));
    m.insert("mouse_time".to_string(), json!(status.2));
    m.insert("id".to_string(), json!(status.3));
    serde_json::to_string(&m).unwrap_or("".to_string())
}

pub fn main_check_connect_status() {
    check_connect_status(true);
}

pub fn main_is_using_public_server() -> bool {
    using_public_server()
}

pub fn main_discover() {
    discover();
}

pub fn main_has_rendezvous_service() -> bool {
    has_rendezvous_service()
}

pub fn main_get_api_server() -> String {
    get_api_server()
}

pub fn main_post_request(url: String, body: String, header: String) {
    post_request(url, body, header)
}

pub fn main_get_local_option(key: String) -> String {
    get_local_option(key)
}

pub fn main_set_local_option(key: String, value: String) {
    set_local_option(key, value)
}

pub fn main_get_my_id() -> String {
    get_id()
}

pub fn main_get_uuid() -> String {
    get_uuid()
}

pub fn main_get_peer_option(id: String, key: String) -> String {
    get_peer_option(id, key)
}

pub fn main_set_peer_option(id: String, key: String, value: String) {
    set_peer_option(id, key, value)
}

pub fn main_forget_password(id: String) {
    forget_password(id)
}

pub fn main_load_recent_peers() {
    if !config::APP_DIR.read().unwrap().is_empty() {
        let peers: Vec<(String, config::PeerInfoSerde)> = PeerConfig::peers()
            .drain(..)
            .map(|(id, _, p)| (id, p.info))
            .collect();
        if let Some(s) = flutter::GLOBAL_EVENT_STREAM.read().unwrap().as_ref() {
            let data = HashMap::from([
                ("name", "load_recent_peers".to_owned()),
                (
                    "peers",
                    serde_json::ser::to_string(&peers).unwrap_or("".to_owned()),
                ),
            ]);
            s.add(serde_json::ser::to_string(&data).unwrap_or("".to_owned()));
        };
    }
}

pub fn main_load_fav_peers() {
    if !config::APP_DIR.read().unwrap().is_empty() {
        let favs = get_fav();
        let peers: Vec<(String, config::PeerInfoSerde)> = PeerConfig::peers()
            .into_iter()
            .filter_map(|(id, _, peer)| {
                if favs.contains(&id) {
                    Some((id, peer.info))
                } else {
                    None
                }
            })
            .collect();
        if let Some(s) = flutter::GLOBAL_EVENT_STREAM.read().unwrap().as_ref() {
            let data = HashMap::from([
                ("name", "load_fav_peers".to_owned()),
                (
                    "peers",
                    serde_json::ser::to_string(&peers).unwrap_or("".to_owned()),
                ),
            ]);
            s.add(serde_json::ser::to_string(&data).unwrap_or("".to_owned()));
        };
    }
}

pub fn main_load_lan_peers() {
    if let Some(s) = flutter::GLOBAL_EVENT_STREAM.read().unwrap().as_ref() {
        let data = HashMap::from([
            ("name", "load_lan_peers".to_owned()),
            ("peers", get_lan_peers()),
        ]);
        s.add(serde_json::ser::to_string(&data).unwrap_or("".to_owned()));
    };
}

/// FFI for **get** commands which are idempotent.
/// Return result in c string.
///
/// # Arguments
///
/// * `name` - name of the command
/// * `arg` - argument of the command
#[no_mangle]
unsafe extern "C" fn get_by_name(name: *const c_char, arg: *const c_char) -> *const c_char {
    let mut res = "".to_owned();
    let arg: &CStr = CStr::from_ptr(arg);
    let name: &CStr = CStr::from_ptr(name);
    if let Ok(name) = name.to_str() {
        match name {
            "peers" => {
                if !config::APP_DIR.read().unwrap().is_empty() {
                    let peers: Vec<(String, config::PeerInfoSerde)> = PeerConfig::peers()
                        .drain(..)
                        .map(|(id, _, p)| (id, p.info))
                        .collect();
                    res = serde_json::ser::to_string(&peers).unwrap_or("".to_owned());
                }
            }
            "remote_id" => {
                if !config::APP_DIR.read().unwrap().is_empty() {
                    res = LocalConfig::get_remote_id();
                }
            }
            // "remember" => {
            //     res = Session::get_remember().to_string();
            // }
            // "toggle_option" => {
            //     if let Ok(arg) = arg.to_str() {
            //         if let Some(v) = Session::get_toggle_option(arg) {
            //             res = v.to_string();
            //         }
            //     }
            // }
            "test_if_valid_server" => {
                if let Ok(arg) = arg.to_str() {
                    res = hbb_common::socket_client::test_if_valid_server(arg);
                }
            }
            "option" => {
                if let Ok(arg) = arg.to_str() {
                    res = Config::get_option(arg);
                }
            }
            // "image_quality" => {
            //     res = Session::get_image_quality();
            // }
            "software_update_url" => {
                res = crate::common::SOFTWARE_UPDATE_URL.lock().unwrap().clone()
            }
            "translate" => {
                if let Ok(arg) = arg.to_str() {
                    if let Ok(m) = serde_json::from_str::<HashMap<String, String>>(arg) {
                        if let Some(locale) = m.get("locale") {
                            if let Some(text) = m.get("text") {
                                res = crate::client::translate_locale(text.to_owned(), locale);
                            }
                        }
                    }
                }
            }
            // "peer_option" => {
            //     if let Ok(arg) = arg.to_str() {
            //         res = Session::get_option(arg);
            //     }
            // }
            "server_id" => {
                res = ui_interface::get_id();
            }
            "temporary_password" => {
                res = password_security::temporary_password();
            }
            "connect_statue" => {
                res = ONLINE
                    .lock()
                    .unwrap()
                    .values()
                    .max()
                    .unwrap_or(&0)
                    .clone()
                    .to_string();
            }
            // File Action
            "get_home_dir" => {
                res = fs::get_home_as_string();
            }
            // "read_local_dir_sync" => {
            //     if let Ok(value) = arg.to_str() {
            //         if let Ok(m) = serde_json::from_str::<HashMap<String, String>>(value) {
            //             if let (Some(path), Some(show_hidden)) =
            //                 (m.get("path"), m.get("show_hidden"))
            //             {
            //                 if let Ok(fd) =
            //                     fs::read_dir(&fs::get_path(path), show_hidden.eq("true"))
            //                 {
            //                     res = make_fd_to_json(fd);
            //                 }
            //             }
            //         }
            //     }
            // }
            // Server Side
            #[cfg(not(any(target_os = "ios")))]
            "clients_state" => {
                res = get_clients_state();
            }
            #[cfg(not(any(target_os = "ios")))]
            "check_clients_length" => {
                if let Ok(value) = arg.to_str() {
                    if value.parse::<usize>().unwrap_or(usize::MAX) != get_clients_length() {
                        res = get_clients_state()
                    }
                }
            }
            "uuid" => {
                res = base64::encode(get_uuid());
            }
            _ => {
                log::error!("Unknown name of get_by_name: {}", name);
            }
        }
    }
    CString::from_vec_unchecked(res.into_bytes()).into_raw()
}

/// FFI for **set** commands which are not idempotent.
///
/// # Arguments
///
/// * `name` - name of the command
/// * `arg` - argument of the command
#[no_mangle]
unsafe extern "C" fn set_by_name(name: *const c_char, value: *const c_char) {
    let value: &CStr = CStr::from_ptr(value);
    if let Ok(value) = value.to_str() {
        let name: &CStr = CStr::from_ptr(name);
        if let Ok(name) = name.to_str() {
            match name {
                "init" => {
                    initialize(value);
                }
                "info1" => {
                    *crate::common::FLUTTER_INFO1.lock().unwrap() = value.to_owned();
                }
                "info2" => {
                    *crate::common::FLUTTER_INFO2.lock().unwrap() = value.to_owned();
                }
                // "connect" => {
                //     Session::start(value, false);
                // }
                // "connect_file_transfer" => {
                //     Session::start(value, true);
                // }
                // "login" => {
                //     if let Ok(m) = serde_json::from_str::<HashMap<String, String>>(value) {
                //         if let Some(password) = m.get("password") {
                //             if let Some(remember) = m.get("remember") {
                //                 Session::login(password, remember == "true");
                //             }
                //         }
                //     }
                // }
                // "close" => {
                //     Session::close();
                // }
                // "refresh" => {
                //     Session::refresh();
                // }
                // "reconnect" => {
                //     Session::reconnect();
                // }
                // "toggle_option" => {
                //     Session::toggle_option(value);
                // }
                // "image_quality" => {
                //     Session::set_image_quality(value);
                // }
                // "lock_screen" => {
                //     Session::lock_screen();
                // }
                // "ctrl_alt_del" => {
                //     Session::ctrl_alt_del();
                // }
                // "switch_display" => {
                //     if let Ok(v) = value.parse::<i32>() {
                //         Session::switch_display(v);
                //     }
                // }
                "remove" => {
                    PeerConfig::remove(value);
                }
                // "input_key" => {
                //     if let Ok(m) = serde_json::from_str::<HashMap<String, String>>(value) {
                //         let alt = m.get("alt").is_some();
                //         let ctrl = m.get("ctrl").is_some();
                //         let shift = m.get("shift").is_some();
                //         let command = m.get("command").is_some();
                //         let down = m.get("down").is_some();
                //         let press = m.get("press").is_some();
                //         if let Some(name) = m.get("name") {
                //             Session::input_key(name, down, press, alt, ctrl, shift, command);
                //         }
                //     }
                // }
                // "input_string" => {
                //     Session::input_string(value);
                // }
                // "chat_client_mode" => {
                //     Session::send_chat(value.to_owned());
                // }

                // TODO
                "send_mouse" => {
                    if let Ok(m) = serde_json::from_str::<HashMap<String, String>>(value) {
                        let id = m.get("id");
                        if id.is_none() {
                            return;
                        }
                        let id = id.unwrap();
                        let alt = m.get("alt").is_some();
                        let ctrl = m.get("ctrl").is_some();
                        let shift = m.get("shift").is_some();
                        let command = m.get("command").is_some();
                        let x = m
                            .get("x")
                            .map(|x| x.parse::<i32>().unwrap_or(0))
                            .unwrap_or(0);
                        let y = m
                            .get("y")
                            .map(|x| x.parse::<i32>().unwrap_or(0))
                            .unwrap_or(0);
                        let mut mask = 0;
                        if let Some(_type) = m.get("type") {
                            mask = match _type.as_str() {
                                "down" => 1,
                                "up" => 2,
                                "wheel" => 3,
                                _ => 0,
                            };
                        }
                        if let Some(buttons) = m.get("buttons") {
                            mask |= match buttons.as_str() {
                                "left" => 1,
                                "right" => 2,
                                "wheel" => 4,
                                _ => 0,
                            } << 3;
                        }
                        if let Some(session) = SESSIONS.read().unwrap().get(id) {
                            session.send_mouse(mask, x, y, alt, ctrl, shift, command);
                        }
                    }
                }
                "option" => {
                    if let Ok(m) = serde_json::from_str::<HashMap<String, String>>(value) {
                        if let Some(name) = m.get("name") {
                            if let Some(value) = m.get("value") {
                                Config::set_option(name.to_owned(), value.to_owned());
                                if name == "custom-rendezvous-server" {
                                    #[cfg(target_os = "android")]
                                    crate::rendezvous_mediator::RendezvousMediator::restart();
                                    #[cfg(any(
                                        target_os = "android",
                                        target_os = "ios",
                                        feature = "cli"
                                    ))]
                                    crate::common::test_rendezvous_server();
                                }
                            }
                        }
                    }
                }
                // "peer_option" => {
                //     if let Ok(m) = serde_json::from_str::<HashMap<String, String>>(value) {
                //         if let Some(name) = m.get("name") {
                //             if let Some(value) = m.get("value") {
                //                 Session::set_option(name.to_owned(), value.to_owned());
                //             }
                //         }
                //     }
                // }
                // "input_os_password" => {
                //     Session::input_os_password(value.to_owned(), true);
                // }
                // // File Action
                // "read_remote_dir" => {
                //     if let Ok(m) = serde_json::from_str::<HashMap<String, String>>(value) {
                //         if let (Some(path), Some(show_hidden), Some(session)) = (
                //             m.get("path"),
                //             m.get("show_hidden"),
                //             Session::get().read().unwrap().as_ref(),
                //         ) {
                //             session.read_remote_dir(path.to_owned(), show_hidden.eq("true"));
                //         }
                //     }
                // }
                // "send_files" => {
                //     if let Ok(m) = serde_json::from_str::<HashMap<String, String>>(value) {
                //         if let (
                //             Some(id),
                //             Some(path),
                //             Some(to),
                //             Some(file_num),
                //             Some(show_hidden),
                //             Some(is_remote),
                //         ) = (
                //             m.get("id"),
                //             m.get("path"),
                //             m.get("to"),
                //             m.get("file_num"),
                //             m.get("show_hidden"),
                //             m.get("is_remote"),
                //         ) {
                //             Session::send_files(
                //                 id.parse().unwrap_or(0),
                //                 path.to_owned(),
                //                 to.to_owned(),
                //                 file_num.parse().unwrap_or(0),
                //                 show_hidden.eq("true"),
                //                 is_remote.eq("true"),
                //             );
                //         }
                //     }
                // }
                // "set_confirm_override_file" => {
                //     if let Ok(m) = serde_json::from_str::<HashMap<String, String>>(value) {
                //         if let (
                //             Some(id),
                //             Some(file_num),
                //             Some(need_override),
                //             Some(remember),
                //             Some(is_upload),
                //         ) = (
                //             m.get("id"),
                //             m.get("file_num"),
                //             m.get("need_override"),
                //             m.get("remember"),
                //             m.get("is_upload"),
                //         ) {
                //             Session::set_confirm_override_file(
                //                 id.parse().unwrap_or(0),
                //                 file_num.parse().unwrap_or(0),
                //                 need_override.eq("true"),
                //                 remember.eq("true"),
                //                 is_upload.eq("true"),
                //             );
                //         }
                //     }
                // }
                // ** TODO ** continue
                // "remove_file" => {
                //     if let Ok(m) = serde_json::from_str::<HashMap<String, String>>(value) {
                //         if let (
                //             Some(id),
                //             Some(path),
                //             Some(file_num),
                //             Some(is_remote),
                //             Some(session),
                //         ) = (
                //             m.get("id"),
                //             m.get("path"),
                //             m.get("file_num"),
                //             m.get("is_remote"),
                //             Session::get().write().unwrap().as_mut(),
                //         ) {
                //             session.remove_file(
                //                 id.parse().unwrap_or(0),
                //                 path.to_owned(),
                //                 file_num.parse().unwrap_or(0),
                //                 is_remote.eq("true"),
                //             );
                //         }
                //     }
                // }
                // "read_dir_recursive" => {
                //     if let Ok(m) = serde_json::from_str::<HashMap<String, String>>(value) {
                //         if let (Some(id), Some(path), Some(is_remote), Some(session)) = (
                //             m.get("id"),
                //             m.get("path"),
                //             m.get("is_remote"),
                //             Session::get().write().unwrap().as_mut(),
                //         ) {
                //             session.remove_dir_all(
                //                 id.parse().unwrap_or(0),
                //                 path.to_owned(),
                //                 is_remote.eq("true"),
                //             );
                //         }
                //     }
                // }
                // "remove_all_empty_dirs" => {
                //     if let Ok(m) = serde_json::from_str::<HashMap<String, String>>(value) {
                //         if let (Some(id), Some(path), Some(is_remote), Some(session)) = (
                //             m.get("id"),
                //             m.get("path"),
                //             m.get("is_remote"),
                //             Session::get().write().unwrap().as_mut(),
                //         ) {
                //             session.remove_dir(
                //                 id.parse().unwrap_or(0),
                //                 path.to_owned(),
                //                 is_remote.eq("true"),
                //             );
                //         }
                //     }
                // }
                // "cancel_job" => {
                //     if let (Ok(id), Some(session)) =
                //         (value.parse(), Session::get().write().unwrap().as_mut())
                //     {
                //         session.cancel_job(id);
                //     }
                // }
                // "create_dir" => {
                //     if let Ok(m) = serde_json::from_str::<HashMap<String, String>>(value) {
                //         if let (Some(id), Some(path), Some(is_remote), Some(session)) = (
                //             m.get("id"),
                //             m.get("path"),
                //             m.get("is_remote"),
                //             Session::get().write().unwrap().as_mut(),
                //         ) {
                //             session.create_dir(
                //                 id.parse().unwrap_or(0),
                //                 path.to_owned(),
                //                 is_remote.eq("true"),
                //             );
                //         }
                //     }
                // }
                // Server Side
                // "update_password" => {
                //     if value.is_empty() {
                //         Config::set_password(&Config::get_auto_password());
                //     } else {
                //         Config::set_password(value);
                //     }
                // }
                #[cfg(target_os = "android")]
                "chat_server_mode" => {
                    if let Ok(m) = serde_json::from_str::<HashMap<String, Value>>(value) {
                        if let (Some(Value::Number(id)), Some(Value::String(text))) =
                            (m.get("id"), m.get("text"))
                        {
                            let id = id.as_i64().unwrap_or(0);
                            connection_manager::send_chat(id as i32, text.to_owned());
                        }
                    }
                }
                "home_dir" => {
                    *config::APP_HOME_DIR.write().unwrap() = value.to_owned();
                }
                #[cfg(target_os = "android")]
                "login_res" => {
                    if let Ok(m) = serde_json::from_str::<HashMap<String, Value>>(value) {
                        if let (Some(Value::Number(id)), Some(Value::Bool(res))) =
                            (m.get("id"), m.get("res"))
                        {
                            let id = id.as_i64().unwrap_or(0);
                            connection_manager::on_login_res(id as i32, *res);
                        }
                    }
                }
                #[cfg(target_os = "android")]
                "stop_service" => {
                    Config::set_option("stop-service".into(), "Y".into());
                    crate::rendezvous_mediator::RendezvousMediator::restart();
                }
                "start_service" => {
                    Config::set_option("stop-service".into(), "".into());
                    start_server(false);
                }
                #[cfg(target_os = "android")]
                "close_conn" => {
                    if let Ok(id) = value.parse::<i32>() {
                        connection_manager::close_conn(id);
                    };
                }
                _ => {
                    log::error!("Unknown name of set_by_name: {}", name);
                }
            }
        }
    }
}

fn handle_query_onlines(onlines: Vec<String>, offlines: Vec<String>) {
    if let Some(s) = flutter::GLOBAL_EVENT_STREAM.read().unwrap().as_ref() {
        let data = HashMap::from([
            ("name", "callback_query_onlines".to_owned()),
            ("onlines", onlines.join(",")),
            ("offlines", offlines.join(",")),
        ]);
        s.add(serde_json::ser::to_string(&data).unwrap_or("".to_owned()));
    };
}

pub fn query_onlines(ids: Vec<String>) {
    crate::rendezvous_mediator::query_online_states(ids, handle_query_onlines)
}

#[cfg(target_os = "android")]
pub mod server_side {
    use jni::{
        objects::{JClass, JString},
        sys::jstring,
        JNIEnv,
    };

    use hbb_common::{config::Config, log};

    use crate::start_server;

    #[no_mangle]
    pub unsafe extern "system" fn Java_com_carriez_flutter_1hbb_MainService_startServer(
        env: JNIEnv,
        _class: JClass,
    ) {
        log::debug!("startServer from java");
        std::thread::spawn(move || start_server(true));
    }

    #[no_mangle]
    pub unsafe extern "system" fn Java_com_carriez_flutter_1hbb_MainService_translateLocale(
        env: JNIEnv,
        _class: JClass,
        locale: JString,
        input: JString,
    ) -> jstring {
        let res = if let (Ok(input), Ok(locale)) = (env.get_string(input), env.get_string(locale)) {
            let input: String = input.into();
            let locale: String = locale.into();
            crate::client::translate_locale(input, &locale)
        } else {
            "".into()
        };
        return env.new_string(res).unwrap_or(input).into_inner();
    }

    #[no_mangle]
    pub unsafe extern "system" fn Java_com_carriez_flutter_1hbb_MainService_refreshScreen(
        _env: JNIEnv,
        _class: JClass,
    ) {
        crate::server::video_service::refresh()
    }
}
