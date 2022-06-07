use crate::client::file_trait::FileManager;
use crate::mobile::connection_manager::{self, get_clients_length, get_clients_state};
use crate::mobile::{self, Session};
use crate::common::{make_fd_to_json};
use flutter_rust_bridge::{StreamSink, ZeroCopyBuffer};
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
    crate::common::test_rendezvous_server();
    crate::common::test_nat_type();
    #[cfg(target_os = "android")]
    crate::common::check_software_update();
}

pub fn start_event_stream(s: StreamSink<String>) -> ResultType<()> {
    let _ = mobile::EVENT_STREAM.write().unwrap().insert(s);
    Ok(())
}

pub fn start_rgba_stream(s: StreamSink<ZeroCopyBuffer<Vec<u8>>>) -> ResultType<()> {
    let _ = mobile::RGBA_STREAM.write().unwrap().insert(s);
    Ok(())
}

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
            "remember" => {
                res = Session::get_remember().to_string();
            }
            "event" => {
                if let Some(e) = Session::pop_event() {
                    res = e;
                }
            }
            "toggle_option" => {
                if let Ok(arg) = arg.to_str() {
                    if let Some(v) = Session::get_toggle_option(arg) {
                        res = v.to_string();
                    }
                }
            }
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
            "image_quality" => {
                res = Session::get_image_quality();
            }
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
            "peer_option" => {
                if let Ok(arg) = arg.to_str() {
                    res = Session::get_option(arg);
                }
            }
            "server_id" => {
                res = Config::get_id();
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
            "read_local_dir_sync" => {
                if let Ok(value) = arg.to_str() {
                    if let Ok(m) = serde_json::from_str::<HashMap<String, String>>(value) {
                        if let (Some(path), Some(show_hidden)) =
                            (m.get("path"), m.get("show_hidden"))
                        {
                            if let Ok(fd) =
                                fs::read_dir(&fs::get_path(path), show_hidden.eq("true"))
                            {
                                res = make_fd_to_json(fd);
                            }
                        }
                    }
                }
            }
            // Server Side
            #[cfg(target_os = "android")]
            "clients_state" => {
                res = get_clients_state();
            }
            #[cfg(target_os = "android")]
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
                    *crate::common::MOBILE_INFO1.lock().unwrap() = value.to_owned();
                }
                "info2" => {
                    *crate::common::MOBILE_INFO2.lock().unwrap() = value.to_owned();
                }
                "connect" => {
                    Session::start(value, false);
                }
                "connect_file_transfer" => {
                    Session::start(value, true);
                }
                "login" => {
                    if let Ok(m) = serde_json::from_str::<HashMap<String, String>>(value) {
                        if let Some(password) = m.get("password") {
                            if let Some(remember) = m.get("remember") {
                                Session::login(password, remember == "true");
                            }
                        }
                    }
                }
                "close" => {
                    Session::close();
                }
                "refresh" => {
                    Session::refresh();
                }
                "reconnect" => {
                    Session::reconnect();
                }
                "toggle_option" => {
                    Session::toggle_option(value);
                }
                "image_quality" => {
                    Session::set_image_quality(value);
                }
                "lock_screen" => {
                    Session::lock_screen();
                }
                "ctrl_alt_del" => {
                    Session::ctrl_alt_del();
                }
                "switch_display" => {
                    if let Ok(v) = value.parse::<i32>() {
                        Session::switch_display(v);
                    }
                }
                "remove" => {
                    PeerConfig::remove(value);
                }
                "input_key" => {
                    if let Ok(m) = serde_json::from_str::<HashMap<String, String>>(value) {
                        let alt = m.get("alt").is_some();
                        let ctrl = m.get("ctrl").is_some();
                        let shift = m.get("shift").is_some();
                        let command = m.get("command").is_some();
                        let down = m.get("down").is_some();
                        let press = m.get("press").is_some();
                        if let Some(name) = m.get("name") {
                            Session::input_key(name, down, press, alt, ctrl, shift, command);
                        }
                    }
                }
                "input_string" => {
                    Session::input_string(value);
                }
                "chat_client_mode" => {
                    Session::send_chat(value.to_owned());
                }
                "send_mouse" => {
                    if let Ok(m) = serde_json::from_str::<HashMap<String, String>>(value) {
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
                        Session::send_mouse(mask, x, y, alt, ctrl, shift, command);
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
                                    crate::common::test_rendezvous_server();
                                }
                            }
                        }
                    }
                }
                "peer_option" => {
                    if let Ok(m) = serde_json::from_str::<HashMap<String, String>>(value) {
                        if let Some(name) = m.get("name") {
                            if let Some(value) = m.get("value") {
                                Session::set_option(name.to_owned(), value.to_owned());
                            }
                        }
                    }
                }
                "input_os_password" => {
                    Session::input_os_password(value.to_owned(), true);
                }
                // File Action
                "read_remote_dir" => {
                    if let Ok(m) = serde_json::from_str::<HashMap<String, String>>(value) {
                        if let (Some(path), Some(show_hidden), Some(session)) = (
                            m.get("path"),
                            m.get("show_hidden"),
                            Session::get().read().unwrap().as_ref(),
                        ) {
                            session.read_remote_dir(path.to_owned(), show_hidden.eq("true"));
                        }
                    }
                }
                "send_files" => {
                    if let Ok(m) = serde_json::from_str::<HashMap<String, String>>(value) {
                        if let (
                            Some(id),
                            Some(path),
                            Some(to),
                            Some(file_num),
                            Some(show_hidden),
                            Some(is_remote),
                        ) = (
                            m.get("id"),
                            m.get("path"),
                            m.get("to"),
                            m.get("file_num"),
                            m.get("show_hidden"),
                            m.get("is_remote"),
                        ) {
                            Session::send_files(
                                id.parse().unwrap_or(0),
                                path.to_owned(),
                                to.to_owned(),
                                file_num.parse().unwrap_or(0),
                                show_hidden.eq("true"),
                                is_remote.eq("true"),
                            );
                        }
                    }
                }
                "set_confirm_override_file" => {
                    if let Ok(m) = serde_json::from_str::<HashMap<String, String>>(value) {
                        if let (
                            Some(id),
                            Some(file_num),
                            Some(need_override),
                            Some(remember),
                            Some(is_upload),
                        ) = (
                            m.get("id"),
                            m.get("file_num"),
                            m.get("need_override"),
                            m.get("remember"),
                            m.get("is_upload"),
                        ) {
                            Session::set_confirm_override_file(
                                id.parse().unwrap_or(0),
                                file_num.parse().unwrap_or(0),
                                need_override.eq("true"),
                                remember.eq("true"),
                                is_upload.eq("true"),
                            );
                        }
                    }
                }
                "remove_file" => {
                    if let Ok(m) = serde_json::from_str::<HashMap<String, String>>(value) {
                        if let (
                            Some(id),
                            Some(path),
                            Some(file_num),
                            Some(is_remote),
                            Some(session),
                        ) = (
                            m.get("id"),
                            m.get("path"),
                            m.get("file_num"),
                            m.get("is_remote"),
                            Session::get().write().unwrap().as_mut(),
                        ) {
                            session.remove_file(
                                id.parse().unwrap_or(0),
                                path.to_owned(),
                                file_num.parse().unwrap_or(0),
                                is_remote.eq("true"),
                            );
                        }
                    }
                }
                "read_dir_recursive" => {
                    if let Ok(m) = serde_json::from_str::<HashMap<String, String>>(value) {
                        if let (Some(id), Some(path), Some(is_remote), Some(session)) = (
                            m.get("id"),
                            m.get("path"),
                            m.get("is_remote"),
                            Session::get().write().unwrap().as_mut(),
                        ) {
                            session.remove_dir_all(
                                id.parse().unwrap_or(0),
                                path.to_owned(),
                                is_remote.eq("true"),
                            );
                        }
                    }
                }
                "remove_all_empty_dirs" => {
                    if let Ok(m) = serde_json::from_str::<HashMap<String, String>>(value) {
                        if let (Some(id), Some(path), Some(is_remote), Some(session)) = (
                            m.get("id"),
                            m.get("path"),
                            m.get("is_remote"),
                            Session::get().write().unwrap().as_mut(),
                        ) {
                            session.remove_dir(
                                id.parse().unwrap_or(0),
                                path.to_owned(),
                                is_remote.eq("true"),
                            );
                        }
                    }
                }
                "cancel_job" => {
                    if let (Ok(id), Some(session)) =
                        (value.parse(), Session::get().write().unwrap().as_mut())
                    {
                        session.cancel_job(id);
                    }
                }
                "create_dir" => {
                    if let Ok(m) = serde_json::from_str::<HashMap<String, String>>(value) {
                        if let (Some(id), Some(path), Some(is_remote), Some(session)) = (
                            m.get("id"),
                            m.get("path"),
                            m.get("is_remote"),
                            Session::get().write().unwrap().as_mut(),
                        ) {
                            session.create_dir(
                                id.parse().unwrap_or(0),
                                path.to_owned(),
                                is_remote.eq("true"),
                            );
                        }
                    }
                }
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
                #[cfg(target_os = "android")]
                "start_service" => {
                    Config::set_option("stop-service".into(), "".into());
                    crate::rendezvous_mediator::RendezvousMediator::restart();
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
