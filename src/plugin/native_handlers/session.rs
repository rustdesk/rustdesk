use std::sync::{atomic::AtomicU64, Arc, RwLock};

use crate::{
    call_if_method, define_method_prefix, flutter::FlutterHandler, return_if_not_method,
    ui_session_interface::Session,
};

use super::PluginNativeHandler;

#[derive(Default)]
/// Session related handler for librustdesk core.
pub struct PluginNativeSessionHandler {
    sessions: Arc<RwLock<Vec<Session<FlutterHandler>>>>,
    id: AtomicU64,
}

lazy_static::lazy_static! {
    pub static ref SESSION_HANDLER: Arc<PluginNativeSessionHandler> = Arc::new(PluginNativeSessionHandler::default());
}

impl PluginNativeHandler for PluginNativeSessionHandler {
    define_method_prefix!("session_");

    fn on_message(
        &self,
        method: &str,
        data: &serde_json::Map<String, serde_json::Value>,
    ) -> Option<super::NR> {
        match method {
            "create_session" => {
                return Some(super::NR {
                    return_type: 1,
                    data: SESSION_HANDLER.create_session() as _,
                });
            }
            "add_session_hook" => {
                if let Some(id) = data.get("id") {
                    if let Some(id) = id.as_u64() {
                        SESSION_HANDLER.add_session_hook(id);
                        return Some(super::NR {
                            return_type: 0,
                            data: std::ptr::null(),
                        });
                    }
                }
            }
            "remove_session_hook" => {
                if let Some(id) = data.get("id") {
                    if let Some(id) = id.as_u64() {
                        SESSION_HANDLER.remove_session_hook(id);
                        return Some(super::NR {
                            return_type: 0,
                            data: std::ptr::null(),
                        });
                    }
                }
            }
            "remove_session" => {
                if let Some(id) = data.get("id") {
                    if let Some(id) = id.as_u64() {
                        SESSION_HANDLER.remove_session(id);
                        return Some(super::NR {
                            return_type: 0,
                            data: std::ptr::null(),
                        });
                    }
                }
            }
            _ => {}
        }
        None
    }

    fn on_message_raw(
        &self,
        method: &str,
        data: &serde_json::Map<String, serde_json::Value>,
        raw: *const std::ffi::c_void,
        raw_len: usize,
    ) -> Option<super::NR> {
        None
    }
}

impl PluginNativeSessionHandler {
    fn create_session(&self) -> u64 {
        let mut sessions = self.sessions.write().unwrap();
        let unique_id = self.id.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let mut session: Session<FlutterHandler> = Session::default();
        session.id = self.get_hook_key(unique_id);
        sessions.push(session);
        return unique_id;
    }

    fn add_session_hook(&self, session_id: u64) {
        let sessions = self.sessions.read().unwrap();
        let session_id = self.get_hook_key(session_id);
        for session in sessions.iter() {
            if session.id == session_id {
                session.ui_handler.add_session_hook(
                    session_id.to_owned(),
                    crate::flutter::SessionHook::OnSessionRgba(session_rgba_cb),
                );
            }
        }
    }

    fn remove_session_hook(&self, session_id: u64) {
        let sessions = self.sessions.read().unwrap();
        let session_id = self.get_hook_key(session_id);
        for session in sessions.iter() {
            if session.id == session_id {
                session.ui_handler.remove_session_hook(&session_id);
            }
        }
    }

    fn remove_session(&self, session_id: u64) {
        let mut sessions = self.sessions.write().unwrap();
        let session_id = self.get_hook_key(session_id);
        for i in 0..sessions.len() {
            if sessions[i].id == session_id {
                sessions.remove(i);
            }
        }
    }

    #[inline]
    fn get_hook_key(&self, id: u64) -> String {
        format!("{}_{}", self.method_prefix(), id)
    }

    // The callback function for rgba data
    fn session_rgba_cb(&self, key: String, rgb: &mut scrap::ImageRgb) {
        todo!()
    }
}

fn session_rgba_cb(key: String, rgb: &mut scrap::ImageRgb) {
    SESSION_HANDLER.session_rgba_cb(key, rgb);
}
