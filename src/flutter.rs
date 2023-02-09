use crate::{
    client::*,
    flutter_ffi::EventToUI,
    ui_session_interface::{io_loop, InvokeUiSession, Session},
};
use flutter_rust_bridge::{StreamSink, ZeroCopyBuffer};
use hbb_common::{
    bail, config::LocalConfig, get_version_number, message_proto::*, rendezvous_proto::ConnType,
    ResultType,
};
use serde_json::json;
use std::{
    collections::HashMap,
    ffi::CString,
    os::raw::{c_char, c_int},
    sync::{Arc, RwLock},
};

pub(super) const APP_TYPE_MAIN: &str = "main";
pub(super) const APP_TYPE_CM: &str = "cm";
pub(super) const APP_TYPE_DESKTOP_REMOTE: &str = "remote";
pub(super) const APP_TYPE_DESKTOP_FILE_TRANSFER: &str = "file transfer";
pub(super) const APP_TYPE_DESKTOP_PORT_FORWARD: &str = "port forward";

lazy_static::lazy_static! {
    pub static ref CUR_SESSION_ID: RwLock<String> = Default::default();
    pub static ref SESSIONS: RwLock<HashMap<String, Session<FlutterHandler>>> = Default::default();
    pub static ref GLOBAL_EVENT_STREAM: RwLock<HashMap<String, StreamSink<String>>> = Default::default(); // rust to dart event channel
}

/// FFI for rustdesk core's main entry.
/// Return true if the app should continue running with UI(possibly Flutter), false if the app should exit.
#[cfg(not(windows))]
#[no_mangle]
pub extern "C" fn rustdesk_core_main() -> bool {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    return crate::core_main::core_main().is_some();
    #[cfg(any(target_os = "android", target_os = "ios"))]
    false
}

#[cfg(target_os = "macos")]
#[no_mangle]
pub extern "C" fn handle_applicationShouldOpenUntitledFile() {
    hbb_common::log::debug!("icon clicked on finder");
    let x = std::env::args().nth(1).unwrap_or_default();
    if x == "--server" || x == "--cm" {
        crate::platform::macos::check_main_window();
    }
}

#[cfg(windows)]
#[no_mangle]
pub extern "C" fn rustdesk_core_main_args(args_len: *mut c_int) -> *mut *mut c_char {
    unsafe { std::ptr::write(args_len, 0) };
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        if let Some(args) = crate::core_main::core_main() {
            return rust_args_to_c_args(args, args_len);
        }
        return std::ptr::null_mut() as _;
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    return std::ptr::null_mut() as _;
}

// https://gist.github.com/iskakaushik/1c5b8aa75c77479c33c4320913eebef6
#[cfg(windows)]
fn rust_args_to_c_args(args: Vec<String>, outlen: *mut c_int) -> *mut *mut c_char {
    let mut v = vec![];

    // Let's fill a vector with null-terminated strings
    for s in args {
        v.push(CString::new(s).unwrap());
    }

    // Turning each null-terminated string into a pointer.
    // `into_raw` takes ownershop, gives us the pointer and does NOT drop the data.
    let mut out = v.into_iter().map(|s| s.into_raw()).collect::<Vec<_>>();

    // Make sure we're not wasting space.
    out.shrink_to_fit();
    assert!(out.len() == out.capacity());

    // Get the pointer to our vector.
    let len = out.len();
    let ptr = out.as_mut_ptr();
    std::mem::forget(out);

    // Let's write back the length the caller can expect
    unsafe { std::ptr::write(outlen, len as c_int) };

    // Finally return the data
    ptr
}

#[no_mangle]
pub unsafe extern "C" fn free_c_args(ptr: *mut *mut c_char, len: c_int) {
    let len = len as usize;

    // Get back our vector.
    // Previously we shrank to fit, so capacity == length.
    let v = Vec::from_raw_parts(ptr, len, len);

    // Now drop one string at a time.
    for elem in v {
        let s = CString::from_raw(elem);
        std::mem::drop(s);
    }

    // Afterwards the vector will be dropped and thus freed.
}

#[derive(Default, Clone)]
pub struct FlutterHandler {
    pub event_stream: Arc<RwLock<Option<StreamSink<EventToUI>>>>,
}

impl FlutterHandler {
    /// Push an event to the event queue.
    /// An event is stored as json in the event queue.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the event.
    /// * `event` - Fields of the event content.
    fn push_event(&self, name: &str, event: Vec<(&str, &str)>) {
        let mut h: HashMap<&str, &str> = event.iter().cloned().collect();
        assert!(h.get("name").is_none());
        h.insert("name", name);
        let out = serde_json::ser::to_string(&h).unwrap_or("".to_owned());
        if let Some(stream) = &*self.event_stream.read().unwrap() {
            stream.add(EventToUI::Event(out));
        }
    }
}

impl InvokeUiSession for FlutterHandler {
    fn set_cursor_data(&self, cd: CursorData) {
        let colors = hbb_common::compress::decompress(&cd.colors);
        self.push_event(
            "cursor_data",
            vec![
                ("id", &cd.id.to_string()),
                ("hotx", &cd.hotx.to_string()),
                ("hoty", &cd.hoty.to_string()),
                ("width", &cd.width.to_string()),
                ("height", &cd.height.to_string()),
                (
                    "colors",
                    &serde_json::ser::to_string(&colors).unwrap_or("".to_owned()),
                ),
            ],
        );
    }

    fn set_cursor_id(&self, id: String) {
        self.push_event("cursor_id", vec![("id", &id.to_string())]);
    }

    fn set_cursor_position(&self, cp: CursorPosition) {
        self.push_event(
            "cursor_position",
            vec![("x", &cp.x.to_string()), ("y", &cp.y.to_string())],
        );
    }

    /// unused in flutter, use switch_display or set_peer_info
    fn set_display(&self, _x: i32, _y: i32, _w: i32, _h: i32, _cursor_embedded: bool) {}

    fn update_privacy_mode(&self) {
        self.push_event("update_privacy_mode", [].into());
    }

    fn set_permission(&self, name: &str, value: bool) {
        self.push_event("permission", vec![(name, &value.to_string())]);
    }

    // unused in flutter
    fn close_success(&self) {}

    fn update_quality_status(&self, status: QualityStatus) {
        const NULL: String = String::new();
        self.push_event(
            "update_quality_status",
            vec![
                ("speed", &status.speed.map_or(NULL, |it| it)),
                ("fps", &status.fps.map_or(NULL, |it| it.to_string())),
                ("delay", &status.delay.map_or(NULL, |it| it.to_string())),
                (
                    "target_bitrate",
                    &status.target_bitrate.map_or(NULL, |it| it.to_string()),
                ),
                (
                    "codec_format",
                    &status.codec_format.map_or(NULL, |it| it.to_string()),
                ),
            ],
        );
    }

    fn set_connection_type(&self, is_secured: bool, direct: bool) {
        self.push_event(
            "connection_ready",
            vec![
                ("secure", &is_secured.to_string()),
                ("direct", &direct.to_string()),
            ],
        );
    }

    fn job_error(&self, id: i32, err: String, file_num: i32) {
        self.push_event(
            "job_error",
            vec![
                ("id", &id.to_string()),
                ("err", &err),
                ("file_num", &file_num.to_string()),
            ],
        );
    }

    fn job_done(&self, id: i32, file_num: i32) {
        self.push_event(
            "job_done",
            vec![("id", &id.to_string()), ("file_num", &file_num.to_string())],
        );
    }

    // unused in flutter
    fn clear_all_jobs(&self) {}

    fn load_last_job(&self, _cnt: i32, job_json: &str) {
        self.push_event("load_last_job", vec![("value", job_json)]);
    }

    fn update_folder_files(
        &self,
        id: i32,
        entries: &Vec<FileEntry>,
        path: String,
        #[allow(unused_variables)] is_local: bool,
        only_count: bool,
    ) {
        // TODO opt
        if only_count {
            self.push_event(
                "update_folder_files",
                vec![("info", &make_fd_flutter(id, entries, only_count))],
            );
        } else {
            self.push_event(
                "file_dir",
                vec![
                    ("value", &crate::common::make_fd_to_json(id, path, entries)),
                    ("is_local", "false"),
                ],
            );
        }
    }

    // unused in flutter
    fn update_transfer_list(&self) {}

    // unused in flutter // TEST flutter
    fn confirm_delete_files(&self, _id: i32, _i: i32, _name: String) {}

    fn override_file_confirm(&self, id: i32, file_num: i32, to: String, is_upload: bool) {
        self.push_event(
            "override_file_confirm",
            vec![
                ("id", &id.to_string()),
                ("file_num", &file_num.to_string()),
                ("read_path", &to),
                ("is_upload", &is_upload.to_string()),
            ],
        );
    }

    fn job_progress(&self, id: i32, file_num: i32, speed: f64, finished_size: f64) {
        self.push_event(
            "job_progress",
            vec![
                ("id", &id.to_string()),
                ("file_num", &file_num.to_string()),
                ("speed", &speed.to_string()),
                ("finished_size", &finished_size.to_string()),
            ],
        );
    }

    // unused in flutter
    fn adapt_size(&self) {}

    fn on_rgba(&self, data: &[u8]) {
        if let Some(stream) = &*self.event_stream.read().unwrap() {
            stream.add(EventToUI::Rgba(ZeroCopyBuffer(data.to_owned())));
        }
    }

    fn set_peer_info(&self, pi: &PeerInfo) {
        let mut displays = Vec::new();
        for ref d in pi.displays.iter() {
            let mut h: HashMap<&str, i32> = Default::default();
            h.insert("x", d.x);
            h.insert("y", d.y);
            h.insert("width", d.width);
            h.insert("height", d.height);
            h.insert("cursor_embedded", if d.cursor_embedded { 1 } else { 0 });
            displays.push(h);
        }
        let displays = serde_json::ser::to_string(&displays).unwrap_or("".to_owned());
        let mut features: HashMap<&str, i32> = Default::default();
        for ref f in pi.features.iter() {
            features.insert("privacy_mode", if f.privacy_mode { 1 } else { 0 });
        }
        // compatible with 1.1.9
        if get_version_number(&pi.version) < get_version_number("1.2.0") {
            features.insert("privacy_mode", 0);
        }
        let features = serde_json::ser::to_string(&features).unwrap_or("".to_owned());
        self.push_event(
            "peer_info",
            vec![
                ("username", &pi.username),
                ("hostname", &pi.hostname),
                ("platform", &pi.platform),
                ("sas_enabled", &pi.sas_enabled.to_string()),
                ("displays", &displays),
                ("version", &pi.version),
                ("features", &features),
                ("current_display", &pi.current_display.to_string()),
            ],
        );
    }

    fn on_connected(&self, _conn_type: ConnType) {}

    fn msgbox(&self, msgtype: &str, title: &str, text: &str, link: &str, retry: bool) {
        let has_retry = if retry { "true" } else { "" };
        self.push_event(
            "msgbox",
            vec![
                ("type", msgtype),
                ("title", title),
                ("text", text),
                ("link", link),
                ("hasRetry", has_retry),
            ],
        );
    }

    fn cancel_msgbox(&self, tag: &str) {
        self.push_event("cancel_msgbox", vec![("tag", tag)]);
    }

    fn new_message(&self, msg: String) {
        self.push_event("chat_client_mode", vec![("text", &msg)]);
    }

    fn switch_display(&self, display: &SwitchDisplay) {
        self.push_event(
            "switch_display",
            vec![
                ("display", &display.display.to_string()),
                ("x", &display.x.to_string()),
                ("y", &display.y.to_string()),
                ("width", &display.width.to_string()),
                ("height", &display.height.to_string()),
                (
                    "cursor_embedded",
                    &{
                        if display.cursor_embedded {
                            1
                        } else {
                            0
                        }
                    }
                    .to_string(),
                ),
            ],
        );
    }

    fn update_block_input_state(&self, on: bool) {
        self.push_event(
            "update_block_input_state",
            [("input_state", if on { "on" } else { "off" })].into(),
        );
    }

    #[cfg(any(target_os = "android", target_os = "ios"))]
    fn clipboard(&self, content: String) {
        self.push_event("clipboard", vec![("content", &content)]);
    }

    fn switch_back(&self, peer_id: &str) {
        self.push_event("switch_back", [("peer_id", peer_id)].into());
    }

    fn on_voice_call_started(&self) {
        self.push_event("on_voice_call_started", [].into());
    }

    fn on_voice_call_closed(&self, reason: &str) {
        self.push_event("on_voice_call_closed", [("reason", reason)].into())
    }

    fn on_voice_call_waiting(&self) {
        self.push_event("on_voice_call_waiting", [].into());
    }

    fn on_voice_call_incoming(&self) {
        self.push_event("on_voice_call_incoming", [].into());
    }
}

/// Create a new remote session with the given id.
///
/// # Arguments
///
/// * `id` - The identifier of the remote session with prefix. Regex: [\w]*[\_]*[\d]+
/// * `is_file_transfer` - If the session is used for file transfer.
/// * `is_port_forward` - If the session is used for port forward.
pub fn session_add(
    id: &str,
    is_file_transfer: bool,
    is_port_forward: bool,
    switch_uuid: &str,
) -> ResultType<()> {
    let session_id = get_session_id(id.to_owned());
    LocalConfig::set_remote_id(&session_id);

    let session: Session<FlutterHandler> = Session {
        id: session_id.clone(),
        ..Default::default()
    };

    // TODO rdp
    let conn_type = if is_file_transfer {
        ConnType::FILE_TRANSFER
    } else if is_port_forward {
        ConnType::PORT_FORWARD
    } else {
        ConnType::DEFAULT_CONN
    };

    let switch_uuid = if switch_uuid.is_empty() {
        None
    } else {
        Some(switch_uuid.to_string())
    };

    session
        .lc
        .write()
        .unwrap()
        .initialize(session_id, conn_type, switch_uuid);

    if let Some(same_id_session) = SESSIONS.write().unwrap().insert(id.to_owned(), session) {
        same_id_session.close();
    }

    Ok(())
}

/// start a session with the given id.
///
/// # Arguments
///
/// * `id` - The identifier of the remote session with prefix. Regex: [\w]*[\_]*[\d]+
/// * `events2ui` - The events channel to ui.
pub fn session_start_(id: &str, event_stream: StreamSink<EventToUI>) -> ResultType<()> {
    if let Some(session) = SESSIONS.write().unwrap().get_mut(id) {
        *session.event_stream.write().unwrap() = Some(event_stream);
        let session = session.clone();
        std::thread::spawn(move || {
            io_loop(session);
        });
        Ok(())
    } else {
        bail!("No session with peer id {}", id)
    }
}

// Server Side
#[cfg(not(any(target_os = "ios")))]
pub mod connection_manager {
    use std::collections::HashMap;

    #[cfg(any(target_os = "android"))]
    use hbb_common::log;
    #[cfg(any(target_os = "android"))]
    use scrap::android::call_main_service_set_by_name;

    use crate::ui_cm_interface::InvokeUiCM;

    use super::GLOBAL_EVENT_STREAM;

    #[derive(Clone)]
    struct FlutterHandler {}

    impl InvokeUiCM for FlutterHandler {
        //TODO port_forward
        fn add_connection(&self, client: &crate::ui_cm_interface::Client) {
            let client_json = serde_json::to_string(&client).unwrap_or("".into());
            // send to Android service, active notification no matter UI is shown or not.
            #[cfg(any(target_os = "android"))]
            if let Err(e) =
                call_main_service_set_by_name("add_connection", Some(&client_json), None)
            {
                log::debug!("call_service_set_by_name fail,{}", e);
            }
            // send to UI, refresh widget
            self.push_event("add_connection", vec![("client", &client_json)]);
        }

        fn remove_connection(&self, id: i32, close: bool) {
            self.push_event(
                "on_client_remove",
                vec![("id", &id.to_string()), ("close", &close.to_string())],
            );
        }

        fn new_message(&self, id: i32, text: String) {
            self.push_event(
                "chat_server_mode",
                vec![("id", &id.to_string()), ("text", &text)],
            );
        }

        fn change_theme(&self, dark: String) {
            self.push_event("theme", vec![("dark", &dark)]);
        }

        fn change_language(&self) {
            self.push_event("language", vec![]);
        }

        fn show_elevation(&self, show: bool) {
            self.push_event("show_elevation", vec![("show", &show.to_string())]);
        }

        fn update_voice_call_state(&self, client: &crate::ui_cm_interface::Client) {
            let client_json = serde_json::to_string(&client).unwrap_or("".into());
            self.push_event("update_voice_call_state", vec![("client", &client_json)]);
        }
    }

    impl FlutterHandler {
        fn push_event(&self, name: &str, event: Vec<(&str, &str)>) {
            let mut h: HashMap<&str, &str> = event.iter().cloned().collect();
            assert!(h.get("name").is_none());
            h.insert("name", name);

            if let Some(s) = GLOBAL_EVENT_STREAM.read().unwrap().get(super::APP_TYPE_CM) {
                s.add(serde_json::ser::to_string(&h).unwrap_or("".to_owned()));
            } else {
                println!(
                    "Push event {} failed. No {} event stream found.",
                    name,
                    super::APP_TYPE_CM
                );
            };
        }
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub fn start_listen_ipc_thread() {
        use crate::ui_cm_interface::{start_ipc, ConnectionManager};

        #[cfg(target_os = "linux")]
        std::thread::spawn(crate::ipc::start_pa);

        let cm = ConnectionManager {
            ui_handler: FlutterHandler {},
        };
        std::thread::spawn(move || start_ipc(cm));
    }

    #[cfg(target_os = "android")]
    use hbb_common::tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

    #[cfg(target_os = "android")]
    pub fn start_channel(
        rx: UnboundedReceiver<crate::ipc::Data>,
        tx: UnboundedSender<crate::ipc::Data>,
    ) {
        use crate::ui_cm_interface::start_listen;
        let cm = crate::ui_cm_interface::ConnectionManager {
            ui_handler: FlutterHandler {},
        };
        std::thread::spawn(move || start_listen(cm, rx, tx));
    }
}

#[inline]
pub fn get_session_id(id: String) -> String {
    return if let Some(index) = id.find('_') {
        id[index + 1..].to_string()
    } else {
        id
    };
}

pub fn make_fd_flutter(id: i32, entries: &Vec<FileEntry>, only_count: bool) -> String {
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

pub fn get_cur_session_id() -> String {
    CUR_SESSION_ID.read().unwrap().clone()
}

pub fn set_cur_session_id(id: String) {
    if get_cur_session_id() != id {
        *CUR_SESSION_ID.write().unwrap() = id;
    }
}
