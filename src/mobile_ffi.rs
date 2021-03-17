use crate::mobile::Session;
use hbb_common::{
    config::{self, Config, PeerConfig},
    log,
};
use std::{
    collections::HashMap,
    ffi::{CStr, CString},
    os::raw::c_char,
};

fn initialize(app_dir: &str) {
    *config::APP_DIR.write().unwrap() = app_dir.to_owned();
    use android_logger::{Config, FilterBuilder};
    android_logger::init_once(
        Config::default()
            .with_min_level(log::Level::Debug) // limit log level
            .with_tag("ffi"), // logs will show under mytag tag
    );
    crate::common::test_rendezvous_server();
    crate::common::test_nat_type();
    #[cfg(target_os = "android")]
    crate::common::check_software_update();
}

#[no_mangle]
extern "C" fn rust_cstr_free(s: *mut c_char) {
    unsafe {
        if s.is_null() {
            return;
        }
        CString::from_raw(s)
    };
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
                        .map(|(id, _, info)| (id, info))
                        .collect();
                    res = serde_json::ser::to_string(&peers).unwrap_or("".to_owned());
                }
            }
            "remote_id" => {
                if !config::APP_DIR.read().unwrap().is_empty() {
                    res = Config::get_remote_id();
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
                    res = crate::common::test_if_valid_server(arg.to_owned());
                }
            }
            "custom-rendezvous-server" => {
                res = Config::get_option("custom-rendezvous-server");
            }
            "relay-server" => {
                res = Config::get_option("relay-server");
            }
            "image_quality" => {
                res = Session::get_image_quality();
            }
            "software_update_url" => {
                res = crate::common::SOFTWARE_UPDATE_URL.lock().unwrap().clone()
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
                    Session::start(value);
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
                        if let Some(name) = m.get("name") {
                            Session::input_key(name, alt, ctrl, shift, command);
                        }
                    }
                }
                "input_string" => {
                    Session::input_string(value);
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
                "custom-rendezvous-server" => {
                    Config::set_option("custom-rendezvous-server".to_owned(), value.to_owned());
                    crate::common::test_rendezvous_server();
                }
                "relay-server" => {
                    Config::set_option("relay-server".to_owned(), value.to_owned());
                }
                _ => {
                    log::error!("Unknown name of set_by_name: {}", name);
                }
            }
        }
    }
}

#[repr(C)]
struct RgbaFrame {
    len: u32,
    data: *mut u8,
}

#[no_mangle]
unsafe extern "C" fn get_rgba() -> *mut RgbaFrame {
    if let Some(mut vec) = Session::rgba() {
        if vec.is_empty() {
            return std::ptr::null_mut();
        }
        assert!(vec.len() == vec.capacity());
        vec.shrink_to_fit();
        let data = vec.as_mut_ptr();
        let len = vec.len();
        std::mem::forget(vec);
        Box::into_raw(Box::new(RgbaFrame {
            len: len as _,
            data,
        }))
    } else {
        std::ptr::null_mut()
    }
}

#[no_mangle]
extern "C" fn free_rgba(f: *mut RgbaFrame) {
    if f.is_null() {
        return;
    }
    unsafe {
        let len = (*f).len as usize;
        drop(Vec::from_raw_parts((*f).data, len, len));
        Box::from_raw(f);
    }
}
