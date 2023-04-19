use std::{
    collections::HashMap,
    ffi::c_char,
    path::Path,
    sync::{Arc, RwLock},
};

use super::{callback_msg, desc::Desc};
use crate::flutter;
use hbb_common::{bail, dlopen::symbor::Library, lazy_static, libc, log, ResultType};

lazy_static::lazy_static! {
    pub static ref PLUGINS: Arc<RwLock<HashMap<String, Plugin>>> = Default::default();
}

/// Initialize the plugins.
/// Return 0 if success.
pub type PluginFuncInit = fn() -> i32;
/// Reset the plugin.
/// Return 0 if success.
pub type PluginFuncReset = fn() -> i32;
/// Clear the plugin.
/// Return 0 if success.
pub type PluginFuncClear = fn() -> i32;
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
    content: *const c_char,
    len: usize,
);
pub type PluginFuncSetCallbackMsg = fn(PluginFuncCallbackMsg);
/// The main function of the plugin.
/// method: The method. "handle_ui" or "handle_peer"
/// args: The arguments.
/// out:  The output. The plugin allocate memory with `libc::malloc` and set the pointer to `out`.
/// out_len: The length of the output.
/// Return 0 if success.
pub type PluginFuncCall = fn(
    method: *const c_char,
    args: *const c_char,
    out: *mut *mut c_char,
    out_len: *mut usize,
) -> i32;

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

fn load_plugin(path: &str) -> ResultType<()> {
    let mut plugin = Plugin::new(path)?;
    let desc = (plugin.fn_desc)();
    let desc_res = Desc::from_cstr(desc);
    unsafe {
        libc::free(desc as _);
    }
    let desc = desc_res?;
    let id = desc.id().to_string();
    (plugin.fn_set_cb_msg)(callback_msg::callback_msg);
    update_config(&desc);
    reload_ui(&desc);
    plugin.desc = Some(desc);
    PLUGINS.write().unwrap().insert(id, plugin);
    Ok(())
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
