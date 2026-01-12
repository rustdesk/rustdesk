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
    config::{keys::OPTION_FILE_TRANSFER_MAX_FILES, Config},
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
    sync::{
        atomic::{AtomicI64, Ordering},
        RwLock,
    },
};

/// Default maximum number of files allowed per transfer request.
/// Unit: number of files (not bytes).
#[cfg(not(any(target_os = "ios")))]
const DEFAULT_MAX_VALIDATED_FILES: usize = 10_000;

/// Maximum number of files allowed in a single file transfer request.
///
/// This limit prevents excessive I/O and memory usage when dealing with
/// large directories. It applies to:
/// - CM-side read jobs (server to client file transfers on Windows)
/// - `AllFiles` recursive directory listing operations
/// - Connection-side read jobs (non-Windows platforms)
///
/// Unit: number of files (not bytes).
/// Default: 10,000 files.
/// Configured via: `OPTION_FILE_TRANSFER_MAX_FILES` ("file-transfer-max-files")
#[cfg(not(any(target_os = "ios")))]
static MAX_VALIDATED_FILES: std::sync::OnceLock<usize> = std::sync::OnceLock::new();

/// Get the maximum number of files allowed per transfer request.
///
/// Initializes the value from configuration (`OPTION_FILE_TRANSFER_MAX_FILES`)
/// on first call. Semantics:
/// - If the option is set to `0`, `DEFAULT_MAX_VALIDATED_FILES` (10,000) is used as a safe upper bound.
/// - If the option is unset, negative, or non-integer,
///   `usize::MAX` is used to represent "no limit" for backward compatibility with older versions
///   that did not enforce any file‑count restriction.
///   (Note: negative values are not valid for `usize` and will cause parsing to fail.)
///
/// Unit: number of files.
#[cfg(not(any(target_os = "ios")))]
#[inline]
pub fn get_max_validated_files() -> usize {
    // If `OPTION_FILE_TRANSFER_MAX_FILES` unset, negative, or non-integer, use
    // `usize::MAX` to represent "no limit", maintaining backward compatibility
    // with versions that had no file transfer restrictions.
    const NO_LIMIT_FILE_COUNT: usize = usize::MAX;
    *MAX_VALIDATED_FILES.get_or_init(|| {
        let c = crate::get_builtin_option(OPTION_FILE_TRANSFER_MAX_FILES)
            .trim()
            .parse::<usize>()
            .unwrap_or(NO_LIMIT_FILE_COUNT);
        if c == 0 {
            DEFAULT_MAX_VALIDATED_FILES
        } else {
            c
        }
    })
}

/// Check if file count exceeds the maximum allowed limit.
///
/// This check is enforced in:
/// - `start_read_job()` for CM-side read jobs
/// - `read_all_files()` for recursive directory listings
/// - `Connection::on_message()` for connection-side read jobs
///
/// # Arguments
/// * `file_count` - Number of files in the transfer request
///
/// # Returns
/// * `Ok(())` if within limit
/// * `Err(String)` with error message if limit exceeded
#[cfg(not(any(target_os = "ios")))]
pub fn check_file_count_limit(file_count: usize) -> Result<(), String> {
    let max_files = get_max_validated_files();
    if file_count > max_files {
        let msg = format!(
            "file transfer rejected: too many files ({} files exceeds limit of {}). \
             Adjust '{}' option to increase limit.",
            file_count, max_files, OPTION_FILE_TRANSFER_MAX_FILES
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
    /// Read jobs for CM-side file reading (server to client transfers)
    read_jobs: Vec<fs::TransferJob>,
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
        use hbb_common::tokio::time::{self, Duration, Instant};

        const MILLI5: Duration = Duration::from_millis(5);
        const SEC30: Duration = Duration::from_secs(30);

        // for tmp use, without real conn id
        let mut write_jobs: Vec<fs::TransferJob> = Vec::new();
        // File timer for processing read_jobs
        let mut file_timer =
            crate::rustdesk_interval(time::interval_at(Instant::now() + SEC30, SEC30));

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
                                            handle_fs(fs, &mut write_jobs, &mut self.read_jobs, &self.tx, Some(&tx_log), self.conn_id).await;
                                        }
                                    } else {
                                        handle_fs(fs, &mut write_jobs, &mut self.read_jobs, &self.tx, Some(&tx_log), self.conn_id).await;
                                    }
                                    // Activate fast timer immediately when read jobs exist.
                                    // This ensures new jobs start processing without waiting for the slow 30s timer.
                                    // Deactivation (back to 30s) happens in tick handler when jobs are exhausted.
                                    if !self.read_jobs.is_empty() {
                                        file_timer = crate::rustdesk_interval(time::interval(MILLI5));
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
                    // For FileBlockFromCM, data is sent separately via send_raw (data field has #[serde(skip)]).
                    // This avoids JSON encoding overhead for large binary data.
                    // This mirrors the WriteBlock pattern in start_ipc (see rx_to_cm handler).
                    //
                    // Note: Empty data (for empty files) is correctly handled. BytesCodec with raw=false
                    // (the default for IPC connections) adds a length prefix, so send_raw(Bytes::new())
                    // sends a 1-byte frame that next_raw() can correctly receive as empty data.
                    if let Data::FileBlockFromCM { id, file_num, ref data, compressed, conn_id } = data {
                        // Send metadata first (data field is skipped by serde), then raw data bytes
                        if let Err(e) = self.stream.send(&Data::FileBlockFromCM {
                            id,
                            file_num,
                            data: bytes::Bytes::new(), // placeholder, skipped by serde
                            compressed,
                            conn_id,
                        }).await {
                            log::error!("error sending FileBlockFromCM metadata: {}", e);
                            break;
                        }
                        if let Err(e) = self.stream.send_raw(data.clone()).await {
                            log::error!("error sending FileBlockFromCM data: {}", e);
                            break;
                        }
                        continue;
                    }
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
                _ = file_timer.tick() => {
                    if !self.read_jobs.is_empty() {
                        let conn_id = self.conn_id;
                        if let Err(e) = handle_read_jobs_tick(&mut self.read_jobs, &self.tx, conn_id).await {
                            log::error!("Error processing read jobs: {}", e);
                        }
                        let log = fs::serialize_transfer_jobs(&self.read_jobs);
                        self.cm.ui_handler.file_transfer_log("transfer", &log);
                    } else {
                        file_timer = crate::rustdesk_interval(time::interval_at(Instant::now() + SEC30, SEC30));
                    }
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
            read_jobs: Vec::new(),
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
    {
        let enabled = crate::Connection::is_permission_enabled_locally(OPTION_ENABLE_FILE_TRANSFER);
        let mut lock = crate::ui_interface::IS_FILE_TRANSFER_ENABLED
            .lock()
            .unwrap();
        ContextSend::enable(enabled);
        *lock = Some(enabled);
    }
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
                // Android doesn't need CM-side file reading (no need_validate_file_read_access)
                let mut read_jobs_placeholder: Vec<fs::TransferJob> = Vec::new();
                handle_fs(
                    fs,
                    &mut write_jobs,
                    &mut read_jobs_placeholder,
                    &tx,
                    None,
                    current_id,
                )
                .await;
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
    read_jobs: &mut Vec<fs::TransferJob>,
    tx: &UnboundedSender<Data>,
    tx_log: Option<&UnboundedSender<String>>,
    _conn_id: i32,
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

            // Convert files to FileEntry
            let file_entries: Vec<FileEntry> = files
                .drain(..)
                .map(|f| FileEntry {
                    name: f.0,
                    modified_time: f.1,
                    ..Default::default()
                })
                .collect();

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
                if let Some(tx) = tx_log {
                    if let Err(e) = tx.send(serialize_transfer_job(&job, false, true, "")) {
                        log::error!("error sending transfer job log via IPC: {}", e);
                    }
                }
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
        ipc::FS::ReadFile {
            path,
            id,
            file_num,
            include_hidden,
            conn_id,
            overwrite_detection,
        } => {
            start_read_job(
                path,
                file_num,
                include_hidden,
                id,
                conn_id,
                overwrite_detection,
                read_jobs,
                tx,
            )
            .await;
        }
        // Cancel an ongoing read job (file transfer from server to client).
        // Note: This only cancels jobs in `read_jobs`. It does NOT cancel `ReadAllFiles`
        // operations, which are one-shot directory scans that complete quickly and don't
        // have persistent job tracking.
        ipc::FS::CancelRead { id, conn_id: _ } => {
            if let Some(job) = fs::remove_job(id, read_jobs) {
                if let Some(tx) = tx_log {
                    if let Err(e) = tx.send(serialize_transfer_job(&job, false, true, "")) {
                        log::error!("error sending transfer job log via IPC: {}", e);
                    }
                }
            }
        }
        ipc::FS::SendConfirmForRead {
            id,
            file_num: _,
            skip,
            offset_blk,
            conn_id: _,
        } => {
            if let Some(job) = fs::get_job(id, read_jobs) {
                let req = FileTransferSendConfirmRequest {
                    id,
                    file_num: job.file_num(),
                    union: if skip {
                        Some(file_transfer_send_confirm_request::Union::Skip(true))
                    } else {
                        Some(file_transfer_send_confirm_request::Union::OffsetBlk(
                            offset_blk,
                        ))
                    },
                    ..Default::default()
                };
                job.confirm(&req).await;
            }
        }
        // Recursively list all files in a directory.
        // This is a one-shot operation that cannot be cancelled via CancelRead.
        // The operation typically completes quickly as it only reads directory metadata,
        // not file contents. File count is limited by `check_file_count_limit()`.
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

/// Validates that a file name does not contain path traversal sequences.
/// This prevents attackers from escaping the base directory by using names like
/// "../../../etc/passwd" or "..\\..\\Windows\\System32\\malicious.dll".
#[cfg(not(any(target_os = "ios")))]
fn validate_file_name_no_traversal(name: &str) -> ResultType<()> {
    // Check for null bytes which could cause path truncation in some APIs
    if name.bytes().any(|b| b == 0) {
        bail!("file name contains null bytes");
    }

    // Check for path traversal patterns
    // We check for both Unix and Windows path separators
    if name
        .split(|c| c == '/' || c == '\\')
        .filter(|s| !s.is_empty())
        .any(|component| component == "..")
    {
        bail!("path traversal detected in file name");
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

    // Check for names starting with path separator:
    // - Unix absolute paths (e.g., "/etc/passwd")
    // - Windows UNC paths (e.g., "\\server\share")
    if name.starts_with('/') || name.starts_with('\\') {
        bail!("absolute path detected in file name");
    }

    Ok(())
}

#[inline]
fn is_single_file_with_empty_name(files: &[(String, u64)]) -> bool {
    files.len() == 1 && files.first().map_or(false, |f| f.0.is_empty())
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
        // In multi-file transfers, empty names are not allowed.
        // Each file must have a valid name to construct the destination path.
        if name.is_empty() {
            bail!("empty file name in multi-file transfer");
        }
        validate_file_name_no_traversal(name)?;
    }
    Ok(())
}

/// Start a read job in CM for file transfer from server to client (Windows only).
///
/// This creates a `TransferJob` using `new_read()`, validates it, and sends the
/// initial file list back to Connection via IPC.
///
/// NOTE: This is the CM-side equivalent of `create_and_start_read_job()` in
/// `src/server/connection.rs`. On non-Windows platforms, Connection handles
/// read jobs directly. Both use `TransferJob::new_read()` with similar logic.
/// When modifying job creation or validation, ensure both paths stay in sync.
#[cfg(not(any(target_os = "ios")))]
async fn start_read_job(
    path: String,
    file_num: i32,
    include_hidden: bool,
    id: i32,
    conn_id: i32,
    overwrite_detection: bool,
    read_jobs: &mut Vec<fs::TransferJob>,
    tx: &UnboundedSender<Data>,
) {
    let path_clone = path.clone();
    let result = spawn_blocking(move || -> ResultType<fs::TransferJob> {
        let data_source = fs::DataSource::FilePath(PathBuf::from(&path));
        fs::TransferJob::new_read(
            id,
            fs::JobType::Generic,
            "".to_string(),
            data_source,
            file_num,
            include_hidden,
            true,
            overwrite_detection,
        )
    })
    .await;

    match result {
        Ok(Ok(mut job)) => {
            // Optional: enforce file count limit for CM-side jobs to avoid
            // excessive I/O. This is applied on the job's file list produced
            // by `new_read`, similar to how AllFiles uses the same helper.
            if let Err(msg) = check_file_count_limit(job.files().len()) {
                if let Err(e) = tx.send(Data::ReadJobInitResult {
                    id,
                    file_num,
                    include_hidden,
                    conn_id,
                    result: Err(msg),
                }) {
                    log::error!("error sending ReadJobInitResult via IPC: {}", e);
                }
                return;
            }

            // Build FileDirectory from the job's file list and serialize
            let files = job.files().to_owned();
            let mut dir = FileDirectory::new();
            dir.id = id;
            dir.path = path_clone.clone();
            dir.entries = files.clone().into();

            let dir_bytes = match dir.write_to_bytes() {
                Ok(bytes) => bytes,
                Err(e) => {
                    if let Err(e) = tx.send(Data::ReadJobInitResult {
                        id,
                        file_num,
                        include_hidden,
                        conn_id,
                        result: Err(format!("serialize failed: {}", e)),
                    }) {
                        log::error!("error sending ReadJobInitResult via IPC: {}", e);
                    }
                    return;
                }
            };

            if let Err(e) = tx.send(Data::ReadJobInitResult {
                id,
                file_num,
                include_hidden,
                conn_id,
                result: Ok(dir_bytes),
            }) {
                log::error!("error sending ReadJobInitResult via IPC: {}", e);
            }

            // Attach connection id so CM can route read blocks back correctly
            job.conn_id = conn_id;
            read_jobs.push(job);
        }
        Ok(Err(e)) => {
            if let Err(e) = tx.send(Data::ReadJobInitResult {
                id,
                file_num,
                include_hidden,
                conn_id,
                result: Err(format!("validation failed: {}", e)),
            }) {
                log::error!("error sending ReadJobInitResult via IPC: {}", e);
            }
        }
        Err(e) => {
            if let Err(e) = tx.send(Data::ReadJobInitResult {
                id,
                file_num,
                include_hidden,
                conn_id,
                result: Err(format!("validation task failed: {}", e)),
            }) {
                log::error!("error sending ReadJobInitResult via IPC: {}", e);
            }
        }
    }
}

/// Process read jobs periodically, reading file blocks and sending them via IPC.
///
/// NOTE: This is the CM-side equivalent of `handle_read_jobs()` in
/// `libs/hbb_common/src/fs.rs`. The logic mirrors that implementation
/// but communicates via IPC instead of direct network stream.
/// When modifying job processing logic, ensure both implementations stay in sync.
#[cfg(not(any(target_os = "ios")))]
async fn handle_read_jobs_tick(
    jobs: &mut Vec<fs::TransferJob>,
    tx: &UnboundedSender<Data>,
    conn_id: i32,
) -> ResultType<()> {
    let mut finished = Vec::new();

    for job in jobs.iter_mut() {
        if job.is_last_job {
            continue;
        }

        // Initialize data stream if needed (opens file, sends digest for overwrite detection)
        if let Err(err) = init_read_job_for_cm(job, tx, conn_id).await {
            if let Err(e) = tx.send(Data::FileReadError {
                id: job.id,
                file_num: job.file_num(),
                err: format!("{}", err),
                conn_id,
            }) {
                log::error!("error sending FileReadError via IPC: {}", e);
            }
            finished.push(job.id);
            continue;
        }

        // Read a block from the file
        match job.read().await {
            Err(err) => {
                if let Err(e) = tx.send(Data::FileReadError {
                    id: job.id,
                    file_num: job.file_num(),
                    err: format!("{}", err),
                    conn_id,
                }) {
                    log::error!("error sending FileReadError via IPC: {}", e);
                }
                // Mark job as finished to prevent infinite retries.
                // Connection side will have already removed cm_read_job_ids
                // after receiving FileReadError, so continuing would be pointless.
                finished.push(job.id);
            }
            Ok(Some(block)) => {
                if let Err(e) = tx.send(Data::FileBlockFromCM {
                    id: block.id,
                    file_num: block.file_num,
                    data: block.data,
                    compressed: block.compressed,
                    conn_id,
                }) {
                    log::error!("error sending FileBlockFromCM via IPC: {}", e);
                }
            }
            Ok(None) => {
                if job.job_completed() {
                    finished.push(job.id);
                    match job.job_error() {
                        Some(err) => {
                            if let Err(e) = tx.send(Data::FileReadError {
                                id: job.id,
                                file_num: job.file_num(),
                                err,
                                conn_id,
                            }) {
                                log::error!("error sending FileReadError via IPC: {}", e);
                            }
                        }
                        None => {
                            if let Err(e) = tx.send(Data::FileReadDone {
                                id: job.id,
                                file_num: job.file_num(),
                                conn_id,
                            }) {
                                log::error!("error sending FileReadDone via IPC: {}", e);
                            }
                        }
                    }
                }
                // else: waiting for confirmation from peer
            }
        }
        // Break to handle jobs one by one.
        break;
    }

    for id in finished {
        let _ = fs::remove_job(id, jobs);
    }

    Ok(())
}

/// Initialize a read job's data stream and handle digest sending for overwrite detection.
///
/// NOTE: This is the CM-side equivalent of `TransferJob::init_data_stream()` in
/// `libs/hbb_common/src/fs.rs`. It calls `init_data_stream_for_cm()` and sends
/// digest via IPC instead of direct network stream.
/// When modifying initialization or digest logic, ensure both paths stay in sync.
#[cfg(not(any(target_os = "ios")))]
async fn init_read_job_for_cm(
    job: &mut fs::TransferJob,
    tx: &UnboundedSender<Data>,
    conn_id: i32,
) -> ResultType<()> {
    // Initialize data stream and get digest info if overwrite detection is needed
    match job.init_data_stream_for_cm().await? {
        Some((last_modified, file_size)) => {
            // Send digest via IPC for overwrite detection
            if let Err(e) = tx.send(Data::FileDigestFromCM {
                id: job.id,
                file_num: job.file_num(),
                last_modified,
                file_size,
                is_resume: job.is_resume,
                conn_id,
            }) {
                log::error!("error sending FileDigestFromCM via IPC: {}", e);
            }
        }
        None => {
            // Job done or already initialized, nothing to do
        }
    }
    Ok(())
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
    let result = spawn_blocking(move || fs::get_recursive_files(&path, include_hidden)).await;

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

    if let Err(e) = tx.send(Data::AllFilesResult {
        id,
        conn_id,
        path: path_clone,
        result,
    }) {
        log::error!("error sending AllFilesResult via IPC: {}", e);
    }
}

#[cfg(not(any(target_os = "ios")))]
async fn read_empty_dirs(dir: &str, include_hidden: bool, tx: &UnboundedSender<Data>) {
    let path = dir.to_owned();
    let path_clone = dir.to_owned();

    if let Ok(Ok(fds)) =
        spawn_blocking(move || fs::get_empty_dirs_recursive(&path, include_hidden)).await
    {
        let mut msg_out = Message::new();
        let mut file_response = FileResponse::new();
        file_response.set_empty_dirs(ReadEmptyDirsResponse {
            path: path_clone,
            empty_dirs: fds,
            ..Default::default()
        });
        msg_out.set_file_response(file_response);
        send_raw(msg_out, tx);
    }
}

#[cfg(not(any(target_os = "ios")))]
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
        spawn_blocking(move || fs::remove_file(&path)).await,
        id,
        file_num,
        tx,
    )
    .await;
}

#[cfg(not(any(target_os = "ios")))]
async fn create_dir(path: String, id: i32, tx: &UnboundedSender<Data>) {
    handle_result(
        spawn_blocking(move || fs::create_dir(&path)).await,
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
            // Rename target must not be empty
            if new_name.is_empty() {
                bail!("new file name cannot be empty");
            }
            // Validate that new_name doesn't contain path traversal
            validate_file_name_no_traversal(&new_name)?;
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
        assert!(super::validate_transfer_file_names(&[("".into(), 100)]).is_ok());
        assert!(
            super::validate_transfer_file_names(&[("".into(), 100), ("file.txt".into(), 100)])
                .is_err()
        );
        assert!(super::validate_transfer_file_names(&[("../passwd".into(), 100)]).is_err());
    }

    /// Tests that symlink creation works on this platform.
    /// This is a helper to verify the test environment supports symlinks.
    #[test]
    #[cfg(not(any(target_os = "ios")))]
    fn test_symlink_creation_works() {
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

        let _ = fs::remove_dir_all(&base_dir);
    }
}
