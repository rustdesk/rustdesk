use std::collections::HashMap;
use std::num::NonZeroI64;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[cfg(windows)]
use clipboard::{cliprdr::CliprdrClientContext, ContextSend};
use crossbeam_queue::ArrayQueue;
use hbb_common::config::{PeerConfig, TransferSerde};
use hbb_common::fs::{
    can_enable_overwrite_detection, get_job, get_string, new_send_confirm, DigestCheckResult,
    RemoveJobMeta,
};
use hbb_common::message_proto::permission_info::Permission;
use hbb_common::protobuf::Message as _;
use hbb_common::rendezvous_proto::ConnType;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use hbb_common::sleep;
use hbb_common::tokio::sync::mpsc::error::TryRecvError;
#[cfg(windows)]
use hbb_common::tokio::sync::Mutex as TokioMutex;
use hbb_common::tokio::{
    self,
    sync::mpsc,
    time::{self, Duration, Instant, Interval},
};
use hbb_common::{allow_err, fs, get_time, log, message_proto::*, Stream};
use scrap::CodecFormat;

use crate::client::{
    new_voice_call_request, Client, MediaData, MediaSender, QualityStatus, MILLI1, SEC30,
};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::common::{self, update_clipboard};
use crate::common::{get_default_sound_input, set_sound_input};
use crate::ui_session_interface::{InvokeUiSession, Session};
#[cfg(not(any(target_os = "ios")))]
use crate::{audio_service, ConnInner, CLIENT_SERVER};
use crate::{client::Data, client::Interface};

pub struct Remote<T: InvokeUiSession> {
    handler: Session<T>,
    video_queue: Arc<ArrayQueue<VideoFrame>>,
    video_sender: MediaSender,
    audio_sender: MediaSender,
    receiver: mpsc::UnboundedReceiver<Data>,
    sender: mpsc::UnboundedSender<Data>,
    // Stop sending local audio to remote client.
    stop_voice_call_sender: Option<std::sync::mpsc::Sender<()>>,
    voice_call_request_timestamp: Option<NonZeroI64>,
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
    elevation_requested: bool,
    fps_control: FpsControl,
    decode_fps: Arc<AtomicUsize>,
}

impl<T: InvokeUiSession> Remote<T> {
    pub fn new(
        handler: Session<T>,
        video_queue: Arc<ArrayQueue<VideoFrame>>,
        video_sender: MediaSender,
        audio_sender: MediaSender,
        receiver: mpsc::UnboundedReceiver<Data>,
        sender: mpsc::UnboundedSender<Data>,
        frame_count: Arc<AtomicUsize>,
        decode_fps: Arc<AtomicUsize>,
    ) -> Self {
        Self {
            handler,
            video_queue,
            video_sender,
            audio_sender,
            receiver,
            sender,
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
            stop_voice_call_sender: None,
            voice_call_request_timestamp: None,
            elevation_requested: false,
            fps_control: Default::default(),
            decode_fps,
        }
    }

    pub async fn io_loop(&mut self, key: &str, token: &str) {
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
            Ok((mut peer, direct, pk)) => {
                self.handler.set_connection_type(peer.is_secured(), direct); // flutter -> connection_ready
                self.handler.set_connection_info(direct, false);
                self.handler
                    .set_fingerprint(crate::common::pk_to_fingerprint(pk.unwrap_or_default()));

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
                let mut fps_instant = Instant::now();

                loop {
                    tokio::select! {
                        res = peer.next() => {
                            if let Some(res) = res {
                                match res {
                                    Err(err) => {
                                        log::error!("Connection closed: {}", err);
                                        self.handler.set_force_relay(direct, received);
                                        let msgtype = "error";
                                        let title = "Connection Error";
                                        let text = err.to_string();
                                        let show_relay_hint = self.handler.show_relay_hint(last_recv_time, msgtype, title, &text);
                                        if show_relay_hint{
                                            self.handler.msgbox("relay-hint", title, &text, "");
                                        } else {
                                            self.handler.msgbox(msgtype, title, &text, "");
                                        }
                                        break;
                                    }
                                    Ok(ref bytes) => {
                                        last_recv_time = Instant::now();
                                        if !received {
                                            received = true;
                                            self.handler.set_connection_info(direct, true);
                                        }
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
                            self.fps_control();
                            let elapsed = fps_instant.elapsed().as_millis();
                            if elapsed < 1000 {
                                continue;
                            }
                            fps_instant = Instant::now();
                            let mut speed = self.data_count.swap(0, Ordering::Relaxed);
                            speed = speed * 1000 / elapsed as usize;
                            let speed = format!("{:.2}kB/s", speed as f32 / 1024 as f32);
                            let mut fps = self.frame_count.swap(0, Ordering::Relaxed) as _;
                            // Correcting the inaccuracy of status_timer
                            fps = fps * 1000 / elapsed as i32;
                            self.handler.update_quality_status(QualityStatus {
                                speed:Some(speed),
                                fps:Some(fps),
                                ..Default::default()
                            });
                        }
                    }
                }
                log::debug!("Exit io_loop of id={}", self.handler.id);
                // Stop client audio server.
                if let Some(s) = self.stop_voice_call_sender.take() {
                    s.send(()).ok();
                }
            }
            Err(err) => {
                self.handler
                    .msgbox("error", "Connection Error", &err.to_string(), "");
            }
        }
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        Client::try_stop_clipboard(&self.handler.id);
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

    fn stop_voice_call(&mut self) {
        let voice_call_sender = std::mem::replace(&mut self.stop_voice_call_sender, None);
        if let Some(stopper) = voice_call_sender {
            let _ = stopper.send(());
        }
    }

    // Start a voice call recorder, records audio and send to remote
    fn start_voice_call(&mut self) -> Option<std::sync::mpsc::Sender<()>> {
        if self.handler.is_file_transfer() || self.handler.is_port_forward() {
            return None;
        }
        // Switch to default input device
        let default_sound_device = get_default_sound_input();
        if let Some(device) = default_sound_device {
            set_sound_input(device);
        }
        // iOS does not have this server.
        #[cfg(not(any(target_os = "ios")))]
        {
            // Create a channel to receive error or closed message
            let (tx, rx) = std::sync::mpsc::channel();
            let (tx_audio_data, mut rx_audio_data) =
                hbb_common::tokio::sync::mpsc::unbounded_channel();
            // Create a stand-alone inner, add subscribe to audio service
            let conn_id = CLIENT_SERVER.write().unwrap().get_new_id();
            let client_conn_inner = ConnInner::new(conn_id.clone(), Some(tx_audio_data), None);
            // now we subscribe
            CLIENT_SERVER.write().unwrap().subscribe(
                audio_service::NAME,
                client_conn_inner.clone(),
                true,
            );
            let tx_audio = self.sender.clone();
            std::thread::spawn(move || {
                loop {
                    // check if client is closed
                    match rx.try_recv() {
                        Ok(_) | Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                            log::debug!("Exit voice call audio service of client");
                            // unsubscribe
                            CLIENT_SERVER.write().unwrap().subscribe(
                                audio_service::NAME,
                                client_conn_inner,
                                false,
                            );
                            break;
                        }
                        _ => {}
                    }
                    match rx_audio_data.try_recv() {
                        Ok((_instant, msg)) => match &msg.union {
                            Some(message::Union::AudioFrame(frame)) => {
                                let mut msg = Message::new();
                                msg.set_audio_frame(frame.clone());
                                tx_audio.send(Data::Message(msg)).ok();
                            }
                            Some(message::Union::Misc(misc)) => {
                                let mut msg = Message::new();
                                msg.set_misc(misc.clone());
                                tx_audio.send(Data::Message(msg)).ok();
                            }
                            _ => {}
                        },
                        Err(err) => {
                            if err == TryRecvError::Empty {
                                // ignore
                            } else {
                                log::debug!("Failed to record local audio channel: {}", err);
                            }
                        }
                    }
                }
            });
            return Some(tx);
        }
        #[cfg(target_os = "ios")]
        {
            None
        }
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
            Data::Login((os_username, os_password, password, remember)) => {
                self.handler
                    .handle_login_from_ui(os_username, os_password, password, remember, peer)
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
                        let req = FileTransferSendConfirmRequest {
                            id,
                            file_num,
                            union: if need_override {
                                Some(file_transfer_send_confirm_request::Union::OffsetBlk(0))
                            } else {
                                Some(file_transfer_send_confirm_request::Union::Skip(true))
                            },
                            ..Default::default()
                        };
                        job.confirm(&req);
                        file_action.set_send_confirm(req);
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
            Data::ElevateDirect => {
                let mut request = ElevationRequest::new();
                request.set_direct(true);
                let mut misc = Misc::new();
                misc.set_elevation_request(request);
                let mut msg = Message::new();
                msg.set_misc(misc);
                allow_err!(peer.send(&msg).await);
                self.elevation_requested = true;
            }
            Data::ElevateWithLogon(username, password) => {
                let mut request = ElevationRequest::new();
                request.set_logon(ElevationRequestWithLogon {
                    username,
                    password,
                    ..Default::default()
                });
                let mut misc = Misc::new();
                misc.set_elevation_request(request);
                let mut msg = Message::new();
                msg.set_misc(misc);
                allow_err!(peer.send(&msg).await);
                self.elevation_requested = true;
            }
            Data::NewVoiceCall => {
                let msg = new_voice_call_request(true);
                // Save the voice call request timestamp for the further validation.
                self.voice_call_request_timestamp = Some(
                    NonZeroI64::new(msg.voice_call_request().req_timestamp)
                        .unwrap_or(NonZeroI64::new(get_time()).unwrap()),
                );
                allow_err!(peer.send(&msg).await);
                self.handler.on_voice_call_waiting();
            }
            Data::CloseVoiceCall => {
                self.stop_voice_call();
                let msg = new_voice_call_request(false);
                self.handler
                    .on_voice_call_closed("Closed manually by the peer");
                allow_err!(peer.send(&msg).await);
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

    fn contains_key_frame(vf: &VideoFrame) -> bool {
        use video_frame::Union::*;
        match &vf.union {
            Some(vf) => match vf {
                Vp8s(f) | Vp9s(f) | H264s(f) | H265s(f) => f.frames.iter().any(|e| e.key),
                _ => false,
            },
            None => false,
        }
    }
    #[inline]
    fn fps_control(&mut self) {
        let len = self.video_queue.len();
        let ctl = &mut self.fps_control;
        // Current full speed decoding fps
        let decode_fps = self.decode_fps.load(std::sync::atomic::Ordering::Relaxed);
        // 500ms
        let debounce = if decode_fps > 10 { decode_fps / 2 } else { 5 };
        if len < debounce || decode_fps == 0 {
            return;
        }
        // First setting , or the length of the queue still increases after setting, or exceed the size of the last setting again
        if ctl.set_times < 10 // enough
            && (ctl.set_times == 0
                || (len > ctl.last_queue_size && ctl.last_set_instant.elapsed().as_secs() > 30))
        {
            // 80% fps to ensure decoding is faster than encoding
            let mut custom_fps = decode_fps as i32 * 4 / 5;
            if custom_fps < 1 {
                custom_fps = 1;
            }
            // send custom fps
            let mut misc = Misc::new();
            misc.set_option(OptionMessage {
                custom_fps,
                ..Default::default()
            });
            let mut msg = Message::new();
            msg.set_misc(misc);
            self.sender.send(Data::Message(msg)).ok();
            ctl.last_queue_size = len;
            ctl.set_times += 1;
            ctl.last_set_instant = Instant::now();
        }
        // send refresh
        if ctl.refresh_times < 10 // enough
            && (len > self.video_queue.capacity() / 2
                    && (ctl.refresh_times == 0 || ctl.last_refresh_instant.elapsed().as_secs() > 30))
        {
            // Refresh causes client set_display, left frames cause flickering.
            while let Some(_) = self.video_queue.pop() {}
            self.handler.refresh_video();
            ctl.refresh_times += 1;
            ctl.last_refresh_instant = Instant::now();
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
                    let incoming_format = CodecFormat::from(&vf);
                    if self.video_format != incoming_format {
                        self.video_format = incoming_format.clone();
                        self.handler.update_quality_status(QualityStatus {
                            codec_format: Some(incoming_format),
                            ..Default::default()
                        })
                    };
                    if Self::contains_key_frame(&vf) {
                        while let Some(_) = self.video_queue.pop() {}
                        self.video_sender
                            .send(MediaData::VideoFrame(Box::new(vf)))
                            .ok();
                    } else {
                        self.video_queue.force_push(vf);
                        self.video_sender.send(MediaData::VideoQueue).ok();
                    }
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
                        if !(self.handler.is_file_transfer() || self.handler.is_port_forward()) {
                            #[cfg(feature = "flutter")]
                            #[cfg(not(any(target_os = "android", target_os = "ios")))]
                            Client::try_start_clipboard(None);
                            #[cfg(not(feature = "flutter"))]
                            #[cfg(not(any(target_os = "android", target_os = "ios")))]
                            Client::try_start_clipboard(Some(
                                crate::client::ClientClipboardContext {
                                    cfg: self.handler.get_permission_config(),
                                    tx: self.sender.clone(),
                                },
                            ));

                            #[cfg(not(any(target_os = "android", target_os = "ios")))]
                            if let Some(msg_out) = Client::get_current_text_clipboard_msg() {
                                let sender = self.sender.clone();
                                let permission_config = self.handler.get_permission_config();
                                tokio::spawn(async move {
                                    // due to clipboard service interval time
                                    sleep(common::CLIPBOARD_INTERVAL as f32 / 1_000.).await;
                                    if permission_config.is_text_clipboard_required() {
                                        sender.send(Data::Message(msg_out)).ok();
                                    }
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
                    if !self.handler.lc.read().unwrap().disable_clipboard.v {
                        #[cfg(not(any(target_os = "android", target_os = "ios")))]
                        update_clipboard(cb, Some(&crate::client::get_old_clipboard_text()));
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
                                                digest.is_identical,
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
                                                    let req = FileTransferSendConfirmRequest {
                                                        id: digest.id,
                                                        file_num: digest.file_num,
                                                        union: Some(file_transfer_send_confirm_request::Union::Skip(true)),
                                                        ..Default::default()
                                                    };
                                                    job.confirm(&req);
                                                    let msg = new_send_confirm(req);
                                                    allow_err!(peer.send(&msg).await);
                                                }
                                                DigestCheckResult::NeedConfirm(digest) => {
                                                    if let Some(overwrite) = overwrite_strategy {
                                                        let req = FileTransferSendConfirmRequest {
                                                            id: digest.id,
                                                            file_num: digest.file_num,
                                                            union: Some(if overwrite {
                                                                file_transfer_send_confirm_request::Union::OffsetBlk(0)
                                                            } else {
                                                                file_transfer_send_confirm_request::Union::Skip(true)
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
                                                            write_path,
                                                            false,
                                                            digest.is_identical,
                                                        );
                                                    }
                                                }
                                                DigestCheckResult::NoSuchFile => {
                                                    let req = FileTransferSendConfirmRequest {
                                                        id: digest.id,
                                                        file_num: digest.file_num,
                                                        union: Some(file_transfer_send_confirm_request::Union::OffsetBlk(0)),
                                                        ..Default::default()
                                                    };
                                                    job.confirm(&req);
                                                    let msg = new_send_confirm(req);
                                                    allow_err!(peer.send(&msg).await);
                                                }
                                            },
                                            Err(err) => {
                                                println!("error receiving digest: {}", err);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Some(file_response::Union::Block(block)) => {
                            if let Some(job) = fs::get_job(block.id, &mut self.write_jobs) {
                                if let Err(_err) = job.write(block).await {
                                    // to-do: add "skip" for writing job
                                }
                                self.update_jobs_status();
                            }
                        }
                        Some(file_response::Union::Done(d)) => {
                            let mut err: Option<String> = None;
                            if let Some(job) = fs::get_job(d.id, &mut self.write_jobs) {
                                job.modify_time();
                                err = job.job_error();
                                fs::remove_job(d.id, &mut self.write_jobs);
                            }
                            self.handle_job_status(d.id, d.file_num, err);
                        }
                        Some(file_response::Union::Error(e)) => {
                            if let Some(_job) = fs::get_job(e.id, &mut self.write_jobs) {
                                fs::remove_job(e.id, &mut self.write_jobs);
                            }
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
                        // https://github.com/rustdesk/rustdesk/issues/3703#issuecomment-1474734754
                        match p.permission.enum_value() {
                            Ok(Permission::Keyboard) => {
                                #[cfg(feature = "flutter")]
                                #[cfg(not(any(target_os = "android", target_os = "ios")))]
                                crate::flutter::update_text_clipboard_required();
                                *self.handler.server_keyboard_enabled.write().unwrap() = p.enabled;
                                self.handler.set_permission("keyboard", p.enabled);
                            }
                            Ok(Permission::Clipboard) => {
                                #[cfg(feature = "flutter")]
                                #[cfg(not(any(target_os = "android", target_os = "ios")))]
                                crate::flutter::update_text_clipboard_required();
                                *self.handler.server_clipboard_enabled.write().unwrap() = p.enabled;
                                self.handler.set_permission("clipboard", p.enabled);
                            }
                            Ok(Permission::Audio) => {
                                self.handler.set_permission("audio", p.enabled);
                            }
                            Ok(Permission::File) => {
                                *self.handler.server_file_transfer_enabled.write().unwrap() =
                                    p.enabled;
                                if !p.enabled && self.handler.is_file_transfer() {
                                    return true;
                                }
                                self.check_clipboard_file_context();
                                self.handler.set_permission("file", p.enabled);
                            }
                            Ok(Permission::Restart) => {
                                self.handler.set_permission("restart", p.enabled);
                            }
                            Ok(Permission::Recording) => {
                                self.handler.set_permission("recording", p.enabled);
                            }
                            _ => {}
                        }
                    }
                    Some(misc::Union::SwitchDisplay(s)) => {
                        self.handler.ui_handler.switch_display(&s);
                        self.video_sender.send(MediaData::Reset).ok();
                        if s.width > 0 && s.height > 0 {
                            self.handler.set_display(
                                s.x,
                                s.y,
                                s.width,
                                s.height,
                                s.cursor_embedded,
                            );
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
                        #[cfg(feature = "flutter")]
                        {
                            if uac {
                                self.handler.msgbox(
                                    "on-uac",
                                    "Prompt",
                                    "Please wait for confirmation of UAC...",
                                    "",
                                );
                            } else {
                                self.handler.cancel_msgbox("on-uac");
                                self.handler.cancel_msgbox("wait-uac");
                                self.handler.cancel_msgbox("elevation-error");
                            }
                        }
                        #[cfg(not(feature = "flutter"))]
                        {
                            let msgtype = "custom-uac-nocancel";
                            let title = "Prompt";
                            let text = "Please wait for confirmation of UAC...";
                            let link = "";
                            if uac {
                                self.handler.msgbox(msgtype, title, text, link);
                            } else {
                                self.handler.cancel_msgbox(&format!(
                                    "{}-{}-{}-{}",
                                    msgtype, title, text, link,
                                ));
                            }
                        }
                    }
                    Some(misc::Union::ForegroundWindowElevated(elevated)) => {
                        #[cfg(feature = "flutter")]
                        {
                            if elevated {
                                self.handler.msgbox(
                                    "on-foreground-elevated",
                                    "Prompt",
                                    "elevated_foreground_window_tip",
                                    "",
                                );
                            } else {
                                self.handler.cancel_msgbox("on-foreground-elevated");
                                self.handler.cancel_msgbox("wait-uac");
                                self.handler.cancel_msgbox("elevation-error");
                            }
                        }
                        #[cfg(not(feature = "flutter"))]
                        {
                            let msgtype = "custom-elevated-foreground-nocancel";
                            let title = "Prompt";
                            let text = "elevated_foreground_window_tip";
                            let link = "";
                            if elevated {
                                self.handler.msgbox(msgtype, title, text, link);
                            } else {
                                self.handler.cancel_msgbox(&format!(
                                    "{}-{}-{}-{}",
                                    msgtype, title, text, link,
                                ));
                            }
                        }
                    }
                    Some(misc::Union::ElevationResponse(err)) => {
                        if err.is_empty() {
                            self.handler.msgbox("wait-uac", "", "", "");
                        } else {
                            self.handler
                                .msgbox("elevation-error", "Elevation Error", &err, "");
                        }
                    }
                    Some(misc::Union::PortableServiceRunning(b)) => {
                        self.handler.portable_service_running(b);
                        if self.elevation_requested && b {
                            self.handler.msgbox(
                                "custom-nocancel-success",
                                "Successful",
                                "Elevate successfully",
                                "",
                            );
                        }
                    }
                    Some(misc::Union::SwitchBack(_)) => {
                        #[cfg(feature = "flutter")]
                        self.handler.switch_back(&self.handler.id);
                    }
                    #[cfg(all(feature = "flutter", feature = "plugin_framework"))]
                    #[cfg(not(any(target_os = "android", target_os = "ios")))]
                    Some(misc::Union::PluginRequest(p)) => {
                        allow_err!(crate::plugin::handle_server_event(&p.id, &self.handler.id, &p.content));
                        // to-do: show message box on UI when error occurs?
                    }
                    #[cfg(all(feature = "flutter", feature = "plugin_framework"))]
                    #[cfg(not(any(target_os = "android", target_os = "ios")))]
                    Some(misc::Union::PluginResponse(p)) => {
                        let name = if p.name.is_empty() {
                            "plugin".to_string()
                        } else {
                            p.name
                        };
                        self.handler.msgbox("custom-nocancel", &name, &p.msg, "");
                    }
                    _ => {}
                },
                Some(message::Union::TestDelay(t)) => {
                    self.handler.handle_test_delay(t, peer).await;
                }
                Some(message::Union::AudioFrame(frame)) => {
                    if !self.handler.lc.read().unwrap().disable_audio.v {
                        self.audio_sender
                            .send(MediaData::AudioFrame(Box::new(frame)))
                            .ok();
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
                    // Links from the remote side must be verified.
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
                Some(message::Union::VoiceCallRequest(request)) => {
                    if request.is_connect {
                        // TODO: maybe we will do a voice call from the peer in the future.
                    } else {
                        log::debug!("The remote has requested to close the voice call");
                        if let Some(sender) = self.stop_voice_call_sender.take() {
                            allow_err!(sender.send(()));
                            self.handler.on_voice_call_closed("");
                        }
                    }
                }
                Some(message::Union::VoiceCallResponse(response)) => {
                    let ts = std::mem::replace(&mut self.voice_call_request_timestamp, None);
                    if let Some(ts) = ts {
                        if response.req_timestamp != ts.get() {
                            log::debug!("Possible encountering a voice call attack.");
                        } else {
                            if response.accepted {
                                // The peer accepted the voice call.
                                self.handler.on_voice_call_started();
                                self.stop_voice_call_sender = self.start_voice_call();
                            } else {
                                // The peer refused the voice call.
                                self.handler.on_voice_call_closed("");
                            }
                        }
                    }
                }
                Some(message::Union::PeerInfo(pi)) => match pi.conn_id {
                    crate::SYNC_PEER_INFO_DISPLAYS => {
                        self.handler.set_displays(&pi.displays);
                    }
                    _ => {}
                },
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
        config.privacy_mode.v = on;
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
            let enabled = *self.handler.server_file_transfer_enabled.read().unwrap()
                && self.handler.lc.read().unwrap().enable_file_transfer.v;
            ContextSend::enable(enabled);
        }
    }

    #[cfg(windows)]
    fn handle_cliprdr_msg(&self, clip: hbb_common::message_proto::Cliprdr) {
        if !self.handler.lc.read().unwrap().disable_clipboard.v {
            #[cfg(feature = "flutter")]
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

struct FpsControl {
    last_queue_size: usize,
    set_times: usize,
    refresh_times: usize,
    last_set_instant: Instant,
    last_refresh_instant: Instant,
}

impl Default for FpsControl {
    fn default() -> Self {
        Self {
            last_queue_size: Default::default(),
            set_times: Default::default(),
            refresh_times: Default::default(),
            last_set_instant: Instant::now(),
            last_refresh_instant: Instant::now(),
        }
    }
}
