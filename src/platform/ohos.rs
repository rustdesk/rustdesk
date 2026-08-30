// Minimal OpenHarmony adapter for native frontend callbacks and mobile clipboard state.
mod input;

use crate::client::{Data, Interface};
use crate::flutter_ffi::{EventToUI, SessionID};
use hbb_common::{
    message_proto::{
        key_event, message, Clipboard, ClipboardFormat, KeyEvent, Message, MultiClipboards,
    },
    ResultType,
};
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    io::Read,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Mutex,
    },
};

pub fn discover_lan_blocking() -> Result<(), String> {
    crate::lan::discover().map_err(|err| err.to_string())
}

pub async fn query_online_states_result(
    ids: Vec<String>,
) -> ResultType<(Vec<String>, Vec<String>)> {
    crate::client::peer_online::query_online_states_result(ids).await
}

pub fn validate_api_server(api_server: &str, use_proxy: bool) -> ResultType<()> {
    let url = format!("{}/api/login-options", api_server.trim_end_matches('/'));
    let response = if use_proxy {
        crate::hbbs_http::create_http_client_with_url(&url)
            .get(&url)
            .timeout(std::time::Duration::from_millis(2_500))
            .send()?
    } else {
        reqwest::blocking::Client::builder()
            .no_proxy()
            .timeout(std::time::Duration::from_millis(2_500))
            .build()?
            .get(&url)
            .send()?
    };
    if !response.status().is_success() {
        hbb_common::bail!(
            "RustDesk API /api/login-options returned HTTP {}",
            response.status().as_u16()
        );
    }
    let _: Vec<String> = response.json()?;
    Ok(())
}

pub type SessionEventCallback = fn(SessionID, EventToUI);
pub use scrap::ohos::DirectRenderTarget;

impl flutter_rust_bridge::support::IntoDart for EventToUI {
    fn into_dart(self) -> flutter_rust_bridge::support::DartAbi {
        use flutter_rust_bridge::rust2dart::IntoIntoDart;

        match self {
            Self::Event(value) => vec![0.into_dart(), value.into_into_dart().into_dart()],
            Self::Rgba(display) => vec![1.into_dart(), display.into_into_dart().into_dart()],
            Self::Texture(display, gpu_texture) => vec![
                2.into_dart(),
                display.into_into_dart().into_dart(),
                gpu_texture.into_into_dart().into_dart(),
            ],
        }
        .into_dart()
    }
}

impl flutter_rust_bridge::support::IntoDartExceptPrimitive for EventToUI {}

impl flutter_rust_bridge::rust2dart::IntoIntoDart<EventToUI> for EventToUI {
    fn into_into_dart(self) -> Self {
        self
    }
}

pub fn get_active_username() -> String {
    "ohos".into()
}

pub fn check_super_user_permission() -> ResultType<bool> {
    Ok(true)
}

#[derive(Default)]
pub struct WakeLock;

impl WakeLock {
    pub fn new(_display: bool, _idle: bool, _sleep: bool) -> Self {
        Self
    }
}

lazy_static::lazy_static! {
    static ref SESSION_EVENT_CALLBACK: Mutex<Option<SessionEventCallback>> = Default::default();
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
static HOST_CLIPBOARD_AVAILABLE: AtomicBool = AtomicBool::new(false);
static HOST_DISPLAY_ID: AtomicU64 = AtomicU64::new(0);
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
    if host_input_authorized() {
        if matches!(
            input::inject_pointer(kind, mask, x, y, HOST_DISPLAY_ID.load(Ordering::Acquire)),
            input::InjectionResult::RetryInFrontend
        ) {
            push_host_input_event(HostInputEvent::Pointer {
                kind: kind.to_owned(),
                mask,
                x,
                y,
            });
        }
    } else {
        push_host_input_event(HostInputEvent::Pointer {
            kind: kind.to_owned(),
            mask,
            x,
            y,
        });
    }
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
    if host_input_authorized() {
        if matches!(
            input::inject_key(event, HOST_DISPLAY_ID.load(Ordering::Acquire)),
            input::InjectionResult::RetryInFrontend
        ) {
            push_host_input_event(HostInputEvent::Key(HostKeyEvent::from_proto(event)));
        }
    } else {
        push_host_input_event(HostInputEvent::Key(HostKeyEvent::from_proto(event)));
    }
}

pub(crate) fn set_host_display_id(display_id: u64) {
    HOST_DISPLAY_ID.store(display_id, Ordering::Release);
}

pub(crate) fn request_host_input_authorization() -> Result<(), String> {
    if !host_input_capable() {
        return Err("HarmonyOS host input is not enabled for this frontend profile".to_owned());
    }
    input::request_authorization()
}

pub(crate) fn host_input_capable() -> bool {
    cfg!(feature = "ohos-flutter")
        && hbb_common::config::LocalConfig::get_option("ohos-host-input-capable") == "Y"
}

pub(crate) fn host_input_authorized() -> bool {
    host_input_capable() && input::is_authorized()
}

pub(crate) fn set_host_clipboard_available(available: bool) {
    HOST_CLIPBOARD_AVAILABLE.store(available, Ordering::Release);
}

pub(crate) fn host_clipboard_available() -> bool {
    cfg!(feature = "ohos-flutter") && HOST_CLIPBOARD_AVAILABLE.load(Ordering::Acquire)
}

pub fn update_host_text_clipboard(content: String) -> bool {
    if !host_clipboard_available() {
        return false;
    }
    update_clipboards(
        false,
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

pub fn take_host_received_text_clipboard() -> Option<String> {
    let clipboards = take_host_received_clipboards()?;
    clipboards.clipboards.into_iter().find_map(|clipboard| {
        (clipboard.format.enum_value() == Ok(ClipboardFormat::Text))
            .then(|| String::from_utf8(clipboard.content.to_vec()).ok())
            .flatten()
    })
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
    HOST_CLIPBOARD_AVAILABLE.store(false, Ordering::Release);
    input::cancel_authorization();
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
    crate::ui_cm_interface::authorize_pending(id)
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

pub fn session_send_clipboards(session_id: SessionID, mut clipboards: MultiClipboards) -> bool {
    let mut msg = Message::new();
    for clipboard in &mut clipboards.clipboards {
        if clipboard.content.len() > 1024 * 1024 {
            clipboard.compress = false;
            continue;
        }
        let compressed = hbb_common::compress::compress(&clipboard.content);
        let use_compressed = compressed.len() < clipboard.content.len();
        if use_compressed {
            clipboard.content = compressed.into();
        }
        clipboard.compress = use_compressed;
    }
    msg.set_multi_clipboards(clipboards);
    session_send_clipboard_msg(session_id, msg, false)
}

pub fn session_core_connection_id(session_id: SessionID) -> Option<String> {
    crate::flutter::sessions::get_session_by_session_id(&session_id)
        .map(|session| session.core_session_id.clone())
}

pub fn session_send_clipboard_msg(session_id: SessionID, msg: Message, is_file: bool) -> bool {
    let Some(session) = crate::flutter::sessions::get_session_by_session_id(&session_id) else {
        return false;
    };
    if !session.is_default() || !session.is_ui_active() {
        return false;
    }
    if is_file {
        #[cfg(any(feature = "unix-file-copy-paste", feature = "cliprdr-file-service"))]
        if crate::is_support_file_copy_paste_num(session.lc.read().unwrap().version)
            && session.is_file_clipboard_required()
        {
            session.send(Data::Message(msg));
            return true;
        }
        return false;
    }
    if !session.is_text_clipboard_required() {
        return false;
    }
    if let Some(message::Union::MultiClipboards(multi_clipboards)) = &msg.union {
        let (version, platform) = session
            .lc
            .read()
            .unwrap()
            .peer_info
            .as_ref()
            .map(|peer| (peer.version.clone(), peer.platform.clone()))
            .unwrap_or_default();
        if let Some(msg_out) = crate::clipboard::get_msg_if_not_support_multi_clip(
            &version,
            &platform,
            multi_clipboards,
        ) {
            session.send(Data::Message(msg_out));
            return true;
        }
    }
    session.send(Data::Message(msg));
    true
}

#[cfg(feature = "cliprdr-file-service")]
pub fn session_send_file_clipboard_snapshot(
    session_id: SessionID,
    conn_id: i32,
    snapshot: clipboard::platform::unix::serv_files::PreparedConnClipFiles,
    msg: Message,
) -> bool {
    let Some(session) = crate::flutter::sessions::get_session_by_session_id(&session_id) else {
        return false;
    };
    if !session.is_default()
        || !session.is_ui_active()
        || !crate::is_support_file_copy_paste_num(session.lc.read().unwrap().version)
        || !session.is_file_clipboard_required()
    {
        return false;
    }
    session.send(Data::ClipboardFileSnapshot((conn_id, snapshot, msg)));
    true
}

#[cfg(feature = "cliprdr-file-service")]
pub fn update_client_file_clipboard(
    session_id: SessionID,
    paths: Vec<String>,
) -> Result<(), String> {
    if paths.is_empty() {
        return Err("file clipboard is empty".to_owned());
    }
    let core_session_id = session_core_connection_id(session_id)
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
    if session_send_file_clipboard_snapshot(session_id, conn_id, snapshot, msg) {
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

#[cfg(feature = "ohos-har")]
pub fn session_start_with_polling_events(session_id: &SessionID, id: &str) -> ResultType<()> {
    let inserted = STARTED_SESSIONS.lock().unwrap().insert(*session_id);
    let already_started = !inserted;
    if let Err(err) =
        crate::flutter::session_start_with_polling_events_(session_id, id, already_started)
    {
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

pub fn register_direct_render_target_lookup(lookup: fn(&str, usize) -> Option<DirectRenderTarget>) {
    scrap::ohos::register_direct_render_target_lookup(lookup);
}
