use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use flutter_rust_bridge::{StreamSink, ZeroCopyBuffer};

use hbb_common::{
    bail, config::LocalConfig, message_proto::*, rendezvous_proto::ConnType, ResultType,
};
use serde_json::json;

use crate::ui_session_interface::{io_loop, InvokeUi, Session};

use crate::{client::*, flutter_ffi::EventToUI};

pub(super) const APP_TYPE_MAIN: &str = "main";
pub(super) const APP_TYPE_DESKTOP_REMOTE: &str = "remote";
pub(super) const APP_TYPE_DESKTOP_FILE_TRANSFER: &str = "file transfer";

lazy_static::lazy_static! {
    pub static ref SESSIONS: RwLock<HashMap<String,Session<FlutterHandler>>> = Default::default();
    pub static ref GLOBAL_EVENT_STREAM: RwLock<HashMap<String, StreamSink<String>>> = Default::default(); // rust to dart event channel
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

impl InvokeUi for FlutterHandler {
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
    fn set_display(&self, _x: i32, _y: i32, _w: i32, _h: i32) {}

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
        is_local: bool,
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
                    ("value", &make_fd_to_json(id, path, entries)),
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
            displays.push(h);
        }
        let displays = serde_json::ser::to_string(&displays).unwrap_or("".to_owned());
        self.push_event(
            "peer_info",
            vec![
                ("username", &pi.username),
                ("hostname", &pi.hostname),
                ("platform", &pi.platform),
                ("sas_enabled", &pi.sas_enabled.to_string()),
                ("displays", &displays),
                ("version", &pi.version),
                ("current_display", &pi.current_display.to_string()),
            ],
        );
    }

    fn msgbox(&self, msgtype: &str, title: &str, text: &str, retry: bool) {
        let has_retry = if retry { "true" } else { "" };
        self.push_event(
            "msgbox",
            vec![
                ("type", msgtype),
                ("title", title),
                ("text", text),
                ("hasRetry", has_retry),
            ],
        );
    }

    fn new_message(&self, msg: String) {
        self.push_event("chat_client_mode", vec![("text", &msg)]);
    }

    fn switch_display(&self, display: &SwitchDisplay) {
        self.push_event(
            "switch_display",
            vec![
                ("display", &display.to_string()),
                ("x", &display.x.to_string()),
                ("y", &display.y.to_string()),
                ("width", &display.width.to_string()),
                ("height", &display.height.to_string()),
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
}

/// Create a new remote session with the given id.
///
/// # Arguments
///
/// * `id` - The identifier of the remote session with prefix. Regex: [\w]*[\_]*[\d]+
/// * `is_file_transfer` - If the session is used for file transfer.
/// * `is_port_forward` - If the session is used for port forward.
pub fn session_add(id: &str, is_file_transfer: bool, is_port_forward: bool) -> ResultType<()> {
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

    session
        .lc
        .write()
        .unwrap()
        .initialize(session_id, conn_type);

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
    use std::{
        collections::HashMap,
        iter::FromIterator,
        sync::{
            atomic::{AtomicI64, Ordering},
            RwLock,
        },
    };

    use serde_derive::Serialize;

    use hbb_common::{
        allow_err,
        config::Config,
        fs::is_write_need_confirmation,
        fs::{self, get_string, new_send_confirm, DigestCheckResult},
        log,
        message_proto::*,
        protobuf::Message as _,
        tokio::{
            self,
            sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
            task::spawn_blocking,
        },
    };
    #[cfg(any(target_os = "android"))]
    use scrap::android::call_main_service_set_by_name;

    use crate::ipc::Data;
    use crate::ipc::{self, new_listener, Connection};

    use super::GLOBAL_EVENT_STREAM;

    #[derive(Debug, Serialize, Clone)]
    struct Client {
        id: i32,
        pub authorized: bool,
        is_file_transfer: bool,
        name: String,
        peer_id: String,
        keyboard: bool,
        clipboard: bool,
        audio: bool,
        file: bool,
        restart: bool,
        #[serde(skip)]
        tx: UnboundedSender<Data>,
    }

    lazy_static::lazy_static! {
        static ref CLIENTS: RwLock<HashMap<i32,Client>> = Default::default();
    }

    static CLICK_TIME: AtomicI64 = AtomicI64::new(0);

    // // TODO clipboard_file
    // enum ClipboardFileData {
    //     #[cfg(windows)]
    //     Clip((i32, ipc::ClipbaordFile)),
    //     Enable((i32, bool)),
    // }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub fn start_listen_ipc_thread() {
        std::thread::spawn(move || start_ipc());
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    #[tokio::main(flavor = "current_thread")]
    async fn start_ipc() {
        // TODO clipboard_file
        // let (tx_file, _rx_file) = mpsc::unbounded_channel::<ClipboardFileData>();
        // #[cfg(windows)]
        // let cm_clip = cm.clone();
        // #[cfg(windows)]
        // std::thread::spawn(move || start_clipboard_file(cm_clip, _rx_file));

        #[cfg(windows)]
        std::thread::spawn(move || {
            log::info!("try create privacy mode window");
            #[cfg(windows)]
            {
                if let Err(e) = crate::platform::windows::check_update_broker_process() {
                    log::warn!(
                        "Failed to check update broker process. Privacy mode may not work properly. {}",
                        e
                    );
                }
            }
            allow_err!(crate::ui::win_privacy::start());
        });

        match new_listener("_cm").await {
            Ok(mut incoming) => {
                while let Some(result) = incoming.next().await {
                    match result {
                        Ok(stream) => {
                            log::debug!("Got new connection");
                            let mut stream = Connection::new(stream);
                            // let tx_file = tx_file.clone();
                            tokio::spawn(async move {
                                // for tmp use, without real conn id
                                let conn_id_tmp = -1;
                                let mut conn_id: i32 = 0;
                                let (tx, mut rx) = mpsc::unbounded_channel::<Data>();
                                let mut write_jobs: Vec<fs::TransferJob> = Vec::new();
                                loop {
                                    tokio::select! {
                                        res = stream.next() => {
                                            match res {
                                                Err(err) => {
                                                    log::info!("cm ipc connection closed: {}", err);
                                                    break;
                                                }
                                                Ok(Some(data)) => {
                                                    match data {
                                                        Data::Login{id, is_file_transfer, port_forward, peer_id, name, authorized, keyboard, clipboard, audio, file, file_transfer_enabled, restart} => {
                                                            log::debug!("conn_id: {}", id);
                                                            conn_id = id;
                                                            // tx_file.send(ClipboardFileData::Enable((id, file_transfer_enabled))).ok();
                                                            on_login(id, is_file_transfer, port_forward, peer_id, name, authorized, keyboard, clipboard, audio, file, restart, tx.clone());
                                                        }
                                                        Data::Close => {
                                                            // tx_file.send(ClipboardFileData::Enable((conn_id, false))).ok();
                                                            log::info!("cm ipc connection closed from connection request");
                                                            break;
                                                        }
                                                        Data::PrivacyModeState((_, _)) => {
                                                            conn_id = conn_id_tmp;
                                                            allow_err!(tx.send(data));
                                                        }
                                                        Data::ClickTime(ms) => {
                                                            CLICK_TIME.store(ms, Ordering::SeqCst);
                                                        }
                                                        Data::ChatMessage { text } => {
                                                            handle_chat(conn_id, text);
                                                        }
                                                        Data::FS(fs) => {
                                                            handle_fs(fs, &mut write_jobs, &tx).await;
                                                        }
                                                        // TODO ClipbaordFile
                                                        // #[cfg(windows)]
                                                        // Data::ClipbaordFile(_clip) => {
                                                        //     tx_file
                                                        //         .send(ClipboardFileData::Clip((id, _clip)))
                                                        //         .ok();
                                                        // }
                                                        // #[cfg(windows)]
                                                        // Data::ClipboardFileEnabled(enabled) => {
                                                        //     tx_file
                                                        //         .send(ClipboardFileData::Enable((id, enabled)))
                                                        //         .ok();
                                                        // }
                                                        _ => {}
                                                    }
                                                }
                                                _ => {}
                                            }
                                        }
                                        Some(data) = rx.recv() => {
                                            if stream.send(&data).await.is_err() {
                                                break;
                                            }
                                        }
                                    }
                                }
                                if conn_id != conn_id_tmp {
                                    remove_connection(conn_id);
                                }
                            });
                        }
                        Err(err) => {
                            log::error!("Couldn't get cm client: {:?}", err);
                        }
                    }
                }
            }
            Err(err) => {
                log::error!("Failed to start cm ipc server: {}", err);
            }
        }
        // crate::platform::quit_gui();
        // TODO flutter quit_gui
    }

    #[cfg(target_os = "android")]
    pub fn start_channel(rx: UnboundedReceiver<Data>, tx: UnboundedSender<Data>) {
        std::thread::spawn(move || start_listen(rx, tx));
    }

    #[cfg(target_os = "android")]
    #[tokio::main(flavor = "current_thread")]
    async fn start_listen(mut rx: UnboundedReceiver<Data>, tx: UnboundedSender<Data>) {
        let mut current_id = 0;
        let mut write_jobs: Vec<fs::TransferJob> = Vec::new();
        loop {
            match rx.recv().await {
                Some(Data::Login {
                    id,
                    is_file_transfer,
                    port_forward,
                    peer_id,
                    name,
                    authorized,
                    keyboard,
                    clipboard,
                    audio,
                    file,
                    restart,
                    ..
                }) => {
                    current_id = id;
                    on_login(
                        id,
                        is_file_transfer,
                        port_forward,
                        peer_id,
                        name,
                        authorized,
                        keyboard,
                        clipboard,
                        audio,
                        file,
                        restart,
                        tx.clone(),
                    );
                }
                Some(Data::ChatMessage { text }) => {
                    handle_chat(current_id, text);
                }
                Some(Data::FS(fs)) => {
                    handle_fs(fs, &mut write_jobs, &tx).await;
                }
                Some(Data::Close) => {
                    break;
                }
                None => {
                    break;
                }
                _ => {}
            }
        }
        remove_connection(current_id);
    }

    fn on_login(
        id: i32,
        is_file_transfer: bool,
        _port_forward: String,
        peer_id: String,
        name: String,
        authorized: bool,
        keyboard: bool,
        clipboard: bool,
        audio: bool,
        file: bool,
        restart: bool,
        tx: mpsc::UnboundedSender<Data>,
    ) {
        let mut client = Client {
            id,
            authorized,
            is_file_transfer,
            name: name.clone(),
            peer_id: peer_id.clone(),
            keyboard,
            clipboard,
            audio,
            file,
            restart,
            tx,
        };
        if authorized {
            client.authorized = true;
            let client_json = serde_json::to_string(&client).unwrap_or("".into());
            // send to Android service, active notification no matter UI is shown or not.
            #[cfg(any(target_os = "android"))]
            if let Err(e) =
                call_main_service_set_by_name("on_client_authorized", Some(&client_json), None)
            {
                log::debug!("call_service_set_by_name fail,{}", e);
            }
            // send to UI, refresh widget
            push_event("on_client_authorized", vec![("client", &client_json)]);
        } else {
            let client_json = serde_json::to_string(&client).unwrap_or("".into());
            // send to Android service, active notification no matter UI is shown or not.
            #[cfg(any(target_os = "android"))]
            if let Err(e) =
                call_main_service_set_by_name("try_start_without_auth", Some(&client_json), None)
            {
                log::debug!("call_service_set_by_name fail,{}", e);
            }
            // send to UI, refresh widget
            push_event("try_start_without_auth", vec![("client", &client_json)]);
        }
        CLIENTS.write().unwrap().insert(id, client);
    }

    fn push_event(name: &str, event: Vec<(&str, &str)>) {
        let mut h: HashMap<&str, &str> = event.iter().cloned().collect();
        assert!(h.get("name").is_none());
        h.insert("name", name);

        if let Some(s) = GLOBAL_EVENT_STREAM
            .read()
            .unwrap()
            .get(super::APP_TYPE_MAIN)
        {
            s.add(serde_json::ser::to_string(&h).unwrap_or("".to_owned()));
        };
    }

    pub fn get_click_time() -> i64 {
        CLICK_TIME.load(Ordering::SeqCst)
    }

    pub fn check_click_time(id: i32) {
        if let Some(client) = CLIENTS.read().unwrap().get(&id) {
            allow_err!(client.tx.send(Data::ClickTime(0)));
        };
    }

    pub fn switch_permission(id: i32, name: String, enabled: bool) {
        if let Some(client) = CLIENTS.read().unwrap().get(&id) {
            allow_err!(client.tx.send(Data::SwitchPermission { name, enabled }));
        };
    }

    pub fn get_clients_state() -> String {
        let clients = CLIENTS.read().unwrap();
        let res = Vec::from_iter(clients.values().cloned());
        serde_json::to_string(&res).unwrap_or("".into())
    }

    pub fn get_clients_length() -> usize {
        let clients = CLIENTS.read().unwrap();
        clients.len()
    }

    pub fn close_conn(id: i32) {
        if let Some(client) = CLIENTS.read().unwrap().get(&id) {
            allow_err!(client.tx.send(Data::Close));
        };
    }

    pub fn on_login_res(id: i32, res: bool) {
        if let Some(client) = CLIENTS.write().unwrap().get_mut(&id) {
            if res {
                allow_err!(client.tx.send(Data::Authorize));
                client.authorized = true;
            } else {
                allow_err!(client.tx.send(Data::Close));
            }
        };
    }

    fn remove_connection(id: i32) {
        let mut clients = CLIENTS.write().unwrap();
        clients.remove(&id);

        if clients
            .iter()
            .filter(|(_k, v)| !v.is_file_transfer)
            .next()
            .is_none()
        {
            #[cfg(any(target_os = "android"))]
            if let Err(e) = call_main_service_set_by_name("stop_capture", None, None) {
                log::debug!("stop_capture err:{}", e);
            }
        }

        push_event("on_client_remove", vec![("id", &id.to_string())]);
    }

    // server mode handle chat from other peers
    fn handle_chat(id: i32, text: String) {
        push_event(
            "chat_server_mode",
            vec![("id", &id.to_string()), ("text", &text)],
        );
    }

    // server mode send chat to peer
    pub fn send_chat(id: i32, text: String) {
        let clients = CLIENTS.read().unwrap();
        if let Some(client) = clients.get(&id) {
            allow_err!(client.tx.send(Data::ChatMessage { text }));
        }
    }

    // handle FS server
    async fn handle_fs(
        fs: ipc::FS,
        write_jobs: &mut Vec<fs::TransferJob>,
        tx: &UnboundedSender<Data>,
    ) {
        match fs {
            ipc::FS::ReadDir {
                dir,
                include_hidden,
            } => {
                read_dir(&dir, include_hidden, tx).await;
            }
            ipc::FS::RemoveDir {
                path,
                id,
                recursive,
            } => {
                remove_dir(path, id, recursive, tx).await;
            }
            ipc::FS::RemoveFile { path, id, file_num } => {
                remove_file(path, id, file_num, tx).await;
            }
            ipc::FS::CreateDir { path, id } => {
                create_dir(path, id, tx).await;
            }
            ipc::FS::NewWrite {
                path,
                id,
                file_num,
                mut files,
                overwrite_detection,
            } => {
                write_jobs.push(fs::TransferJob::new_write(
                    id,
                    "".to_string(),
                    path,
                    file_num,
                    false,
                    false,
                    files
                        .drain(..)
                        .map(|f| FileEntry {
                            name: f.0,
                            modified_time: f.1,
                            ..Default::default()
                        })
                        .collect(),
                    overwrite_detection,
                ));
            }
            ipc::FS::CancelWrite { id } => {
                if let Some(job) = fs::get_job(id, write_jobs) {
                    job.remove_download_file();
                    fs::remove_job(id, write_jobs);
                }
            }
            ipc::FS::WriteDone { id, file_num } => {
                if let Some(job) = fs::get_job(id, write_jobs) {
                    job.modify_time();
                    send_raw(fs::new_done(id, file_num), tx);
                    fs::remove_job(id, write_jobs);
                }
            }
            ipc::FS::WriteBlock {
                id,
                file_num,
                data,
                compressed,
            } => {
                if let Some(job) = fs::get_job(id, write_jobs) {
                    if let Err(err) = job
                        .write(
                            FileTransferBlock {
                                id,
                                file_num,
                                data,
                                compressed,
                                ..Default::default()
                            },
                            None,
                        )
                        .await
                    {
                        send_raw(fs::new_error(id, err, file_num), &tx);
                    }
                }
            }
            ipc::FS::CheckDigest {
                id,
                file_num,
                file_size,
                last_modified,
                is_upload,
            } => {
                if let Some(job) = fs::get_job(id, write_jobs) {
                    let mut req = FileTransferSendConfirmRequest {
                        id,
                        file_num,
                        union: Some(file_transfer_send_confirm_request::Union::OffsetBlk(0)),
                        ..Default::default()
                    };
                    let digest = FileTransferDigest {
                        id,
                        file_num,
                        last_modified,
                        file_size,
                        ..Default::default()
                    };
                    if let Some(file) = job.files().get(file_num as usize) {
                        let path = get_string(&job.join(&file.name));
                        match is_write_need_confirmation(&path, &digest) {
                            Ok(digest_result) => {
                                match digest_result {
                                    DigestCheckResult::IsSame => {
                                        req.set_skip(true);
                                        let msg_out = new_send_confirm(req);
                                        send_raw(msg_out, &tx);
                                    }
                                    DigestCheckResult::NeedConfirm(mut digest) => {
                                        // upload to server, but server has the same file, request
                                        digest.is_upload = is_upload;
                                        let mut msg_out = Message::new();
                                        let mut fr = FileResponse::new();
                                        fr.set_digest(digest);
                                        msg_out.set_file_response(fr);
                                        send_raw(msg_out, &tx);
                                    }
                                    DigestCheckResult::NoSuchFile => {
                                        let msg_out = new_send_confirm(req);
                                        send_raw(msg_out, &tx);
                                    }
                                }
                            }
                            Err(err) => {
                                send_raw(fs::new_error(id, err, file_num), &tx);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    async fn read_dir(dir: &str, include_hidden: bool, tx: &UnboundedSender<Data>) {
        let path = {
            if dir.is_empty() {
                Config::get_home()
            } else {
                fs::get_path(dir)
            }
        };
        if let Ok(Ok(fd)) = spawn_blocking(move || fs::read_dir(&path, include_hidden)).await {
            let mut msg_out = Message::new();
            let mut file_response = FileResponse::new();
            file_response.set_dir(fd);
            msg_out.set_file_response(file_response);
            send_raw(msg_out, tx);
        }
    }

    async fn handle_result<F: std::fmt::Display, S: std::fmt::Display>(
        res: std::result::Result<std::result::Result<(), F>, S>,
        id: i32,
        file_num: i32,
        tx: &UnboundedSender<Data>,
    ) {
        match res {
            Err(err) => {
                send_raw(fs::new_error(id, err, file_num), tx);
            }
            Ok(Err(err)) => {
                send_raw(fs::new_error(id, err, file_num), tx);
            }
            Ok(Ok(())) => {
                send_raw(fs::new_done(id, file_num), tx);
            }
        }
    }

    async fn remove_file(path: String, id: i32, file_num: i32, tx: &UnboundedSender<Data>) {
        handle_result(
            spawn_blocking(move || fs::remove_file(&path)).await,
            id,
            file_num,
            tx,
        )
        .await;
    }

    async fn create_dir(path: String, id: i32, tx: &UnboundedSender<Data>) {
        handle_result(
            spawn_blocking(move || fs::create_dir(&path)).await,
            id,
            0,
            tx,
        )
        .await;
    }

    async fn remove_dir(path: String, id: i32, recursive: bool, tx: &UnboundedSender<Data>) {
        let path = fs::get_path(&path);
        handle_result(
            spawn_blocking(move || {
                if recursive {
                    fs::remove_all_empty_dir(&path)
                } else {
                    std::fs::remove_dir(&path).map_err(|err| err.into())
                }
            })
            .await,
            id,
            0,
            tx,
        )
        .await;
    }

    fn send_raw(msg: Message, tx: &UnboundedSender<Data>) {
        match msg.write_to_bytes() {
            Ok(bytes) => {
                allow_err!(tx.send(Data::RawMessage(bytes)));
            }
            err => allow_err!(err),
        }
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

pub fn make_fd_to_json(id: i32, path: String, entries: &Vec<FileEntry>) -> String {
    let mut fd_json = serde_json::Map::new();
    fd_json.insert("id".into(), json!(id));
    fd_json.insert("path".into(), json!(path));

    let mut entries_out = vec![];
    for entry in entries {
        let mut entry_map = serde_json::Map::new();
        entry_map.insert("entry_type".into(), json!(entry.entry_type.value()));
        entry_map.insert("name".into(), json!(entry.name));
        entry_map.insert("size".into(), json!(entry.size));
        entry_map.insert("modified_time".into(), json!(entry.modified_time));
        entries_out.push(entry_map);
    }
    fd_json.insert("entries".into(), json!(entries_out));
    serde_json::to_string(&fd_json).unwrap_or("".into())
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
