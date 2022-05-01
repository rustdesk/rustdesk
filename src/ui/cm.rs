use crate::ipc::{self, new_listener, Connection, Data};
#[cfg(windows)]
use clipboard::{
    create_cliprdr_context, empty_clipboard, get_rx_clip_client, server_clip_file, set_conn_enabled,
};
use hbb_common::{
    allow_err,
    config::{Config, ICON},
    fs, log,
    message_proto::*,
    protobuf::Message as _,
    tokio::{self, sync::mpsc, task::spawn_blocking},
};
use sciter::{make_args, Element, Value, HELEMENT};
use std::{
    collections::HashMap,
    ops::Deref,
    sync::{Arc, RwLock},
};

pub struct ConnectionManagerInner {
    root: Option<Element>,
    senders: HashMap<i32, mpsc::UnboundedSender<Data>>,
}

#[derive(Clone)]
pub struct ConnectionManager(Arc<RwLock<ConnectionManagerInner>>);

impl Deref for ConnectionManager {
    type Target = Arc<RwLock<ConnectionManagerInner>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ConnectionManager {
    pub fn new() -> Self {
        #[cfg(target_os = "linux")]
        std::thread::spawn(start_pa);
        let inner = ConnectionManagerInner {
            root: None,
            senders: HashMap::new(),
        };
        let cm = Self(Arc::new(RwLock::new(inner)));
        let cloned = cm.clone();
        std::thread::spawn(move || start_ipc(cloned));
        cm
    }

    fn get_icon(&mut self) -> String {
        ICON.to_owned()
    }

    #[inline]
    fn call(&self, func: &str, args: &[Value]) {
        let r = self.read().unwrap();
        if let Some(ref e) = r.root {
            allow_err!(e.call_method(func, &super::value_crash_workaround(args)[..]));
        }
    }

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
        tx: mpsc::UnboundedSender<Data>,
    ) {
        self.call(
            "addConnection",
            &make_args!(
                id,
                is_file_transfer,
                port_forward,
                peer_id,
                name,
                authorized,
                keyboard,
                clipboard,
                audio,
                file
            ),
        );
        self.write().unwrap().senders.insert(id, tx);
    }

    fn remove_connection(&self, id: i32) {
        self.write().unwrap().senders.remove(&id);
        if self.read().unwrap().senders.len() == 0 {
            std::process::exit(0);
        }
        self.call("removeConnection", &make_args!(id));
    }

    async fn handle_data(
        &self,
        id: i32,
        data: Data,
        _tx_clip_file: &mpsc::UnboundedSender<ClipboardFileData>,
        write_jobs: &mut Vec<fs::TransferJob>,
        conn: &mut Connection,
    ) {
        match data {
            Data::ChatMessage { text } => {
                self.call("newMessage", &make_args!(id, text));
            }
            Data::FS(v) => match v {
                ipc::FS::ReadDir {
                    dir,
                    include_hidden,
                } => {
                    Self::read_dir(&dir, include_hidden, conn).await;
                }
                ipc::FS::RemoveDir {
                    path,
                    id,
                    recursive,
                } => {
                    Self::remove_dir(path, id, recursive, conn).await;
                }
                ipc::FS::RemoveFile { path, id, file_num } => {
                    Self::remove_file(path, id, file_num, conn).await;
                }
                ipc::FS::CreateDir { path, id } => {
                    Self::create_dir(path, id, conn).await;
                }
                ipc::FS::NewWrite {
                    path,
                    id,
                    mut files,
                } => {
                    write_jobs.push(fs::TransferJob::new_write(
                        id,
                        path,
                        files
                            .drain(..)
                            .map(|f| FileEntry {
                                name: f.0,
                                modified_time: f.1,
                                ..Default::default()
                            })
                            .collect(),
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
                        Self::send(fs::new_done(id, file_num), conn).await;
                        fs::remove_job(id, write_jobs);
                    }
                }
                ipc::FS::WriteBlock {
                    id,
                    file_num,
                    data,
                    compressed,
                } => {
                    let raw = if let Ok(bytes) = conn.next_raw().await {
                        Some(bytes)
                    } else {
                        None
                    };
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
                                raw.as_ref().map(|x| &x[..]),
                            )
                            .await
                        {
                            Self::send(fs::new_error(id, err, file_num), conn).await;
                        }
                    }
                }
            },
            #[cfg(windows)]
            Data::ClipbaordFile(_clip) => {
                _tx_clip_file
                    .send(ClipboardFileData::Clip((id, _clip)))
                    .ok();
            }
            #[cfg(windows)]
            Data::ClipboardFileEnabled(enabled) => {
                _tx_clip_file
                    .send(ClipboardFileData::Enable((id, enabled)))
                    .ok();
            }
            _ => {}
        }
    }

    async fn read_dir(dir: &str, include_hidden: bool, conn: &mut Connection) {
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
            Self::send(msg_out, conn).await;
        }
    }

    async fn handle_result<F: std::fmt::Display, S: std::fmt::Display>(
        res: std::result::Result<std::result::Result<(), F>, S>,
        id: i32,
        file_num: i32,
        conn: &mut Connection,
    ) {
        match res {
            Err(err) => {
                Self::send(fs::new_error(id, err, file_num), conn).await;
            }
            Ok(Err(err)) => {
                Self::send(fs::new_error(id, err, file_num), conn).await;
            }
            Ok(Ok(())) => {
                Self::send(fs::new_done(id, file_num), conn).await;
            }
        }
    }

    async fn remove_file(path: String, id: i32, file_num: i32, conn: &mut Connection) {
        Self::handle_result(
            spawn_blocking(move || fs::remove_file(&path)).await,
            id,
            file_num,
            conn,
        )
        .await;
    }

    async fn create_dir(path: String, id: i32, conn: &mut Connection) {
        Self::handle_result(
            spawn_blocking(move || fs::create_dir(&path)).await,
            id,
            0,
            conn,
        )
        .await;
    }

    async fn remove_dir(path: String, id: i32, recursive: bool, conn: &mut Connection) {
        let path = fs::get_path(&path);
        Self::handle_result(
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
            conn,
        )
        .await;
    }

    async fn send(msg: Message, conn: &mut Connection) {
        match msg.write_to_bytes() {
            Ok(bytes) => allow_err!(conn.send(&Data::RawMessage(bytes)).await),
            err => allow_err!(err),
        }
    }

    fn switch_permission(&self, id: i32, name: String, enabled: bool) {
        let lock = self.read().unwrap();
        if let Some(s) = lock.senders.get(&id) {
            allow_err!(s.send(Data::SwitchPermission { name, enabled }));
        }
    }

    fn close(&self, id: i32) {
        let lock = self.read().unwrap();
        if let Some(s) = lock.senders.get(&id) {
            allow_err!(s.send(Data::Close));
        }
    }

    fn send_msg(&self, id: i32, text: String) {
        let lock = self.read().unwrap();
        if let Some(s) = lock.senders.get(&id) {
            allow_err!(s.send(Data::ChatMessage { text }));
        }
    }

    fn authorize(&self, id: i32) {
        let lock = self.read().unwrap();
        if let Some(s) = lock.senders.get(&id) {
            allow_err!(s.send(Data::Authorize));
        }
    }

    fn t(&self, name: String) -> String {
        crate::client::translate(name)
    }
}

impl sciter::EventHandler for ConnectionManager {
    fn attached(&mut self, root: HELEMENT) {
        self.write().unwrap().root = Some(Element::from(root));
    }

    sciter::dispatch_script_call! {
        fn t(String);
        fn get_icon();
        fn close(i32);
        fn authorize(i32);
        fn switch_permission(i32, String, bool);
        fn send_msg(i32, String);
    }
}

enum ClipboardFileData {
    #[cfg(windows)]
    Clip((i32, ipc::ClipbaordFile)),
    Enable((i32, bool)),
}

#[tokio::main(flavor = "current_thread")]
async fn start_ipc(cm: ConnectionManager) {
    let (tx_file, _rx_file) = mpsc::unbounded_channel::<ClipboardFileData>();
    #[cfg(windows)]
    let cm_clip = cm.clone();
    #[cfg(windows)]
    std::thread::spawn(move || start_clipboard_file(cm_clip, _rx_file));

    match new_listener("_cm").await {
        Ok(mut incoming) => {
            while let Some(result) = incoming.next().await {
                match result {
                    Ok(stream) => {
                        log::debug!("Got new connection");
                        let mut stream = Connection::new(stream);
                        let cm = cm.clone();
                        let tx_file = tx_file.clone();
                        tokio::spawn(async move {
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
                                                    Data::Login{id, is_file_transfer, port_forward, peer_id, name, authorized, keyboard, clipboard, audio, file, file_transfer_enabled} => {
                                                        log::debug!("conn_id: {}", id);
                                                        conn_id = id;
                                                        tx_file.send(ClipboardFileData::Enable((id, file_transfer_enabled))).ok();
                                                        cm.add_connection(id, is_file_transfer, port_forward, peer_id, name, authorized, keyboard, clipboard, audio, file, tx.clone());
                                                    }
                                                    Data::Close => {
                                                        tx_file.send(ClipboardFileData::Enable((conn_id, false))).ok();
                                                        log::info!("cm ipc connection closed from connection request");
                                                        break;
                                                    }
                                                    _ => {
                                                        cm.handle_data(conn_id, data, &tx_file, &mut write_jobs, &mut stream).await;
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
                                }
                            }
                            cm.remove_connection(conn_id);
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
    std::process::exit(-1);
}

#[cfg(target_os = "linux")]
#[tokio::main(flavor = "current_thread")]
async fn start_pa() {
    use crate::audio_service::AUDIO_DATA_SIZE_U8;
    use hbb_common::config::APP_NAME;

    match new_listener("_pa").await {
        Ok(mut incoming) => {
            loop {
                if let Some(result) = incoming.next().await {
                    match result {
                        Ok(stream) => {
                            let mut stream = Connection::new(stream);
                            let mut device: String = "".to_owned();
                            if let Some(Ok(Some(Data::Config((_, Some(x)))))) =
                                stream.next_timeout2(1000).await
                            {
                                device = x;
                            }
                            if !device.is_empty() {
                                device = crate::platform::linux::get_pa_source_name(&device);
                            }
                            if device.is_empty() {
                                device = crate::platform::linux::get_pa_monitor();
                            }
                            if device.is_empty() {
                                continue;
                            }
                            let spec = pulse::sample::Spec {
                                format: pulse::sample::Format::F32le,
                                channels: 2,
                                rate: crate::platform::linux::PA_SAMPLE_RATE,
                            };
                            log::info!("pa monitor: {:?}", device);
                            // systemctl --user status pulseaudio.service
                            let mut buf: Vec<u8> = vec![0; AUDIO_DATA_SIZE_U8];
                            match psimple::Simple::new(
                                None,                             // Use the default server
                                APP_NAME,                         // Our application’s name
                                pulse::stream::Direction::Record, // We want a record stream
                                Some(&device),                    // Use the default device
                                "record",                         // Description of our stream
                                &spec,                            // Our sample format
                                None,                             // Use default channel map
                                None, // Use default buffering attributes
                            ) {
                                Ok(s) => loop {
                                    if let Ok(_) = s.read(&mut buf) {
                                        let out =
                                            if buf.iter().filter(|x| **x != 0).next().is_none() {
                                                vec![]
                                            } else {
                                                buf.clone()
                                            };
                                        if let Err(err) = stream.send_raw(out).await {
                                            log::error!("Failed to send audio data:{}", err);
                                            break;
                                        }
                                    }
                                },
                                Err(err) => {
                                    log::error!("Could not create simple pulse: {}", err);
                                }
                            }
                        }
                        Err(err) => {
                            log::error!("Couldn't get pa client: {:?}", err);
                        }
                    }
                }
            }
        }
        Err(err) => {
            log::error!("Failed to start pa ipc server: {}", err);
        }
    }
}

#[cfg(windows)]
#[tokio::main(flavor = "current_thread")]
async fn start_clipboard_file(
    cm: ConnectionManager,
    mut rx: mpsc::UnboundedReceiver<ClipboardFileData>,
) {
    let mut cliprdr_context = None;
    let mut rx_clip_client = get_rx_clip_client().lock().await;

    loop {
        tokio::select! {
            clip_file = rx_clip_client.recv() => match clip_file {
                Some((conn_id, clip)) => {
                    cmd_inner_send(
                        &cm,
                        conn_id,
                        Data::ClipbaordFile(clip)
                    );
                }
                None => {
                    //
                }
            },
            server_msg = rx.recv() => match server_msg {
                Some(ClipboardFileData::Clip((conn_id, clip))) => {
                    if let Some(ctx) = cliprdr_context.as_mut() {
                        server_clip_file(ctx, conn_id, clip);
                    }
                }
                Some(ClipboardFileData::Enable((id, enabled))) => {
                    if enabled && cliprdr_context.is_none() {
                        cliprdr_context = Some(match create_cliprdr_context(true, false) {
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
                    set_conn_enabled(id, enabled);
                    if !enabled {
                        if let Some(ctx) = cliprdr_context.as_mut() {
                            empty_clipboard(ctx, id);
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

#[cfg(windows)]
fn cmd_inner_send(cm: &ConnectionManager, id: i32, data: Data) {
    let lock = cm.read().unwrap();
    if id != 0 {
        if let Some(s) = lock.senders.get(&id) {
            allow_err!(s.send(data));
        }
    } else {
        for s in lock.senders.values() {
            allow_err!(s.send(data.clone()));
        }
    }
}
