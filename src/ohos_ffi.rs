// OpenHarmony controller (主控端) FFI.
//
// The HarmonyOS app (`harmonyos/`) loads `librustdesk.so`, dlsym's the
// `#[no_mangle]` functions below and wraps them in a NAPI module. The event
// JSON pushed through `event_cb` follows the same schema as the Flutter UI
// (`FlutterHandler::push_event`), so `docs/harmonyos-migration/ohos-controller-ffi.md`
// is the single contract shared by both sides.
//
// This module exists only for the `aarch64-unknown-linux-ohos` controller
// build; it never compiles on the existing desktop / mobile targets.

use crate::client::file_trait::FileManager;
use crate::client::{Data, Interface, QualityStatus};
use crate::ui_session_interface::{io_loop, InvokeUiSession, Session};
use base::message_proto::*;
use hbb_common::{config::Config, lazy_static, log, rendezvous_proto::ConnType, SessionID};
use scrap::ImageRgb;
use serde::Serialize;
use serde_json::json;
use std::{
    collections::HashMap,
    ffi::{c_char, c_int, CStr, CString},
    sync::{Arc, RwLock},
};
use uuid::Uuid;

type EventCallback = extern "C" fn(json: *const c_char);
type RgbaCallback = unsafe extern "C" fn(
    display: c_int,
    data: *const u8,
    len: c_int,
    width: c_int,
    height: c_int,
    stride: c_int,
);
type ClipboardCallback = extern "C" fn(text: *const c_char);

#[derive(Clone, Copy)]
struct Callbacks {
    event: Option<EventCallback>,
    rgba: Option<RgbaCallback>,
    clipboard: Option<ClipboardCallback>,
}

unsafe impl Send for Callbacks {}
unsafe impl Sync for Callbacks {}

static CALLBACKS: RwLock<Option<Callbacks>> = RwLock::new(None);

fn with_event_cb(f: impl FnOnce(EventCallback)) {
    if let Some(cb) = CALLBACKS.read().unwrap().as_ref() {
        if let Some(event) = cb.event {
            f(event);
        }
    }
}

fn push_event<V>(name: &str, event: &[(&str, V)])
where
    V: Sized + Serialize + Clone,
{
    let mut h: HashMap<&str, serde_json::Value> =
        event.iter().map(|(k, v)| (*k, json!(*v))).collect();
    h.insert("name", json!(name));
    let out = serde_json::ser::to_string(&h).unwrap_or("".to_owned());
    with_event_cb(|cb| {
        // The callback must copy; the string is freed when this returns.
        if let Ok(c) = CString::new(out) {
            cb(c.as_ptr());
        }
    });
}

#[derive(Clone)]
pub struct OhosHandler {
    session_id: SessionID,
}

impl Default for OhosHandler {
    fn default() -> Self {
        Self {
            session_id: Default::default(),
        }
    }
}

impl OhosHandler {
    fn with_session_id(&self, f: impl FnOnce(SessionID)) {
        f(self.session_id);
    }
}

impl InvokeUiSession for OhosHandler {
    // Colors arrive validated and uncompressed from the receive loop; base64
    // keeps the JSON event single-line for the ArkTS JSON parser. Order with
    // the cursor_id events around each shape matters and is preserved by the
    // synchronous event callback.
    fn set_cursor_data(&self, cd: CursorData) {
        use hbb_common::base64::Engine;
        let colors = hbb_common::base64::engine::general_purpose::STANDARD.encode(&cd.colors);
        push_event(
            "cursor_data",
            &[
                ("id", &cd.id.to_string()),
                ("hotx", &cd.hotx.to_string()),
                ("hoty", &cd.hoty.to_string()),
                ("width", &cd.width.to_string()),
                ("height", &cd.height.to_string()),
                ("colors", &colors),
            ],
        );
    }

    fn set_cursor_id(&self, id: String) {
        push_event("cursor_id", &[("id", &id)]);
    }

    fn set_cursor_position(&self, cp: CursorPosition) {
        push_event(
            "cursor_position",
            &[("x", &cp.x.to_string()), ("y", &cp.y.to_string())],
        );
    }

    fn set_display(&self, _x: i32, _y: i32, _w: i32, _h: i32, _cursor_embedded: bool, _scale: f64) {}

    fn switch_display(&self, display: &SwitchDisplay) {
        let resolutions = serialize_resolutions(&display.resolutions.resolutions);
        push_event(
            "switch_display",
            &[
                ("display", &display.display.to_string()),
                ("x", &display.x.to_string()),
                ("y", &display.y.to_string()),
                ("width", &display.width.to_string()),
                ("height", &display.height.to_string()),
                ("cursor_embedded", &display.cursor_embedded.to_string()),
                ("resolutions", &resolutions),
            ],
        );
    }

    fn set_peer_info(&self, pi: &PeerInfo) {
        let displays = make_displays_msg(&pi.displays);
        let mut features: HashMap<&str, bool> = Default::default();
        for ref f in pi.features.iter() {
            features.insert("privacy_mode", f.privacy_mode);
        }
        if hbb_common::get_version_number(&pi.version) < hbb_common::get_version_number("1.2.0") {
            features.insert("privacy_mode", false);
        }
        let features = serde_json::ser::to_string(&features).unwrap_or("".to_owned());
        let resolutions = serialize_resolutions(&pi.resolutions.resolutions);
        push_event(
            "peer_info",
            &[
                ("username", &pi.username),
                ("hostname", &pi.hostname),
                ("platform", &pi.platform),
                ("sas_enabled", &pi.sas_enabled.to_string()),
                ("displays", &displays),
                ("version", &pi.version),
                ("features", &features),
                ("current_display", &pi.current_display.to_string()),
                ("resolutions", &resolutions),
            ],
        );
    }

    fn set_displays(&self, displays: &Vec<DisplayInfo>) {
        push_event("sync_peer_info", &[("displays", &make_displays_msg(displays))]);
    }

    fn set_platform_additions(&self, data: &str) {
        push_event("sync_platform_additions", &[("platform_additions", &data)]);
    }

    fn on_connected(&self, _conn_type: ConnType) {}

    fn update_privacy_mode(&self) {
        push_event::<&str>("update_privacy_mode", &[]);
    }

    fn set_permission(&self, name: &str, value: bool) {
        push_event("permission", &[(name, &value.to_string())]);
    }

    fn close_success(&self) {}

    fn new_message(&self, msg: String) {
        push_event("chat_client_mode", &[("text", &msg)]);
    }

    fn update_transfer_list(&self) {}

    fn update_quality_status(&self, status: QualityStatus) {
        const NULL: String = String::new();
        push_event(
            "update_quality_status",
            &[
                ("speed", &status.speed.map_or(NULL, |it| it)),
                (
                    "fps",
                    &serde_json::ser::to_string(&status.fps).unwrap_or(NULL.to_owned()),
                ),
                ("delay", &status.delay.map_or(NULL, |it| it.to_string())),
                (
                    "target_bitrate",
                    &status.target_bitrate.map_or(NULL, |it| it.to_string()),
                ),
                (
                    "codec_format",
                    &status.codec_format.map_or(NULL, |it| it.to_string()),
                ),
                ("chroma", &status.chroma.map_or(NULL, |it| it.to_string())),
            ],
        );
    }

    fn set_connection_type(&self, is_secured: bool, direct: bool, stream_type: &str) {
        push_event(
            "connection_ready",
            &[
                ("secure", &is_secured.to_string()),
                ("direct", &direct.to_string()),
                ("stream_type", &stream_type.to_string()),
            ],
        );
    }

    fn set_fingerprint(&self, fingerprint: String) {
        push_event("fingerprint", &[("fingerprint", &fingerprint)]);
    }

    fn job_error(&self, id: i32, err: String, file_num: i32) {
        push_event(
            "job_error",
            &[
                ("id", &id.to_string()),
                ("err", &err),
                ("file_num", &file_num.to_string()),
            ],
        );
    }

    fn job_done(&self, id: i32, file_num: i32) {
        push_event(
            "job_done",
            &[("id", &id.to_string()), ("file_num", &file_num.to_string())],
        );
    }

    fn clear_all_jobs(&self) {}

    fn load_last_job(&self, _cnt: i32, job_json: &str, _auto_start: bool) {
        push_event("load_last_job", &[("value", job_json)]);
    }

    fn update_folder_files(
        &self,
        id: i32,
        entries: &Vec<FileEntry>,
        path: String,
        _is_local: bool,
        only_count: bool,
    ) {
        if only_count {
            push_event(
                "update_folder_files",
                &[("info", &make_fd_flutter(id, entries, only_count))],
            );
        } else {
            push_event(
                "file_dir",
                &[
                    ("is_local", "false"),
                    ("value", &crate::common::make_fd_to_json(id, path, entries)),
                ],
            );
        }
    }

    fn confirm_delete_files(&self, _id: i32, _i: i32, _name: String) {}

    fn override_file_confirm(
        &self,
        id: i32,
        file_num: i32,
        to: String,
        is_upload: bool,
        is_identical: bool,
    ) {
        push_event(
            "override_file_confirm",
            &[
                ("id", &id.to_string()),
                ("file_num", &file_num.to_string()),
                ("read_path", &to),
                ("is_upload", &is_upload.to_string()),
                ("is_identical", &is_identical.to_string()),
            ],
        );
    }

    fn update_block_input_state(&self, _on: bool) {}

    fn job_progress(&self, id: i32, file_num: i32, speed: f64, finished_size: f64) {
        push_event(
            "job_progress",
            &[
                ("id", &id.to_string()),
                ("file_num", &file_num.to_string()),
                ("speed", &speed.to_string()),
                ("finished_size", &finished_size.to_string()),
            ],
        );
    }

    fn adapt_size(&self) {}

    fn on_rgba(&self, display: usize, rgba: &mut ImageRgb) {
        if let Some(rgba_cb) = CALLBACKS.read().unwrap().as_ref().and_then(|c| c.rgba) {
            // `align` is the byte alignment, not the stride; the real row
            // stride is what libyuv filled: raw.len() / h.
            let stride = if rgba.h > 0 {
                rgba.raw.len() / rgba.h
            } else {
                0
            };
            // The callback copies synchronously; the buffer is reused afterwards.
            unsafe {
                rgba_cb(
                    display as _,
                    rgba.raw.as_ptr(),
                    rgba.raw.len() as _,
                    rgba.w as _,
                    rgba.h as _,
                    stride as _,
                );
            }
        }
    }

    fn msgbox(&self, msgtype: &str, title: &str, text: &str, link: &str, retry: bool) {
        let has_retry = if retry { "true" } else { "" };
        push_event(
            "msgbox",
            &[
                ("type", msgtype),
                ("title", title),
                ("text", text),
                ("link", link),
                ("hasRetry", has_retry),
            ],
        );
    }

    fn clipboard(&self, content: String) {
        if let Some(clipboard_cb) = CALLBACKS
            .read()
            .unwrap()
            .as_ref()
            .and_then(|c| c.clipboard)
        {
            if let Ok(c) = CString::new(content) {
                clipboard_cb(c.as_ptr());
            }
        }
    }

    fn cancel_msgbox(&self, tag: &str) {
        push_event("cancel_msgbox", &[("tag", tag)]);
    }

    fn switch_back(&self, _id: &str) {}

    fn portable_service_running(&self, _running: bool) {}

    fn on_voice_call_started(&self) {}
    fn on_voice_call_closed(&self, _reason: &str) {}
    fn on_voice_call_waiting(&self) {}
    fn on_voice_call_incoming(&self) {}

    fn get_rgba(&self, _display: usize) -> *const u8 {
        std::ptr::null()
    }

    fn next_rgba(&self, _display: usize) {}

    fn set_multiple_windows_session(&self, _sessions: Vec<WindowsSession>) {}

    fn set_current_display(&self, _disp_idx: i32) {}

    fn update_record_status(&self, _start: bool) {}

    fn printer_request(&self, _id: i32, _path: String) {}

    fn handle_screenshot_resp(&self, _sid: String, _msg: String) {}

    fn handle_terminal_response(&self, _response: TerminalResponse) {}
}

fn make_displays_msg(displays: &Vec<DisplayInfo>) -> String {
    #[derive(Debug, serde::Serialize)]
    struct DisplayInfoSerde {
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        cursor_embedded: bool,
    }
    let mut displays_serde = Vec::new();
    for ref d in displays.iter() {
        displays_serde.push(DisplayInfoSerde {
            x: d.x,
            y: d.y,
            width: d.width,
            height: d.height,
            cursor_embedded: d.cursor_embedded,
        });
    }
    serde_json::ser::to_string(&displays_serde).unwrap_or("".to_owned())
}

fn serialize_resolutions(resolutions: &Vec<Resolution>) -> String {
    #[derive(Debug, serde::Serialize)]
    struct ResolutionSerde {
        width: i32,
        height: i32,
    }
    let v: Vec<ResolutionSerde> = resolutions
        .iter()
        .map(|r| ResolutionSerde {
            width: r.width,
            height: r.height,
        })
        .collect();
    serde_json::ser::to_string(&v).unwrap_or("".to_string())
}

fn make_fd_flutter(id: i32, entries: &Vec<FileEntry>, only_count: bool) -> String {
    let mut m = serde_json::Map::new();
    m.insert("id".into(), json!(id));
    let mut a = vec![];
    let mut n: u64 = 0;
    for entry in entries {
        n += entry.size;
        if only_count {
            continue;
        }
        let mut e = serde_json::Map::new();
        e.insert("name".into(), json!(entry.name.to_owned()));
        let tmp = entry.entry_type.value();
        e.insert("type".into(), json!(if tmp == 0 { 1 } else { tmp }));
        e.insert("time".into(), json!(entry.modified_time as f64));
        e.insert("size".into(), json!(entry.size as f64));
        a.push(e);
    }
    if only_count {
        m.insert("num_entries".into(), json!(entries.len() as i32));
    } else {
        m.insert("entries".into(), json!(a));
    }
    m.insert("total_size".into(), json!(n as f64));
    serde_json::to_string(&m).unwrap_or("".into())
}

// ---------------------------------------------------------------------------
// Session registry
// ---------------------------------------------------------------------------

pub type OhosSession = Arc<Session<OhosHandler>>;

lazy_static::lazy_static! {
    // session id -> session
    static ref SESSIONS: RwLock<HashMap<SessionID, OhosSession>> = Default::default();
}

fn get_session_by_session_id(id: &SessionID) -> Option<OhosSession> {
    SESSIONS.read().unwrap().get(id).cloned()
}

fn remove_session_by_session_id(id: &SessionID) -> Option<OhosSession> {
    SESSIONS.write().unwrap().remove(id)
}

// ---------------------------------------------------------------------------
// C ABI helpers
// ---------------------------------------------------------------------------

fn cstr<'a>(p: *const c_char) -> &'a str {
    if p.is_null() {
        return "";
    }
    unsafe { CStr::from_ptr(p) }.to_str().unwrap_or("")
}

fn new_cstring(s: String) -> *mut c_char {
    match CString::new(s) {
        Ok(c) => c.into_raw(),
        Err(_) => CString::default().into_raw(),
    }
}

fn parse_session_id(p: *const c_char) -> Option<SessionID> {
    Uuid::parse_str(cstr(p)).ok()
}

// ---------------------------------------------------------------------------
// C ABI
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn ohos_version() -> *mut c_char {
    // concat! only takes literals, so build the string at runtime.
    // The caller owns it and must free it with `ohos_free_string`.
    new_cstring(crate::VERSION.to_owned())
}

#[no_mangle]
pub extern "C" fn ohos_free_string(s: *mut c_char) {
    if !s.is_null() {
        drop(unsafe { CString::from_raw(s) });
    }
}

/// Initialize config paths and logging. Call once before everything else.
#[no_mangle]
pub extern "C" fn ohos_init(
    home_dir: *const c_char,
    app_dir: *const c_char,
    custom_client_config: *const c_char,
) -> c_int {
    // hbb_common's APP_HOME_DIR/APP_DIR statics are android/ios-gated; on ohos
    // the non-mobile `Config::get_home()` reads $HOME, so the sandbox home
    // passed by the ArkTS side is set as the HOME environment variable.
    std::env::set_var("HOME", cstr(home_dir));
    let custom = cstr(custom_client_config);
    if custom.is_empty() {
        crate::load_custom_client();
    } else {
        crate::read_custom_client(custom);
    }
    let _logger = hbb_common::init_log(false, "rustdesk-ohos");
    log::info!(
        "ohos_init, home: {}, app_dir: {}",
        cstr(home_dir),
        cstr(app_dir)
    );
    0
}

/// Read a config option (e.g. "custom-rendezvous-server", "key"). The caller
/// owns the returned string and must free it with `ohos_free_string`.
#[no_mangle]
pub extern "C" fn ohos_get_option(key: *const c_char) -> *mut c_char {
    new_cstring(Config::get_option(cstr(key)))
}

/// Write a config option; persists to RustDesk2.toml. An empty value removes
/// the key (reset to default). Takes effect on the next session connection.
#[no_mangle]
pub extern "C" fn ohos_set_option(key: *const c_char, value: *const c_char) -> c_int {
    Config::set_option(cstr(key).to_owned(), cstr(value).to_owned());
    0
}

#[no_mangle]
pub extern "C" fn ohos_set_callbacks(
    event_cb: EventCallback,
    rgba_cb: RgbaCallback,
    clipboard_cb: ClipboardCallback,
) -> c_int {
    *CALLBACKS.write().unwrap() = Some(Callbacks {
        event: Some(event_cb),
        rgba: Some(rgba_cb),
        clipboard: Some(clipboard_cb),
    });
    0
}

#[no_mangle]
pub extern "C" fn ohos_session_new(
    peer_id: *const c_char,
    is_file_transfer: c_int,
    password: *const c_char,
    force_relay: c_int,
) -> *mut c_char {
    let peer_id = cstr(peer_id).to_owned();
    if peer_id.is_empty() {
        return new_cstring("".to_owned());
    }
    let session_id = Uuid::new_v4();
    let conn_type = if is_file_transfer != 0 {
        ConnType::FILE_TRANSFER
    } else {
        ConnType::DEFAULT_CONN
    };
    let mut session: Session<OhosHandler> = Session {
        password: cstr(password).to_owned(),
        server_keyboard_enabled: Arc::new(RwLock::new(true)),
        server_file_transfer_enabled: Arc::new(RwLock::new(true)),
        server_clipboard_enabled: Arc::new(RwLock::new(true)),
        reconnect_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        ..Default::default()
    };
    session.ui_handler.session_id = session_id;
    session.lc.write().unwrap().initialize(
        peer_id.clone(),
        conn_type,
        None,
        force_relay != 0,
        None,
        None,
        None,
    );
    let session: OhosSession = Arc::new(session);
    SESSIONS.write().unwrap().insert(session_id, session);
    new_cstring(session_id.to_string())
}

#[no_mangle]
pub extern "C" fn ohos_session_start(session_id: *const c_char) -> c_int {
    let Some(session_id) = parse_session_id(session_id) else {
        return -1;
    };
    let Some(session) = get_session_by_session_id(&session_id) else {
        return -1;
    };
    let session = (*session).clone();
    std::thread::spawn(move || {
        let round = session.connection_round_state.lock().unwrap().new_round();
        // `io_loop` is `#[tokio::main]`: it builds its own current-thread
        // runtime and blocks this thread until the session ends.
        io_loop(session, round);
    });
    0
}

#[no_mangle]
pub extern "C" fn ohos_session_login(
    session_id: *const c_char,
    password: *const c_char,
    remember: c_int,
) -> c_int {
    let Some(session_id) = parse_session_id(session_id) else {
        return -1;
    };
    if let Some(session) = get_session_by_session_id(&session_id) {
        // The OS-username/password prompt is a desktop flow; empty on M1.
        session.login("".to_owned(), "".to_owned(), cstr(password).to_owned(), remember != 0);
        return 0;
    }
    -1
}

#[no_mangle]
pub extern "C" fn ohos_session_send2fa(
    session_id: *const c_char,
    code: *const c_char,
    trust_this_device: c_int,
) -> c_int {
    let Some(session_id) = parse_session_id(session_id) else {
        return -1;
    };
    if let Some(session) = get_session_by_session_id(&session_id) {
        session.send2fa(cstr(code).to_owned(), trust_this_device != 0);
        return 0;
    }
    -1
}

#[no_mangle]
pub extern "C" fn ohos_session_reconnect(session_id: *const c_char, force_relay: c_int) -> c_int {
    let Some(session_id) = parse_session_id(session_id) else {
        return -1;
    };
    if let Some(session) = get_session_by_session_id(&session_id) {
        session.reconnect(force_relay != 0);
        return 0;
    }
    -1
}

#[no_mangle]
pub extern "C" fn ohos_session_close(session_id: *const c_char) -> c_int {
    let Some(session_id) = parse_session_id(session_id) else {
        return -1;
    };
    if let Some(session) = remove_session_by_session_id(&session_id) {
        crate::keyboard::release_remote_keys("map");
        session.close();
        return 0;
    }
    -1
}

#[no_mangle]
pub extern "C" fn ohos_send_mouse(
    session_id: *const c_char,
    mask: c_int,
    x: c_int,
    y: c_int,
    alt: c_int,
    ctrl: c_int,
    shift: c_int,
    command: c_int,
) -> c_int {
    let Some(session_id) = parse_session_id(session_id) else {
        return -1;
    };
    if let Some(session) = get_session_by_session_id(&session_id) {
        session.send_mouse(mask, x, y, alt != 0, ctrl != 0, shift != 0, command != 0);
        return 0;
    }
    -1
}

#[no_mangle]
pub extern "C" fn ohos_input_key(
    session_id: *const c_char,
    name: *const c_char,
    down: c_int,
    press: c_int,
    alt: c_int,
    ctrl: c_int,
    shift: c_int,
    command: c_int,
) -> c_int {
    let Some(session_id) = parse_session_id(session_id) else {
        return -1;
    };
    if let Some(session) = get_session_by_session_id(&session_id) {
        session.input_key(
            cstr(name),
            down != 0,
            press != 0,
            alt != 0,
            ctrl != 0,
            shift != 0,
            command != 0,
        );
        return 0;
    }
    -1
}

#[no_mangle]
pub extern "C" fn ohos_input_string(session_id: *const c_char, value: *const c_char) -> c_int {
    let Some(session_id) = parse_session_id(session_id) else {
        return -1;
    };
    if let Some(session) = get_session_by_session_id(&session_id) {
        session.input_string(cstr(value));
        return 0;
    }
    -1
}

#[no_mangle]
pub extern "C" fn ohos_session_refresh(session_id: *const c_char) -> c_int {
    let Some(session_id) = parse_session_id(session_id) else {
        return -1;
    };
    if let Some(session) = get_session_by_session_id(&session_id) {
        session.refresh_video(0);
        return 0;
    }
    -1
}

#[no_mangle]
pub extern "C" fn ohos_switch_display(session_id: *const c_char, display: c_int) -> c_int {
    let Some(session_id) = parse_session_id(session_id) else {
        return -1;
    };
    if let Some(session) = get_session_by_session_id(&session_id) {
        session.switch_display(display);
        return 0;
    }
    -1
}

#[no_mangle]
pub extern "C" fn ohos_send_clipboard(session_id: *const c_char, text: *const c_char) -> c_int {
    let Some(session_id) = parse_session_id(session_id) else {
        return -1;
    };
    if let Some(session) = get_session_by_session_id(&session_id) {
        let mut msg = Message::new();
        msg.set_clipboard(Clipboard {
            content: cstr(text).as_bytes().to_vec().into(),
            ..Default::default()
        });
        session.send(Data::Message(msg));
        return 0;
    }
    -1
}

#[no_mangle]
pub extern "C" fn ohos_read_dir(
    session_id: *const c_char,
    path: *const c_char,
    include_hidden: c_int,
    // Always the remote side on the controller; kept for contract symmetry.
    _is_remote: c_int,
) -> c_int {
    let Some(session_id) = parse_session_id(session_id) else {
        return -1;
    };
    if let Some(session) = get_session_by_session_id(&session_id) {
        session.read_remote_dir(cstr(path).to_owned(), include_hidden != 0);
        return 0;
    }
    -1
}

#[no_mangle]
pub extern "C" fn ohos_send_files(
    session_id: *const c_char,
    act_id: c_int,
    path: *const c_char,
    to: *const c_char,
    file_num: c_int,
    include_hidden: c_int,
    is_remote: c_int,
) -> c_int {
    let Some(session_id) = parse_session_id(session_id) else {
        return -1;
    };
    if let Some(session) = get_session_by_session_id(&session_id) {
        session.send_files(
            act_id,
            base::fs::JobType::Generic.into(),
            cstr(path).to_owned(),
            cstr(to).to_owned(),
            file_num,
            include_hidden != 0,
            is_remote != 0,
        );
        return 0;
    }
    -1
}

#[no_mangle]
pub extern "C" fn ohos_set_confirm_override_file(
    session_id: *const c_char,
    act_id: c_int,
    file_num: c_int,
    need_override: c_int,
    remember: c_int,
    is_upload: c_int,
) -> c_int {
    let Some(session_id) = parse_session_id(session_id) else {
        return -1;
    };
    if let Some(session) = get_session_by_session_id(&session_id) {
        session.set_confirm_override_file(
            act_id,
            file_num,
            need_override != 0,
            remember != 0,
            is_upload != 0,
        );
        return 0;
    }
    -1
}

#[no_mangle]
pub extern "C" fn ohos_cancel_job(session_id: *const c_char, act_id: c_int) -> c_int {
    let Some(session_id) = parse_session_id(session_id) else {
        return -1;
    };
    if let Some(session) = get_session_by_session_id(&session_id) {
        session.cancel_job(act_id);
        return 0;
    }
    -1
}

#[no_mangle]
pub extern "C" fn ohos_remove_file(
    session_id: *const c_char,
    act_id: c_int,
    path: *const c_char,
    file_num: c_int,
    is_remote: c_int,
) -> c_int {
    let Some(session_id) = parse_session_id(session_id) else {
        return -1;
    };
    if let Some(session) = get_session_by_session_id(&session_id) {
        session.remove_file(act_id, cstr(path).to_owned(), file_num, is_remote != 0);
        return 0;
    }
    -1
}
