use std::ops::{Deref, DerefMut};
use std::{
    collections::HashMap,
    iter::FromIterator,
    sync::{
        atomic::{AtomicI64, Ordering},
        RwLock,
    },
};

use clipboard::empty_clipboard;
use hbb_common::chrono::Duration;
use serde_derive::Serialize;

use crate::ipc::Data;
use crate::ipc::{self, new_listener, Connection};
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
        sync::mpsc::{self, UnboundedSender},
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

#[derive(Debug)]
pub enum ClipboardFileData {
    #[cfg(windows)]
    Clip((i32, ipc::ClipbaordFile)),
    Enable((i32, bool)),
}

async fn cm_ipc_task_wait_login<T: InvokeUiCM>(
    mut stream: Connection,
    cm: ConnectionManager<T>,
    tx: mpsc::UnboundedSender<Data>,
) -> (Connection, ConnectionManager<T>, Option<(i32, bool)>) {
    let mut ret = None;
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
                            Data::Login{id, is_file_transfer, port_forward, peer_id, name, authorized, keyboard, clipboard, audio, file, file_transfer_enabled, restart, recording} => {
                                log::debug!("conn_id: {}", id);
                                cm.add_connection(id, is_file_transfer, port_forward, peer_id, name, authorized, keyboard, clipboard, audio, file, restart, recording, tx);
                                ret = Some((id, file_transfer_enabled));
                                break;
                            }
                            Data::Close => {
                                log::info!("cm ipc connection closed from connection request");
                                break;
                            }
                            Data::Disconnected => {
                                log::info!("cm ipc connection disconnect");
                                break;
                            }
                            _ => {

                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    (stream, cm, ret)
}

async fn cm_ipc_task_loop<T: InvokeUiCM>(
    mut stream: Connection,
    cm: ConnectionManager<T>,
    tx_file: mpsc::UnboundedSender<ClipboardFileData>,
    tx: mpsc::UnboundedSender<Data>,
    mut rx: mpsc::UnboundedReceiver<Data>,
    mut conn_id: i32,
    #[cfg(windows)] file_transfer_enabled: bool,
) {
    use hbb_common::config::LocalConfig;

    // for tmp use, without real conn id
    let conn_id_tmp = -1;
    let mut write_jobs: Vec<fs::TransferJob> = Vec::new();
    let mut close = true;

    #[cfg(windows)]
    allow_err!(tx_file.send(ClipboardFileData::Enable((conn_id, file_transfer_enabled))));

    #[cfg(windows)]
    let rx_clip_client1 = clipboard::get_rx_cliprdr_server(conn_id);
    #[cfg(windows)]
    let mut rx_clip_client = rx_clip_client1.lock().await;

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
                            Data::Close => {
                                allow_err!(tx_file.send(ClipboardFileData::Enable((conn_id, false))));
                                log::info!("cm ipc connection closed from connection request");
                                break;
                            }
                            Data::Disconnected => {
                                close = false;
                                allow_err!(tx_file.send(ClipboardFileData::Enable((conn_id, false))));
                                log::info!("cm ipc connection disconnect");
                                break;
                            }
                            Data::PrivacyModeState((id, _)) => {
                                conn_id = conn_id_tmp;
                                allow_err!(tx.send(data));
                            }
                            Data::ClickTime(ms) => {
                                CLICK_TIME.store(ms, Ordering::SeqCst);
                            }
                            Data::ChatMessage { text } => {
                                cm.new_message(conn_id, text);
                            }
                            Data::FS(fs) => {
                                handle_fs(fs, &mut write_jobs, &tx).await;
                            }
                            #[cfg(windows)]
                            Data::ClipbaordFile(_clip) => {
                                allow_err!(tx_file.send(ClipboardFileData::Clip((conn_id, _clip))));
                            }
                            #[cfg(windows)]
                            Data::ClipboardFileEnabled(enabled) => {
                                allow_err!(tx_file.send(ClipboardFileData::Enable((conn_id, enabled))));
                            }
                            Data::Theme(dark) => {
                                cm.change_theme(dark);
                            }
                            Data::Language(lang) => {
                                LocalConfig::set_option("lang".to_owned(), lang);
                                cm.change_language();
                            }
                            _ => {

                            }
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
            clip_file = rx_clip_client.recv() => match clip_file {
                Some(clip) => {
                    allow_err!(tx.send(Data::ClipbaordFile(clip)));
                }
                None => {
                    //
                }
            },
        }
    }
    if conn_id != conn_id_tmp {
        cm.remove_connection(conn_id, close);
    }
}

async fn cm_ipc_task<T: InvokeUiCM>(
    stream: Connection,
    cm: ConnectionManager<T>,
    tx_file: mpsc::UnboundedSender<ClipboardFileData>,
) {
    let (tx, rx) = mpsc::unbounded_channel::<Data>();
    let (stream, cm, wait_res) = cm_ipc_task_wait_login(stream, cm, tx.clone()).await;
    if let Some((conn_id, file_transfer_enabled)) = wait_res {
        cm_ipc_task_loop(stream, cm, tx_file, tx, rx, conn_id, file_transfer_enabled).await;
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::main(flavor = "current_thread")]
pub async fn start_ipc<T: InvokeUiCM>(cm: ConnectionManager<T>) {
    #[cfg(windows)]
    let cm_clip = cm.clone();

    let (tx_file, _rx_file) = mpsc::unbounded_channel::<ClipboardFileData>();
    std::thread::spawn(move || start_clipboard_file(_rx_file));

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
                        tokio::spawn(cm_ipc_task(
                            Connection::new(stream),
                            cm.clone(),
                            tx_file.clone(),
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

#[cfg(windows)]
#[tokio::main(flavor = "current_thread")]
pub async fn start_clipboard_file(mut rx: mpsc::UnboundedReceiver<ClipboardFileData>) {
    let mut cliprdr_context = None;

    loop {
        tokio::select! {
            server_msg = rx.recv() => match server_msg {
                Some(ClipboardFileData::Clip((conn_id, clip))) => {
                    if let Some(ctx) = cliprdr_context.as_mut() {
                        clipboard::server_clip_file(ctx, conn_id, clip);
                    }
                }
                Some(ClipboardFileData::Enable((id, enabled))) => {
                    if enabled && cliprdr_context.is_none() {
                        cliprdr_context = Some(match clipboard::create_cliprdr_context(true, false) {
                            Ok(context) => {
                                log::info!("clipboard context for file transfer created.");
                                context
                            }
                            Err(err) => {
                                log::error!(
                                    "Create clipboard context for file transfer: {}",
                                    err.to_string()
                                );
                                return;
                            }
                        });
                    }
                    clipboard::set_conn_enabled(id, enabled);
                    if !enabled {
                        if let Some(ctx) = cliprdr_context.as_mut() {
                            clipboard::empty_clipboard(ctx, id);
                        }
                    }
                }
                None => {
                    break
                }
            }
        }
    }
}
