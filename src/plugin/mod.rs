use hbb_common::{bail, libc, log, ResultType};
#[cfg(target_os = "windows")]
use std::env;
use std::{
    ffi::{c_char, c_int, c_void, CStr},
    path::PathBuf,
    ptr::null,
};

mod callback_ext;
mod callback_msg;
mod config;
pub mod desc;
mod errno;
pub mod ipc;
mod manager;
pub mod native;
pub mod native_handlers;
mod plog;
mod plugins;

pub use manager::{
    install::{change_uninstall_plugin, install_plugin_with_url},
    install_plugin, load_plugin_list, remove_uninstalled, uninstall_plugin,
};
pub use plugins::{
    handle_client_event, handle_listen_event, handle_server_event, handle_ui_event, load_plugin,
    reload_plugin, sync_ui, unload_plugin,
};

const MSG_TO_UI_TYPE_PLUGIN_EVENT: &str = "plugin_event";
const MSG_TO_UI_TYPE_PLUGIN_RELOAD: &str = "plugin_reload";
const MSG_TO_UI_TYPE_PLUGIN_OPTION: &str = "plugin_option";
const MSG_TO_UI_TYPE_PLUGIN_MANAGER: &str = "plugin_manager";

pub const EVENT_ON_CONN_CLIENT: &str = "on_conn_client";
pub const EVENT_ON_CONN_SERVER: &str = "on_conn_server";
pub const EVENT_ON_CONN_CLOSE_CLIENT: &str = "on_conn_close_client";
pub const EVENT_ON_CONN_CLOSE_SERVER: &str = "on_conn_close_server";

static PLUGIN_SOURCE_LOCAL_DIR: &str = "plugins";

pub use config::{ManagerConfig, PeerConfig, SharedConfig};

/// Common plugin return.
///
/// [Note]
/// The msg must be nullptr if code is errno::ERR_SUCCESS.
/// The msg must be freed by caller if code is not errno::ERR_SUCCESS.
#[repr(C)]
#[derive(Debug)]
pub struct PluginReturn {
    pub code: c_int,
    pub msg: *const c_char,
}

impl PluginReturn {
    pub fn success() -> Self {
        Self {
            code: errno::ERR_SUCCESS,
            msg: null(),
        }
    }

    #[inline]
    pub fn is_success(&self) -> bool {
        self.code == errno::ERR_SUCCESS
    }

    pub fn new(code: c_int, msg: &str) -> Self {
        Self {
            code,
            msg: str_to_cstr_ret(msg),
        }
    }

    pub fn get_code_msg(&mut self, id: &str) -> (i32, String) {
        if self.is_success() {
            (self.code, "".to_owned())
        } else {
            if self.msg.is_null() {
                log::warn!(
                    "The message pointer from the plugin '{}' is null, but the error code is {}",
                    id,
                    self.code
                );
                return (self.code, "".to_owned());
            }
            let msg = cstr_to_string(self.msg).unwrap_or_default();
            free_c_ptr(self.msg as _);
            self.msg = null();
            (self.code as _, msg)
        }
    }
}

fn is_server_running() -> bool {
    crate::common::is_server() || crate::common::is_server_running()
}

pub fn init() {
    if !is_server_running() {
        std::thread::spawn(move || manager::start_ipc());
    } else {
        if let Err(e) = remove_uninstalled() {
            log::error!("Failed to remove plugins: {}", e);
        }
    }
    match manager::get_uninstall_id_set() {
        Ok(ids) => {
            if let Err(e) = plugins::load_plugins(&ids) {
                log::error!("Failed to load plugins: {}", e);
            }
        }
        Err(e) => {
            log::error!("Failed to load plugins: {}", e);
        }
    }
}

#[inline]
#[cfg(target_os = "windows")]
fn get_share_dir() -> ResultType<PathBuf> {
    Ok(PathBuf::from(env::var("ProgramData")?))
}

#[inline]
#[cfg(target_os = "linux")]
fn get_share_dir() -> ResultType<PathBuf> {
    Ok(PathBuf::from("/usr/share"))
}

#[inline]
#[cfg(target_os = "macos")]
fn get_share_dir() -> ResultType<PathBuf> {
    Ok(PathBuf::from("/Library/Application Support"))
}

#[inline]
fn get_plugins_dir() -> ResultType<PathBuf> {
    Ok(get_share_dir()?
        .join("RustDesk")
        .join(PLUGIN_SOURCE_LOCAL_DIR))
}

#[inline]
fn get_plugin_dir(id: &str) -> ResultType<PathBuf> {
    Ok(get_plugins_dir()?.join(id))
}

#[inline]
fn get_uninstall_file_path() -> ResultType<PathBuf> {
    Ok(get_plugins_dir()?.join("uninstall_list"))
}

#[inline]
fn cstr_to_string(cstr: *const c_char) -> ResultType<String> {
    if cstr.is_null() {
        bail!("failed to convert string, the pointer is null");
    }
    Ok(String::from_utf8(unsafe {
        CStr::from_ptr(cstr).to_bytes().to_vec()
    })?)
}

#[inline]
fn str_to_cstr_ret(s: &str) -> *const c_char {
    let mut s = s.as_bytes().to_vec();
    s.push(0);
    unsafe {
        let r = libc::malloc(s.len()) as *mut c_char;
        libc::memcpy(
            r as *mut libc::c_void,
            s.as_ptr() as *const libc::c_void,
            s.len(),
        );
        r
    }
}

#[inline]
fn free_c_ptr(p: *mut c_void) {
    if !p.is_null() {
        unsafe {
            libc::free(p);
        }
    }
}
