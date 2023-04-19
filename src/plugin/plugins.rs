use std::{
    collections::HashMap,
    ffi::c_char,
    path::Path,
    sync::{Arc, RwLock},
};

use super::{callback_msg, desc::Desc, errno::*, get_code_msg_from_ret};
use crate::flutter;
use hbb_common::{
    bail,
    dlopen::symbor::Library,
    lazy_static, libc, log,
    message_proto::{Message, Misc, PluginResponse},
    ResultType,
};

const METHOD_HANDLE_UI: &[u8; 10] = b"handle_ui\0";
const METHOD_HANDLE_PEER: &[u8; 12] = b"handle_peer\0";

lazy_static::lazy_static! {
    pub static ref PLUGINS: Arc<RwLock<HashMap<String, Plugin>>> = Default::default();
}

/// Initialize the plugins.
///
/// Return null ptr if success.
/// Return the error message if failed.  `i32-String` without dash, i32 is a signed little-endian number, the String is utf8 string.
/// The plugin allocate memory with `libc::malloc` and return the pointer.
pub type PluginFuncInit = fn() -> *const c_void;
/// Reset the plugin.
///
/// Return null ptr if success.
/// Return the error message if failed.  `i32-String` without dash, i32 is a signed little-endian number, the String is utf8 string.
/// The plugin allocate memory with `libc::malloc` and return the pointer.
pub type PluginFuncReset = fn() -> *const c_void;
/// Clear the plugin.
///
/// Return null ptr if success.
/// Return the error message if failed.  `i32-String` without dash, i32 is a signed little-endian number, the String is utf8 string.
/// The plugin allocate memory with `libc::malloc` and return the pointer.
pub type PluginFuncClear = fn() -> *const c_void;
/// Get the description of the plugin.
/// Return the description. The plugin allocate memory with `libc::malloc` and return the pointer.
pub type PluginFuncDesc = fn() -> *const c_char;
/// Callback to send message to peer or ui.
/// peer, target, id are utf8 strings(null terminated).
///
/// peer:    The peer id.
/// target:  "peer" or "ui".
/// id:      The id of this plugin.
/// content: The content.
/// len:     The length of the content.
type PluginFuncCallbackMsg = fn(
    peer: *const c_char,
    target: *const c_char,
    id: *const c_char,
    content: *const c_void,
    len: usize,
);
pub type PluginFuncSetCallbackMsg = fn(PluginFuncCallbackMsg);
/// The main function of the plugin.
/// method: The method. "handle_ui" or "handle_peer"
/// args: The arguments.
///
/// Return null ptr if success.
/// Return the error message if failed.  `i32-String` without dash, i32 is a signed little-endian number, the String is utf8 string.
/// The plugin allocate memory with `libc::malloc` and return the pointer.
pub type PluginFuncCall =
    fn(method: *const c_char, args: *const c_void, len: usize) -> *const c_void;

macro_rules! make_plugin {
    ($($field:ident : $tp:ty),+) => {
        pub struct Plugin {
            _lib: Library,
            path: String,
            desc: Option<Desc>,
            $($field: $tp),+
        }

        impl Plugin {
            fn new(path: &str) -> ResultType<Self> {
                let lib = match Library::open(path) {
                    Ok(lib) => lib,
                    Err(e) => {
                        bail!("Failed to load library {}, {}", path, e);
                    }
                };

                $(let $field = match unsafe { lib.symbol::<$tp>(stringify!($field)) } {
                        Ok(m) => {
                            log::info!("method found {}", stringify!($field));
                            *m
                        },
                        Err(e) => {
                            bail!("Failed to load {} func {}, {}", path, stringify!($field), e);
                        }
                    }
                ;)+

                Ok(Self {
                    _lib: lib,
                    path: path.to_string(),
                    desc: None,
                    $( $field ),+
                })
            }
        }
    }
}

make_plugin!(
    fn_init: PluginFuncInit,
    fn_reset: PluginFuncReset,
    fn_clear: PluginFuncClear,
    fn_desc: PluginFuncDesc,
    fn_set_cb_msg: PluginFuncSetCallbackMsg,
    fn_call: PluginFuncCall
);

pub fn load_plugins<P: AsRef<Path>>(dir: P) -> ResultType<()> {
    for entry in std::fs::read_dir(dir)? {
        match entry {
            Ok(entry) => {
                let path = entry.path();
                if path.is_file() {
                    let path = path.to_str().unwrap_or("");
                    if path.ends_with(".so") {
                        if let Err(e) = load_plugin(path) {
                            log::error!("{e}");
                        }
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to read dir entry, {}", e);
            }
        }
    }
    Ok(())
}

pub fn unload_plugin(id: &str) {
    if let Some(plugin) = PLUGINS.write().unwrap().remove(id) {
        let _ret = (plugin.fn_clear)();
    }
}

pub fn reload_plugin(id: &str) -> ResultType<()> {
    let path = match PLUGINS.read().unwrap().get(id) {
        Some(plugin) => plugin.path.clone(),
        None => bail!("Plugin {} not found", id),
    };
    unload_plugin(id);
    load_plugin(&path)
}

pub fn load_plugin(path: &str) -> ResultType<()> {
    let mut plugin = Plugin::new(path)?;
    let desc = (plugin.fn_desc)();
    let desc_res = Desc::from_cstr(desc);
    unsafe {
        libc::free(desc as _);
    }
    let desc = desc_res?;
    let id = desc.id().to_string();
    // to-do validate plugin
    (plugin.fn_set_cb_msg)(callback_msg::callback_msg);
    update_config(&desc);
    reload_ui(&desc);
    plugin.desc = Some(desc);
    PLUGINS.write().unwrap().insert(id, plugin);
    Ok(())
}

fn handle_event(method: &[u8], id: &str, event: &[u8]) -> ResultType<()> {
    match PLUGINS.read().unwrap().get(id) {
        Some(plugin) => {
            let ret = (plugin.fn_call)(method.as_ptr() as _, event.as_ptr(), event.len());
            if ret.is_null() {
                Ok(())
            } else {
                let (code, msg) = get_code_msg_from_ret(ret);
                unsafe {
                    libc::free(ret);
                }
                bail!(
                    "Failed to handle plugin event, id: {}, method: {}, code: {}, msg: {}",
                    id,
                    std::string::String::from_utf8(method.to_vec()).unwrap_or_default(),
                    code,
                    msg
                );
            }
        }
        None => bail!("Plugin {} not found", id),
    }
}

#[inline]
pub fn handle_ui_event(id: &str, event: &[u8]) -> ResultType<()> {
    handle_event(METHOD_HANDLE_UI, id, event)
}

#[inline]
pub fn handle_server_event(id: &str, event: &[u8]) -> ResultType<()> {
    handle_event(METHOD_HANDLE_PEER, id, event)
}

#[inline]
pub fn handle_client_event(id: &str, event: &[u8]) -> Option<Message> {
    match PLUGINS.read().unwrap().get(id) {
        Some(plugin) => {
            let ret = (plugin.fn_call)(
                METHOD_HANDLE_PEER.as_ptr() as _,
                event.as_ptr(),
                event.len(),
            );
            if ret.is_null() {
                None
            } else {
                let (code, msg) = get_code_msg_from_ret(ret);
                unsafe {
                    libc::free(ret);
                }
                if code > ERR_RUSTDESK_HANDLE_BASE && code < ERR_PLUGIN_HANDLE_BASE {
                    let name = match PLUGINS.read().unwrap().get(id) {
                        Some(plugin) => plugin.desc.as_ref().unwrap().name(),
                        None => "",
                    };
                    match code {
                        ERR_CALL_NOT_SUPPORTED_METHOD => Some(make_plugin_response(
                            id,
                            name,
                            "plugin method is not supported",
                        )),
                        ERR_CALL_INVALID_ARGS => Some(make_plugin_response(
                            id,
                            name,
                            "plugin arguments is invalid",
                        )),
                        _ => Some(make_plugin_response(id, name, &msg)),
                    }
                } else {
                    log::error!(
                        "Failed to handle client event, code: {}, msg: {}",
                        code,
                        msg
                    );
                    None
                }
            }
        }
        None => Some(make_plugin_response(id, "", "plugin not found")),
    }
}

fn make_plugin_response(id: &str, name: &str, msg: &str) -> Message {
    let mut misc = Misc::new();
    misc.set_plugin_response(PluginResponse {
        id: id.to_owned(),
        name: name.to_owned(),
        msg: msg.to_owned(),
        ..Default::default()
    });
    let mut msg_out = Message::new();
    msg_out.set_misc(misc);
    msg_out
}

fn update_config(desc: &Desc) {
    super::config::set_local_items(desc.id(), &desc.config().local);
    super::config::set_peer_items(desc.id(), &desc.config().peer);
}

fn reload_ui(desc: &Desc) {
    for (location, ui) in desc.location().ui.iter() {
        let v: Vec<&str> = location.split('|').collect();
        // The first element is the "client" or "host".
        // The second element is the "main", "remote", "cm", "file transfer", "port forward".
        if v.len() >= 2 {
            let available_channels = vec![
                flutter::APP_TYPE_MAIN,
                flutter::APP_TYPE_DESKTOP_REMOTE,
                flutter::APP_TYPE_CM,
                flutter::APP_TYPE_DESKTOP_FILE_TRANSFER,
                flutter::APP_TYPE_DESKTOP_PORT_FORWARD,
            ];
            if available_channels.contains(&v[1]) {
                if let Ok(ui) = serde_json::to_string(&ui) {
                    let mut m = HashMap::new();
                    m.insert("name", "plugin_reload");
                    m.insert("ui", &ui);
                    flutter::push_global_event(v[1], serde_json::to_string(&m).unwrap());
                }
            }
        }
    }
}
