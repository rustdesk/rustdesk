// Minimal OpenHarmony adapter for native frontend callbacks and mobile clipboard state.
use crate::flutter_ffi::{EventToUI, SessionID};
use hbb_common::{
    message_proto::{key_event, Clipboard, ClipboardFormat, KeyEvent, MultiClipboards},
    ResultType,
};
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    io::Read,
    path::PathBuf,
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
    static ref CLIENT_CLIPBOARD: Mutex<ClientClipboardState> = Default::default();
    static ref CLIENT_RECEIVED_CLIPBOARDS: Mutex<HashMap<SessionID, VecDeque<MultiClipboards>>> = Default::default();
    static ref CLIENT_CLIPBOARD_FILE_ROOTS: Mutex<HashMap<SessionID, PathBuf>> = Default::default();
    static ref CLIENT_CLIPBOARD_CONN_IDS: Mutex<HashMap<String, i32>> = Default::default();
    static ref HOST_INPUT_EVENTS: Mutex<VecDeque<HostInputEvent>> = Default::default();
    static ref HOST_POINTER_POSITION: Mutex<(i32, i32)> = Default::default();
    static ref HOST_AUDIO: Mutex<VecDeque<Vec<u8>>> = Default::default();
    static ref HOST_RECEIVED_CLIPBOARD: Mutex<Option<MultiClipboards>> = Default::default();
    static ref HOST_SERVER: Mutex<Option<crate::server::ServerPtr>> = Default::default();
}

static HOST_THREAD_STARTED: AtomicBool = AtomicBool::new(false);
static HOST_ENABLED: AtomicBool = AtomicBool::new(false);
const HOST_INPUT_EVENTS_CAPACITY: usize = 256;
const HOST_AUDIO_FRAMES_CAPACITY: usize = 16;

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HostInputEvent {
    Pointer {
        kind: String,
        mask: i32,
        x: i32,
        y: i32,
    },
    Key(HostKeyEvent),
}

impl HostInputEvent {
    fn is_pointer_move(&self) -> bool {
        matches!(
            self,
            Self::Pointer { kind, mask, .. }
                if (kind == "mouse" && *mask & crate::common::input::MOUSE_TYPE_MASK
                    == crate::common::input::MOUSE_TYPE_MOVE)
                    || (kind == "touch" && *mask == 5)
        )
    }

    fn same_pointer_stream(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (
                Self::Pointer { kind: first, .. },
                Self::Pointer { kind: second, .. }
            ) if first == second
        )
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostKeyEvent {
    pub mode: i32,
    pub mode_name: String,
    pub down: bool,
    pub press: bool,
    pub union_kind: String,
    pub control_key: Option<i32>,
    pub control_key_name: Option<String>,
    pub chr: Option<u32>,
    pub unicode: Option<u32>,
    pub seq: Option<String>,
    pub modifiers: Vec<i32>,
    pub modifier_names: Vec<String>,
}

impl HostKeyEvent {
    fn from_proto(event: &KeyEvent) -> Self {
        let mode = event.mode.value();
        let mode_name = event
            .mode
            .enum_value()
            .map(|value| format!("{value:?}"))
            .unwrap_or_else(|value| format!("Unknown({value})"));
        let (union_kind, control_key, control_key_name, chr, unicode, seq) =
            match event.union.as_ref() {
                Some(key_event::Union::ControlKey(value)) => (
                    "controlKey".to_owned(),
                    Some(value.value()),
                    Some(
                        value
                            .enum_value()
                            .map(|value| format!("{value:?}"))
                            .unwrap_or_else(|value| format!("Unknown({value})")),
                    ),
                    None,
                    None,
                    None,
                ),
                Some(key_event::Union::Chr(value)) => {
                    ("chr".to_owned(), None, None, Some(*value), None, None)
                }
                Some(key_event::Union::Unicode(value)) => {
                    ("unicode".to_owned(), None, None, None, Some(*value), None)
                }
                Some(key_event::Union::Seq(value)) => (
                    "seq".to_owned(),
                    None,
                    None,
                    None,
                    None,
                    Some(value.clone()),
                ),
                None => ("none".to_owned(), None, None, None, None, None),
                Some(_) => ("unknown".to_owned(), None, None, None, None, None),
            };
        let modifiers = event.modifiers.iter().map(|value| value.value()).collect();
        let modifier_names = event
            .modifiers
            .iter()
            .map(|value| {
                value
                    .enum_value()
                    .map(|value| format!("{value:?}"))
                    .unwrap_or_else(|value| format!("Unknown({value})"))
            })
            .collect();
        Self {
            mode,
            mode_name,
            down: event.down,
            press: event.press,
            union_kind,
            control_key,
            control_key_name,
            chr,
            unicode,
            seq,
            modifiers,
            modifier_names,
        }
    }
}

pub fn push_host_screen_frame_rgba(rgba: &[u8], width: usize, height: usize) -> bool {
    if !configure_host_screen(width, height) {
        return false;
    }
    scrap::ohos::push_screen_frame_rgba(rgba, width, height)
}

/// Configure controlled-host display geometry before host startup or capture consent.
/// Returns false for invalid geometry.
pub fn configure_host_screen(width: usize, height: usize) -> bool {
    let Some(changed) = scrap::ohos::configure_screen_size(width, height) else {
        return false;
    };
    if changed {
        crate::server::video_service::refresh();
        let server = HOST_SERVER.lock().unwrap().clone();
        if let Some(server) = server {
            server.read().unwrap().set_video_service_opt(
                None,
                crate::server::video_service::OPTION_REFRESH,
                "1",
            );
        }
    }
    true
}

pub(crate) fn register_host_server(server: crate::server::ServerPtr) {
    *HOST_SERVER.lock().unwrap() = Some(server);
}

pub fn host_screen_size() -> (usize, usize) {
    scrap::ohos::screen_size()
}

fn normalize_host_pointer(
    kind: &str,
    mask: i32,
    x: i32,
    y: i32,
    position: &mut (i32, i32),
) -> (i32, i32) {
    if kind != "mouse" {
        return (x, y);
    }
    let event_type = mask & crate::common::input::MOUSE_TYPE_MASK;
    match event_type {
        crate::common::input::MOUSE_TYPE_MOVE => {
            *position = (x, y);
            (x, y)
        }
        crate::common::input::MOUSE_TYPE_DOWN | crate::common::input::MOUSE_TYPE_UP => {
            // RustDesk's button packets intentionally carry no pointer coordinates;
            // their x/y fields are zero. Button injection must use the most recent
            // absolute move position instead of moving the host cursor to (0, 0).
            *position
        }
        _ => (x, y),
    }
}

pub(crate) fn queue_host_pointer(kind: &str, mask: i32, x: i32, y: i32) {
    let (x, y) =
        normalize_host_pointer(kind, mask, x, y, &mut HOST_POINTER_POSITION.lock().unwrap());
    push_host_input_event(HostInputEvent::Pointer {
        kind: kind.to_owned(),
        mask,
        x,
        y,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn button_packets_reuse_last_mouse_position() {
        let mut position = (0, 0);
        assert_eq!(
            normalize_host_pointer("mouse", 0, 960, 640, &mut position),
            (960, 640)
        );
        assert_eq!(
            normalize_host_pointer(
                "mouse",
                crate::common::input::MOUSE_BUTTON_RIGHT << 3
                    | crate::common::input::MOUSE_TYPE_DOWN,
                0,
                0,
                &mut position,
            ),
            (960, 640)
        );
        assert_eq!(
            normalize_host_pointer(
                "mouse",
                crate::common::input::MOUSE_BUTTON_RIGHT << 3 | crate::common::input::MOUSE_TYPE_UP,
                0,
                0,
                &mut position,
            ),
            (960, 640)
        );
    }
}

pub(crate) fn queue_host_key(event: &KeyEvent) {
    push_host_input_event(HostInputEvent::Key(HostKeyEvent::from_proto(event)));
}

fn push_host_input_event(event: HostInputEvent) {
    let mut events = HOST_INPUT_EVENTS.lock().unwrap();
    if event.is_pointer_move() {
        if let Some(last) = events.back_mut() {
            if last.is_pointer_move() && last.same_pointer_stream(&event) {
                *last = event;
                return;
            }
        }
    }
    if events.len() >= HOST_INPUT_EVENTS_CAPACITY {
        if let Some(position) = events.iter().position(HostInputEvent::is_pointer_move) {
            events.remove(position);
        } else if event.is_pointer_move() {
            return;
        } else {
            events.pop_front();
        }
    }
    events.push_back(event);
}

pub fn poll_host_input_event() -> Option<HostInputEvent> {
    HOST_INPUT_EVENTS.lock().unwrap().pop_front()
}

pub fn poll_host_input_event_json() -> Option<String> {
    poll_host_input_event().and_then(|event| serde_json::to_string(&event).ok())
}

pub fn push_host_audio_f32_stereo(data: &[u8]) {
    if data.is_empty() || data.len() % std::mem::size_of::<f32>() != 0 {
        return;
    }
    let mut frames = HOST_AUDIO.lock().unwrap();
    if frames.len() >= HOST_AUDIO_FRAMES_CAPACITY {
        frames.pop_front();
    }
    frames.push_back(data.to_vec());
}

pub(crate) fn take_host_audio_f32_stereo() -> Option<Vec<u8>> {
    HOST_AUDIO.lock().unwrap().pop_front()
}

pub fn start_host() -> bool {
    enable_host(false)
}

fn enable_host(force_restart: bool) -> bool {
    hbb_common::config::Config::set_option("stop-service".to_owned(), String::new());
    hbb_common::config::Config::set_option("direct-server".to_owned(), "Y".to_owned());
    let was_enabled = HOST_ENABLED.swap(true, Ordering::SeqCst);
    crate::common::set_server_running(true);

    if HOST_THREAD_STARTED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        let spawn_result = std::thread::Builder::new()
            .name("ohos-host".to_owned())
            .spawn(|| {
                crate::start_server(true);
                HOST_THREAD_STARTED.store(false, Ordering::SeqCst);
                HOST_ENABLED.store(false, Ordering::SeqCst);
                crate::common::set_server_running(false);
            });
        if let Err(err) = spawn_result {
            hbb_common::log::error!("Failed to start OHOS host thread: {err}");
            hbb_common::config::Config::set_option("stop-service".to_owned(), "Y".to_owned());
            HOST_THREAD_STARTED.store(false, Ordering::SeqCst);
            HOST_ENABLED.store(false, Ordering::SeqCst);
            crate::common::set_server_running(false);
            return false;
        }
    } else if force_restart || !was_enabled {
        crate::RendezvousMediator::restart();
    }
    true
}

pub fn restart_host() {
    let _ = enable_host(true);
}

pub fn stop_host() {
    hbb_common::config::Config::set_option("stop-service".to_owned(), "Y".to_owned());
    let was_enabled = HOST_ENABLED.swap(false, Ordering::SeqCst);
    crate::common::set_server_running(false);
    crate::ui_cm_interface::clear_host_clients();
    HOST_INPUT_EVENTS.lock().unwrap().clear();
    *HOST_POINTER_POSITION.lock().unwrap() = (0, 0);
    HOST_AUDIO.lock().unwrap().clear();
    scrap::ohos::reset_screen_state();
    HOST_RECEIVED_CLIPBOARD.lock().unwrap().take();
    if was_enabled && HOST_THREAD_STARTED.load(Ordering::SeqCst) {
        crate::RendezvousMediator::restart();
    }
}

pub fn reset_host_screen() {
    scrap::ohos::reset_screen_state();
}

pub fn host_is_started() -> bool {
    HOST_ENABLED.load(Ordering::SeqCst)
}

pub fn host_clients_state() -> String {
    crate::ui_cm_interface::get_clients_state()
}
pub fn host_client_count() -> usize {
    crate::ui_cm_interface::get_clients_length()
}
pub fn host_authorize_client(id: i32) -> bool {
    crate::ui_cm_interface::authorize(id)
}
pub fn host_close_client(id: i32) -> bool {
    crate::ui_cm_interface::reject_pending(id)
}

pub(crate) fn receive_host_clipboards(mut clipboards: MultiClipboards) {
    for clipboard in &mut clipboards.clipboards {
        if clipboard.compress {
            clipboard.content = hbb_common::compress::decompress(&clipboard.content).into();
            clipboard.compress = false;
        }
    }
    *HOST_RECEIVED_CLIPBOARD.lock().unwrap() = Some(clipboards);
}

pub fn take_host_received_clipboards() -> Option<MultiClipboards> {
    HOST_RECEIVED_CLIPBOARD.lock().unwrap().take()
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

pub fn update_client_clipboards(clipboards: MultiClipboards) -> bool {
    let mut state = CLIENT_CLIPBOARD.lock().unwrap();
    if !state.enabled {
        return false;
    }
    state.clipboards = Some(clipboards);
    true
}

pub(crate) fn receive_client_clipboards(session_id: &SessionID, mut clipboards: MultiClipboards) {
    const MAX_CLIENT_CLIPBOARD_BYTES: usize = 64 * 1024 * 1024;
    const MAX_CLIENT_CLIPBOARD_FORMATS: usize = 16;
    let mut aggregate_size = 0usize;
    clipboards.clipboards.truncate(MAX_CLIENT_CLIPBOARD_FORMATS);
    clipboards.clipboards.retain_mut(|clipboard| {
        if clipboard.compress {
            let Ok(content) = decompress_clipboard_content(
                &clipboard.content,
                MAX_CLIENT_CLIPBOARD_BYTES,
            ) else {
                return false;
            };
            clipboard.content = content.into();
            clipboard.compress = false;
        }
        let Some(next_size) = aggregate_size.checked_add(clipboard.content.len()) else {
            return false;
        };
        if next_size > MAX_CLIENT_CLIPBOARD_BYTES {
            return false;
        }
        aggregate_size = next_size;
        true
    });
    if clipboards.clipboards.is_empty() {
        return;
    }
    let mut queues = CLIENT_RECEIVED_CLIPBOARDS.lock().unwrap();
    let queue = queues.entry(*session_id).or_default();
    if queue.len() >= 4 {
        queue.pop_front();
    }
    queue.push_back(clipboards);
}

fn decompress_clipboard_content(data: &[u8], limit: usize) -> Result<Vec<u8>, String> {
    let decoder = zstd::Decoder::new(data).map_err(|error| error.to_string())?;
    let mut content = Vec::new();
    decoder
        .take(limit.saturating_add(1) as u64)
        .read_to_end(&mut content)
        .map_err(|error| error.to_string())?;
    if content.len() > limit {
        return Err("decompressed clipboard content exceeds the size limit".to_owned());
    }
    Ok(content)
}

pub fn take_client_received_clipboards(session_id: &SessionID) -> Option<MultiClipboards> {
    let mut queues = CLIENT_RECEIVED_CLIPBOARDS.lock().unwrap();
    let queue = queues.get_mut(session_id)?;
    let clipboards = queue.pop_front();
    if queue.is_empty() {
        queues.remove(session_id);
    }
    clipboards
}

#[cfg(feature = "cliprdr-file-service")]
pub fn set_client_clipboard_file_root(session_id: &SessionID, root: String) -> Result<(), String> {
    let root = PathBuf::from(root);
    let has_unsafe_component = root.components().any(|component| {
        matches!(
            component,
            std::path::Component::CurDir | std::path::Component::ParentDir
        )
    });
    let has_dedicated_parent = root
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
        == Some("rustdesk-clipboard-in");
    if !root.is_absolute()
        || has_unsafe_component
        || root.file_name().is_none()
        || !has_dedicated_parent
    {
        return Err("clipboard file root must be a dedicated session directory".to_owned());
    }
    std::fs::create_dir_all(&root)
        .map_err(|error| format!("failed to create clipboard file root: {error}"))?;
    CLIENT_CLIPBOARD_FILE_ROOTS
        .lock()
        .unwrap()
        .insert(*session_id, root);
    Ok(())
}

#[cfg(feature = "cliprdr-file-service")]
pub(crate) fn get_client_clipboard_file_root(session_id: &SessionID) -> Option<PathBuf> {
    CLIENT_CLIPBOARD_FILE_ROOTS
        .lock()
        .unwrap()
        .get(session_id)
        .cloned()
}

#[cfg(feature = "cliprdr-file-service")]
pub(crate) fn set_client_clipboard_conn_id(core_session_id: String, conn_id: i32) {
    CLIENT_CLIPBOARD_CONN_IDS
        .lock()
        .unwrap()
        .insert(core_session_id, conn_id);
}

#[cfg(feature = "cliprdr-file-service")]
pub(crate) fn clear_client_clipboard_conn_id(core_session_id: &str, conn_id: i32) {
    let mut conn_ids = CLIENT_CLIPBOARD_CONN_IDS.lock().unwrap();
    if conn_ids.get(core_session_id) == Some(&conn_id) {
        conn_ids.remove(core_session_id);
    }
    clipboard::platform::unix::serv_files::clear_conn_files(conn_id);
}

#[cfg(feature = "cliprdr-file-service")]
pub fn update_client_file_clipboard(
    session_id: SessionID,
    paths: Vec<String>,
) -> Result<(), String> {
    if paths.is_empty() {
        return Err("file clipboard is empty".to_owned());
    }
    let core_session_id = crate::flutter::session_core_connection_id(session_id)
        .ok_or_else(|| "active session clipboard connection is not ready".to_owned())?;
    let conn_id = CLIENT_CLIPBOARD_CONN_IDS
        .lock()
        .unwrap()
        .get(&core_session_id)
        .copied()
        .ok_or_else(|| "active session clipboard connection is not ready".to_owned())?;
    let snapshot = clipboard::platform::unix::serv_files::prepare_files_for_conn(&paths)
        .map_err(|e| format!("failed to stage file clipboard: {e}"))?;
    let msg =
        crate::clipboard_file::clip_2_msg(crate::clipboard_file::unix_file_clip::get_format_list());
    if crate::flutter::session_send_file_clipboard_snapshot(session_id, conn_id, snapshot, msg) {
        Ok(())
    } else {
        Err("active session is not ready for file clipboard".to_owned())
    }
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
    CLIENT_RECEIVED_CLIPBOARDS
        .lock()
        .unwrap()
        .remove(session_id);
    CLIENT_CLIPBOARD_FILE_ROOTS
        .lock()
        .unwrap()
        .remove(session_id);
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
