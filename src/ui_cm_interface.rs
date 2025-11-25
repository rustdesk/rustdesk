#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::ipc::Connection;
#[cfg(not(any(target_os = "ios")))]
use crate::ipc::{self, Data};
#[cfg(target_os = "windows")]
use crate::{clipboard::ClipboardSide, ipc::ClipboardNonFile};
#[cfg(target_os = "windows")]
use clipboard::ContextSend;
#[cfg(not(any(target_os = "ios")))]
use hbb_common::fs::serialize_transfer_job;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use hbb_common::tokio::sync::mpsc::unbounded_channel;
use hbb_common::{
    allow_err, bail,
    config::{
        keys::OPTION_ENABLE_FILE_TRANSFER_HASH_VALIDATION, keys::OPTION_FILE_TRANSFER_MAX_FILES,
        Config,
    },
    fs::{self, get_string, is_write_need_confirmation, new_send_confirm, DigestCheckResult},
    log,
    message_proto::*,
    protobuf::Message as _,
    tokio::{
        self,
        sync::mpsc::{self, UnboundedSender},
        task::spawn_blocking,
    },
    ResultType,
};
#[cfg(target_os = "windows")]
use hbb_common::{
    config::{keys::*, option2bool},
    tokio::sync::Mutex as TokioMutex,
};
use serde_derive::Serialize;
#[cfg(any(target_os = "android", target_os = "ios", feature = "flutter"))]
use std::iter::FromIterator;
#[cfg(not(any(target_os = "ios")))]
use std::path::PathBuf;
#[cfg(target_os = "windows")]
use std::sync::Arc;
use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    path::Path,
    sync::{
        atomic::{AtomicI64, Ordering},
        RwLock,
    },
};

#[cfg(not(any(target_os = "ios")))]
const DEFAULT_MAX_VALIDATED_FILES: usize = 10_000;

/// Maximum number of files allowed in a single file transfer request.
/// This prevents excessive I/O when dealing with large directories.
/// Initialized once at startup from configuration.
#[cfg(not(any(target_os = "ios")))]
static MAX_VALIDATED_FILES: std::sync::OnceLock<usize> = std::sync::OnceLock::new();

/// Get the maximum number of validated files allowed.
/// Initializes the value from configuration on first call.
#[cfg(not(any(target_os = "ios")))]
#[inline]
pub fn get_max_validated_files() -> usize {
    *MAX_VALIDATED_FILES.get_or_init(|| {
        crate::get_builtin_option(OPTION_FILE_TRANSFER_MAX_FILES)
            .parse::<usize>()
            .unwrap_or(DEFAULT_MAX_VALIDATED_FILES)
    })
}

/// Check if file count exceeds the maximum allowed limit.
/// Returns Ok(()) if within limit, or Err with formatted error message.
#[cfg(not(any(target_os = "ios")))]
pub fn check_file_count_limit(file_count: usize) -> Result<(), String> {
    let max_files = get_max_validated_files();
    if file_count > max_files {
        let msg = format!(
            "too many files in directory ({} > {})",
            file_count, max_files
        );
        log::warn!("{}", msg);
        Err(msg)
    } else {
        Ok(())
    }
}

#[derive(Serialize, Clone)]
pub struct Client {
    pub id: i32,
    pub authorized: bool,
    pub disconnected: bool,
    pub is_file_transfer: bool,
    pub is_view_camera: bool,
    pub is_terminal: bool,
    pub port_forward: String,
    pub name: String,
    pub peer_id: String,
    pub keyboard: bool,
    pub clipboard: bool,
    pub audio: bool,
    pub file: bool,
    pub restart: bool,
    pub recording: bool,
    pub block_input: bool,
    pub from_switch: bool,
    pub in_voice_call: bool,
    pub incoming_voice_call: bool,
    #[serde(skip)]
    #[cfg(not(any(target_os = "ios")))]
    tx: UnboundedSender<Data>,
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
struct IpcTaskRunner<T: InvokeUiCM> {
    stream: Connection,
    cm: ConnectionManager<T>,
    tx: mpsc::UnboundedSender<Data>,
    rx: mpsc::UnboundedReceiver<Data>,
    close: bool,
    running: bool,
    conn_id: i32,
    #[cfg(target_os = "windows")]
    file_transfer_enabled: bool,
    #[cfg(target_os = "windows")]
    file_transfer_enabled_peer: bool,
}

lazy_static::lazy_static! {
    static ref CLIENTS: RwLock<HashMap<i32, Client>> = Default::default();
}

static CLICK_TIME: AtomicI64 = AtomicI64::new(0);

#[derive(Clone)]
pub struct ConnectionManager<T: InvokeUiCM> {
    pub ui_handler: T,
}

pub trait InvokeUiCM: Send + Clone + 'static + Sized {
    fn add_connection(&self, client: &Client);

    fn remove_connection(&self, id: i32, close: bool);

    fn new_message(&self, id: i32, text: String);

    fn change_theme(&self, dark: String);

    fn change_language(&self);

    fn show_elevation(&self, show: bool);

    fn update_voice_call_state(&self, client: &Client);

    fn file_transfer_log(&self, action: &str, log: &str);
}

impl<T: InvokeUiCM> Deref for ConnectionManager<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.ui_handler
    }
}

impl<T: InvokeUiCM> DerefMut for ConnectionManager<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ui_handler
    }
}

impl<T: InvokeUiCM> ConnectionManager<T> {
    fn add_connection(
        &self,
        id: i32,
        is_file_transfer: bool,
        is_view_camera: bool,
        is_terminal: bool,
        port_forward: String,
        peer_id: String,
        name: String,
        authorized: bool,
        keyboard: bool,
        clipboard: bool,
        audio: bool,
        file: bool,
        restart: bool,
        recording: bool,
        block_input: bool,
        from_switch: bool,
        #[cfg(not(any(target_os = "ios")))] tx: mpsc::UnboundedSender<Data>,
    ) {
        let client = Client {
            id,
            authorized,
            disconnected: false,
            is_file_transfer,
            is_view_camera,
            is_terminal,
            port_forward,
            name: name.clone(),
            peer_id: peer_id.clone(),
            keyboard,
            clipboard,
            audio,
            file,
            restart,
            recording,
            block_input,
            from_switch,
            #[cfg(not(any(target_os = "ios")))]
            tx,
            in_voice_call: false,
            incoming_voice_call: false,
        };
        CLIENTS
            .write()
            .unwrap()
            .retain(|_, c| !(c.disconnected && c.peer_id == client.peer_id));
        CLIENTS.write().unwrap().insert(id, client.clone());
        self.ui_handler.add_connection(&client);
    }

    #[inline]
    #[cfg(target_os = "windows")]
    fn is_authorized(&self, id: i32) -> bool {
        CLIENTS
            .read()
            .unwrap()
            .get(&id)
            .map(|c| c.authorized)
            .unwrap_or(false)
    }

    fn remove_connection(&self, id: i32, close: bool) {
        if close {
            CLIENTS.write().unwrap().remove(&id);
        } else {
            CLIENTS
                .write()
                .unwrap()
                .get_mut(&id)
                .map(|c| c.disconnected = true);
        }

        #[cfg(target_os = "windows")]
        {
            crate::clipboard::try_empty_clipboard_files(ClipboardSide::Host, id);
        }

        #[cfg(any(target_os = "android"))]
        if CLIENTS
            .read()
            .unwrap()
            .iter()
            .filter(|(_k, v)| !v.is_file_transfer && !v.is_terminal)
            .next()
            .is_none()
        {
            if let Err(e) =
                scrap::android::call_main_service_set_by_name("stop_capture", None, None)
            {
                log::debug!("stop_capture err:{}", e);
            }
        }

        self.ui_handler.remove_connection(id, close);
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    fn show_elevation(&self, show: bool) {
        self.ui_handler.show_elevation(show);
    }

    #[cfg(not(target_os = "ios"))]
    fn voice_call_started(&self, id: i32) {
        if let Some(client) = CLIENTS.write().unwrap().get_mut(&id) {
            client.incoming_voice_call = false;
            client.in_voice_call = true;
            self.ui_handler.update_voice_call_state(client);
        }
    }

    #[cfg(not(target_os = "ios"))]
    fn voice_call_incoming(&self, id: i32) {
        if let Some(client) = CLIENTS.write().unwrap().get_mut(&id) {
            client.incoming_voice_call = true;
            client.in_voice_call = false;
            self.ui_handler.update_voice_call_state(client);
        }
    }

    #[cfg(not(target_os = "ios"))]
    fn voice_call_closed(&self, id: i32, _reason: &str) {
        if let Some(client) = CLIENTS.write().unwrap().get_mut(&id) {
            client.incoming_voice_call = false;
            client.in_voice_call = false;
            self.ui_handler.update_voice_call_state(client);
        }
    }
}

#[inline]
#[cfg(not(any(target_os = "ios")))]
pub fn check_click_time(id: i32) {
    if let Some(client) = CLIENTS.read().unwrap().get(&id) {
        allow_err!(client.tx.send(Data::ClickTime(0)));
    };
}

#[inline]
pub fn get_click_time() -> i64 {
    CLICK_TIME.load(Ordering::SeqCst)
}

#[inline]
#[cfg(not(any(target_os = "ios")))]
pub fn authorize(id: i32) {
    if let Some(client) = CLIENTS.write().unwrap().get_mut(&id) {
        client.authorized = true;
        allow_err!(client.tx.send(Data::Authorize));
    };
}

#[inline]
#[cfg(not(any(target_os = "ios")))]
pub fn close(id: i32) {
    if let Some(client) = CLIENTS.read().unwrap().get(&id) {
        allow_err!(client.tx.send(Data::Close));
    };
}

#[inline]
pub fn remove(id: i32) {
    CLIENTS.write().unwrap().remove(&id);
}

// server mode send chat to peer
#[inline]
#[cfg(not(any(target_os = "ios")))]
pub fn send_chat(id: i32, text: String) {
    let clients = CLIENTS.read().unwrap();
    if let Some(client) = clients.get(&id) {
        allow_err!(client.tx.send(Data::ChatMessage { text }));
    }
}

#[inline]
#[cfg(not(any(target_os = "ios")))]
pub fn switch_permission(id: i32, name: String, enabled: bool) {
    if let Some(client) = CLIENTS.read().unwrap().get(&id) {
        allow_err!(client.tx.send(Data::SwitchPermission { name, enabled }));
    };
}

#[inline]
#[cfg(target_os = "android")]
pub fn switch_permission_all(name: String, enabled: bool) {
    for (_, client) in CLIENTS.read().unwrap().iter() {
        allow_err!(client.tx.send(Data::SwitchPermission {
            name: name.clone(),
            enabled
        }));
    }
}

#[cfg(any(target_os = "android", target_os = "ios", feature = "flutter"))]
#[inline]
pub fn get_clients_state() -> String {
    let clients = CLIENTS.read().unwrap();
    let res = Vec::from_iter(clients.values().cloned());
    serde_json::to_string(&res).unwrap_or("".into())
}

#[inline]
pub fn get_clients_length() -> usize {
    let clients = CLIENTS.read().unwrap();
    clients.len()
}

#[inline]
#[cfg(feature = "flutter")]
#[cfg(not(any(target_os = "ios")))]
pub fn switch_back(id: i32) {
    if let Some(client) = CLIENTS.read().unwrap().get(&id) {
        allow_err!(client.tx.send(Data::SwitchSidesBack));
    };
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
impl<T: InvokeUiCM> IpcTaskRunner<T> {
    async fn run(&mut self) {
        use hbb_common::config::LocalConfig;

        // for tmp use, without real conn id
        let mut write_jobs: Vec<fs::TransferJob> = Vec::new();

        #[cfg(target_os = "windows")]
        let is_authorized = self.cm.is_authorized(self.conn_id);

        #[cfg(target_os = "windows")]
        let rx_clip_holder;
        let mut rx_clip;
        let _tx_clip;
        #[cfg(target_os = "windows")]
        if self.conn_id > 0 && is_authorized {
            log::debug!("Clipboard is enabled from client peer: type 1");
            let conn_id = self.conn_id;
            rx_clip_holder = (
                clipboard::get_rx_cliprdr_server(conn_id),
                Some(crate::SimpleCallOnReturn {
                    b: true,
                    f: Box::new(move || {
                        clipboard::remove_channel_by_conn_id(conn_id);
                    }),
                }),
            );
            rx_clip = rx_clip_holder.0.lock().await;
        } else {
            log::debug!("Clipboard is enabled from client peer, actually useless: type 2");
            let rx_clip2;
            (_tx_clip, rx_clip2) = unbounded_channel::<clipboard::ClipboardFile>();
            rx_clip_holder = (Arc::new(TokioMutex::new(rx_clip2)), None);
            rx_clip = rx_clip_holder.0.lock().await;
        }
        #[cfg(not(target_os = "windows"))]
        {
            (_tx_clip, rx_clip) = unbounded_channel::<i32>();
        }

        #[cfg(target_os = "windows")]
        {
            if ContextSend::is_enabled() {
                log::debug!("Clipboard is enabled");
                allow_err!(
                    self.stream
                        .send(&Data::ClipboardFile(clipboard::ClipboardFile::MonitorReady))
                        .await
                );
            }
        }
        let (tx_log, mut rx_log) = mpsc::unbounded_channel::<String>();

        self.running = false;
        loop {
            tokio::select! {
                res = self.stream.next() => {
                    match res {
                        Err(err) => {
                            log::info!("cm ipc connection closed: {}", err);
                            break;
                        }
                        Ok(Some(data)) => {
                            match data {
                                Data::Login{id, is_file_transfer, is_view_camera, is_terminal, port_forward, peer_id, name, authorized, keyboard, clipboard, audio, file, file_transfer_enabled: _file_transfer_enabled, restart, recording, block_input, from_switch} => {
                                    log::debug!("conn_id: {}", id);
                                    self.cm.add_connection(id, is_file_transfer, is_view_camera, is_terminal, port_forward, peer_id, name, authorized, keyboard, clipboard, audio, file, restart, recording, block_input, from_switch, self.tx.clone());
                                    self.conn_id = id;
                                    #[cfg(target_os = "windows")]
                                    {
                                        self.file_transfer_enabled = _file_transfer_enabled;
                                    }
                                    self.running = true;
                                    break;
                                }
                                Data::Close => {
                                    log::info!("cm ipc connection closed from connection request");
                                    break;
                                }
                                Data::Disconnected => {
                                    self.close = false;
                                    log::info!("cm ipc connection disconnect");
                                    break;
                                }
                                Data::PrivacyModeState((_id, _, _)) => {
                                    #[cfg(windows)]
                                    cm_inner_send(_id, data);
                                }
                                Data::ClickTime(ms) => {
                                    CLICK_TIME.store(ms, Ordering::SeqCst);
                                }
                                Data::ChatMessage { text } => {
                                    self.cm.new_message(self.conn_id, text);
                                }
                                Data::FS(mut fs) => {
                                    if let ipc::FS::WriteBlock { id, file_num, data: _, compressed } = fs {
                                        if let Ok(bytes) = self.stream.next_raw().await {
                                            fs = ipc::FS::WriteBlock{id, file_num, data:bytes.into(), compressed};
                                            handle_fs(fs, &mut write_jobs, &self.tx, Some(&tx_log)).await;
                                        }
                                    } else {
                                        handle_fs(fs, &mut write_jobs, &self.tx, Some(&tx_log)).await;
                                    }
                                    let log = fs::serialize_transfer_jobs(&write_jobs);
                                    self.cm.ui_handler.file_transfer_log("transfer", &log);
                                }
                                Data::FileTransferLog((action, log)) => {
                                    self.cm.ui_handler.file_transfer_log(&action, &log);
                                }
                                #[cfg(target_os = "windows")]
                                Data::ClipboardFile(_clip) => {
                                    let is_stopping_allowed = _clip.is_beginning_message();
                                    let is_clipboard_enabled = ContextSend::is_enabled();
                                    let file_transfer_enabled = self.file_transfer_enabled;
                                    let stop = !is_stopping_allowed && !(is_clipboard_enabled && file_transfer_enabled);
                                    log::debug!(
                                        "Process clipboard message from client peer, stop: {}, is_stopping_allowed: {}, is_clipboard_enabled: {}, file_transfer_enabled: {}",
                                        stop, is_stopping_allowed, is_clipboard_enabled, file_transfer_enabled);
                                    if stop {
                                        ContextSend::set_is_stopped();
                                    } else {
                                        if !is_authorized {
                                            log::debug!("Clipboard message from client peer, but not authorized");
                                            continue;
                                        }
                                        let conn_id = self.conn_id;
                                        let _ = ContextSend::proc(|context| -> ResultType<()> {
                                            context.server_clip_file(conn_id, _clip)
                                                .map_err(|e| e.into())
                                        });
                                    }
                                }
                                Data::ClipboardFileEnabled(_enabled) => {
                                    #[cfg(target_os = "windows")]
                                    {
                                        self.file_transfer_enabled_peer = _enabled;
                                    }
                                }
                                Data::Theme(dark) => {
                                    self.cm.change_theme(dark);
                                }
                                Data::Language(lang) => {
                                    LocalConfig::set_option("lang".to_owned(), lang);
                                    self.cm.change_language();
                                }
                                Data::DataPortableService(ipc::DataPortableService::CmShowElevation(show)) => {
                                    self.cm.show_elevation(show);
                                }
                                Data::StartVoiceCall => {
                                    self.cm.voice_call_started(self.conn_id);
                                }
                                Data::VoiceCallIncoming => {
                                    self.cm.voice_call_incoming(self.conn_id);
                                }
                                Data::CloseVoiceCall(reason) => {
                                    self.cm.voice_call_closed(self.conn_id, reason.as_str());
                                }
                                #[cfg(target_os = "windows")]
                                Data::ClipboardNonFile(_) => {
                                    match crate::clipboard::check_clipboard_cm() {
                                        Ok(multi_clipoards) => {
                                            let mut raw_contents = bytes::BytesMut::new();
                                            let mut main_data = vec![];
                                            for c in multi_clipoards.clipboards.into_iter() {
                                                let content_len = c.content.len();
                                                let (content, next_raw) = {
                                                    // TODO: find out a better threshold
                                                    if content_len > 1024 * 3 {
                                                        raw_contents.extend(c.content);
                                                        (bytes::Bytes::new(), true)
                                                    } else {
                                                        (c.content, false)
                                                    }
                                                };
                                                main_data.push(ClipboardNonFile {
                                                    compress: c.compress,
                                                    content,
                                                    content_len,
                                                    next_raw,
                                                    width: c.width,
                                                    height: c.height,
                                                    format: c.format.value(),
                                                    special_name: c.special_name,
                                                });
                                            }
                                            allow_err!(self.stream.send(&Data::ClipboardNonFile(Some(("".to_owned(), main_data)))).await);
                                            if !raw_contents.is_empty() {
                                                allow_err!(self.stream.send_raw(raw_contents.into()).await);
                                            }
                                        }
                                        Err(e) => {
                                            log::debug!("Failed to get clipboard content. {}", e);
                                            allow_err!(self.stream.send(&Data::ClipboardNonFile(Some((format!("{}", e), vec![])))).await);
                                        }
                                    }
                                }
                                _ => {

                                }
                            }
                        }
                        _ => {}
                    }
                }
                Some(data) = self.rx.recv() => {
                    if let Err(e) = self.stream.send(&data).await {
                        log::error!("error encountered in IPC task, quitting: {}", e);
                        break;
                    }
                    match &data {
                        Data::SwitchPermission{name: _name, enabled: _enabled} => {
                            #[cfg(target_os = "windows")]
                            if _name == "file" {
                                self.file_transfer_enabled = *_enabled;
                            }
                        }
                        Data::Authorize => {
                            self.running = true;
                            break;
                        }
                        _ => {
                        }
                    }
                },
                clip_file = rx_clip.recv() => match clip_file {
                    Some(_clip) => {
                        #[cfg(target_os = "windows")]
                        {
                            let is_stopping_allowed = _clip.is_stopping_allowed();
                            let is_clipboard_enabled = ContextSend::is_enabled();
                            let file_transfer_enabled = self.file_transfer_enabled;
                            let file_transfer_enabled_peer = self.file_transfer_enabled_peer;
                            let stop = is_stopping_allowed && !(is_clipboard_enabled && file_transfer_enabled && file_transfer_enabled_peer);
                            log::debug!(
                                "Process clipboard message from clip, stop: {}, is_stopping_allowed: {}, is_clipboard_enabled: {}, file_transfer_enabled: {}, file_transfer_enabled_peer: {}",
                                stop, is_stopping_allowed, is_clipboard_enabled, file_transfer_enabled, file_transfer_enabled_peer);
                            if stop {
                                ContextSend::set_is_stopped();
                            } else {
                                if _clip.is_beginning_message() && crate::get_builtin_option(OPTION_ONE_WAY_FILE_TRANSFER) == "Y" {
                                    // If one way file transfer is enabled, don't send clipboard file to client
                                    // Don't call `ContextSend::set_is_stopped()`, because it will stop bidirectional file copy&paste.
                                } else {
                                    allow_err!(self.tx.send(Data::ClipboardFile(_clip)));
                                }
                            }
                        }
                    }
                    None => {
                        //
                    }
                },
                Some(job_log) = rx_log.recv() => {
                    self.cm.ui_handler.file_transfer_log("transfer", &job_log);
                }
            }
        }
    }

    async fn ipc_task(stream: Connection, cm: ConnectionManager<T>) {
        log::debug!("ipc task begin");
        let (tx, rx) = mpsc::unbounded_channel::<Data>();
        let mut task_runner = Self {
            stream,
            cm,
            tx,
            rx,
            close: true,
            running: true,
            conn_id: 0,
            #[cfg(target_os = "windows")]
            file_transfer_enabled: false,
            #[cfg(target_os = "windows")]
            file_transfer_enabled_peer: false,
        };

        while task_runner.running {
            task_runner.run().await;
        }
        if task_runner.conn_id > 0 {
            task_runner
                .cm
                .remove_connection(task_runner.conn_id, task_runner.close);
        }
        log::debug!("ipc task end");
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::main(flavor = "current_thread")]
pub async fn start_ipc<T: InvokeUiCM>(cm: ConnectionManager<T>) {
    #[cfg(target_os = "windows")]
    ContextSend::enable(option2bool(
        OPTION_ENABLE_FILE_TRANSFER,
        &Config::get_option(OPTION_ENABLE_FILE_TRANSFER),
    ));
    match ipc::new_listener("_cm").await {
        Ok(mut incoming) => {
            while let Some(result) = incoming.next().await {
                match result {
                    Ok(stream) => {
                        log::debug!("Got new connection");
                        tokio::spawn(IpcTaskRunner::<T>::ipc_task(
                            Connection::new(stream),
                            cm.clone(),
                        ));
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
    quit_cm();
}

#[cfg(target_os = "android")]
#[tokio::main(flavor = "current_thread")]
pub async fn start_listen<T: InvokeUiCM>(
    cm: ConnectionManager<T>,
    mut rx: mpsc::UnboundedReceiver<Data>,
    tx: mpsc::UnboundedSender<Data>,
) {
    let mut current_id = 0;
    let mut write_jobs: Vec<fs::TransferJob> = Vec::new();
    loop {
        match rx.recv().await {
            Some(Data::Login {
                id,
                is_file_transfer,
                is_view_camera,
                is_terminal,
                port_forward,
                peer_id,
                name,
                authorized,
                keyboard,
                clipboard,
                audio,
                file,
                restart,
                recording,
                block_input,
                from_switch,
                ..
            }) => {
                current_id = id;
                cm.add_connection(
                    id,
                    is_file_transfer,
                    is_view_camera,
                    is_terminal,
                    port_forward,
                    peer_id,
                    name,
                    authorized,
                    keyboard,
                    clipboard,
                    audio,
                    file,
                    restart,
                    recording,
                    block_input,
                    from_switch,
                    tx.clone(),
                );
            }
            Some(Data::ChatMessage { text }) => {
                cm.new_message(current_id, text);
            }
            Some(Data::FS(fs)) => {
                handle_fs(fs, &mut write_jobs, &tx, None).await;
            }
            Some(Data::Close) => {
                break;
            }
            Some(Data::StartVoiceCall) => {
                cm.voice_call_started(current_id);
            }
            Some(Data::VoiceCallIncoming) => {
                cm.voice_call_incoming(current_id);
            }
            Some(Data::CloseVoiceCall(reason)) => {
                cm.voice_call_closed(current_id, reason.as_str());
            }
            None => {
                break;
            }
            _ => {}
        }
    }
    cm.remove_connection(current_id, true);
}

#[cfg(not(any(target_os = "ios")))]
async fn handle_fs(
    fs: ipc::FS,
    write_jobs: &mut Vec<fs::TransferJob>,
    tx: &UnboundedSender<Data>,
    tx_log: Option<&UnboundedSender<String>>,
) {
    match fs {
        ipc::FS::ReadEmptyDirs {
            dir,
            include_hidden,
        } => {
            read_empty_dirs(&dir, include_hidden, tx).await;
        }
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
            total_size,
            conn_id,
        } => {
            // Validate file names to prevent path traversal attacks.
            // This must be done BEFORE any path operations to ensure attackers cannot
            // escape the target directory using names like "../../malicious.txt"
            if let Err(e) = validate_transfer_file_names(&files) {
                log::warn!("Path traversal attempt detected for {}: {}", path, e);
                send_raw(fs::new_error(id, e, file_num), tx);
                return;
            }

            // Check write access to the parent directory
            let path_obj = Path::new(&path);
            let Some(filename) = path_obj.file_name() else {
                log::warn!("Write access denied for {}: No filename provided", path);
                send_raw(
                    fs::new_error(
                        id,
                        "No filename provided in the specified path".to_string(),
                        file_num,
                    ),
                    tx,
                );
                return;
            };

            let canonical_base = match validate_parent_and_canonicalize(&path_obj) {
                Ok(parent) => parent.join(filename),
                Err(e) => {
                    log::warn!("Write access denied for {}: {}", path, e);
                    send_raw(fs::new_error(id, e, file_num), tx);
                    return;
                }
            };

            // Convert files to FileEntry and validate write paths
            let file_entries: Vec<FileEntry> = files
                .drain(..)
                .map(|f| FileEntry {
                    name: f.0,
                    modified_time: f.1,
                    ..Default::default()
                })
                .collect();

            // Validate that all intermediate directories for each file are accessible
            if let Err(e) = validate_write_paths(&canonical_base, &file_entries) {
                log::warn!("Write path validation failed for {}: {}", path, e);
                send_raw(fs::new_error(id, e, file_num), tx);
                return;
            }

            // cm has no show_hidden context
            // dummy remote, show_hidden, is_remote
            let mut job = fs::TransferJob::new_write(
                id,
                fs::JobType::Generic,
                "".to_string(),
                fs::DataSource::FilePath(PathBuf::from(&path)),
                file_num,
                false,
                false,
                file_entries,
                overwrite_detection,
            );
            job.total_size = total_size;
            job.conn_id = conn_id;
            write_jobs.push(job);
        }
        ipc::FS::CancelWrite { id } => {
            if let Some(job) = fs::remove_job(id, write_jobs) {
                job.remove_download_file();
                tx_log.map(|tx: &UnboundedSender<String>| {
                    tx.send(serialize_transfer_job(&job, false, true, ""))
                });
            }
        }
        ipc::FS::WriteDone { id, file_num } => {
            if let Some(job) = fs::remove_job(id, write_jobs) {
                job.modify_time();
                send_raw(fs::new_done(id, file_num), tx);
                tx_log.map(|tx| tx.send(serialize_transfer_job(&job, true, false, "")));
            }
        }
        ipc::FS::WriteError { id, file_num, err } => {
            if let Some(job) = fs::remove_job(id, write_jobs) {
                tx_log.map(|tx| tx.send(serialize_transfer_job(&job, false, false, &err)));
                send_raw(fs::new_error(job.id(), err, file_num), tx);
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
                    .write(FileTransferBlock {
                        id,
                        file_num,
                        data,
                        compressed,
                        ..Default::default()
                    })
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
            is_resume,
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
                    if let fs::DataSource::FilePath(p) = &job.data_source {
                        let path = get_string(&fs::TransferJob::join(p, &file.name));
                        match is_write_need_confirmation(is_resume, &path, &digest) {
                            Ok(digest_result) => {
                                job.set_digest(file_size, last_modified);
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
        }
        ipc::FS::SendConfirm(bytes) => {
            if let Ok(r) = FileTransferSendConfirmRequest::parse_from_bytes(&bytes) {
                if let Some(job) = fs::get_job(r.id, write_jobs) {
                    job.confirm(&r).await;
                }
            }
        }
        ipc::FS::Rename { id, path, new_name } => {
            rename_file(path, new_name, id, tx).await;
        }
        ipc::FS::ValidateReadAccess {
            path,
            id,
            file_num,
            include_hidden,
            conn_id,
        } => {
            validate_read_access(path, file_num, include_hidden, id, conn_id, tx).await;
        }
        ipc::FS::ReadAllFiles {
            path,
            id,
            include_hidden,
            conn_id,
        } => {
            read_all_files(path, include_hidden, id, conn_id, tx).await;
        }
        _ => {}
    }
}

fn compute_hash(path: &Path) -> ResultType<Option<String>> {
    if !Config::get_bool_option(OPTION_ENABLE_FILE_TRANSFER_HASH_VALIDATION) {
        // Verify read access; close immediately.
        let _ = std::fs::File::open(path)?;
        return Ok(None);
    }
    let mut file = std::fs::File::open(path)?;
    fs::compute_file_hash_sync(&mut file, Some(fs::MAX_HASH_BYTES))
}

// Although the following function is typically only needed on Windows,
// we include it for other platforms (except iOS) as well to maintain consistency,
// and it does not add significant overhead.
//
/// Validates that all parent directories of the given path are accessible (readable).
/// This prevents bypassing directory-level access restrictions by directly accessing
/// child files/directories using their full paths.
///
/// On Windows, denying access to a directory only prevents listing its contents,
/// but child items can still be accessed if their full paths are known.
/// This function ensures that if any parent directory is inaccessible,
/// the entire path is considered inaccessible.
///
/// NOTE: Android uses a different permission model (scoped storage / SAF) where
/// parent directories like `/storage` or `/storage/emulated` may not be
/// listable even though a child such as `/storage/emulated/0` is fully
/// accessible to the app. Enforcing directory-list permissions on all
/// ancestors would therefore break legitimate access on Android.
/// For this reason we only apply the strict ancestor checks on desktop
/// platforms and treat Android as a special case.
#[cfg(all(not(target_os = "ios"), not(target_os = "android")))]
fn check_parent_directories_access(path: &Path) -> ResultType<()> {
    // ancestors() yields the path itself first, skip(1) to start from parent
    for ancestor in path.ancestors().skip(1) {
        // Skip empty ancestor path components
        if ancestor.as_os_str().is_empty() {
            continue;
        }

        // Check directory list permission for all ancestors including root.
        // While canonicalize() validates path reachability (Traverse permission),
        // we also enforce List permission to prevent accessing files when parent
        // directories are intentionally hidden. This implements defense-in-depth:
        // even if root directory list access is rarely restricted, checking it
        // ensures consistent security policy across all path levels.
        if ancestor.is_dir() {
            if let Err(e) = std::fs::read_dir(ancestor) {
                log::error!(
                    "access denied to parent directory '{}': {}",
                    ancestor.display(),
                    e
                );
                bail!("access denied: insufficient permissions to access path");
            }
        }
    }
    Ok(())
}

/// Android: do not enforce ancestor listability checks.
///
/// On Android, storage access typically goes through scoped storage/S-A-F.
/// It is common that parent directories are not listable while a specific
/// subtree (e.g. `/storage/emulated/0`) remains readable. Requiring all
/// ancestors to be listable would cause legitimate paths to be rejected and
/// break file transfer browsing.
#[cfg(target_os = "android")]
fn check_parent_directories_access(_path: &Path) -> ResultType<()> {
    Ok(())
}

/// Validates and canonicalizes a path, checking parent directory access.
/// It validates the path itself exists (via canonicalize) and
/// that all parent directories are accessible.
/// This is the main helper function to ensure a path is accessible before operations.
#[inline]
#[cfg(not(any(target_os = "ios")))]
fn validate_and_canonicalize(path: &Path) -> ResultType<PathBuf> {
    let canonical = path.canonicalize()?;
    check_parent_directories_access(&canonical)?;
    Ok(canonical)
}

/// Validates parent directory access and canonicalizes the parent path.
/// Used for operations like create_dir where the target itself doesn't exist yet.
#[inline]
#[cfg(not(any(target_os = "ios")))]
fn validate_parent_and_canonicalize(path: &Path) -> ResultType<PathBuf> {
    let parent = match path.parent() {
        Some(p) => p,
        None => {
            bail!("invalid path: no parent directory");
        }
    };
    let canonical = parent.canonicalize()?;
    check_parent_directories_access(&canonical)?;
    Ok(canonical)
}

/// Validates parent directory access without canonicalizing the target path.
/// This is used for operations like remove_file and remove_dir where we need to
/// preserve symlink semantics (remove the link itself, not its target).
#[inline]
#[cfg(not(any(target_os = "ios")))]
fn validate_parent_access(path: &Path) -> ResultType<()> {
    let parent = match path.parent() {
        Some(p) => p,
        None => {
            bail!("invalid path: no parent directory");
        }
    };
    // Canonicalize and validate the parent directory to ensure it's accessible
    let canonical_parent = parent.canonicalize()?;
    check_parent_directories_access(&canonical_parent)?;
    Ok(())
}

/// Validates that a file name does not contain path traversal sequences.
/// This prevents attackers from escaping the base directory by using names like
/// "../../../etc/passwd" or "..\\..\\Windows\\System32\\malicious.dll".
#[cfg(not(any(target_os = "ios")))]
fn validate_file_name_no_traversal(name: &str) -> ResultType<()> {
    // Check for null bytes which could cause path truncation in some APIs
    if name.bytes().any(|b| b == 0) {
        bail!("null bytes not allowed in file name");
    }

    // Check for path traversal patterns
    // We check for both Unix and Windows path separators
    let components: Vec<&str> = name
        .split(|c| c == '/' || c == '\\')
        .filter(|s| !s.is_empty())
        .collect();

    for component in &components {
        if *component == ".." {
            bail!("path traversal detected in file name");
        }
    }

    // On Windows, also check for drive letters (e.g., "C:")
    #[cfg(windows)]
    {
        if name.len() >= 2 {
            let bytes = name.as_bytes();
            if bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
                bail!("absolute path detected in file name");
            }
        }
    }

    // Check for names starting with path separator (absolute paths on Unix)
    if name.starts_with('/') || name.starts_with('\\') {
        bail!("absolute path detected in file name");
    }

    Ok(())
}

#[inline]
fn is_single_file_with_empty_name(files: &[(String, u64)]) -> bool {
    files.len() == 1 && files[0].0.is_empty()
}

/// Validates all file names in a transfer request to prevent path traversal attacks.
/// Returns an error if any file name contains dangerous path components.
#[cfg(not(any(target_os = "ios")))]
fn validate_transfer_file_names(files: &[(String, u64)]) -> ResultType<()> {
    if is_single_file_with_empty_name(files) {
        // Allow empty name for single file.
        // The full path is provided in the `path` parameter for single file transfers.
        return Ok(());
    }

    for (name, _) in files {
        validate_file_name_no_traversal(name)?;
    }
    Ok(())
}

/// Validates that all files in a write operation have accessible parent directories.
/// This prevents writing files to directories where intermediate paths are restricted.
/// Walks upward from each file path to find the first existing directory and validates
/// its accessibility. This allows write operations into directories that don't exist yet.
#[cfg(not(any(target_os = "ios")))]
fn validate_write_paths(canonical_base: &PathBuf, files: &[FileEntry]) -> ResultType<()> {
    for file in files {
        if file.name.is_empty() {
            continue;
        }

        // Start from the full file path and walk upwards to find the first
        // existing directory. For example, given "base/dir1/dir2/file.txt":
        //   1. Check base/dir1/dir2/file.txt (target file, won't exist yet)
        //   2. Check base/dir1/dir2 (parent directory)
        //   3. Check base/dir1 (grandparent directory)
        //   4. Check base (great-grandparent directory)
        // Stop at the first existing directory and validate it.
        let full_path = canonical_base.join(&file.name);
        let mut current = Some(full_path.as_path());

        while let Some(path) = current {
            if path.exists() {
                // Found the first existing ancestor. Validate it with
                // validate_and_canonicalize (which checks the entire parent
                // chain). Return a generic error to avoid leaking paths.
                if let Err(e) = validate_and_canonicalize(path) {
                    log::debug!("validate_and_canonicalize failed: {:?}", e);
                    bail!("access denied");
                }
                break;
            }

            current = path.parent();
        }
    }

    Ok(())
}

#[cfg(not(any(target_os = "ios")))]
async fn validate_read_access(
    path: String,
    file_num: i32,
    include_hidden: bool,
    id: i32,
    conn_id: i32,
    tx: &UnboundedSender<Data>,
) {
    let result = spawn_blocking(move || {
        let path_obj = Path::new(&path);

        match validate_and_canonicalize(&path_obj) {
            Ok(canonical_path) => {
                let canonical_str = canonical_path.to_string_lossy().to_string();

                if canonical_path.is_file() {
                    match std::fs::metadata(&canonical_path) {
                        Ok(meta) => {
                            let size = meta.len();
                            let modified_time = meta
                                .modified()
                                .ok()
                                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                .map(|d| d.as_secs())
                                .unwrap_or(0);

                            let hash = compute_hash(&canonical_path)?;

                            let file_entry = ipc::ValidatedFile {
                                name: String::new(),
                                size,
                                modified_time,
                                hash,
                            };
                            Ok((path, vec![file_entry]))
                        }
                        Err(e) => {
                            log::error!("stat file failed: {}", e);
                            bail!("stat file failed")
                        }
                    }
                } else if canonical_path.is_dir() {
                    match fs::get_recursive_files(&canonical_str, include_hidden) {
                        Ok(files) => {
                            // Check file count limit to prevent excessive I/O
                            if let Err(msg) = check_file_count_limit(files.len()) {
                                bail!(msg);
                            }

                            let mut validated_files = Vec::with_capacity(files.len());
                            for f in files {
                                let full_path = canonical_path.join(&f.name);

                                // Validate parent directory access for files in subdirectories
                                if Path::new(&f.name).components().count() > 1 {
                                    if let Some(parent) = full_path.parent() {
                                        if let Err(e) = validate_and_canonicalize(parent) {
                                            log::error!(
                                                "access denied to parent of '{}': {}",
                                                f.name,
                                                e
                                            );
                                            bail!("access denied");
                                        }
                                    }
                                }

                                // Check file accessibility before computing hash
                                if let Err(e) = std::fs::metadata(&full_path) {
                                    log::error!("access denied to '{}': {}", f.name, e);
                                    bail!("access denied");
                                }
                                let hash = compute_hash(&full_path)?;
                                validated_files.push(ipc::ValidatedFile {
                                    name: f.name,
                                    size: f.size,
                                    modified_time: f.modified_time,
                                    hash,
                                });
                            }
                            Ok((path, validated_files))
                        }
                        Err(e) => {
                            log::error!(
                                "list directory failed, dir: {}, error: {}",
                                canonical_str,
                                e
                            );
                            bail!("list directory failed")
                        }
                    }
                } else {
                    log::error!("path is neither file nor directory: {}", canonical_str);
                    bail!("invalid path")
                }
            }
            Err(e) => {
                log::error!("canonicalize failed: {}", e);
                bail!("Failed to validate access");
            }
        }
    })
    .await;

    let result = match result {
        Ok(Ok((path, files))) => Ok((path, files)),
        Ok(Err(e)) => Err(format!("validation failed: {}", e)),
        Err(e) => Err(format!("validation task failed: {}", e)),
    };

    let _ = tx.send(Data::ReadAccessValidated {
        id,
        file_num,
        include_hidden,
        conn_id,
        result,
    });
}

#[cfg(not(any(target_os = "ios")))]
async fn read_all_files(
    path: String,
    include_hidden: bool,
    id: i32,
    conn_id: i32,
    tx: &UnboundedSender<Data>,
) {
    let path_clone = path.clone();
    let result = spawn_blocking(move || {
        let path_obj = Path::new(&path);

        // Canonicalize the path so that both ACL checks and recursive listing
        // operate on the same, fully-resolved filesystem path.
        match validate_and_canonicalize(&path_obj) {
            Ok(canonical_path) => {
                let canonical_str = canonical_path.to_string_lossy().to_string();
                fs::get_recursive_files(&canonical_str, include_hidden)
            }
            Err(e) => {
                log::error!("canonicalize failed: {}", e);
                bail!("Failed to read files");
            }
        }
    })
    .await;

    let result = match result {
        Ok(Ok(files)) => {
            // Check file count limit to prevent excessive I/O and resource usage
            if let Err(msg) = check_file_count_limit(files.len()) {
                Err(msg)
            } else {
                // Serialize FileDirectory to protobuf bytes
                let mut fd = FileDirectory::new();
                fd.id = id;
                fd.path = path_clone.clone();
                fd.entries = files.into();
                match fd.write_to_bytes() {
                    Ok(bytes) => Ok(bytes),
                    Err(e) => Err(format!("serialize failed: {}", e)),
                }
            }
        }
        Ok(Err(e)) => Err(format!("{}", e)),
        Err(e) => Err(format!("task failed: {}", e)),
    };

    let _ = tx.send(Data::AllFilesResult {
        id,
        conn_id,
        path: path_clone,
        result,
    });
}

#[cfg(not(any(target_os = "ios")))]
async fn read_empty_dirs(dir: &str, include_hidden: bool, tx: &UnboundedSender<Data>) {
    let path = dir.to_owned();
    let original_path = dir.to_owned();

    let result = spawn_blocking(
        move || -> ResultType<(String, String, Vec<FileDirectory>)> {
            let path_obj = Path::new(&path);
            let canonical = validate_and_canonicalize(&path_obj)?;
            let canonical_str = canonical.to_string_lossy().to_string();
            let fds = fs::get_empty_dirs_recursive(&canonical_str, include_hidden)?;
            Ok((path, canonical_str, fds))
        },
    )
    .await;

    match result {
        Ok(Ok((original_path, canonical_str, mut fds))) => {
            // Convert canonical paths back to original path base
            // e.g., "\\?\C:\Users\test\a\b" -> "C:\Users\test\Desktop\test copy\a\b"
            // by replacing the canonical prefix with the original path
            for fd in fds.iter_mut() {
                let fd_path_obj = std::path::Path::new(&fd.path);
                let canonical_path_obj = std::path::Path::new(&canonical_str);
                match fd_path_obj.strip_prefix(canonical_path_obj) {
                    Ok(relative_path) => {
                        if relative_path.as_os_str().is_empty() {
                            fd.path = original_path.clone();
                        } else {
                            fd.path = std::path::Path::new(&original_path)
                                .join(relative_path)
                                .to_string_lossy()
                                .to_string();
                        }
                    }
                    Err(_) => {
                        // This should not happen in normal cases. Log a warning and keep the path as-is.
                        log::warn!(
                            "Path '{}' does not start with canonical '{}', keeping as-is",
                            fd.path,
                            canonical_str
                        );
                    }
                }
            }
            let mut msg_out = Message::new();
            let mut file_response = FileResponse::new();
            file_response.set_empty_dirs(ReadEmptyDirsResponse {
                path: original_path,
                empty_dirs: fds,
                ..Default::default()
            });
            msg_out.set_file_response(file_response);
            send_raw(msg_out, tx);
        }
        Ok(Err(e)) => {
            log::error!("read_empty_dirs failed for '{}': {}", original_path, e);
            send_raw(fs::new_error(0, "Failed to read empty dirs", 0), tx);
        }
        Err(e) => {
            log::error!("read_empty_dirs task failed for '{}': {}", original_path, e);
            send_raw(fs::new_error(0, "Failed to read empty dirs", 0), tx);
        }
    }
}

#[cfg(not(any(target_os = "ios")))]
async fn read_dir(dir: &str, include_hidden: bool, tx: &UnboundedSender<Data>) {
    // See fs::read_dir for root handling on Windows
    let is_windows_root = cfg!(windows) && dir == "/";
    let path = {
        if dir.is_empty() {
            Config::get_home()
        } else {
            fs::get_path(dir)
        }
    };
    let original_path = get_string(&path);
    let result = spawn_blocking(move || {
        let canonical = if is_windows_root {
            path
        } else {
            validate_and_canonicalize(&path)?
        };
        fs::read_dir(&canonical, include_hidden)
    })
    .await;

    match result {
        Ok(Ok(mut fd)) => {
            fd.path = original_path;
            let mut msg_out = Message::new();
            let mut file_response = FileResponse::new();
            file_response.set_dir(fd);
            msg_out.set_file_response(file_response);
            send_raw(msg_out, tx);
        }
        Ok(Err(e)) => {
            log::error!("read_dir failed for '{}': {}", dir, e);
            send_raw(fs::new_error(0, "Failed to read dir", 0), tx);
        }
        Err(e) => {
            log::error!("read_dir task failed for '{}': {}", dir, e);
            send_raw(fs::new_error(0, "Failed to read dir", 0), tx);
        }
    }
}

#[cfg(not(any(target_os = "ios")))]
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

#[cfg(not(any(target_os = "ios")))]
async fn remove_file(path: String, id: i32, file_num: i32, tx: &UnboundedSender<Data>) {
    handle_result(
        spawn_blocking(move || {
            let path_obj = Path::new(&path);
            // Validate parent access without canonicalizing to preserve symlink semantics
            validate_parent_access(&path_obj)?;
            fs::remove_file(&path)
        })
        .await,
        id,
        file_num,
        tx,
    )
    .await;
}

#[cfg(not(any(target_os = "ios")))]
async fn create_dir(path: String, id: i32, tx: &UnboundedSender<Data>) {
    handle_result(
        spawn_blocking(move || {
            let path_obj = Path::new(&path);
            // For create_dir, check parent of the new directory.
            // The canonicalized path is not needed; call for validation side effect only.
            let _ = validate_parent_and_canonicalize(&path_obj)?;
            fs::create_dir(&path)
        })
        .await,
        id,
        0,
        tx,
    )
    .await;
}

#[cfg(not(any(target_os = "ios")))]
async fn rename_file(path: String, new_name: String, id: i32, tx: &UnboundedSender<Data>) {
    handle_result(
        spawn_blocking(move || {
            // Validate that new_name doesn't contain path traversal
            validate_file_name_no_traversal(&new_name)?;

            // validate_and_canonicalize ensures the source path is accessible
            // validate_file_name_no_traversal already ensures new_name has no path traversal
            let _ = validate_and_canonicalize(&Path::new(&path))?;

            fs::rename_file(&path, &new_name)
        })
        .await,
        id,
        0,
        tx,
    )
    .await;
}

#[cfg(not(any(target_os = "ios")))]
async fn remove_dir(path: String, id: i32, recursive: bool, tx: &UnboundedSender<Data>) {
    let path = fs::get_path(&path);
    handle_result(
        spawn_blocking(move || {
            let path_obj = Path::new(&path);
            // Validate parent access without canonicalizing to preserve symlink semantics
            validate_parent_access(&path_obj)?;
            if recursive {
                fs::remove_all_empty_dir(&path_obj)
            } else {
                std::fs::remove_dir(&path_obj).map_err(|err| err.into())
            }
        })
        .await,
        id,
        0,
        tx,
    )
    .await;
}

#[cfg(not(any(target_os = "ios")))]
fn send_raw(msg: Message, tx: &UnboundedSender<Data>) {
    match msg.write_to_bytes() {
        Ok(bytes) => {
            allow_err!(tx.send(Data::RawMessage(bytes)));
        }
        err => allow_err!(err),
    }
}

#[cfg(windows)]
fn cm_inner_send(id: i32, data: Data) {
    let lock = CLIENTS.read().unwrap();
    if id != 0 {
        if let Some(s) = lock.get(&id) {
            allow_err!(s.tx.send(data));
        }
    } else {
        for s in lock.values() {
            allow_err!(s.tx.send(data.clone()));
        }
    }
}

pub fn can_elevate() -> bool {
    #[cfg(windows)]
    return !crate::platform::is_installed();
    #[cfg(not(windows))]
    return false;
}

pub fn elevate_portable(_id: i32) {
    #[cfg(windows)]
    {
        let lock = CLIENTS.read().unwrap();
        if let Some(s) = lock.get(&_id) {
            allow_err!(s.tx.send(ipc::Data::DataPortableService(
                ipc::DataPortableService::RequestStart
            )));
        }
    }
}

#[cfg(any(target_os = "android", target_os = "ios", feature = "flutter"))]
#[inline]
pub fn handle_incoming_voice_call(id: i32, accept: bool) {
    if let Some(client) = CLIENTS.read().unwrap().get(&id) {
        // Not handled in iOS yet.
        #[cfg(not(any(target_os = "ios")))]
        allow_err!(client.tx.send(Data::VoiceCallResponse(accept)));
    };
}

#[cfg(any(target_os = "android", target_os = "ios", feature = "flutter"))]
#[inline]
pub fn close_voice_call(id: i32) {
    if let Some(client) = CLIENTS.read().unwrap().get(&id) {
        // Not handled in iOS yet.
        #[cfg(not(any(target_os = "ios")))]
        allow_err!(client.tx.send(Data::CloseVoiceCall("".to_owned())));
    };
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn quit_cm() {
    // in case of std::process::exit not work
    log::info!("quit cm");
    CLIENTS.write().unwrap().clear();
    crate::platform::quit_gui();
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::ipc::Data;
    use hbb_common::{
        message_proto::{FileDirectory, Message},
        tokio::{runtime::Runtime, sync::mpsc::unbounded_channel},
    };
    use std::fs;

    #[cfg(windows)]
    fn get_username() -> Option<String> {
        std::env::var("USERNAME").ok().filter(|u| !u.is_empty())
    }

    #[cfg(windows)]
    fn setup_acl_deny(path: &std::path::Path, username: &str) -> bool {
        use std::process::Command;
        Command::new("icacls")
            .arg(path)
            .arg("/deny")
            .arg(format!("{}:(RX)", username))
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    #[cfg(windows)]
    fn cleanup_acl_deny(path: &std::path::Path, username: &str) {
        use std::process::Command;
        let _ = Command::new("icacls")
            .arg(path)
            .arg("/remove:d")
            .arg(username)
            .status();
    }

    fn make_file_entry(name: &str) -> FileEntry {
        FileEntry {
            name: name.to_string(),
            entry_type: FileType::File.into(),
            size: 100,
            modified_time: 0,
            ..Default::default()
        }
    }

    /// Test validate_and_canonicalize fails when parent directory ACL denies access.
    #[test]
    #[cfg(windows)]
    fn validate_and_canonicalize_acl_denied() {
        use std::process::Command;

        if Command::new("icacls").arg("/?").output().is_err() {
            return;
        }

        let username = match get_username() {
            Some(u) => u,
            None => return,
        };

        let base_dir = std::env::temp_dir().join("rustdesk_acl_test");
        let denied_dir = base_dir.join("denied_dir");
        let file_path = denied_dir.join("file.txt");

        let _ = fs::remove_dir_all(&base_dir);
        fs::create_dir_all(&denied_dir).unwrap();
        fs::write(&file_path, b"test").unwrap();

        if !setup_acl_deny(&denied_dir, &username) {
            let _ = fs::remove_dir_all(&base_dir);
            return;
        }

        // Check opening the file directly still works (it should).
        let direct_open = fs::File::open(&file_path);
        assert!(
            direct_open.is_ok(),
            "expected direct file open to succeed despite parent ACL"
        );

        // Now any attempt to access a child under `denied_dir` via directory
        // listing should fail, and validate_and_canonicalize should surface
        // an error for the file path.
        let res = super::validate_and_canonicalize(&file_path);
        assert!(
            res.is_err(),
            "expected ACL validation failure for {:?}",
            file_path
        );

        cleanup_acl_deny(&denied_dir, &username);
        let _ = fs::remove_dir_all(&base_dir);
    }

    #[test]
    #[cfg(not(any(target_os = "ios")))]
    fn read_all_files_success() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let (tx, mut rx) = unbounded_channel();
            let dir = std::env::temp_dir().join("rustdesk_read_all_test");
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("test.txt"), b"hello").unwrap();

            let path_str = dir.to_string_lossy().to_string();
            super::read_all_files(path_str.clone(), false, 1, 2, &tx).await;

            match rx.recv().await.unwrap() {
                Data::AllFilesResult { result, .. } => {
                    let bytes = result.unwrap();
                    let fd = FileDirectory::parse_from_bytes(&bytes).unwrap();
                    assert!(!fd.entries.is_empty());
                }
                _ => panic!("unexpected data"),
            }
            let _ = fs::remove_dir_all(&dir);
        });
    }

    #[test]
    #[cfg(not(any(target_os = "ios")))]
    fn read_dir_success() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let (tx, mut rx) = unbounded_channel();
            let dir = std::env::temp_dir().join("rustdesk_read_dir_test");
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();

            super::read_dir(&dir.to_string_lossy(), false, &tx).await;

            match rx.recv().await.unwrap() {
                Data::RawMessage(bytes) => {
                    let mut msg = Message::new();
                    msg.merge_from_bytes(&bytes).unwrap();
                    assert!(msg
                        .file_response()
                        .dir()
                        .path
                        .contains("rustdesk_read_dir_test"));
                }
                _ => panic!("unexpected data"),
            }
            let _ = fs::remove_dir_all(&dir);
        });
    }

    #[test]
    #[cfg(not(any(target_os = "ios")))]
    fn validate_file_name_security() {
        // Null byte injection
        assert!(super::validate_file_name_no_traversal("file\0.txt").is_err());
        assert!(super::validate_file_name_no_traversal("test\0").is_err());

        // Path traversal
        assert!(super::validate_file_name_no_traversal("../etc/passwd").is_err());
        assert!(super::validate_file_name_no_traversal("foo/../bar").is_err());
        assert!(super::validate_file_name_no_traversal("..").is_err());

        // Absolute paths
        assert!(super::validate_file_name_no_traversal("/etc/passwd").is_err());
        assert!(super::validate_file_name_no_traversal("\\Windows").is_err());
        #[cfg(windows)]
        assert!(super::validate_file_name_no_traversal("C:\\Windows").is_err());

        // Valid paths
        assert!(super::validate_file_name_no_traversal("file.txt").is_ok());
        assert!(super::validate_file_name_no_traversal("subdir/file.txt").is_ok());
        assert!(super::validate_file_name_no_traversal("").is_ok());
    }

    #[test]
    #[cfg(not(any(target_os = "ios")))]
    fn validate_transfer_file_names_security() {
        assert!(super::validate_transfer_file_names(&[("file.txt".into(), 100)]).is_ok());
        assert!(super::validate_transfer_file_names(&[("../passwd".into(), 100)]).is_err());
    }

    #[test]
    #[cfg(not(any(target_os = "ios")))]
    fn validate_write_paths_basic() {
        let base = std::env::temp_dir().join("rustdesk_write_test1");
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();

        // Simple file, nested dirs, and empty name
        let files = vec![
            make_file_entry("test.txt"),
            make_file_entry("dir1/dir2/file.txt"),
            make_file_entry(""),
        ];
        assert!(super::validate_write_paths(&base, &files).is_ok());

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    #[cfg(not(any(target_os = "ios")))]
    fn validate_write_paths_partial_hierarchy() {
        let base = std::env::temp_dir().join("rustdesk_write_test2");
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        fs::create_dir(base.join("dir1")).unwrap();

        let files = vec![make_file_entry("dir1/dir2/file.txt")];
        assert!(super::validate_write_paths(&base, &files).is_ok());

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    #[cfg(windows)]
    fn validate_write_paths_acl_denied() {
        use std::process::Command;

        if Command::new("icacls").arg("/?").output().is_err() {
            return;
        }

        let username = match get_username() {
            Some(u) => u,
            None => return,
        };

        let base = std::env::temp_dir().join("rustdesk_write_acl_test");
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();

        let denied = base.join("denied");
        fs::create_dir(&denied).unwrap();

        if !setup_acl_deny(&denied, &username) {
            let _ = fs::remove_dir_all(&base);
            return;
        }

        let files = vec![make_file_entry("denied/file.txt")];
        let result = super::validate_write_paths(&base, &files);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("access denied"));

        cleanup_acl_deny(&denied, &username);
        let _ = fs::remove_dir_all(&base);
    }

    /// Test validate_parent_access with symlinks.
    /// Verifies that validate_parent_access checks the parent of the symlink itself
    /// (not following the link), which is critical for delete operations to remove
    /// the link rather than the target. Also tests broken symlinks and contrasts
    /// with validate_and_canonicalize which does follow symlinks.
    #[test]
    #[cfg(not(any(target_os = "ios")))]
    fn validate_parent_access_symlink() {
        let base_dir = std::env::temp_dir().join("rustdesk_symlink_test");
        let _ = fs::remove_dir_all(&base_dir);
        fs::create_dir_all(&base_dir).unwrap();

        // Create target file in a subdirectory
        let target_dir = base_dir.join("target_dir");
        fs::create_dir_all(&target_dir).unwrap();
        let target_file = target_dir.join("target.txt");
        fs::write(&target_file, b"content").unwrap();

        // Create symlink in a different directory
        let link_dir = base_dir.join("link_dir");
        fs::create_dir_all(&link_dir).unwrap();
        let link_path = link_dir.join("link.txt");

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            if symlink(&target_file, &link_path).is_err() {
                let _ = fs::remove_dir_all(&base_dir);
                return;
            }
        }

        #[cfg(windows)]
        {
            use std::os::windows::fs::symlink_file;
            if symlink_file(&target_file, &link_path).is_err() {
                // Skip if no permission (needs admin or dev mode on Windows)
                let _ = fs::remove_dir_all(&base_dir);
                return;
            }
        }

        // Test 1: Contrast with validate_and_canonicalize which follows symlinks
        let canonical = super::validate_and_canonicalize(&link_path).unwrap();
        assert!(
            canonical.to_string_lossy().contains("target_dir"),
            "canonicalize should follow symlink to target"
        );

        // Test 2: Broken symlink - delete target to break the link
        //         validate_parent_access should check link_dir (symlink's parent),
        //         not target_dir (target's parent)
        fs::remove_file(&target_file).unwrap();
        let result = super::validate_parent_access(&link_path);
        assert!(
            result.is_ok(),
            "validate_parent_access should succeed even for broken symlink"
        );

        let _ = fs::remove_dir_all(&base_dir);
    }

    /// Test that Path::strip_prefix correctly handles Windows verbatim paths (\\?\).
    /// This validates the path conversion logic used in read_empty_dirs.
    #[test]
    #[cfg(windows)]
    fn test_strip_prefix_with_verbatim_path() {
        // Test basic verbatim path
        let canonical = r"\\?\C:\Users\test";
        let full_path = r"\\?\C:\Users\test\subdir";
        let relative = std::path::Path::new(full_path)
            .strip_prefix(canonical)
            .unwrap();
        assert_eq!(relative, std::path::Path::new("subdir"));

        // Test with multiple nested directories
        let canonical = r"\\?\C:\Users\test";
        let full_path = r"\\?\C:\Users\test\dir1\dir2\dir3";
        let relative = std::path::Path::new(full_path)
            .strip_prefix(canonical)
            .unwrap();
        assert_eq!(relative, std::path::Path::new(r"dir1\dir2\dir3"));
    }

    /// Test the path conversion logic used in read_empty_dirs for reconstructing
    /// original paths from canonical paths.
    #[test]
    #[cfg(windows)]
    fn test_path_reconstruction_from_canonical() {
        let original_path = r"C:\Users\test copy";
        let canonical_str = r"\\?\C:\Users\test copy";
        let fd_path = r"\\?\C:\Users\test copy\subdir\file";

        let fd_path_obj = std::path::Path::new(fd_path);
        let canonical_path_obj = std::path::Path::new(canonical_str);

        match fd_path_obj.strip_prefix(canonical_path_obj) {
            Ok(relative_path) => {
                let reconstructed = std::path::Path::new(original_path)
                    .join(relative_path)
                    .to_string_lossy()
                    .to_string();
                assert_eq!(reconstructed, r"C:\Users\test copy\subdir\file");
            }
            Err(_) => panic!("strip_prefix should succeed"),
        }
    }
}
