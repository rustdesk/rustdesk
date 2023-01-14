#[cfg(any(target_os = "android", target_os = "ios", feature = "flutter"))]
use std::iter::FromIterator;
#[cfg(windows)]
use std::sync::Arc;
use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    sync::{
        atomic::{AtomicI64, Ordering},
        RwLock,
    },
};

#[cfg(windows)]
use clipboard::{cliprdr::CliprdrClientContext, empty_clipboard, set_conn_enabled, ContextSend};
use serde_derive::Serialize;

use crate::ipc::{self, Connection, Data};
#[cfg(windows)]
use hbb_common::tokio::sync::Mutex as TokioMutex;
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
        sync::mpsc::{self, unbounded_channel, UnboundedSender},
        task::spawn_blocking,
    },
};

#[derive(Serialize, Clone)]
pub struct Client {
    pub id: i32,
    pub authorized: bool,
    pub disconnected: bool,
    pub is_file_transfer: bool,
    pub port_forward: String,
    pub name: String,
    pub peer_id: String,
    pub keyboard: bool,
    pub clipboard: bool,
    pub audio: bool,
    pub file: bool,
    pub restart: bool,
    pub recording: bool,
    #[serde(skip)]
    tx: UnboundedSender<Data>,
}

struct IpcTaskRunner<T: InvokeUiCM> {
    stream: Connection,
    cm: ConnectionManager<T>,
    tx: mpsc::UnboundedSender<Data>,
    rx: mpsc::UnboundedReceiver<Data>,
    close: bool,
    running: bool,
    authorized: bool,
    conn_id: i32,
    #[cfg(windows)]
    file_transfer_enabled: bool,
}

lazy_static::lazy_static! {
    static ref CLIENTS: RwLock<HashMap<i32, Client>> = Default::default();
    static ref CLICK_TIME: AtomicI64 = AtomicI64::new(0);
}

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
        tx: mpsc::UnboundedSender<Data>,
    ) {
        let client = Client {
            id,
            authorized,
            disconnected: false,
            is_file_transfer,
            port_forward,
            name: name.clone(),
            peer_id: peer_id.clone(),
            keyboard,
            clipboard,
            audio,
            file,
            restart,
            recording,
            tx,
        };
        CLIENTS
            .write()
            .unwrap()
            .retain(|_, c| !(c.disconnected && c.peer_id == client.peer_id));
        CLIENTS.write().unwrap().insert(id, client.clone());
        self.ui_handler.add_connection(&client);
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

        #[cfg(any(target_os = "android"))]
        if CLIENTS
            .read()
            .unwrap()
            .iter()
            .filter(|(_k, v)| !v.is_file_transfer)
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

    fn show_elevation(&self, show: bool) {
        self.ui_handler.show_elevation(show);
    }
}

#[inline]
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
pub fn authorize(id: i32) {
    if let Some(client) = CLIENTS.write().unwrap().get_mut(&id) {
        client.authorized = true;
        allow_err!(client.tx.send(Data::Authorize));
    };
}

#[inline]
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
pub fn send_chat(id: i32, text: String) {
    let clients = CLIENTS.read().unwrap();
    if let Some(client) = clients.get(&id) {
        allow_err!(client.tx.send(Data::ChatMessage { text }));
    }
}

#[inline]
pub fn switch_permission(id: i32, name: String, enabled: bool) {
    if let Some(client) = CLIENTS.read().unwrap().get(&id) {
        allow_err!(client.tx.send(Data::SwitchPermission { name, enabled }));
    };
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

impl<T: InvokeUiCM> IpcTaskRunner<T> {
    #[cfg(windows)]
    async fn enable_cliprdr_file_context(&mut self, conn_id: i32, enabled: bool) {
        if conn_id == 0 {
            return;
        }

        let pre_enabled = ContextSend::is_enabled();
        ContextSend::enable(enabled);
        if !pre_enabled && ContextSend::is_enabled() {
            allow_err!(
                self.stream
                    .send(&Data::ClipboardFile(clipboard::ClipboardFile::MonitorReady))
                    .await
            );
        }
        set_conn_enabled(conn_id, enabled);
        if !enabled {
            ContextSend::proc(|context: &mut Box<CliprdrClientContext>| -> u32 {
                empty_clipboard(context, conn_id);
                0
            });
        }
    }

    async fn run(&mut self) {
        use hbb_common::config::LocalConfig;

        // for tmp use, without real conn id
        let mut write_jobs: Vec<fs::TransferJob> = Vec::new();

        #[cfg(windows)]
        if self.conn_id > 0 {
            self.enable_cliprdr_file_context(self.conn_id, self.file_transfer_enabled)
                .await;
        }

        #[cfg(windows)]
        let rx_clip1;
        let mut rx_clip;
        let _tx_clip;
        #[cfg(windows)]
        if self.conn_id > 0 && self.authorized {
            rx_clip1 = clipboard::get_rx_cliprdr_server(self.conn_id);
            rx_clip = rx_clip1.lock().await;
        } else {
            let rx_clip2;
            (_tx_clip, rx_clip2) = unbounded_channel::<clipboard::ClipboardFile>();
            rx_clip1 = Arc::new(TokioMutex::new(rx_clip2));
            rx_clip = rx_clip1.lock().await;
        }
        #[cfg(not(windows))]
        {
            (_tx_clip, rx_clip) = unbounded_channel::<i32>();
        }

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
                                Data::Login{id, is_file_transfer, port_forward, peer_id, name, authorized, keyboard, clipboard, audio, file, file_transfer_enabled: _file_transfer_enabled, restart, recording} => {
                                    log::debug!("conn_id: {}", id);
                                    self.cm.add_connection(id, is_file_transfer, port_forward, peer_id, name, authorized, keyboard, clipboard, audio, file, restart, recording, self.tx.clone());
                                    self.authorized = authorized;
                                    self.conn_id = id;
                                    #[cfg(windows)]
                                    {
                                        self.file_transfer_enabled = _file_transfer_enabled;
                                    }
                                    self.running = true;
                                    break;
                                }
                                Data::Close => {
                                    #[cfg(windows)]
                                    self.enable_cliprdr_file_context(self.conn_id, false).await;
                                    log::info!("cm ipc connection closed from connection request");
                                    break;
                                }
                                Data::Disconnected => {
                                    self.close = false;
                                    #[cfg(windows)]
                                    self.enable_cliprdr_file_context(self.conn_id, false).await;
                                    log::info!("cm ipc connection disconnect");
                                    break;
                                }
                                Data::PrivacyModeState((_id, _)) => {
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
                                            handle_fs(fs, &mut write_jobs, &self.tx).await;
                                        }
                                    } else {
                                        handle_fs(fs, &mut write_jobs, &self.tx).await;
                                    }
                                }
                                #[cfg(windows)]
                                Data::ClipboardFile(_clip) => {
                                    #[cfg(windows)]
                                    {
                                        let conn_id = self.conn_id;
                                        ContextSend::proc(|context: &mut Box<CliprdrClientContext>| -> u32 {
                                            clipboard::server_clip_file(context, conn_id, _clip)
                                        });
                                    }
                                }
                                #[cfg(windows)]
                                Data::ClipboardFileEnabled(_enabled) => {
                                    #[cfg(windows)]
                                    self.enable_cliprdr_file_context(self.conn_id, _enabled).await;
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
                                _ => {

                                }
                            }
                        }
                        _ => {}
                    }
                }
                Some(data) = self.rx.recv() => {
                    if self.stream.send(&data).await.is_err() {
                        break;
                    }
                }
                clip_file = rx_clip.recv() => match clip_file {
                    Some(_clip) => {
                        #[cfg(windows)]
                        allow_err!(self.tx.send(Data::ClipboardFile(_clip)));
                    }
                    None => {
                        //
                    }
                },
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
            authorized: false,
            conn_id: 0,
            #[cfg(windows)]
            file_transfer_enabled: false,
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
    crate::platform::quit_gui();
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
                ..
            }) => {
                current_id = id;
                cm.add_connection(
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
                    recording,
                    tx.clone(),
                );
            }
            Some(Data::ChatMessage { text }) => {
                cm.new_message(current_id, text);
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
    cm.remove_connection(current_id, true);
}

async fn handle_fs(fs: ipc::FS, write_jobs: &mut Vec<fs::TransferJob>, tx: &UnboundedSender<Data>) {
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
            // cm has no show_hidden context
            // dummy remote, show_hidden, is_remote
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
        ipc::FS::WriteError { id, file_num, err } => {
            if let Some(job) = fs::get_job(id, write_jobs) {
                send_raw(fs::new_error(job.id(), err, file_num), tx);
                fs::remove_job(job.id(), write_jobs);
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
    {
        return !crate::platform::is_installed() && !crate::portable_service::client::running();
    }
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
