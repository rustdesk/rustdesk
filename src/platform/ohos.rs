// Minimal OpenHarmony adapter for native frontend callbacks and mobile clipboard state.
use crate::flutter_ffi::{EventToUI, SessionID};
use hbb_common::{
    message_proto::{Clipboard, ClipboardFormat, MultiClipboards},
    ResultType,
};
use std::{collections::HashSet, sync::Mutex};

pub type SessionEventCallback = fn(SessionID, EventToUI);
pub type RenderStatsCallback = fn(String, usize, Option<u64>);
pub use scrap::ohos::DirectRenderTarget;

lazy_static::lazy_static! {
    static ref SESSION_EVENT_CALLBACK: Mutex<Option<SessionEventCallback>> = Default::default();
    static ref RENDER_STATS_CALLBACK: Mutex<Option<RenderStatsCallback>> = Default::default();
    static ref STARTED_SESSIONS: Mutex<HashSet<SessionID>> = Default::default();
    static ref CLIPBOARDS_HOST: Mutex<Option<MultiClipboards>> = Default::default();
    static ref CLIENT_CLIPBOARD: Mutex<ClientClipboardState> = Default::default();
}

struct ClientClipboardState {
    enabled: bool,
    clipboards: Option<MultiClipboards>,
}

impl Default for ClientClipboardState {
    fn default() -> Self {
        Self {
            enabled: true,
            clipboards: None,
        }
    }
}

pub fn set_client_clipboard_enabled(enabled: bool) {
    let mut state = CLIENT_CLIPBOARD.lock().unwrap();
    state.enabled = enabled;
    if !enabled {
        state.clipboards.take();
    }
}

pub fn update_client_text_clipboard(content: String) -> bool {
    let mut state = CLIENT_CLIPBOARD.lock().unwrap();
    if !state.enabled {
        return false;
    }
    state.clipboards = Some(MultiClipboards {
        clipboards: vec![Clipboard {
            content: content.into_bytes().into(),
            format: ClipboardFormat::Text.into(),
            ..Default::default()
        }],
        ..Default::default()
    });
    true
}

pub fn update_clipboards(client: bool, clipboards: MultiClipboards) {
    if client {
        CLIENT_CLIPBOARD.lock().unwrap().clipboards = Some(clipboards);
    } else {
        *CLIPBOARDS_HOST.lock().unwrap() = Some(clipboards);
    }
}

pub(crate) fn get_clipboards(client: bool) -> Option<MultiClipboards> {
    if client {
        CLIENT_CLIPBOARD.lock().unwrap().clipboards.take()
    } else {
        CLIPBOARDS_HOST.lock().unwrap().take()
    }
}

pub fn register_session_event_callback(callback: SessionEventCallback) {
    *SESSION_EVENT_CALLBACK.lock().unwrap() = Some(callback);
}

pub fn session_start_with_polling_events(session_id: &SessionID, id: &str) -> ResultType<()> {
    let inserted = STARTED_SESSIONS.lock().unwrap().insert(*session_id);
    let already_started = !inserted;
    if let Err(err) = crate::flutter::session_start_(session_id, id, already_started) {
        if inserted {
            STARTED_SESSIONS.lock().unwrap().remove(session_id);
        }
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
    scrap::ohos::register_render_stats_callback(notify_frame_rendered);
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
