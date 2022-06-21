use crate::client::file_trait::FileManager;
use crate::flutter::connection_manager::{self, get_clients_length, get_clients_state};
use crate::flutter::{self, make_fd_to_json, Session, SESSIONS};
use crate::start_server;
use crate::ui_interface;
use flutter_rust_bridge::{StreamSink, SyncReturn, ZeroCopyBuffer};
use hbb_common::ResultType;
use hbb_common::{
    config::{self, Config, LocalConfig, PeerConfig, ONLINE},
    fs, log,
};
use serde_json::{Number, Value};
use std::{
    collections::HashMap,
    ffi::{CStr, CString},
    os::raw::c_char,
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

pub fn session_read_dir_recursive(id: String, act_id: i32, path: String, is_remote: bool) {
    if let Some(session) = SESSIONS.read().unwrap().get(&id) {
        session.remove_dir_all(act_id, path, is_remote);
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
            "server_password" => {
                res = Config::get_password();
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
                res = base64::encode(crate::get_uuid());
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
                "update_password" => {
                    if value.is_empty() {
                        Config::set_password(&Config::get_auto_password());
                    } else {
                        Config::set_password(value);
                    }
                }
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

#[cfg(target_os = "android")]
pub mod server_side {
    use hbb_common::{config::Config, log};
    use jni::{
        objects::{JClass, JString},
        sys::jstring,
        JNIEnv,
    };

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
