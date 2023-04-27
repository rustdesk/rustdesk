use super::{desc::Desc, errno::*, *};
#[cfg(not(debug_assertions))]
use crate::common::is_server;
use crate::flutter;
use hbb_common::{
    allow_err, bail,
    dlopen::symbor::Library,
    lazy_static, log,
    message_proto::{Message, Misc, PluginFailure, PluginRequest},
    ResultType,
};
use serde_derive::Serialize;
use std::{
    collections::HashMap,
    ffi::{c_char, c_void},
    path::PathBuf,
    sync::{Arc, RwLock}, os::raw::c_uint,
};

const METHOD_HANDLE_UI: &[u8; 10] = b"handle_ui\0";
const METHOD_HANDLE_PEER: &[u8; 12] = b"handle_peer\0";
pub const METHOD_HANDLE_LISTEN_EVENT: &[u8; 20] = b"handle_listen_event\0";

lazy_static::lazy_static! {
    static ref PLUGIN_INFO: Arc<RwLock<HashMap<String, PluginInfo>>> = Default::default();
    static ref PLUGINS: Arc<RwLock<HashMap<String, Plugin>>> = Default::default();
}

struct PluginInfo {
    path: String,
    desc: Desc,
}

/// Initialize the plugins.
///
/// data: The initialize data.
///
/// Return null ptr if success.
/// Return the error message if failed.  `i32-String` without dash, i32 is a signed little-endian number, the String is utf8 string.
/// The plugin allocate memory with `libc::malloc` and return the pointer.
type PluginFuncInit = extern "C" fn(data: *const InitData) -> *const c_void;
/// Reset the plugin.
///
/// data: The initialize data.
///
/// Return null ptr if success.
/// Return the error message if failed.  `i32-String` without dash, i32 is a signed little-endian number, the String is utf8 string.
/// The plugin allocate memory with `libc::malloc` and return the pointer.
type PluginFuncReset = extern "C" fn(data: *const InitData) -> *const c_void;
/// Clear the plugin.
///
/// Return null ptr if success.
/// Return the error message if failed.  `i32-String` without dash, i32 is a signed little-endian number, the String is utf8 string.
/// The plugin allocate memory with `libc::malloc` and return the pointer.
type PluginFuncClear = extern "C" fn() -> *const c_void;
/// Get the description of the plugin.
/// Return the description. The plugin allocate memory with `libc::malloc` and return the pointer.
type PluginFuncDesc = extern "C" fn() -> *const c_char;
/// Callback to send message to peer or ui.
/// peer, target, id are utf8 strings(null terminated).
///
/// peer:    The peer id.
/// target:  "peer" or "ui".
/// id:      The id of this plugin.
/// content: The content.
/// len:     The length of the content.
type CallbackMsg = extern "C" fn(
    peer: *const c_char,
    target: *const c_char,
    id: *const c_char,
    content: *const c_void,
    len: usize,
);
/// Callback to get the config.
/// peer, key are utf8 strings(null terminated).
///
/// peer: The peer id.
/// id:  The id of this plugin.
/// key:  The key of the config.
///
/// The returned string is utf8 string(null terminated) and must be freed by caller.
type CallbackGetConf =
    extern "C" fn(peer: *const c_char, id: *const c_char, key: *const c_char) -> *const c_char;
/// Get local peer id.
///
/// The returned string is utf8 string(null terminated) and must be freed by caller.
type CallbackGetId = extern "C" fn() -> *const c_char;
/// Callback to log.
///
/// level, msg are utf8 strings(null terminated).
/// level: "error", "warn", "info", "debug", "trace".
/// msg:   The message.
type CallbackLog = extern "C" fn(level: *const c_char, msg: *const c_char);

/// Callback to the librustdesk core.
/// 
/// method: the method name of this callback.
/// json: the json data for the parameters. The argument *must* be non-null.
/// raw: the binary data for this call, nullable.
/// raw_len: the length of this binary data, only valid when we pass raw data to `raw`.
type CallbackNative = extern "C" fn(method: *const c_char, json: *const c_char, raw: *const c_void, raw_len: usize) -> super::native::NativeReturnValue;
/// The main function of the plugin on the client(self) side.
///
/// method: The method. "handle_ui" or "handle_peer"
/// peer:  The peer id.
/// args: The arguments.
/// len:  The length of the arguments.
///
/// Return null ptr if success.
/// Return the error message if failed.  `i32-String` without dash, i32 is a signed little-endian number, the String is utf8 string.
/// The plugin allocate memory with `libc::malloc` and return the pointer.
type PluginFuncClientCall = extern "C" fn(
    method: *const c_char,
    peer: *const c_char,
    args: *const c_void,
    len: usize,
) -> *const c_void;
/// The main function of the plugin on the server(remote) side.
///
/// method: The method. "handle_ui" or "handle_peer"
/// peer:  The peer id.
/// args: The arguments.
/// len:  The length of the arguments.
/// out:  The output.
///       The plugin allocate memory with `libc::malloc` and return the pointer.
/// out_len: The length of the output.
///
/// Return null ptr if success.
/// Return the error message if failed.  `i32-String` without dash, i32 is a signed little-endian number, the String is utf8 string.
/// The plugin allocate memory with `libc::malloc` and return the pointer.
type PluginFuncServerCall = extern "C" fn(
    method: *const c_char,
    peer: *const c_char,
    args: *const c_void,
    len: usize,
    out: *mut *mut c_void,
    out_len: *mut usize,
) -> *const c_void;

/// The plugin callbacks.
/// msg: The callback to send message to peer or ui.
/// get_conf: The callback to get the config.
/// log: The callback to log.
#[repr(C)]
#[derive(Copy, Clone)]
struct Callbacks {
    msg: CallbackMsg,
    get_conf: CallbackGetConf,
    get_id: CallbackGetId,
    log: CallbackLog,
    native: CallbackNative
}

/// The plugin initialize data.
/// version: The version of the plugin, can't be nullptr.
/// local_peer_id: The local peer id, can't be nullptr.
/// cbs: The callbacks.
#[repr(C)]
struct InitData {
    version: *const c_char,
    cbs: Callbacks,
}

impl Drop for InitData {
    fn drop(&mut self) {
        free_c_ptr(self.version as _);
    }
}

macro_rules! make_plugin {
    ($($field:ident : $tp:ty),+) => {
        #[allow(dead_code)]
        pub struct Plugin {
            _lib: Library,
            id: Option<String>,
            path: String,
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
                            log::debug!("{} method found {}", path, stringify!($field));
                            *m
                        },
                        Err(e) => {
                            bail!("Failed to load {} func {}, {}", path, stringify!($field), e);
                        }
                    }
                ;)+

                Ok(Self {
                    _lib: lib,
                    id: None,
                    path: path.to_string(),
                    $( $field ),+
                })
            }

            fn desc(&self) -> ResultType<Desc> {
                let desc_ret = (self.desc)();
                let desc = Desc::from_cstr(desc_ret);
                free_c_ptr(desc_ret as _);
                desc
            }

            fn init(&self, data: &InitData, path: &str) -> ResultType<()> {
                let init_ret = (self.init)(data as _);
                if !init_ret.is_null() {
                    let (code, msg) = get_code_msg_from_ret(init_ret);
                    free_c_ptr(init_ret as _);
                    bail!(
                        "Failed to init plugin {}, code: {}, msg: {}",
                        path,
                        code,
                        msg
                    );
                }
                Ok(())
            }

            fn clear(&self, id: &str) {
                let clear_ret = (self.clear)();
                if !clear_ret.is_null() {
                    let (code, msg) = get_code_msg_from_ret(clear_ret);
                    free_c_ptr(clear_ret as _);
                    log::error!(
                        "Failed to clear plugin {}, code: {}, msg: {}",
                        id,
                        code,
                        msg
                    );
                }
            }
        }

        impl Drop for Plugin {
            fn drop(&mut self) {
                let id = self.id.as_ref().unwrap_or(&self.path);
                self.clear(id);
            }
        }
    }
}

make_plugin!(
    init: PluginFuncInit,
    reset: PluginFuncReset,
    clear: PluginFuncClear,
    desc: PluginFuncDesc,
    client_call: PluginFuncClientCall,
    server_call: PluginFuncServerCall
);

#[derive(Serialize)]
pub struct MsgListenEvent {
    pub event: String,
}

#[cfg(target_os = "windows")]
const DYLIB_SUFFIX: &str = ".dll";
#[cfg(target_os = "linux")]
const DYLIB_SUFFIX: &str = ".so";
#[cfg(target_os = "macos")]
const DYLIB_SUFFIX: &str = ".dylib";

pub fn load_plugins() -> ResultType<()> {
    let exe = std::env::current_exe()?.to_string_lossy().to_string();
    match PathBuf::from(&exe).parent() {
        Some(dir) => {
            for entry in std::fs::read_dir(dir)? {
                match entry {
                    Ok(entry) => {
                        let path = entry.path();
                        if path.is_file() {
                            let filename = entry.file_name();
                            let filename = filename.to_str().unwrap_or("");
                            if filename.starts_with("plugin_") && filename.ends_with(DYLIB_SUFFIX) {
                                if let Err(e) = load_plugin(Some(path.to_str().unwrap_or("")), None)
                                {
                                    log::error!("Failed to load plugin {}, {}", filename, e);
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
        None => {
            bail!("Failed to get parent dir of {}", exe);
        }
    }
}

pub fn unload_plugins() {
    log::info!("Plugins unloaded");
    PLUGINS.write().unwrap().clear();
    if change_manager() {
        super::config::ManagerConfig::remove_plugins(false);
    }
}

pub fn unload_plugin(id: &str) {
    log::info!("Plugin {} unloaded", id);
    PLUGINS.write().unwrap().remove(id);
    if change_manager() {
        allow_err!(super::config::ManagerConfig::remove_plugin(id, false));
    }
}

pub fn reload_plugin(id: &str) -> ResultType<()> {
    let path = match PLUGIN_INFO.read().unwrap().get(id) {
        Some(plugin) => plugin.path.clone(),
        None => bail!("Plugin {} not found", id),
    };
    unload_plugin(id);
    load_plugin(Some(&path), Some(id))
}

fn load_plugin_path(path: &str) -> ResultType<()> {
    let plugin = Plugin::new(path)?;
    let desc = plugin.desc()?;

    // to-do validate plugin
    // to-do check the plugin id (make sure it does not use another plugin's id)

    let init_data = InitData {
        version: str_to_cstr_ret(crate::VERSION),
        cbs: Callbacks {
            msg: callback_msg::cb_msg,
            get_conf: config::cb_get_conf,
            get_id: config::cb_get_local_peer_id,
            log: super::plog::plugin_log,
            native: super::native::cb_native_data
        },
    };
    plugin.init(&init_data, path)?;

    if change_manager() {
        super::config::ManagerConfig::add_plugin(desc.id())?;
    }

    // update ui
    // Ui may be not ready now, so we need to update again once ui is ready.
    update_ui_plugin_desc(&desc, None);
    reload_ui(&desc, None);

    // add plugins
    let id = desc.id().to_string();
    let plugin_info = PluginInfo {
        path: path.to_string(),
        desc,
    };
    PLUGIN_INFO.write().unwrap().insert(id.clone(), plugin_info);
    PLUGINS.write().unwrap().insert(id.clone(), plugin);

    log::info!("Plugin {} loaded", id);
    Ok(())
}

pub fn sync_ui(sync_to: String) {
    for plugin in PLUGIN_INFO.read().unwrap().values() {
        update_ui_plugin_desc(&plugin.desc, Some(&sync_to));
        reload_ui(&plugin.desc, Some(&sync_to));
    }
}

pub fn load_plugin(path: Option<&str>, id: Option<&str>) -> ResultType<()> {
    match (path, id) {
        (Some(path), _) => load_plugin_path(path),
        (None, Some(id)) => {
            let path = match PLUGIN_INFO.read().unwrap().get(id) {
                Some(plugin) => plugin.path.clone(),
                None => bail!("Plugin {} not found", id),
            };
            load_plugin_path(&path)
        }
        (None, None) => {
            bail!("path and id are both None");
        }
    }
}

fn handle_event(method: &[u8], id: &str, peer: &str, event: &[u8]) -> ResultType<()> {
    let mut peer: String = peer.to_owned();
    peer.push('\0');
    match PLUGINS.read().unwrap().get(id) {
        Some(plugin) => {
            let ret = (plugin.client_call)(
                method.as_ptr() as _,
                peer.as_ptr() as _,
                event.as_ptr() as _,
                event.len(),
            );
            if ret.is_null() {
                Ok(())
            } else {
                let (code, msg) = get_code_msg_from_ret(ret);
                free_c_ptr(ret as _);
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
pub fn handle_ui_event(id: &str, peer: &str, event: &[u8]) -> ResultType<()> {
    handle_event(METHOD_HANDLE_UI, id, peer, event)
}

#[inline]
pub fn handle_server_event(id: &str, peer: &str, event: &[u8]) -> ResultType<()> {
    handle_event(METHOD_HANDLE_PEER, id, peer, event)
}

fn _handle_listen_event(event: String, peer: String) {
    let mut plugins = Vec::new();
    for info in PLUGIN_INFO.read().unwrap().values() {
        if info.desc.listen_events().contains(&event.to_string()) {
            plugins.push(info.desc.id().to_string());
        }
    }

    if plugins.is_empty() {
        return;
    }

    if let Ok(evt) = serde_json::to_string(&MsgListenEvent {
        event: event.clone(),
    }) {
        let mut evt_bytes = evt.as_bytes().to_vec();
        evt_bytes.push(0);
        let mut peer: String = peer.to_owned();
        peer.push('\0');
        for id in plugins {
            match PLUGINS.read().unwrap().get(&id) {
                Some(plugin) => {
                    let ret = (plugin.client_call)(
                        METHOD_HANDLE_LISTEN_EVENT.as_ptr() as _,
                        peer.as_ptr() as _,
                        evt_bytes.as_ptr() as _,
                        evt_bytes.len(),
                    );
                    if !ret.is_null() {
                        let (code, msg) = get_code_msg_from_ret(ret);
                        free_c_ptr(ret as _);
                        log::error!(
                            "Failed to handle plugin listen event, id: {}, event: {}, code: {}, msg: {}",
                            id,
                            event,
                            code,
                            msg
                        );
                    }
                }
                None => {
                    log::error!("Plugin {} not found when handle_listen_event", id);
                }
            }
        }
    }
}

#[inline]
pub fn handle_listen_event(event: String, peer: String) {
    std::thread::spawn(|| _handle_listen_event(event, peer));
}

#[inline]
pub fn handle_client_event(id: &str, peer: &str, event: &[u8]) -> Message {
    let mut peer: String = peer.to_owned();
    peer.push('\0');
    match PLUGINS.read().unwrap().get(id) {
        Some(plugin) => {
            let mut out = std::ptr::null_mut();
            let mut out_len: usize = 0;
            let ret = (plugin.server_call)(
                METHOD_HANDLE_PEER.as_ptr() as _,
                peer.as_ptr() as _,
                event.as_ptr() as _,
                event.len(),
                &mut out as _,
                &mut out_len as _,
            );
            if ret.is_null() {
                let msg = make_plugin_request(id, out, out_len);
                free_c_ptr(out as _);
                msg
            } else {
                let (code, msg) = get_code_msg_from_ret(ret);
                free_c_ptr(ret as _);
                if code > ERR_RUSTDESK_HANDLE_BASE && code < ERR_PLUGIN_HANDLE_BASE {
                    log::debug!(
                        "Plugin {} failed to handle client event, code: {}, msg: {}",
                        id,
                        code,
                        msg
                    );
                    let name = match PLUGIN_INFO.read().unwrap().get(id) {
                        Some(plugin) => plugin.desc.name(),
                        None => "???",
                    }
                    .to_owned();
                    match code {
                        ERR_CALL_NOT_SUPPORTED_METHOD => {
                            make_plugin_failure(id, &name, "Plugin method is not supported")
                        }
                        ERR_CALL_INVALID_ARGS => {
                            make_plugin_failure(id, &name, "Plugin arguments is invalid")
                        }
                        _ => make_plugin_failure(id, &name, &msg),
                    }
                } else {
                    log::error!(
                        "Plugin {} failed to handle client event, code: {}, msg: {}",
                        id,
                        code,
                        msg
                    );
                    let msg = make_plugin_request(id, out, out_len);
                    free_c_ptr(out as _);
                    msg
                }
            }
        }
        None => make_plugin_failure(id, "", "Plugin not found"),
    }
}

fn make_plugin_request(id: &str, content: *const c_void, len: usize) -> Message {
    let mut misc = Misc::new();
    misc.set_plugin_request(PluginRequest {
        id: id.to_owned(),
        content: unsafe { std::slice::from_raw_parts(content as *const u8, len) }
            .clone()
            .into(),
        ..Default::default()
    });
    let mut msg_out = Message::new();
    msg_out.set_misc(misc);
    msg_out
}

fn make_plugin_failure(id: &str, name: &str, msg: &str) -> Message {
    let mut misc = Misc::new();
    misc.set_plugin_failure(PluginFailure {
        id: id.to_owned(),
        name: name.to_owned(),
        msg: msg.to_owned(),
        ..Default::default()
    });
    let mut msg_out = Message::new();
    msg_out.set_misc(misc);
    msg_out
}

#[inline]
fn change_manager() -> bool {
    #[cfg(debug_assertions)]
    let change_manager = true;
    #[cfg(not(debug_assertions))]
    let change_manager = is_server();
    change_manager
}

fn reload_ui(desc: &Desc, sync_to: Option<&str>) {
    for (location, ui) in desc.location().ui.iter() {
        if let Ok(ui) = serde_json::to_string(&ui) {
            let make_event = |ui: &str| {
                let mut m = HashMap::new();
                m.insert("name", MSG_TO_UI_TYPE_PLUGIN_RELOAD);
                m.insert("id", desc.id());
                m.insert("location", &location);
                // Do not depend on the "location" and plugin desc on the ui side.
                // Send the ui field to ensure the ui is valid.
                m.insert("ui", ui);
                serde_json::to_string(&m).unwrap_or("".to_owned())
            };
            match sync_to {
                Some(channel) => {
                    let _res = flutter::push_global_event(channel, make_event(&ui));
                }
                None => {
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
                            let _res = flutter::push_global_event(v[1], make_event(&ui));
                        }
                    }
                }
            }
        }
    }
}

fn update_ui_plugin_desc(desc: &Desc, sync_to: Option<&str>) {
    // This function is rarely used. There's no need to care about serialization efficiency here.
    if let Ok(desc_str) = serde_json::to_string(desc) {
        let mut m = HashMap::new();
        m.insert("name", MSG_TO_UI_TYPE_PLUGIN_DESC);
        m.insert("desc", &desc_str);
        let event = serde_json::to_string(&m).unwrap_or("".to_owned());
        match sync_to {
            Some(channel) => {
                let _res = flutter::push_global_event(channel, event.clone());
            }
            None => {
                let _res = flutter::push_global_event(flutter::APP_TYPE_MAIN, event.clone());
                let _res =
                    flutter::push_global_event(flutter::APP_TYPE_DESKTOP_REMOTE, event.clone());
                let _res = flutter::push_global_event(flutter::APP_TYPE_CM, event.clone());
            }
        }
    }
}

pub(super) fn get_desc_conf(id: &str) -> Option<super::desc::Config> {
    PLUGIN_INFO
        .read()
        .unwrap()
        .get(id)
        .map(|info| info.desc.config().clone())
}
