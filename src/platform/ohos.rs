// Minimal OpenHarmony adapter for native frontend callbacks and mobile clipboard state.
use crate::flutter_ffi::{EventToUI, SessionID};
use hbb_common::{
    message_proto::{Clipboard, ClipboardFormat, MultiClipboards},
    ResultType,
};
use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};

pub type SessionEventCallback = fn(SessionID, EventToUI);
pub type RenderStatsCallback = fn(String, usize, Option<u64>);
pub use scrap::ohos::DirectRenderTarget;

lazy_static::lazy_static! {
    static ref SESSION_EVENT_CALLBACK: Mutex<Option<SessionEventCallback>> = Default::default();
    static ref RENDER_STATS_CALLBACK: Mutex<Option<RenderStatsCallback>> = Default::default();
    static ref STARTED_SESSIONS: Mutex<HashSet<SessionID>> = Default::default();
    static ref CLIPBOARDS_HOST: Mutex<Option<MultiClipboards>> = Default::default();
    static ref CLIPBOARDS_CLIENT: Mutex<Option<MultiClipboards>> = Default::default();
}

static CLIENT_CLIPBOARD_ENABLED: AtomicBool = AtomicBool::new(true);

pub fn set_client_clipboard_enabled(enabled: bool) {
    CLIENT_CLIPBOARD_ENABLED.store(enabled, Ordering::Relaxed);
    if !enabled {
        CLIPBOARDS_CLIENT.lock().unwrap().take();
    }
}

pub fn update_client_text_clipboard(content: String) -> bool {
    if !CLIENT_CLIPBOARD_ENABLED.load(Ordering::Relaxed) {
        return false;
    }
    update_clipboards(
        true,
        MultiClipboards {
            clipboards: vec![Clipboard {
                content: content.into_bytes().into(),
                format: ClipboardFormat::Text.into(),
                ..Default::default()
            }],
            ..Default::default()
        },
    );
    true
}

pub fn update_clipboards(client: bool, clipboards: MultiClipboards) {
    if client {
        *CLIPBOARDS_CLIENT.lock().unwrap() = Some(clipboards);
    } else {
        *CLIPBOARDS_HOST.lock().unwrap() = Some(clipboards);
    }
}

pub(crate) fn get_clipboards(client: bool) -> Option<MultiClipboards> {
    if client {
        CLIPBOARDS_CLIENT.lock().ok()?.take()
    } else {
        CLIPBOARDS_HOST.lock().ok()?.take()
    }
}

pub fn register_session_event_callback(callback: SessionEventCallback) {
    *SESSION_EVENT_CALLBACK.lock().unwrap() = Some(callback);
}

pub fn session_start_with_polling_events(session_id: &SessionID, id: &str) -> ResultType<()> {
    let already_started = !STARTED_SESSIONS.lock().unwrap().insert(*session_id);
    if let Err(err) = crate::flutter::session_start_(session_id, id, already_started) {
        STARTED_SESSIONS.lock().unwrap().remove(session_id);
        return Err(err);
    }
    Ok(())
}

pub(crate) fn emit_session_event(session_id: &SessionID, event: EventToUI) -> bool {
    let callback = *SESSION_EVENT_CALLBACK.lock().unwrap();
    if let Some(callback) = callback {
        callback(*session_id, event);
        true
    } else {
        false
    }
}

pub(crate) fn finish_session(session_id: &SessionID) {
    STARTED_SESSIONS.lock().unwrap().remove(session_id);
}

pub fn register_render_stats_callback(callback: RenderStatsCallback) {
    *RENDER_STATS_CALLBACK.lock().unwrap() = Some(callback);
}

pub(crate) fn notify_frame_rendered(session: String, display: usize, latency: Option<u64>) {
    let callback = *RENDER_STATS_CALLBACK.lock().unwrap();
    if let Some(callback) = callback {
        callback(session, display, latency);
    }
}

pub fn register_direct_render_target_lookup(lookup: fn(&str, usize) -> Option<DirectRenderTarget>) {
    scrap::ohos::register_direct_render_target_lookup(lookup);
}
