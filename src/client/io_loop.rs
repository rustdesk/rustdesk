use crate::client::{
    Client, CodecFormat, MediaData, MediaSender, QualityStatus, MILLI1, SEC30,
    SERVER_CLIPBOARD_ENABLED, SERVER_FILE_TRANSFER_ENABLED, SERVER_KEYBOARD_ENABLED,
};
use crate::common;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::common::{check_clipboard, update_clipboard, ClipboardContext, CLIPBOARD_INTERVAL};

#[cfg(windows)]
use clipboard::{cliprdr::CliprdrClientContext, ContextSend};

use crate::ui_session_interface::{InvokeUiSession, Session};
use crate::{client::Data, client::Interface};

use hbb_common::config::{PeerConfig, TransferSerde};
use hbb_common::fs::{
    can_enable_overwrite_detection, get_job, get_string, new_send_confirm, DigestCheckResult,
    RemoveJobMeta,
};
use hbb_common::message_proto::permission_info::Permission;
use hbb_common::protobuf::Message as _;
use hbb_common::rendezvous_proto::ConnType;
use hbb_common::tokio::{
    self,
    sync::mpsc,
    time::{self, Duration, Instant, Interval},
};
#[cfg(windows)]
use hbb_common::tokio::sync::Mutex as TokioMutex;
use hbb_common::{
    allow_err,
    message_proto::*,
    sleep,
};
use hbb_common::{fs, log, Stream};
use std::collections::HashMap;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

pub struct Remote<T: InvokeUiSession> {
    handler: Session<T>,
    video_sender: MediaSender,
    audio_sender: MediaSender,
    receiver: mpsc::UnboundedReceiver<Data>,
    sender: mpsc::UnboundedSender<Data>,
    old_clipboard: Arc<Mutex<String>>,
    read_jobs: Vec<fs::TransferJob>,
    write_jobs: Vec<fs::TransferJob>,
    remove_jobs: HashMap<i32, RemoveJob>,
    timer: Interval,
    last_update_jobs_status: (Instant, HashMap<i32, u64>),
    first_frame: bool,
    #[cfg(windows)]
    client_conn_id: i32, // used for clipboard
    data_count: Arc<AtomicUsize>,
    frame_count: Arc<AtomicUsize>,
    video_format: CodecFormat,
}

impl<T: InvokeUiSession> Remote<T> {
    pub fn new(
        handler: Session<T>,
        video_sender: MediaSender,
        audio_sender: MediaSender,
        receiver: mpsc::UnboundedReceiver<Data>,
        sender: mpsc::UnboundedSender<Data>,
        frame_count: Arc<AtomicUsize>,
    ) -> Self {
        Self {
            handler,
            video_sender,
            audio_sender,
            receiver,
            sender,
            old_clipboard: Default::default(),
            read_jobs: Vec::new(),
            write_jobs: Vec::new(),
            remove_jobs: Default::default(),
            timer: time::interval(SEC30),
            last_update_jobs_status: (Instant::now(), Default::default()),
            first_frame: false,
            #[cfg(windows)]
            client_conn_id: 0,
            data_count: Arc::new(AtomicUsize::new(0)),
            frame_count,
            video_format: CodecFormat::Unknown,
        }
    }

    pub async fn io_loop(&mut self, key: &str, token: &str) {
        let stop_clipboard = self.start_clipboard();
        let mut last_recv_time = Instant::now();
        let mut received = false;
        let conn_type = if self.handler.is_file_transfer() {
            ConnType::FILE_TRANSFER
        } else {
            ConnType::default()
        };
        match Client::start(
            &self.handler.id,
            key,
            token,
            conn_type,
            self.handler.clone(),
        )
        .await
        {
            Ok((mut peer, direct)) => {
                SERVER_KEYBOARD_ENABLED.store(true, Ordering::SeqCst);
                SERVER_CLIPBOARD_ENABLED.store(true, Ordering::SeqCst);
                SERVER_FILE_TRANSFER_ENABLED.store(true, Ordering::SeqCst);
                self.handler.set_connection_type(peer.is_secured(), direct); // flutter -> connection_ready

                // just build for now
                #[cfg(not(windows))]
                let (_tx_holder, mut rx_clip_client) = mpsc::unbounded_channel::<i32>();

                #[cfg(windows)]
                let (_tx_holder, rx) = mpsc::unbounded_channel();
                #[cfg(windows)]
                let mut rx_clip_client_lock = Arc::new(TokioMutex::new(rx));
                #[cfg(windows)]
                {
                    let is_conn_not_default = self.handler.is_file_transfer()
                        || self.handler.is_port_forward()
                        || self.handler.is_rdp();
                    if !is_conn_not_default {
                        (self.client_conn_id, rx_clip_client_lock) =
                            clipboard::get_rx_cliprdr_client(&self.handler.id);
                    };
                }
                #[cfg(windows)]
                let mut rx_clip_client = rx_clip_client_lock.lock().await;

                let mut status_timer = time::interval(Duration::new(1, 0));

                loop {
                    tokio::select! {
                        res = peer.next() => {
                            if let Some(res) = res {
                                match res {
                                    Err(err) => {
                                        log::error!("Connection closed: {}", err);
                                        self.handler.set_force_relay(direct, received);
                                        self.handler.msgbox("error", "Connection Error", &err.to_string(), "");
                                        break;
                                    }
                                    Ok(ref bytes) => {
                                        last_recv_time = Instant::now();
                                        received = true;
                                        self.data_count.fetch_add(bytes.len(), Ordering::Relaxed);
                                        if !self.handle_msg_from_peer(bytes, &mut peer).await {
                                            break
                                        }
                                    }
                                }
                            } else {
                                if self.handler.is_restarting_remote_device() {
                                    log::info!("Restart remote device");
                                    self.handler.msgbox("restarting", "Restarting Remote Device", "remote_restarting_tip", "");
                                } else {
                                    log::info!("Reset by the peer");
                                    self.handler.msgbox("error", "Connection Error", "Reset by the peer", "");
                                }
                                break;
                            }
                        }
                        d = self.receiver.recv() => {
                            if let Some(d) = d {
                                if !self.handle_msg_from_ui(d, &mut peer).await {
                                    break;
                                }
                            }
                        }
                        _msg = rx_clip_client.recv() => {
                            #[cfg(windows)]
                            match _msg {
                                Some(clip) => {
                                    allow_err!(peer.send(&crate::clipboard_file::clip_2_msg(clip)).await);
                                }
                                None => {
                                    // unreachable!()
                                }
                            }
                        }
                        _ = self.timer.tick() => {
                            if last_recv_time.elapsed() >= SEC30 {
                                self.handler.msgbox("error", "Connection Error", "Timeout", "");
                                break;
                            }
                            if !self.read_jobs.is_empty() {
                                if let Err(err) = fs::handle_read_jobs(&mut self.read_jobs, &mut peer).await {
                                    self.handler.msgbox("error", "Connection Error", &err.to_string(), "");
                                    break;
                                }
                                self.update_jobs_status();
                            } else {
                                self.timer = time::interval_at(Instant::now() + SEC30, SEC30);
                            }
                        }
                        _ = status_timer.tick() => {
                            let speed = self.data_count.swap(0, Ordering::Relaxed);
                            let speed = format!("{:.2}kB/s", speed as f32 / 1024 as f32);
                            let fps = self.frame_count.swap(0, Ordering::Relaxed) as _;
                            self.handler.update_quality_status(QualityStatus {
                                speed:Some(speed),
                                fps:Some(fps),
                                ..Default::default()
                            });
                        }
                    }
                }
                log::debug!("Exit io_loop of id={}", self.handler.id);
            }
            Err(err) => {
                self.handler
                    .msgbox("error", "Connection Error", &err.to_string(), "");
            }
        }
        if let Some(stop) = stop_clipboard {
            stop.send(()).ok();
        }
        SERVER_KEYBOARD_ENABLED.store(false, Ordering::SeqCst);
        SERVER_CLIPBOARD_ENABLED.store(false, Ordering::SeqCst);
        SERVER_FILE_TRANSFER_ENABLED.store(false, Ordering::SeqCst);
    }

    fn handle_job_status(&mut self, id: i32, file_num: i32, err: Option<String>) {
        if let Some(job) = self.remove_jobs.get_mut(&id) {
            if job.no_confirm {
                let file_num = (file_num + 1) as usize;
                if file_num < job.files.len() {
                    let path = format!("{}{}{}", job.path, job.sep, job.files[file_num].name);
                    self.sender
                        .send(Data::RemoveFile((id, path, file_num as i32, job.is_remote)))
                        .ok();
                    let elapsed = job.last_update_job_status.elapsed().as_millis() as i32;
                    if elapsed >= 1000 {
                        job.last_update_job_status = Instant::now();
                    } else {
                        return;
                    }
                } else {
                    self.remove_jobs.remove(&id);
                }
            }
        }
        if let Some(err) = err {
            self.handler.job_error(id, err, file_num);
        } else {
            self.handler.job_done(id, file_num);
        }
    }

    fn start_clipboard(&mut self) -> Option<std::sync::mpsc::Sender<()>> {
        if self.handler.is_file_transfer() || self.handler.is_port_forward() {
            return None;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let old_clipboard = self.old_clipboard.clone();
        let tx_protobuf = self.sender.clone();
        let lc = self.handler.lc.clone();
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        match ClipboardContext::new() {
            Ok(mut ctx) => {
                // ignore clipboard update before service start
                check_clipboard(&mut ctx, Some(&old_clipboard));
                std::thread::spawn(move || loop {
                    std::thread::sleep(Duration::from_millis(CLIPBOARD_INTERVAL));
                    match rx.try_recv() {
                        Ok(_) | Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                            log::debug!("Exit clipboard service of client");
                            break;
                        }
                        _ => {}
                    }
                    if !SERVER_CLIPBOARD_ENABLED.load(Ordering::SeqCst)
                        || !SERVER_KEYBOARD_ENABLED.load(Ordering::SeqCst)
                        || lc.read().unwrap().disable_clipboard
                    {
                        continue;
                    }
                    if let Some(msg) = check_clipboard(&mut ctx, Some(&old_clipboard)) {
                        tx_protobuf.send(Data::Message(msg)).ok();
                    }
                });
            }
            Err(err) => {
                log::error!("Failed to start clipboard service of client: {}", err);
            }
        }
        Some(tx)
    }

    async fn handle_msg_from_ui(&mut self, data: Data, peer: &mut Stream) -> bool {
        match data {
            Data::Close => {
                let mut misc = Misc::new();
                misc.set_close_reason("".to_owned());
                let mut msg = Message::new();
                msg.set_misc(misc);
                allow_err!(peer.send(&msg).await);
                return false;
            }
            Data::Login((password, remember)) => {
                self.handler
                    .handle_login_from_ui(password, remember, peer)
                    .await;
            }
            Data::ToggleClipboardFile => {
                self.check_clipboard_file_context();
            }
            Data::Message(msg) => {
                allow_err!(peer.send(&msg).await);
            }
            Data::SendFiles((id, path, to, file_num, include_hidden, is_remote)) => {
                log::info!("send files, is remote {}", is_remote);
                let od = can_enable_overwrite_detection(self.handler.lc.read().unwrap().version);
                if is_remote {
                    log::debug!("New job {}, write to {} from remote {}", id, to, path);
                    self.write_jobs.push(fs::TransferJob::new_write(
                        id,
                        path.clone(),
                        to,
                        file_num,
                        include_hidden,
                        is_remote,
                        Vec::new(),
                        od,
                    ));
                    allow_err!(
                        peer.send(&fs::new_send(id, path, file_num, include_hidden))
                            .await
                    );
                } else {
                    match fs::TransferJob::new_read(
                        id,
                        to.clone(),
                        path.clone(),
                        file_num,
                        include_hidden,
                        is_remote,
                        od,
                    ) {
                        Err(err) => {
                            self.handle_job_status(id, -1, Some(err.to_string()));
                        }
                        Ok(job) => {
                            log::debug!(
                                "New job {}, read {} to remote {}, {} files",
                                id,
                                path,
                                to,
                                job.files().len()
                            );
                            self.handler.update_folder_files(
                                job.id(),
                                job.files(),
                                path,
                                !is_remote,
                                true,
                            );
                            #[cfg(not(windows))]
                            let files = job.files().clone();
                            #[cfg(windows)]
                            let mut files = job.files().clone();
                            #[cfg(windows)]
                            if self.handler.peer_platform() != "Windows" {
                                // peer is not windows, need transform \ to /
                                fs::transform_windows_path(&mut files);
                            }
                            self.read_jobs.push(job);
                            self.timer = time::interval(MILLI1);
                            allow_err!(peer.send(&fs::new_receive(id, to, file_num, files)).await);
                        }
                    }
                }
            }
            Data::AddJob((id, path, to, file_num, include_hidden, is_remote)) => {
                let od = can_enable_overwrite_detection(self.handler.lc.read().unwrap().version);
                if is_remote {
                    log::debug!(
                        "new write waiting job {}, write to {} from remote {}",
                        id,
                        to,
                        path
                    );
                    let mut job = fs::TransferJob::new_write(
                        id,
                        path.clone(),
                        to,
                        file_num,
                        include_hidden,
                        is_remote,
                        Vec::new(),
                        od,
                    );
                    job.is_last_job = true;
                    self.write_jobs.push(job);
                } else {
                    match fs::TransferJob::new_read(
                        id,
                        to.clone(),
                        path.clone(),
                        file_num,
                        include_hidden,
                        is_remote,
                        od,
                    ) {
                        Err(err) => {
                            self.handle_job_status(id, -1, Some(err.to_string()));
                        }
                        Ok(mut job) => {
                            log::debug!(
                                "new read waiting job {}, read {} to remote {}, {} files",
                                id,
                                path,
                                to,
                                job.files().len()
                            );
                            self.handler.update_folder_files(
                                job.id(),
                                job.files(),
                                path,
                                !is_remote,
                                true,
                            );
                            job.is_last_job = true;
                            self.read_jobs.push(job);
                            self.timer = time::interval(MILLI1);
                        }
                    }
                }
            }
            Data::ResumeJob((id, is_remote)) => {
                if is_remote {
                    if let Some(job) = get_job(id, &mut self.write_jobs) {
                        job.is_last_job = false;
                        allow_err!(
                            peer.send(&fs::new_send(
                                id,
                                job.remote.clone(),
                                job.file_num,
                                job.show_hidden
                            ))
                            .await
                        );
                    }
                } else {
                    if let Some(job) = get_job(id, &mut self.read_jobs) {
                        job.is_last_job = false;
                        allow_err!(
                            peer.send(&fs::new_receive(
                                id,
                                job.path.to_string_lossy().to_string(),
                                job.file_num,
                                job.files.clone()
                            ))
                            .await
                        );
                    }
                }
            }
            Data::SetNoConfirm(id) => {
                if let Some(job) = self.remove_jobs.get_mut(&id) {
                    job.no_confirm = true;
                }
            }
            Data::ConfirmDeleteFiles((id, file_num)) => {
                if let Some(job) = self.remove_jobs.get_mut(&id) {
                    let i = file_num as usize;
                    if i < job.files.len() {
                        self.handler.ui_handler.confirm_delete_files(
                            id,
                            file_num,
                            job.files[i].name.clone(),
                        );
                    }
                }
            }
            Data::SetConfirmOverrideFile((id, file_num, need_override, remember, is_upload)) => {
                if is_upload {
                    if let Some(job) = fs::get_job(id, &mut self.read_jobs) {
                        if remember {
                            job.set_overwrite_strategy(Some(need_override));
                        }
                        job.confirm(&FileTransferSendConfirmRequest {
                            id,
                            file_num,
                            union: if need_override {
                                Some(file_transfer_send_confirm_request::Union::OffsetBlk(0))
                            } else {
                                Some(file_transfer_send_confirm_request::Union::Skip(true))
                            },
                            ..Default::default()
                        });
                    }
                } else {
                    if let Some(job) = fs::get_job(id, &mut self.write_jobs) {
                        if remember {
                            job.set_overwrite_strategy(Some(need_override));
                        }
                        let mut msg = Message::new();
                        let mut file_action = FileAction::new();
                        file_action.set_send_confirm(FileTransferSendConfirmRequest {
                            id,
                            file_num,
                            union: if need_override {
                                Some(file_transfer_send_confirm_request::Union::OffsetBlk(0))
                            } else {
                                Some(file_transfer_send_confirm_request::Union::Skip(true))
                            },
                            ..Default::default()
                        });
                        msg.set_file_action(file_action);
                        allow_err!(peer.send(&msg).await);
                    }
                }
            }
            Data::RemoveDirAll((id, path, is_remote, include_hidden)) => {
                let sep = self.handler.get_path_sep(is_remote);
                if is_remote {
                    let mut msg_out = Message::new();
                    let mut file_action = FileAction::new();
                    file_action.set_all_files(ReadAllFiles {
                        id,
                        path: path.clone(),
                        include_hidden,
                        ..Default::default()
                    });
                    msg_out.set_file_action(file_action);
                    allow_err!(peer.send(&msg_out).await);
                    self.remove_jobs
                        .insert(id, RemoveJob::new(Vec::new(), path, sep, is_remote));
                } else {
                    match fs::get_recursive_files(&path, include_hidden) {
                        Ok(entries) => {
                            self.handler.update_folder_files(
                                id,
                                &entries,
                                path.clone(),
                                !is_remote,
                                false,
                            );
                            self.remove_jobs
                                .insert(id, RemoveJob::new(entries, path, sep, is_remote));
                        }
                        Err(err) => {
                            self.handle_job_status(id, -1, Some(err.to_string()));
                        }
                    }
                }
            }
            Data::CancelJob(id) => {
                let mut msg_out = Message::new();
                let mut file_action = FileAction::new();
                file_action.set_cancel(FileTransferCancel {
                    id: id,
                    ..Default::default()
                });
                msg_out.set_file_action(file_action);
                allow_err!(peer.send(&msg_out).await);
                if let Some(job) = fs::get_job(id, &mut self.write_jobs) {
                    job.remove_download_file();
                    fs::remove_job(id, &mut self.write_jobs);
                }
                fs::remove_job(id, &mut self.read_jobs);
                self.remove_jobs.remove(&id);
            }
            Data::RemoveDir((id, path)) => {
                let mut msg_out = Message::new();
                let mut file_action = FileAction::new();
                file_action.set_remove_dir(FileRemoveDir {
                    id,
                    path,
                    recursive: true,
                    ..Default::default()
                });
                msg_out.set_file_action(file_action);
                allow_err!(peer.send(&msg_out).await);
            }
            Data::RemoveFile((id, path, file_num, is_remote)) => {
                if is_remote {
                    let mut msg_out = Message::new();
                    let mut file_action = FileAction::new();
                    file_action.set_remove_file(FileRemoveFile {
                        id,
                        path,
                        file_num,
                        ..Default::default()
                    });
                    msg_out.set_file_action(file_action);
                    allow_err!(peer.send(&msg_out).await);
                } else {
                    match fs::remove_file(&path) {
                        Err(err) => {
                            self.handle_job_status(id, file_num, Some(err.to_string()));
                        }
                        Ok(()) => {
                            self.handle_job_status(id, file_num, None);
                        }
                    }
                }
            }
            Data::CreateDir((id, path, is_remote)) => {
                if is_remote {
                    let mut msg_out = Message::new();
                    let mut file_action = FileAction::new();
                    file_action.set_create(FileDirCreate {
                        id,
                        path,
                        ..Default::default()
                    });
                    msg_out.set_file_action(file_action);
                    allow_err!(peer.send(&msg_out).await);
                } else {
                    match fs::create_dir(&path) {
                        Err(err) => {
                            self.handle_job_status(id, -1, Some(err.to_string()));
                        }
                        Ok(()) => {
                            self.handle_job_status(id, -1, None);
                        }
                    }
                }
            }
            Data::RecordScreen(start, w, h, id) => {
                let _ = self
                    .video_sender
                    .send(MediaData::RecordScreen(start, w, h, id));
            }
            _ => {}
        }
        true
    }

    #[inline]
    fn update_job_status(
        job: &fs::TransferJob,
        elapsed: i32,
        last_update_jobs_status: &mut (Instant, HashMap<i32, u64>),
        handler: &mut Session<T>,
    ) {
        if elapsed <= 0 {
            return;
        }
        let transferred = job.transferred();
        let last_transferred = {
            if let Some(v) = last_update_jobs_status.1.get(&job.id()) {
                v.to_owned()
            } else {
                0
            }
        };
        last_update_jobs_status.1.insert(job.id(), transferred);
        let speed = (transferred - last_transferred) as f64 / (elapsed as f64 / 1000.);
        let file_num = job.file_num() - 1;
        handler.job_progress(job.id(), file_num, speed, job.finished_size() as f64);
    }

    fn update_jobs_status(&mut self) {
        let elapsed = self.last_update_jobs_status.0.elapsed().as_millis() as i32;
        if elapsed >= 1000 {
            for job in self.read_jobs.iter() {
                Self::update_job_status(
                    job,
                    elapsed,
                    &mut self.last_update_jobs_status,
                    &mut self.handler,
                );
            }
            for job in self.write_jobs.iter() {
                Self::update_job_status(
                    job,
                    elapsed,
                    &mut self.last_update_jobs_status,
                    &mut self.handler,
                );
            }
            self.last_update_jobs_status.0 = Instant::now();
        }
    }

    pub async fn sync_jobs_status_to_local(&mut self) -> bool {
        log::info!("sync transfer job status");
        let mut config: PeerConfig = self.handler.load_config();
        let mut transfer_metas = TransferSerde::default();
        for job in self.read_jobs.iter() {
            let json_str = serde_json::to_string(&job.gen_meta()).unwrap_or_default();
            transfer_metas.read_jobs.push(json_str);
        }
        for job in self.write_jobs.iter() {
            let json_str = serde_json::to_string(&job.gen_meta()).unwrap_or_default();
            transfer_metas.write_jobs.push(json_str);
        }
        log::info!("meta: {:?}", transfer_metas);
        config.transfer = transfer_metas;
        self.handler.save_config(config);
        true
    }

    async fn send_opts_after_login(&self, peer: &mut Stream) {
        if let Some(opts) = self
            .handler
            .lc
            .read()
            .unwrap()
            .get_option_message_after_login()
        {
            let mut misc = Misc::new();
            misc.set_option(opts);
            let mut msg_out = Message::new();
            msg_out.set_misc(misc);
            allow_err!(peer.send(&msg_out).await);
        }
    }

    async fn handle_msg_from_peer(&mut self, data: &[u8], peer: &mut Stream) -> bool {
        if let Ok(msg_in) = Message::parse_from_bytes(&data) {
            match msg_in.union {
                Some(message::Union::VideoFrame(vf)) => {
                    if !self.first_frame {
                        self.first_frame = true;
                        self.handler.close_success();
                        self.handler.adapt_size();
                        self.send_opts_after_login(peer).await;
                    }
                    let incomming_format = CodecFormat::from(&vf);
                    if self.video_format != incomming_format {
                        self.video_format = incomming_format.clone();
                        self.handler.update_quality_status(QualityStatus {
                            codec_format: Some(incomming_format),
                            ..Default::default()
                        })
                    };
                    self.video_sender.send(MediaData::VideoFrame(vf)).ok();
                }
                Some(message::Union::Hash(hash)) => {
                    self.handler
                        .handle_hash(&self.handler.password.clone(), hash, peer)
                        .await;
                }
                Some(message::Union::LoginResponse(lr)) => match lr.union {
                    Some(login_response::Union::Error(err)) => {
                        if !self.handler.handle_login_error(&err) {
                            return false;
                        }
                    }
                    Some(login_response::Union::PeerInfo(pi)) => {
                        self.handler.handle_peer_info(pi);
                        self.check_clipboard_file_context();
                        if !(self.handler.is_file_transfer()
                            || self.handler.is_port_forward()
                            || !SERVER_CLIPBOARD_ENABLED.load(Ordering::SeqCst)
                            || !SERVER_KEYBOARD_ENABLED.load(Ordering::SeqCst)
                            || self.handler.lc.read().unwrap().disable_clipboard)
                        {
                            let txt = self.old_clipboard.lock().unwrap().clone();
                            if !txt.is_empty() {
                                let msg_out = crate::create_clipboard_msg(txt);
                                let sender = self.sender.clone();
                                tokio::spawn(async move {
                                    // due to clipboard service interval time
                                    sleep(common::CLIPBOARD_INTERVAL as f32 / 1_000.).await;
                                    sender.send(Data::Message(msg_out)).ok();
                                });
                            }
                        }

                        if self.handler.is_file_transfer() {
                            self.handler.load_last_jobs();
                        }
                    }
                    _ => {}
                },
                Some(message::Union::CursorData(cd)) => {
                    self.handler.set_cursor_data(cd);
                }
                Some(message::Union::CursorId(id)) => {
                    self.handler.set_cursor_id(id.to_string());
                }
                Some(message::Union::CursorPosition(cp)) => {
                    self.handler.set_cursor_position(cp);
                }
                Some(message::Union::Clipboard(cb)) => {
                    if !self.handler.lc.read().unwrap().disable_clipboard {
                        #[cfg(not(any(target_os = "android", target_os = "ios")))]
                        update_clipboard(cb, Some(&self.old_clipboard));
                        #[cfg(any(target_os = "android", target_os = "ios"))]
                        {
                            let content = if cb.compress {
                                hbb_common::compress::decompress(&cb.content)
                            } else {
                                cb.content.into()
                            };
                            if let Ok(content) = String::from_utf8(content) {
                                self.handler.clipboard(content);
                            }
                        }
                    }
                }
                #[cfg(windows)]
                Some(message::Union::Cliprdr(clip)) => {
                    self.handle_cliprdr_msg(clip);
                }
                Some(message::Union::FileResponse(fr)) => {
                    match fr.union {
                        Some(file_response::Union::Dir(fd)) => {
                            #[cfg(windows)]
                            let entries = fd.entries.to_vec();
                            #[cfg(not(windows))]
                            let mut entries = fd.entries.to_vec();
                            #[cfg(not(windows))]
                            {
                                if self.handler.peer_platform() == "Windows" {
                                    fs::transform_windows_path(&mut entries);
                                }
                            }
                            self.handler
                                .update_folder_files(fd.id, &entries, fd.path, false, false);
                            if let Some(job) = fs::get_job(fd.id, &mut self.write_jobs) {
                                log::info!("job set_files: {:?}", entries);
                                job.set_files(entries);
                            } else if let Some(job) = self.remove_jobs.get_mut(&fd.id) {
                                job.files = entries;
                            }
                        }
                        Some(file_response::Union::Digest(digest)) => {
                            if digest.is_upload {
                                if let Some(job) = fs::get_job(digest.id, &mut self.read_jobs) {
                                    if let Some(file) = job.files().get(digest.file_num as usize) {
                                        let read_path = get_string(&job.join(&file.name));
                                        let overwrite_strategy = job.default_overwrite_strategy();
                                        if let Some(overwrite) = overwrite_strategy {
                                            let req = FileTransferSendConfirmRequest {
                                                id: digest.id,
                                                file_num: digest.file_num,
                                                union: Some(if overwrite {
                                                    file_transfer_send_confirm_request::Union::OffsetBlk(0)
                                                } else {
                                                    file_transfer_send_confirm_request::Union::Skip(
                                                        true,
                                                    )
                                                }),
                                                ..Default::default()
                                            };
                                            job.confirm(&req);
                                            let msg = new_send_confirm(req);
                                            allow_err!(peer.send(&msg).await);
                                        } else {
                                            self.handler.override_file_confirm(
                                                digest.id,
                                                digest.file_num,
                                                read_path,
                                                true,
                                            );
                                        }
                                    }
                                }
                            } else {
                                if let Some(job) = fs::get_job(digest.id, &mut self.write_jobs) {
                                    if let Some(file) = job.files().get(digest.file_num as usize) {
                                        let write_path = get_string(&job.join(&file.name));
                                        let overwrite_strategy = job.default_overwrite_strategy();
                                        match fs::is_write_need_confirmation(&write_path, &digest) {
                                            Ok(res) => match res {
                                                DigestCheckResult::IsSame => {
                                                    let msg= new_send_confirm(FileTransferSendConfirmRequest {
                                                        id: digest.id,
                                                        file_num: digest.file_num,
                                                        union: Some(file_transfer_send_confirm_request::Union::Skip(true)),
                                                        ..Default::default()
                                                    });
                                                    allow_err!(peer.send(&msg).await);
                                                }
                                                DigestCheckResult::NeedConfirm(digest) => {
                                                    if let Some(overwrite) = overwrite_strategy {
                                                        let msg = new_send_confirm(
                                                            FileTransferSendConfirmRequest {
                                                                id: digest.id,
                                                                file_num: digest.file_num,
                                                                union: Some(if overwrite {
                                                                    file_transfer_send_confirm_request::Union::OffsetBlk(0)
                                                                } else {
                                                                    file_transfer_send_confirm_request::Union::Skip(true)
                                                                }),
                                                                ..Default::default()
                                                            },
                                                        );
                                                        allow_err!(peer.send(&msg).await);
                                                    } else {
                                                        self.handler.override_file_confirm(
                                                            digest.id,
                                                            digest.file_num,
                                                            write_path,
                                                            false,
                                                        );
                                                    }
                                                }
                                                DigestCheckResult::NoSuchFile => {
                                                    let msg = new_send_confirm(
                                                    FileTransferSendConfirmRequest {
                                                        id: digest.id,
                                                        file_num: digest.file_num,
                                                        union: Some(file_transfer_send_confirm_request::Union::OffsetBlk(0)),
                                                        ..Default::default()
                                                    },
                                                );
                                                    allow_err!(peer.send(&msg).await);
                                                }
                                            },
                                            Err(err) => {
                                                println!("error recving digest: {}", err);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Some(file_response::Union::Block(block)) => {
                            log::info!(
                                "file response block, file id:{}, file num: {}",
                                block.id,
                                block.file_num
                            );
                            if let Some(job) = fs::get_job(block.id, &mut self.write_jobs) {
                                if let Err(_err) = job.write(block, None).await {
                                    // to-do: add "skip" for writing job
                                }
                                self.update_jobs_status();
                            }
                        }
                        Some(file_response::Union::Done(d)) => {
                            if let Some(job) = fs::get_job(d.id, &mut self.write_jobs) {
                                job.modify_time();
                                fs::remove_job(d.id, &mut self.write_jobs);
                            }
                            self.handle_job_status(d.id, d.file_num, None);
                        }
                        Some(file_response::Union::Error(e)) => {
                            self.handle_job_status(e.id, e.file_num, Some(e.error));
                        }
                        _ => {}
                    }
                }
                Some(message::Union::Misc(misc)) => match misc.union {
                    Some(misc::Union::AudioFormat(f)) => {
                        self.audio_sender.send(MediaData::AudioFormat(f)).ok();
                    }
                    Some(misc::Union::ChatMessage(c)) => {
                        self.handler.new_message(c.text);
                    }
                    Some(misc::Union::PermissionInfo(p)) => {
                        log::info!("Change permission {:?} -> {}", p.permission, p.enabled);
                        match p.permission.enum_value_or_default() {
                            Permission::Keyboard => {
                                SERVER_KEYBOARD_ENABLED.store(p.enabled, Ordering::SeqCst);
                                self.handler.set_permission("keyboard", p.enabled);
                            }
                            Permission::Clipboard => {
                                SERVER_CLIPBOARD_ENABLED.store(p.enabled, Ordering::SeqCst);
                                self.handler.set_permission("clipboard", p.enabled);
                            }
                            Permission::Audio => {
                                self.handler.set_permission("audio", p.enabled);
                            }
                            Permission::File => {
                                SERVER_FILE_TRANSFER_ENABLED.store(p.enabled, Ordering::SeqCst);
                                if !p.enabled && self.handler.is_file_transfer() {
                                    return true;
                                }
                                self.check_clipboard_file_context();
                                self.handler.set_permission("file", p.enabled);
                            }
                            Permission::Restart => {
                                self.handler.set_permission("restart", p.enabled);
                            }
                            Permission::Recording => {
                                self.handler.set_permission("recording", p.enabled);
                            }
                        }
                    }
                    Some(misc::Union::SwitchDisplay(s)) => {
                        self.handler.ui_handler.switch_display(&s);
                        self.video_sender.send(MediaData::Reset).ok();
                        if s.width > 0 && s.height > 0 {
                            self.handler.set_display(s.x, s.y, s.width, s.height);
                        }
                    }
                    Some(misc::Union::CloseReason(c)) => {
                        self.handler.msgbox("error", "Connection Error", &c, "");
                        return false;
                    }
                    Some(misc::Union::BackNotification(notification)) => {
                        if !self.handle_back_notification(notification).await {
                            return false;
                        }
                    }
                    Some(misc::Union::Uac(uac)) => {
                        if uac {
                            self.handler.msgbox(
                                "custom-uac-nocancel",
                                "Warning",
                                "uac_warning",
                                "",
                            );
                        }
                    }
                    Some(misc::Union::ForegroundWindowElevated(elevated)) => {
                        if elevated {
                            self.handler.msgbox(
                                "custom-elevated-foreground-nocancel",
                                "Warning",
                                "elevated_foreground_window_warning",
                                "",
                            );
                        }
                    }
                    _ => {}
                },
                Some(message::Union::TestDelay(t)) => {
                    self.handler.handle_test_delay(t, peer).await;
                }
                Some(message::Union::AudioFrame(frame)) => {
                    if !self.handler.lc.read().unwrap().disable_audio {
                        self.audio_sender.send(MediaData::AudioFrame(frame)).ok();
                    }
                }
                Some(message::Union::FileAction(action)) => match action.union {
                    Some(file_action::Union::SendConfirm(c)) => {
                        if let Some(job) = fs::get_job(c.id, &mut self.read_jobs) {
                            job.confirm(&c);
                        }
                    }
                    _ => {}
                },
                Some(message::Union::MessageBox(msgbox)) => {
                    let mut link = msgbox.link;
                    if !link.starts_with("rustdesk://") {
                        if let Some(v) = hbb_common::config::HELPER_URL.get(&link as &str) {
                            link = v.to_string();
                        } else {
                            log::warn!("Message box ignore link {} for security", &link);
                            link = "".to_string();
                        }
                    }
                    self.handler
                        .msgbox(&msgbox.msgtype, &msgbox.title, &msgbox.text, &link);
                }
                _ => {}
            }
        }
        true
    }

    async fn handle_back_notification(&mut self, notification: BackNotification) -> bool {
        match notification.union {
            Some(back_notification::Union::BlockInputState(state)) => {
                self.handle_back_msg_block_input(
                    state.enum_value_or(back_notification::BlockInputState::BlkStateUnknown),
                )
                .await;
            }
            Some(back_notification::Union::PrivacyModeState(state)) => {
                if !self
                    .handle_back_msg_privacy_mode(
                        state.enum_value_or(back_notification::PrivacyModeState::PrvStateUnknown),
                    )
                    .await
                {
                    return false;
                }
            }
            _ => {}
        }
        true
    }

    #[inline(always)]
    fn update_block_input_state(&mut self, on: bool) {
        self.handler.update_block_input_state(on);
    }

    async fn handle_back_msg_block_input(&mut self, state: back_notification::BlockInputState) {
        match state {
            back_notification::BlockInputState::BlkOnSucceeded => {
                self.update_block_input_state(true);
            }
            back_notification::BlockInputState::BlkOnFailed => {
                self.handler
                    .msgbox("custom-error", "Block user input", "Failed", "");
                self.update_block_input_state(false);
            }
            back_notification::BlockInputState::BlkOffSucceeded => {
                self.update_block_input_state(false);
            }
            back_notification::BlockInputState::BlkOffFailed => {
                self.handler
                    .msgbox("custom-error", "Unblock user input", "Failed", "");
            }
            _ => {}
        }
    }

    #[inline(always)]
    fn update_privacy_mode(&mut self, on: bool) {
        let mut config = self.handler.load_config();
        config.privacy_mode = on;
        self.handler.save_config(config);

        self.handler.update_privacy_mode();
    }

    async fn handle_back_msg_privacy_mode(
        &mut self,
        state: back_notification::PrivacyModeState,
    ) -> bool {
        match state {
            back_notification::PrivacyModeState::PrvOnByOther => {
                self.handler.msgbox(
                    "error",
                    "Connecting...",
                    "Someone turns on privacy mode, exit",
                    "",
                );
                return false;
            }
            back_notification::PrivacyModeState::PrvNotSupported => {
                self.handler
                    .msgbox("custom-error", "Privacy mode", "Unsupported", "");
                self.update_privacy_mode(false);
            }
            back_notification::PrivacyModeState::PrvOnSucceeded => {
                self.handler
                    .msgbox("custom-nocancel", "Privacy mode", "In privacy mode", "");
                self.update_privacy_mode(true);
            }
            back_notification::PrivacyModeState::PrvOnFailedDenied => {
                self.handler
                    .msgbox("custom-error", "Privacy mode", "Peer denied", "");
                self.update_privacy_mode(false);
            }
            back_notification::PrivacyModeState::PrvOnFailedPlugin => {
                self.handler
                    .msgbox("custom-error", "Privacy mode", "Please install plugins", "");
                self.update_privacy_mode(false);
            }
            back_notification::PrivacyModeState::PrvOnFailed => {
                self.handler
                    .msgbox("custom-error", "Privacy mode", "Failed", "");
                self.update_privacy_mode(false);
            }
            back_notification::PrivacyModeState::PrvOffSucceeded => {
                self.handler
                    .msgbox("custom-nocancel", "Privacy mode", "Out privacy mode", "");
                self.update_privacy_mode(false);
            }
            back_notification::PrivacyModeState::PrvOffByPeer => {
                self.handler
                    .msgbox("custom-error", "Privacy mode", "Peer exit", "");
                self.update_privacy_mode(false);
            }
            back_notification::PrivacyModeState::PrvOffFailed => {
                self.handler
                    .msgbox("custom-error", "Privacy mode", "Failed to turn off", "");
            }
            back_notification::PrivacyModeState::PrvOffUnknown => {
                self.handler
                    .msgbox("custom-error", "Privacy mode", "Turned off", "");
                // log::error!("Privacy mode is turned off with unknown reason");
                self.update_privacy_mode(false);
            }
            _ => {}
        }
        true
    }

    fn check_clipboard_file_context(&self) {
        #[cfg(windows)]
        {
            let enabled = SERVER_FILE_TRANSFER_ENABLED.load(Ordering::SeqCst)
                && self.handler.lc.read().unwrap().enable_file_transfer;
            ContextSend::enable(enabled);
        }
    }

    #[cfg(windows)]
    fn handle_cliprdr_msg(&self, clip: hbb_common::message_proto::Cliprdr) {
        if !self.handler.lc.read().unwrap().disable_clipboard {
            #[cfg(any(target_os = "android", target_os = "ios", feature = "flutter"))]
            if let Some(hbb_common::message_proto::cliprdr::Union::FormatList(_)) = &clip.union {
                if self.client_conn_id
                    != clipboard::get_client_conn_id(&crate::flutter::get_cur_session_id())
                        .unwrap_or(0)
                {
                    return;
                }
            }

            if let Some(clip) = crate::clipboard_file::msg_2_clip(clip) {
                ContextSend::proc(|context: &mut Box<CliprdrClientContext>| -> u32 {
                    clipboard::server_clip_file(context, self.client_conn_id, clip)
                });
            }
        }
    }
}

struct RemoveJob {
    files: Vec<FileEntry>,
    path: String,
    sep: &'static str,
    is_remote: bool,
    no_confirm: bool,
    last_update_job_status: Instant,
}

impl RemoveJob {
    fn new(files: Vec<FileEntry>, path: String, sep: &'static str, is_remote: bool) -> Self {
        Self {
            files,
            path,
            sep,
            is_remote,
            no_confirm: false,
            last_update_job_status: Instant::now(),
        }
    }

    pub fn _gen_meta(&self) -> RemoveJobMeta {
        RemoveJobMeta {
            path: self.path.clone(),
            is_remote: self.is_remote,
            no_confirm: self.no_confirm,
        }
    }
}
