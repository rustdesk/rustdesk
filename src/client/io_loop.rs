#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::clipboard::{update_clipboard, ClipboardSide};
#[cfg(not(any(target_os = "ios")))]
use crate::{audio_service, clipboard::CLIPBOARD_INTERVAL, ConnInner, CLIENT_SERVER};
use crate::{
    client::{
        self, new_voice_call_request, Client, Data, Interface, MediaData, MediaSender,
        QualityStatus, MILLI1, SEC30,
    },
    common::get_default_sound_input,
    ui_session_interface::{InvokeUiSession, Session},
};
#[cfg(feature = "unix-file-copy-paste")]
use crate::{clipboard::try_empty_clipboard_files, clipboard_file::unix_file_clip};
#[cfg(any(
    target_os = "windows",
    all(target_os = "macos", feature = "unix-file-copy-paste")
))]
use clipboard::ContextSend;
use crossbeam_queue::ArrayQueue;
#[cfg(not(target_os = "ios"))]
use hbb_common::tokio::sync::mpsc::error::TryRecvError;
use hbb_common::{
    allow_err,
    config::{self, LocalConfig, PeerConfig, TransferSerde},
    fs::{
        self, can_enable_overwrite_detection, get_job, get_string, new_send_confirm,
        DigestCheckResult, RemoveJobMeta,
    },
    get_time, log,
    message_proto::{permission_info::Permission, *},
    protobuf::Message as _,
    rendezvous_proto::ConnType,
    timeout,
    tokio::{
        self,
        sync::mpsc,
        time::{self, Duration, Instant},
    },
    Stream,
};
#[cfg(any(target_os = "windows", feature = "unix-file-copy-paste"))]
use hbb_common::{tokio::sync::Mutex as TokioMutex, ResultType};
use scrap::CodecFormat;
use std::{
    collections::HashMap,
    ffi::c_void,
    num::NonZeroI64,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, RwLock,
    },
};

pub struct Remote<T: InvokeUiSession> {
    handler: Session<T>,
    audio_sender: MediaSender,
    receiver: mpsc::UnboundedReceiver<Data>,
    sender: mpsc::UnboundedSender<Data>,
    // Stop sending local audio to remote client.
    stop_voice_call_sender: Option<std::sync::mpsc::Sender<()>>,
    voice_call_request_timestamp: Option<NonZeroI64>,
    read_jobs: Vec<fs::TransferJob>,
    write_jobs: Vec<fs::TransferJob>,
    remove_jobs: HashMap<i32, RemoveJob>,
    timer: crate::RustDeskInterval,
    last_update_jobs_status: (Instant, HashMap<i32, u64>),
    is_connected: bool,
    first_frame: bool,
    #[cfg(any(target_os = "windows", feature = "unix-file-copy-paste"))]
    client_conn_id: i32, // used for file clipboard
    data_count: Arc<AtomicUsize>,
    video_format: CodecFormat,
    elevation_requested: bool,
    peer_info: ParsedPeerInfo,
    video_threads: HashMap<usize, VideoThread>,
    chroma: Arc<RwLock<Option<Chroma>>>,
    last_record_state: bool,
}

#[derive(Default)]
struct ParsedPeerInfo {
    platform: String,
    is_installed: bool,
    idd_impl: String,
    support_view_camera: bool,
}

impl ParsedPeerInfo {
    fn is_support_virtual_display(&self) -> bool {
        self.is_installed
            && self.platform == "Windows"
            && (self.idd_impl == "rustdesk_idd" || self.idd_impl == "amyuni_idd")
    }
}

impl<T: InvokeUiSession> Remote<T> {
    pub fn new(
        handler: Session<T>,
        receiver: mpsc::UnboundedReceiver<Data>,
        sender: mpsc::UnboundedSender<Data>,
    ) -> Self {
        Self {
            handler,
            audio_sender: crate::client::start_audio_thread(),
            receiver,
            sender,
            read_jobs: Vec::new(),
            write_jobs: Vec::new(),
            remove_jobs: Default::default(),
            timer: crate::rustdesk_interval(time::interval(SEC30)),
            last_update_jobs_status: (Instant::now(), Default::default()),
            is_connected: false,
            first_frame: false,
            #[cfg(any(target_os = "windows", feature = "unix-file-copy-paste"))]
            client_conn_id: 0,
            data_count: Arc::new(AtomicUsize::new(0)),
            video_format: CodecFormat::Unknown,
            stop_voice_call_sender: None,
            voice_call_request_timestamp: None,
            elevation_requested: false,
            peer_info: Default::default(),
            video_threads: Default::default(),
            chroma: Default::default(),
            last_record_state: false,
        }
    }

    pub async fn io_loop(&mut self, key: &str, token: &str, round: u32) {
        #[cfg(target_os = "windows")]
        let _file_clip_context_holder = {
            // `is_port_forward()` will not reach here, but we still check it for clarity.
            if !self.handler.is_file_transfer()
                && !self.handler.is_port_forward()
                && !self.handler.is_view_camera()
            {
                // It is ok to call this function multiple times.
                ContextSend::enable(true);
                Some(crate::SimpleCallOnReturn {
                    b: true,
                    f: Box::new(|| {
                        // No need to call `enable(false)` for sciter version, because each client of sciter version is a new process.
                        // It's better to check if the peers are windows(support file copy&paste), but it's not necessary.
                        #[cfg(feature = "flutter")]
                        if !crate::flutter::sessions::has_sessions_running(ConnType::DEFAULT_CONN) {
                            ContextSend::enable(false);
                        };
                    }),
                })
            } else {
                None
            }
        };

        let mut last_recv_time = Instant::now();
        let mut received = false;
        let conn_type = if self.handler.is_file_transfer() {
            ConnType::FILE_TRANSFER
        } else if self.handler.is_view_camera() {
            ConnType::VIEW_CAMERA
        } else {
            ConnType::default()
        };

        match Client::start(
            &self.handler.get_id(),
            key,
            token,
            conn_type,
            self.handler.clone(),
        )
        .await
        {
            Ok(((mut peer, direct, pk), (feedback, rendezvous_server))) => {
                self.handler
                    .connection_round_state
                    .lock()
                    .unwrap()
                    .set_connected();
                self.handler.set_connection_type(peer.is_secured(), direct); // flutter -> connection_ready
                self.handler.update_direct(Some(direct));
                if conn_type == ConnType::DEFAULT_CONN || conn_type == ConnType::VIEW_CAMERA {
                    self.handler
                        .set_fingerprint(crate::common::pk_to_fingerprint(pk.unwrap_or_default()));
                }

                // just build for now
                #[cfg(not(any(target_os = "windows", feature = "unix-file-copy-paste")))]
                let (_tx_holder, mut rx_clip_client) = mpsc::unbounded_channel::<i32>();

                #[cfg(any(target_os = "windows", feature = "unix-file-copy-paste"))]
                let (_tx_holder, rx) = mpsc::unbounded_channel();
                #[cfg(any(target_os = "windows", feature = "unix-file-copy-paste"))]
                let mut rx_clip_client_holder = (Arc::new(TokioMutex::new(rx)), None);
                #[cfg(any(target_os = "windows", feature = "unix-file-copy-paste"))]
                {
                    let is_conn_not_default = self.handler.is_file_transfer()
                        || self.handler.is_port_forward()
                        || self.handler.is_rdp()
                        || self.handler.is_view_camera();
                    if !is_conn_not_default {
                        (self.client_conn_id, rx_clip_client_holder.0) =
                            clipboard::get_rx_cliprdr_client(&self.handler.get_id());
                        log::debug!("get cliprdr client for conn_id {}", self.client_conn_id);
                        let client_conn_id = self.client_conn_id;
                        rx_clip_client_holder.1 = Some(crate::SimpleCallOnReturn {
                            b: true,
                            f: Box::new(move || {
                                clipboard::remove_channel_by_conn_id(client_conn_id);
                            }),
                        });
                    };
                }
                #[cfg(any(target_os = "windows", feature = "unix-file-copy-paste"))]
                let mut rx_clip_client = rx_clip_client_holder.0.lock().await;

                let mut status_timer =
                    crate::rustdesk_interval(time::interval(Duration::new(1, 0)));
                let mut fps_instant = Instant::now();

                let _keep_it = client::hc_connection(feedback, rendezvous_server, token).await;

                loop {
                    tokio::select! {
                        res = peer.next() => {
                            if let Some(res) = res {
                                match res {
                                    Err(err) => {
                                        self.handler.on_establish_connection_error(err.to_string());
                                        break;
                                    }
                                    Ok(ref bytes) => {
                                        last_recv_time = Instant::now();
                                        if !received {
                                            received = true;
                                            self.handler.update_received(true);
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
                                    self.handler.msgbox("restarting", "Restarting remote device", "remote_restarting_tip", "");
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
                            #[cfg(any(target_os = "windows", feature = "unix-file-copy-paste"))]
                            self.handle_local_clipboard_msg(&mut peer, _msg).await;
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
                                self.timer = crate::rustdesk_interval(time::interval_at(Instant::now() + SEC30, SEC30));
                            }
                        }
                        _ = status_timer.tick() => {
                            let elapsed = fps_instant.elapsed().as_millis();
                            if elapsed < 1000 {
                                continue;
                            }
                            fps_instant = Instant::now();
                            let mut speed = self.data_count.swap(0, Ordering::Relaxed);
                            speed = speed * 1000 / elapsed as usize;
                            let speed = format!("{:.2}kB/s", speed as f32 / 1024 as f32);

                            let fps = self.video_threads.iter().map(|(k, v)| {
                                // Correcting the inaccuracy of status_timer
                                (k.clone(), (*v.frame_count.read().unwrap() as i32) * 1000 / elapsed as i32)
                            }).collect::<HashMap<usize, i32>>();
                            self.video_threads.iter().for_each(|(_, v)| {
                                *v.frame_count.write().unwrap() = 0;
                            });
                            self.fps_control(direct, fps.clone());
                            let chroma = self.chroma.read().unwrap().clone();
                            let chroma = match chroma {
                                Some(Chroma::I444) => "4:4:4",
                                Some(Chroma::I420) => "4:2:0",
                                None => "-",
                            };
                            let chroma = Some(chroma.to_string());
                            let codec_format = if self.video_format == CodecFormat::Unknown {
                                None
                            } else {
                                Some(self.video_format.clone())
                            };
                            self.handler.update_quality_status(QualityStatus {
                                speed: Some(speed),
                                fps,
                                chroma,
                                codec_format,
                                ..Default::default()
                            });
                        }
                    }
                }
                log::debug!("Exit io_loop of id={}", self.handler.get_id());
                // Stop client audio server.
                if let Some(s) = self.stop_voice_call_sender.take() {
                    s.send(()).ok();
                }
            }
            Err(err) => {
                self.handler.on_establish_connection_error(err.to_string());
            }
        }
        // set_disconnected_ok is used to check if new connection round is started.
        let _set_disconnected_ok = self
            .handler
            .connection_round_state
            .lock()
            .unwrap()
            .set_disconnected(round);

        #[cfg(not(target_os = "ios"))]
        if !self.handler.is_view_camera() && _set_disconnected_ok {
            Client::try_stop_clipboard();
        }

        #[cfg(any(target_os = "windows", feature = "unix-file-copy-paste"))]
        if !self.handler.is_view_camera() && _set_disconnected_ok {
            crate::clipboard::try_empty_clipboard_files(ClipboardSide::Client, self.client_conn_id);
        }
    }

    #[cfg(any(target_os = "windows", feature = "unix-file-copy-paste"))]
    async fn handle_local_clipboard_msg(
        &self,
        peer: &mut crate::client::FramedStream,
        msg: Option<clipboard::ClipboardFile>,
    ) {
        match msg {
            Some(clip) => match clip {
                clipboard::ClipboardFile::NotifyCallback {
                    r#type,
                    title,
                    text,
                } => {
                    self.handler.msgbox(&r#type, &title, &text, "");
                }
                _ => {
                    let is_stopping_allowed = clip.is_stopping_allowed();
                    let server_file_transfer_enabled =
                        *self.handler.server_file_transfer_enabled.read().unwrap();
                    let file_transfer_enabled =
                        self.handler.lc.read().unwrap().enable_file_copy_paste.v;
                    let view_only = self.handler.lc.read().unwrap().view_only.v;
                    let stop = is_stopping_allowed
                        && (view_only
                            || !self.is_connected
                            || !(server_file_transfer_enabled && file_transfer_enabled));
                    log::debug!(
                        "Process clipboard message from system, stop: {}, is_stopping_allowed: {}, view_only: {}, server_file_transfer_enabled: {}, file_transfer_enabled: {}",
                        view_only, stop, is_stopping_allowed, server_file_transfer_enabled, file_transfer_enabled
                    );
                    if stop {
                        #[cfg(target_os = "windows")]
                        {
                            ContextSend::set_is_stopped();
                        }
                    } else {
                        #[cfg(target_os = "windows")]
                        if let Err(e) = ContextSend::make_sure_enabled() {
                            log::error!("failed to restart clipboard context: {}", e);
                            // to-do: Show msgbox with "Don't show again" option
                        };
                        log::debug!("Send system clipboard message to remote");
                        let msg = crate::clipboard_file::clip_2_msg(clip);
                        allow_err!(peer.send(&msg).await);
                    }
                }
            },
            None => {
                // unreachable!()
            }
        }
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
        // iOS does not have this server.
        #[cfg(not(any(target_os = "ios")))]
        {
            // NOTE:
            // The client server and --server both use the same sound input device.
            // It's better to distinguish the server side and client side.
            // But it' not necessary for now, because it's not a common case.
            // And it is immediately known when the input device is changed.
            crate::audio_service::set_voice_call_input_device(get_default_sound_input(), false);
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
                            crate::audio_service::set_voice_call_input_device(None, true);
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
            #[cfg(all(target_os = "windows", not(feature = "flutter")))]
            Data::ToggleClipboardFile => {
                self.check_clipboard_file_context();
            }
            Data::Message(msg) => {
                match &msg.union {
                    Some(message::Union::Misc(misc)) => match misc.union {
                        Some(misc::Union::RefreshVideo(_)) => {
                            self.video_threads.iter().for_each(|(_, v)| {
                                *v.discard_queue.write().unwrap() = true;
                            });
                        }
                        Some(misc::Union::RefreshVideoDisplay(display)) => {
                            if let Some(v) = self.video_threads.get_mut(&(display as usize)) {
                                *v.discard_queue.write().unwrap() = true;
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                }
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
                            let total_size = job.total_size();
                            self.read_jobs.push(job);
                            self.timer = crate::rustdesk_interval(time::interval(MILLI1));
                            allow_err!(
                                peer.send(&fs::new_receive(id, to, file_num, files, total_size))
                                    .await
                            );
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
                            self.timer = crate::rustdesk_interval(time::interval(MILLI1));
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
                                job.files.clone(),
                                job.total_size(),
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
            Data::RenameFile((id, path, new_name, is_remote)) => {
                if is_remote {
                    let mut msg_out = Message::new();
                    let mut file_action = FileAction::new();
                    file_action.set_rename(FileRename {
                        id,
                        path,
                        new_name,
                        ..Default::default()
                    });
                    msg_out.set_file_action(file_action);
                    allow_err!(peer.send(&msg_out).await);
                } else {
                    let err = fs::rename_file(&path, &new_name)
                        .err()
                        .map(|e| e.to_string());
                    self.handle_job_status(id, -1, err);
                }
            }
            Data::RecordScreen(start) => {
                self.handler.lc.write().unwrap().record_state = start;
                self.update_record_state();
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
            Data::ResetDecoder(display) => match display {
                Some(display) => {
                    if let Some(v) = self.video_threads.get_mut(&display) {
                        v.video_sender.send(MediaData::Reset).ok();
                    }
                }
                None => {
                    for (_, v) in self.video_threads.iter_mut() {
                        v.video_sender.send(MediaData::Reset).ok();
                    }
                }
            },
            _ => {}
        }
        true
    }

    #[inline]
    fn update_job_status(
        job: &fs::TransferJob,
        elapsed: i32,
        last_update_jobs_status: &mut (Instant, HashMap<i32, u64>),
        handler: &Session<T>,
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
                    &self.handler,
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
        if config.transfer != transfer_metas {
            config.transfer = transfer_metas;
            self.handler.save_config(config);
        }
        true
    }

    async fn send_toggle_virtual_display_msg(&self, peer: &mut Stream) {
        if !self.peer_info.is_support_virtual_display() {
            return;
        }
        let lc = self.handler.lc.read().unwrap();
        let displays = lc.get_option("virtual-display");
        for d in displays.split(',') {
            if let Ok(index) = d.parse::<i32>() {
                let mut misc = Misc::new();
                misc.set_toggle_virtual_display(ToggleVirtualDisplay {
                    display: index,
                    on: true,
                    ..Default::default()
                });
                let mut msg_out = Message::new();
                msg_out.set_misc(misc);
                allow_err!(peer.send(&msg_out).await);
            }
        }
    }

    async fn send_toggle_privacy_mode_msg(&self, peer: &mut Stream) {
        let lc = self.handler.lc.read().unwrap();
        if lc.version >= hbb_common::get_version_number("1.2.4")
            && lc.get_toggle_option("privacy-mode")
        {
            let impl_key = lc.get_option("privacy-mode-impl-key");
            if impl_key == crate::privacy_mode::PRIVACY_MODE_IMPL_WIN_VIRTUAL_DISPLAY
                && !self.peer_info.is_support_virtual_display()
            {
                return;
            }
            let mut misc = Misc::new();
            misc.set_toggle_privacy_mode(TogglePrivacyMode {
                impl_key,
                on: true,
                ..Default::default()
            });
            let mut msg_out = Message::new();
            msg_out.set_misc(misc);
            allow_err!(peer.send(&msg_out).await);
        }
    }

    fn contains_key_frame(vf: &VideoFrame) -> bool {
        use video_frame::Union::*;
        match &vf.union {
            Some(vf) => match vf {
                Vp8s(f) | Vp9s(f) | Av1s(f) | H264s(f) | H265s(f) => f.frames.iter().any(|e| e.key),
                _ => false,
            },
            None => false,
        }
    }

    // Currently, this function only considers decoding speed and queue length, not network delay.
    // The controlled end can consider auto fps as the maximum decoding fps.
    #[inline]
    fn fps_control(&mut self, direct: bool, real_fps_map: HashMap<usize, i32>) {
        self.video_threads.iter_mut().for_each(|(k, v)| {
            let real_fps = real_fps_map.get(k).cloned().unwrap_or_default();
            if real_fps == 0 {
                v.fps_control.inactive_counter += 1;
            } else {
                v.fps_control.inactive_counter = 0;
            }
        });
        let custom_fps = self.handler.lc.read().unwrap().custom_fps.clone();
        let custom_fps = custom_fps.lock().unwrap().clone();
        let mut custom_fps = custom_fps.unwrap_or(30);
        if custom_fps < 5 || custom_fps > 120 {
            custom_fps = 30;
        }
        let inactive_threshold = 15;
        let max_queue_len = self
            .video_threads
            .iter()
            .map(|v| v.1.video_queue.read().unwrap().len())
            .max()
            .unwrap_or_default();
        let min_decode_fps = self
            .video_threads
            .iter()
            .filter(|v| v.1.fps_control.inactive_counter < inactive_threshold)
            .map(|v| *v.1.decode_fps.read().unwrap())
            .min()
            .flatten();
        let Some(min_decode_fps) = min_decode_fps else {
            return;
        };
        let mut limited_fps = if direct {
            min_decode_fps * 9 / 10 // 30 got 27
        } else {
            min_decode_fps * 4 / 5 // 30 got 24
        };
        if limited_fps > custom_fps {
            limited_fps = custom_fps;
        }
        let last_auto_fps = self.handler.lc.read().unwrap().last_auto_fps.clone();
        let displays = self.video_threads.keys().cloned().collect::<Vec<_>>();
        let mut fps_trending = |display: usize| {
            let thread = self.video_threads.get_mut(&display)?;
            let ctl = &mut thread.fps_control;
            let len = thread.video_queue.read().unwrap().len();
            let decode_fps = thread.decode_fps.read().unwrap().clone()?;
            let last_auto_fps = last_auto_fps.clone().unwrap_or(custom_fps as _);
            if ctl.inactive_counter > inactive_threshold {
                return None;
            }
            if len > 1 && last_auto_fps > limited_fps || len > std::cmp::max(1, decode_fps / 2) {
                ctl.idle_counter = 0;
                return Some(false);
            }
            if len <= 1 {
                ctl.idle_counter += 1;
                if ctl.idle_counter > 3 && last_auto_fps + 3 <= limited_fps {
                    return Some(true);
                }
            }
            if len > 1 {
                ctl.idle_counter = 0;
            }
            None
        };
        let trendings: Vec<_> = displays.iter().map(|k| fps_trending(*k)).collect();
        let should_decrease = trendings.iter().any(|v| *v == Some(false));
        let should_increase = !should_decrease && trendings.iter().any(|v| *v == Some(true));
        if last_auto_fps.is_none() || should_decrease || should_increase {
            // limited_fps to ensure decoding is faster than encoding
            let mut auto_fps = limited_fps;
            if should_decrease && limited_fps < max_queue_len {
                auto_fps = limited_fps / 2;
            }
            if auto_fps < 1 {
                auto_fps = 1;
            }
            if Some(auto_fps) != last_auto_fps {
                let mut misc = Misc::new();
                misc.set_option(OptionMessage {
                    custom_fps: auto_fps as _,
                    ..Default::default()
                });
                let mut msg = Message::new();
                msg.set_misc(misc);
                self.sender.send(Data::Message(msg)).ok();
                log::info!("Set fps to {}", auto_fps);
                self.handler.lc.write().unwrap().last_auto_fps = Some(auto_fps);
            }
        }
        // send refresh
        for (display, thread) in self.video_threads.iter_mut() {
            let ctl = &mut thread.fps_control;
            let video_queue = thread.video_queue.read().unwrap();
            let tolerable = std::cmp::min(min_decode_fps, video_queue.capacity() / 2);
            if ctl.refresh_times < 20 // enough
                    && (video_queue.len() > tolerable
                            && (ctl.refresh_times == 0 || ctl.last_refresh_instant.map(|t|t.elapsed().as_secs() > 10).unwrap_or(false)))
            {
                // Refresh causes client set_display, left frames cause flickering.
                drop(video_queue);
                self.handler.refresh_video(*display as _);
                log::info!("Refresh display {} to reduce delay", display);
                ctl.refresh_times += 1;
                ctl.last_refresh_instant = Some(Instant::now());
            }
        }
    }

    fn check_view_camera_support(&self, peer_version: &str, peer_platform: &str) -> bool {
        if self.peer_info.support_view_camera {
            return true;
        }
        if hbb_common::get_version_number(&peer_version) < hbb_common::get_version_number("1.3.9")
            && (peer_platform == "Windows" || peer_platform == "Linux")
        {
            self.handler.msgbox(
                "error",
                "Download new version",
                "upgrade_remote_rustdesk_client_to_{1.3.9}_tip",
                "",
            );
        } else {
            self.handler.on_error("view_camera_unsupported_tip");
        }
        return false;
    }

    async fn handle_msg_from_peer(&mut self, data: &[u8], peer: &mut Stream) -> bool {
        if let Ok(msg_in) = Message::parse_from_bytes(&data) {
            match msg_in.union {
                Some(message::Union::VideoFrame(vf)) => {
                    if !self.first_frame {
                        self.first_frame = true;
                        self.handler.close_success();
                        self.handler.adapt_size();
                        self.send_toggle_virtual_display_msg(peer).await;
                        self.send_toggle_privacy_mode_msg(peer).await;
                    }
                    self.video_format = CodecFormat::from(&vf);

                    let display = vf.display as usize;
                    if !self.video_threads.contains_key(&display) {
                        self.new_video_thread(display);
                    }
                    let Some(thread) = self.video_threads.get_mut(&display) else {
                        return true;
                    };
                    if Self::contains_key_frame(&vf) {
                        thread
                            .video_sender
                            .send(MediaData::VideoFrame(Box::new(vf)))
                            .ok();
                    } else {
                        let video_queue = thread.video_queue.read().unwrap();
                        if video_queue.force_push(vf).is_some() {
                            drop(video_queue);
                            self.handler.refresh_video(display as _);
                        } else {
                            thread.video_sender.send(MediaData::VideoQueue).ok();
                        }
                    }
                }
                Some(message::Union::Hash(hash)) => {
                    self.handler
                        .handle_hash(&self.handler.password.clone(), hash, peer)
                        .await;
                }
                Some(message::Union::LoginResponse(lr)) => match lr.union {
                    Some(login_response::Union::Error(err)) => {
                        if err == client::REQUIRE_2FA {
                            self.handler.lc.write().unwrap().enable_trusted_devices =
                                lr.enable_trusted_devices;
                        }
                        if !self.handler.handle_login_error(&err) {
                            return false;
                        }
                    }
                    Some(login_response::Union::PeerInfo(pi)) => {
                        let peer_version = pi.version.clone();
                        let peer_platform = pi.platform.clone();
                        self.set_peer_info(&pi);
                        if self.handler.is_view_camera() {
                            if !self.check_view_camera_support(&peer_version, &peer_platform) {
                                self.handler.lc.write().unwrap().handle_peer_info(&pi);
                                return false;
                            }
                        }
                        self.handler.handle_peer_info(pi);
                        #[cfg(all(target_os = "windows", not(feature = "flutter")))]
                        self.check_clipboard_file_context();
                        if !(self.handler.is_file_transfer()
                            || self.handler.is_port_forward()
                            || self.handler.is_view_camera())
                        {
                            #[cfg(feature = "flutter")]
                            #[cfg(not(target_os = "ios"))]
                            let rx = Client::try_start_clipboard(None);
                            #[cfg(not(feature = "flutter"))]
                            #[cfg(not(any(target_os = "android", target_os = "ios")))]
                            let rx = Client::try_start_clipboard(Some(
                                crate::client::ClientClipboardContext {
                                    cfg: self.handler.get_permission_config(),
                                    tx: self.sender.clone(),
                                    #[cfg(feature = "unix-file-copy-paste")]
                                    is_file_supported: crate::is_support_file_copy_paste(
                                        &peer_version,
                                    ),
                                },
                            ));
                            // To make sure current text clipboard data is updated.
                            #[cfg(not(target_os = "ios"))]
                            if let Some(mut rx) = rx {
                                timeout(CLIPBOARD_INTERVAL, rx.recv()).await.ok();
                            }

                            #[cfg(not(any(target_os = "android", target_os = "ios")))]
                            if self.handler.lc.read().unwrap().sync_init_clipboard.v {
                                if let Some(msg_out) = crate::clipboard::get_current_clipboard_msg(
                                    &peer_version,
                                    &peer_platform,
                                    crate::clipboard::ClipboardSide::Client,
                                ) {
                                    let sender = self.sender.clone();
                                    let permission_config = self.handler.get_permission_config();
                                    tokio::spawn(async move {
                                        if permission_config.is_text_clipboard_required() {
                                            sender.send(Data::Message(msg_out)).ok();
                                        }
                                    });
                                }
                            }
                            // to-do: Android, is `sync_init_clipboard` really needed?
                            // https://github.com/rustdesk/rustdesk/discussions/9010

                            #[cfg(feature = "flutter")]
                            #[cfg(not(target_os = "ios"))]
                            crate::flutter::update_text_clipboard_required();

                            #[cfg(all(feature = "flutter", feature = "unix-file-copy-paste"))]
                            crate::flutter::update_file_clipboard_required();

                            // on connection established client
                            #[cfg(all(feature = "flutter", feature = "plugin_framework"))]
                            #[cfg(not(any(target_os = "android", target_os = "ios")))]
                            crate::plugin::handle_listen_event(
                                crate::plugin::EVENT_ON_CONN_CLIENT.to_owned(),
                                self.handler.get_id(),
                            );
                        }

                        if self.handler.is_file_transfer() {
                            self.handler.load_last_jobs();
                        }

                        self.is_connected = true;
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
                        update_clipboard(vec![cb], ClipboardSide::Client);
                        #[cfg(target_os = "ios")]
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
                        #[cfg(target_os = "android")]
                        crate::clipboard::handle_msg_clipboard(cb);
                    }
                }
                Some(message::Union::MultiClipboards(_mcb)) => {
                    if !self.handler.lc.read().unwrap().disable_clipboard.v {
                        #[cfg(not(any(target_os = "android", target_os = "ios")))]
                        update_clipboard(_mcb.clipboards, ClipboardSide::Client);
                        #[cfg(target_os = "android")]
                        crate::clipboard::handle_msg_multi_clipboards(_mcb);
                    }
                }
                #[cfg(any(target_os = "windows", feature = "unix-file-copy-paste"))]
                Some(message::Union::Cliprdr(clip)) => {
                    self.handle_cliprdr_msg(clip, peer).await;
                }
                Some(message::Union::FileResponse(fr)) => {
                    match fr.union {
                        Some(file_response::Union::EmptyDirs(res)) => {
                            self.handler.update_empty_dirs(res);
                        }
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
                                *self.handler.server_keyboard_enabled.write().unwrap() = p.enabled;
                                #[cfg(feature = "flutter")]
                                #[cfg(not(target_os = "ios"))]
                                crate::flutter::update_text_clipboard_required();
                                #[cfg(all(feature = "flutter", feature = "unix-file-copy-paste"))]
                                crate::flutter::update_file_clipboard_required();
                                self.handler.set_permission("keyboard", p.enabled);
                            }
                            Ok(Permission::Clipboard) => {
                                *self.handler.server_clipboard_enabled.write().unwrap() = p.enabled;
                                #[cfg(feature = "flutter")]
                                #[cfg(not(target_os = "ios"))]
                                crate::flutter::update_text_clipboard_required();
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
                                #[cfg(all(feature = "flutter", feature = "unix-file-copy-paste"))]
                                crate::flutter::update_file_clipboard_required();
                                self.handler.set_permission("file", p.enabled);
                                #[cfg(feature = "unix-file-copy-paste")]
                                if !p.enabled {
                                    try_empty_clipboard_files(
                                        ClipboardSide::Client,
                                        self.client_conn_id,
                                    );
                                }
                            }
                            Ok(Permission::Camera) => {
                                self.handler.set_permission("camera", p.enabled);
                            }
                            Ok(Permission::Restart) => {
                                self.handler.set_permission("restart", p.enabled);
                            }
                            Ok(Permission::Recording) => {
                                self.handler.lc.write().unwrap().record_permission = p.enabled;
                                self.update_record_state();
                                self.handler.set_permission("recording", p.enabled);
                            }
                            Ok(Permission::BlockInput) => {
                                self.handler.set_permission("block_input", p.enabled);
                            }
                            _ => {}
                        }
                    }
                    Some(misc::Union::SwitchDisplay(s)) => {
                        self.handler.handle_peer_switch_display(&s);
                        if let Some(thread) = self.video_threads.get_mut(&(s.display as usize)) {
                            thread.video_sender.send(MediaData::Reset).ok();
                        }

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
                        let keyboard = self.handler.server_keyboard_enabled.read().unwrap().clone();
                        #[cfg(feature = "flutter")]
                        {
                            if uac && keyboard {
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
                            if uac && keyboard {
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
                        let keyboard = self.handler.server_keyboard_enabled.read().unwrap().clone();
                        #[cfg(feature = "flutter")]
                        {
                            if elevated && keyboard {
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
                            if elevated && keyboard {
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
                            self.handler.cancel_msgbox("wait-uac");
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
                        self.handler.switch_back(&self.handler.get_id());
                    }
                    #[cfg(all(feature = "flutter", feature = "plugin_framework"))]
                    #[cfg(not(any(target_os = "android", target_os = "ios")))]
                    Some(misc::Union::PluginRequest(p)) => {
                        allow_err!(crate::plugin::handle_server_event(
                            &p.id,
                            &self.handler.get_id(),
                            &p.content
                        ));
                        // to-do: show message box on UI when error occurs?
                    }
                    #[cfg(all(feature = "flutter", feature = "plugin_framework"))]
                    #[cfg(not(any(target_os = "android", target_os = "ios")))]
                    Some(misc::Union::PluginFailure(p)) => {
                        let name = if p.name.is_empty() {
                            "plugin".to_string()
                        } else {
                            p.name
                        };
                        self.handler.msgbox("custom-nocancel", &name, &p.msg, "");
                    }
                    Some(misc::Union::SupportedEncoding(e)) => {
                        log::info!("update supported encoding:{:?}", e);
                        self.handler.lc.write().unwrap().supported_encoding = e;
                    }
                    Some(misc::Union::FollowCurrentDisplay(d_idx)) => {
                        self.handler.set_current_display(d_idx);
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
                    if let Some(v) = config::HELPER_URL.get(&link as &str) {
                        link = v.to_string();
                    } else {
                        log::warn!("Message box ignore link {} for security", &link);
                        link = "".to_string();
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
                Some(message::Union::PeerInfo(pi)) => {
                    self.handler.set_displays(&pi.displays);
                    self.handler.set_platform_additions(&pi.platform_additions);
                }
                _ => {}
            }
        }
        true
    }

    fn set_peer_info(&mut self, pi: &PeerInfo) {
        self.peer_info.platform = pi.platform.clone();
        if let Ok(platform_additions) =
            serde_json::from_str::<HashMap<String, serde_json::Value>>(&pi.platform_additions)
        {
            self.peer_info.is_installed = platform_additions
                .get("is_installed")
                .map(|v| v.as_bool())
                .flatten()
                .unwrap_or(false);
            self.peer_info.idd_impl = platform_additions
                .get("idd_impl")
                .map(|v| v.as_str())
                .flatten()
                .unwrap_or_default()
                .to_string();
            self.peer_info.support_view_camera = platform_additions
                .get("support_view_camera")
                .map(|v| v.as_bool())
                .flatten()
                .unwrap_or(false);
        }
    }

    async fn handle_back_notification(&mut self, notification: BackNotification) -> bool {
        match notification.union {
            Some(back_notification::Union::BlockInputState(state)) => {
                self.handle_back_msg_block_input(
                    state.enum_value_or(back_notification::BlockInputState::BlkStateUnknown),
                    notification.details,
                )
                .await;
            }
            Some(back_notification::Union::PrivacyModeState(state)) => {
                if !self
                    .handle_back_msg_privacy_mode(
                        state.enum_value_or(back_notification::PrivacyModeState::PrvStateUnknown),
                        notification.details,
                        notification.impl_key,
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

    async fn handle_back_msg_block_input(
        &mut self,
        state: back_notification::BlockInputState,
        details: String,
    ) {
        match state {
            back_notification::BlockInputState::BlkOnSucceeded => {
                self.update_block_input_state(true);
            }
            back_notification::BlockInputState::BlkOnFailed => {
                self.handler.msgbox(
                    "custom-error",
                    "Block user input",
                    if details.is_empty() {
                        "Failed"
                    } else {
                        &details
                    },
                    "",
                );
                self.update_block_input_state(false);
            }
            back_notification::BlockInputState::BlkOffSucceeded => {
                self.update_block_input_state(false);
            }
            back_notification::BlockInputState::BlkOffFailed => {
                self.handler.msgbox(
                    "custom-error",
                    "Unblock user input",
                    if details.is_empty() {
                        "Failed"
                    } else {
                        &details
                    },
                    "",
                );
            }
            _ => {}
        }
    }

    #[inline(always)]
    fn update_privacy_mode(&mut self, impl_key: String, on: bool) {
        let mut config = self.handler.load_config();
        config.privacy_mode.v = on;
        if on {
            // For compatibility, version < 1.2.4, the default value is 'privacy_mode_impl_mag'.
            let impl_key = if impl_key.is_empty() {
                "privacy_mode_impl_mag".to_string()
            } else {
                impl_key
            };
            config
                .options
                .insert("privacy-mode-impl-key".to_string(), impl_key);
        }
        self.handler.save_config(config);

        self.handler.update_privacy_mode();
    }

    async fn handle_back_msg_privacy_mode(
        &mut self,
        state: back_notification::PrivacyModeState,
        details: String,
        impl_key: String,
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
                self.update_privacy_mode(impl_key, false);
            }
            back_notification::PrivacyModeState::PrvOnSucceeded => {
                self.handler
                    .msgbox("custom-nocancel", "Privacy mode", "Enter privacy mode", "");
                self.update_privacy_mode(impl_key, true);
            }
            back_notification::PrivacyModeState::PrvOnFailedDenied => {
                self.handler
                    .msgbox("custom-error", "Privacy mode", "Peer denied", "");
                self.update_privacy_mode(impl_key, false);
            }
            back_notification::PrivacyModeState::PrvOnFailedPlugin => {
                self.handler
                    .msgbox("custom-error", "Privacy mode", "Please install plugins", "");
                self.update_privacy_mode(impl_key, false);
            }
            back_notification::PrivacyModeState::PrvOnFailed => {
                self.handler.msgbox(
                    "custom-error",
                    "Privacy mode",
                    if details.is_empty() {
                        "Failed"
                    } else {
                        &details
                    },
                    "",
                );
                self.update_privacy_mode(impl_key, false);
            }
            back_notification::PrivacyModeState::PrvOffSucceeded => {
                self.handler
                    .msgbox("custom-nocancel", "Privacy mode", "Exit privacy mode", "");
                self.update_privacy_mode(impl_key, false);
            }
            back_notification::PrivacyModeState::PrvOffByPeer => {
                self.handler
                    .msgbox("custom-error", "Privacy mode", "Peer exit", "");
                self.update_privacy_mode(impl_key, false);
            }
            back_notification::PrivacyModeState::PrvOffFailed => {
                self.handler.msgbox(
                    "custom-error",
                    "Privacy mode",
                    if details.is_empty() {
                        "Failed to turn off"
                    } else {
                        &details
                    },
                    "",
                );
            }
            back_notification::PrivacyModeState::PrvOffUnknown => {
                self.handler
                    .msgbox("custom-error", "Privacy mode", "Turned off", "");
                // log::error!("Privacy mode is turned off with unknown reason");
                self.update_privacy_mode(impl_key, false);
            }
            _ => {}
        }
        true
    }

    #[cfg(all(target_os = "windows", not(feature = "flutter")))]
    fn check_clipboard_file_context(&self) {
        let enabled = *self.handler.server_file_transfer_enabled.read().unwrap()
            && self.handler.lc.read().unwrap().enable_file_copy_paste.v;
        ContextSend::enable(enabled);
    }

    #[cfg(any(target_os = "windows", feature = "unix-file-copy-paste"))]
    async fn handle_cliprdr_msg(
        &mut self,
        clip: hbb_common::message_proto::Cliprdr,
        peer: &mut Stream,
    ) {
        log::debug!("handling cliprdr msg from server peer");
        #[cfg(feature = "flutter")]
        if let Some(hbb_common::message_proto::cliprdr::Union::FormatList(_)) = &clip.union {
            if self.client_conn_id
                != clipboard::get_client_conn_id(&crate::flutter::get_cur_peer_id()).unwrap_or(0)
            {
                return;
            }
        }

        let Some(clip) = crate::clipboard_file::msg_2_clip(clip) else {
            log::warn!("failed to decode cliprdr msg from server peer");
            return;
        };

        let is_stopping_allowed = clip.is_beginning_message();
        let file_transfer_enabled = self.handler.is_file_clipboard_required();
        let stop = is_stopping_allowed && !file_transfer_enabled;
        log::debug!(
                "Process clipboard message from server peer, stop: {}, is_stopping_allowed: {}, file_transfer_enabled: {}",
                stop, is_stopping_allowed, file_transfer_enabled);
        if !stop {
            #[cfg(any(
                target_os = "windows",
                all(target_os = "macos", feature = "unix-file-copy-paste")
            ))]
            if let Err(e) = ContextSend::make_sure_enabled() {
                log::error!("failed to restart clipboard context: {}", e);
            };
            #[cfg(target_os = "windows")]
            {
                let _ = ContextSend::proc(|context| -> ResultType<()> {
                    context
                        .server_clip_file(self.client_conn_id, clip)
                        .map_err(|e| e.into())
                });
            }
            #[cfg(feature = "unix-file-copy-paste")]
            if crate::is_support_file_copy_paste_num(self.handler.lc.read().unwrap().version) {
                let mut out_msg = None;

                #[cfg(target_os = "macos")]
                if clipboard::platform::unix::macos::should_handle_msg(&clip) {
                    if let Err(e) = ContextSend::proc(|context| -> ResultType<()> {
                        context
                            .server_clip_file(self.client_conn_id, clip)
                            .map_err(|e| e.into())
                    }) {
                        log::error!("failed to handle cliprdr msg: {}", e);
                    }
                } else {
                    out_msg = unix_file_clip::serve_clip_messages(
                        ClipboardSide::Client,
                        clip,
                        self.client_conn_id,
                    );
                }

                #[cfg(not(target_os = "macos"))]
                {
                    out_msg = unix_file_clip::serve_clip_messages(
                        ClipboardSide::Client,
                        clip,
                        self.client_conn_id,
                    );
                }

                if let Some(msg) = out_msg {
                    allow_err!(peer.send(&msg).await);
                }
            }
        }
    }

    fn new_video_thread(&mut self, display: usize) {
        let video_queue = Arc::new(RwLock::new(ArrayQueue::new(client::VIDEO_QUEUE_SIZE)));
        let (video_sender, video_receiver) = std::sync::mpsc::channel::<MediaData>();
        let decode_fps = Arc::new(RwLock::new(None));
        let frame_count = Arc::new(RwLock::new(0));
        let discard_queue = Arc::new(RwLock::new(false));
        let video_thread = VideoThread {
            video_queue: video_queue.clone(),
            video_sender,
            decode_fps: decode_fps.clone(),
            frame_count: frame_count.clone(),
            fps_control: Default::default(),
            discard_queue: discard_queue.clone(),
        };
        let handler = self.handler.ui_handler.clone();
        crate::client::start_video_thread(
            self.handler.clone(),
            display,
            video_receiver,
            video_queue,
            decode_fps,
            self.chroma.clone(),
            discard_queue,
            move |display: usize,
                  data: &mut scrap::ImageRgb,
                  _texture: *mut c_void,
                  pixelbuffer: bool| {
                *frame_count.write().unwrap() += 1;
                if pixelbuffer {
                    handler.on_rgba(display, data);
                } else {
                    #[cfg(all(feature = "vram", feature = "flutter"))]
                    handler.on_texture(display, _texture);
                }
            },
        );
        self.video_threads.insert(display, video_thread);
        if self.video_threads.len() == 1 {
            let auto_record =
                LocalConfig::get_bool_option(config::keys::OPTION_ALLOW_AUTO_RECORD_OUTGOING);
            self.handler.lc.write().unwrap().record_state = auto_record;
            self.update_record_state();
        }
    }

    fn update_record_state(&mut self) {
        // state
        let permission = self.handler.lc.read().unwrap().record_permission;
        if !permission {
            self.handler.lc.write().unwrap().record_state = false;
        }
        let state = self.handler.lc.read().unwrap().record_state;
        let start = state && permission;
        if self.last_record_state == start {
            return;
        }
        self.last_record_state = start;
        log::info!("record screen start: {start}");
        // update local
        for (_, v) in self.video_threads.iter_mut() {
            v.video_sender.send(MediaData::RecordScreen(start)).ok();
        }
        self.handler.update_record_status(start);
        // update remote
        let mut misc = Misc::new();
        misc.set_client_record_status(start);
        let mut msg = Message::new();
        msg.set_misc(misc);
        self.sender.send(Data::Message(msg)).ok();
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

#[derive(Debug, Default)]
struct FpsControl {
    refresh_times: usize,
    last_refresh_instant: Option<Instant>,
    idle_counter: usize,
    inactive_counter: usize,
}

struct VideoThread {
    video_queue: Arc<RwLock<ArrayQueue<VideoFrame>>>,
    video_sender: MediaSender,
    decode_fps: Arc<RwLock<Option<usize>>>,
    frame_count: Arc<RwLock<usize>>,
    discard_queue: Arc<RwLock<bool>>,
    fps_control: FpsControl,
}

impl Drop for VideoThread {
    fn drop(&mut self) {
        // since channels are buffered, messages sent before the disconnect will still be properly received.
        *self.discard_queue.write().unwrap() = true;
    }
}
