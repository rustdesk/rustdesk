use std::ffi::{c_char};

use crate::{
    flutter::{FlutterHandler, SESSIONS},
    plugins::PLUGIN_REGISTRAR,
    ui_session_interface::Session,
};

// API provided by RustDesk.
pub type LoadPluginFunc = fn(*const c_char) -> i32;
pub type UnloadPluginFunc = fn(*const c_char) -> i32;
pub type AddSessionFunc = fn(session_id: String) -> bool;
pub type RemoveSessionFunc = fn(session_id: &String) -> bool;
pub type AddSessionHookFunc = fn(session_id: String, key: String, hook: SessionHook) -> bool;
pub type RemoveSessionHookFunc = fn(session_id: String, key: &String) -> bool;

/// Hooks for session.
#[derive(Clone)]
pub enum SessionHook {
    OnSessionRgba(fn(String, Vec<i8>) -> Vec<i8>),
}

// #[repr(C)]
pub struct RustDeskApiTable {
    pub(crate) load_plugin: LoadPluginFunc,
    pub(crate) unload_plugin: UnloadPluginFunc,
    pub add_session: AddSessionFunc,
    pub remove_session: RemoveSessionFunc,
    pub add_session_hook: AddSessionHookFunc,
    pub remove_session_hook: RemoveSessionHookFunc,
}

fn load_plugin(path: *const c_char) -> i32 {
    PLUGIN_REGISTRAR.load_plugin(path)
}

fn unload_plugin(path: *const c_char) -> i32 {
    PLUGIN_REGISTRAR.unload_plugin(path)
}

fn add_session(session_id: String) -> bool {
    // let mut sessions = SESSIONS.write().unwrap();
    // if sessions.contains_key(&session.id) {
    //     return false;
    // }
    // let _ = sessions.insert(session.id.to_owned(), session);
    // true
    false
}

fn remove_session(session_id: &String) -> bool {
    let mut sessions = SESSIONS.write().unwrap();
    if !sessions.contains_key(session_id) {
        return false;
    }
    let _ = sessions.remove(session_id);
    true
}

fn add_session_hook(session_id: String, key: String, hook: SessionHook) -> bool {
    let sessions = SESSIONS.read().unwrap();
    if let Some(session) = sessions.get(&session_id) {
        return session.add_session_hook(key, hook);
    }
    false
}

fn remove_session_hook(session_id: String, key: &String) -> bool {
    let sessions = SESSIONS.read().unwrap();
    if let Some(session) = sessions.get(&session_id) {
        return session.remove_session_hook(key);
    }
    false
}

impl Default for RustDeskApiTable {
    fn default() -> Self {
        Self {
            load_plugin,
            unload_plugin,
            add_session,
            remove_session,
            add_session_hook,
            remove_session_hook,
        }
    }
}
