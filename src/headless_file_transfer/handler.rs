use super::completion::TransferCompletion;
use crate::{
    client::QualityStatus, headless_auth::AuthPrompt, ui_session_interface::InvokeUiSession,
};
use hbb_common::{log, message_proto::*, rendezvous_proto::ConnType};
#[cfg(all(feature = "vram", feature = "flutter"))]
use std::ffi::c_void;
use std::sync::{mpsc::Sender, Arc, Mutex};

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum HeadlessFileTransferEvent {
    PeerPlatform(String),
    Connected,
    Auth(AuthPrompt),
    Files {
        id: i32,
        entries: Vec<FileEntry>,
        path: String,
        is_local: bool,
        only_count: bool,
    },
    Conflict {
        id: i32,
        file_num: i32,
        destination: String,
        is_upload: bool,
        is_identical: bool,
    },
    Progress {
        id: i32,
        file_num: i32,
        speed: u64,
        finished_size: u64,
    },
    Completed(TransferCompletion),
    JobFailed {
        id: i32,
        file_num: i32,
        message: String,
    },
    ProtocolFailed(String),
    ConnectionFailed(String),
}

#[derive(Clone, Default)]
pub(crate) struct HeadlessFileTransferHandler {
    event_tx: Arc<Mutex<Option<Sender<HeadlessFileTransferEvent>>>>,
}

impl HeadlessFileTransferHandler {
    pub(crate) fn new(event_tx: Sender<HeadlessFileTransferEvent>) -> Self {
        Self {
            event_tx: Arc::new(Mutex::new(Some(event_tx))),
        }
    }

    fn emit(&self, event: HeadlessFileTransferEvent) -> bool {
        let event_tx = match self.event_tx.lock() {
            Ok(event_tx) => event_tx,
            Err(_) => {
                log::error!("Headless file-transfer event sender lock is poisoned");
                return false;
            }
        };
        let Some(event_tx) = event_tx.as_ref() else {
            log::error!("Headless file-transfer event sender is unavailable");
            return false;
        };
        if event_tx.send(event).is_err() {
            log::error!("Failed to emit headless file-transfer event: receiver is closed");
            return false;
        }
        true
    }
}

fn progress_value(value: f64) -> u64 {
    if value.is_finite() && value >= 0.0 {
        value as u64
    } else {
        0
    }
}

impl InvokeUiSession for HeadlessFileTransferHandler {
    fn set_cursor_data(&self, _cursor_data: CursorData) {}

    fn set_cursor_id(&self, _id: String) {}

    fn set_cursor_position(&self, _cursor_position: CursorPosition) {}

    fn set_display(
        &self,
        _x: i32,
        _y: i32,
        _width: i32,
        _height: i32,
        _cursor_embedded: bool,
        _scale: f64,
    ) {
    }

    fn switch_display(&self, _display: &SwitchDisplay) {}

    fn set_peer_info(&self, peer_info: &PeerInfo) {
        let _ = self.emit(HeadlessFileTransferEvent::PeerPlatform(
            peer_info.platform.clone(),
        ));
    }

    fn set_displays(&self, _displays: &Vec<DisplayInfo>) {}

    fn set_platform_additions(&self, _data: &str) {}

    fn on_connected(&self, conn_type: ConnType) {
        if conn_type == ConnType::FILE_TRANSFER {
            let _ = self.emit(HeadlessFileTransferEvent::Connected);
        }
    }

    fn update_privacy_mode(&self) {}

    fn set_permission(&self, _name: &str, _value: bool) {}

    fn close_success(&self) {}

    fn update_quality_status(&self, _quality_status: QualityStatus) {}

    fn set_connection_type(&self, _is_secured: bool, _direct: bool, _stream_type: &str) {}

    fn set_fingerprint(&self, _fingerprint: String) {}

    fn job_error(&self, id: i32, error: String, file_num: i32) {
        let _ = self.emit(HeadlessFileTransferEvent::JobFailed {
            id,
            file_num,
            message: error,
        });
    }

    fn job_done(&self, _id: i32, _file_num: i32) {}

    fn file_transfer_job_completed(&self, job_json: &str) {
        let event = match TransferCompletion::parse(job_json) {
            Ok(completion) => HeadlessFileTransferEvent::Completed(completion),
            Err(_) => HeadlessFileTransferEvent::ProtocolFailed(
                "invalid file-transfer completion event".to_owned(),
            ),
        };
        let _ = self.emit(event);
    }

    fn clear_all_jobs(&self) {}

    fn new_message(&self, _message: String) {}

    fn update_transfer_list(&self) {}

    fn load_last_job(&self, _count: i32, _job_json: &str, _auto_start: bool) {}

    fn update_folder_files(
        &self,
        id: i32,
        entries: &Vec<FileEntry>,
        path: String,
        is_local: bool,
        only_count: bool,
    ) {
        let _ = self.emit(HeadlessFileTransferEvent::Files {
            id,
            entries: entries.clone(),
            path,
            is_local,
            only_count,
        });
    }

    fn confirm_delete_files(&self, _id: i32, _file_num: i32, _name: String) {}

    fn override_file_confirm(
        &self,
        id: i32,
        file_num: i32,
        destination: String,
        is_upload: bool,
        is_identical: bool,
    ) {
        let _ = self.emit(HeadlessFileTransferEvent::Conflict {
            id,
            file_num,
            destination,
            is_upload,
            is_identical,
        });
    }

    fn update_block_input_state(&self, _on: bool) {}

    fn job_progress(&self, id: i32, file_num: i32, speed: f64, finished_size: f64) {
        let _ = self.emit(HeadlessFileTransferEvent::Progress {
            id,
            file_num,
            speed: progress_value(speed),
            finished_size: progress_value(finished_size),
        });
    }

    fn adapt_size(&self) {}

    fn on_rgba(&self, _display: usize, _rgba: &mut scrap::ImageRgb) {}

    fn msgbox(&self, msgtype: &str, title: &str, text: &str, _link: &str, _retry: bool) {
        let event = match msgtype {
            "input-password" => Some(HeadlessFileTransferEvent::Auth(AuthPrompt::Password {
                retry: false,
            })),
            "re-input-password" => Some(HeadlessFileTransferEvent::Auth(AuthPrompt::Password {
                retry: true,
            })),
            "input-2fa" => Some(HeadlessFileTransferEvent::Auth(AuthPrompt::TwoFactor)),
            _ if msgtype.contains("insecure-connection") => Some(HeadlessFileTransferEvent::Auth(
                AuthPrompt::InsecureConnection,
            )),
            _ if msgtype.contains("error") => {
                let message = if !text.is_empty() {
                    text
                } else if !title.is_empty() {
                    title
                } else {
                    msgtype
                };
                Some(HeadlessFileTransferEvent::ConnectionFailed(
                    message.to_owned(),
                ))
            }
            _ => None,
        };

        if let Some(event) = event {
            let _ = self.emit(event);
        }
    }

    #[cfg(any(target_os = "android", target_os = "ios"))]
    fn clipboard(&self, _content: String) {}

    fn cancel_msgbox(&self, _tag: &str) {}

    fn switch_back(&self, _id: &str) {}

    fn portable_service_running(&self, _running: bool) {}

    fn on_voice_call_started(&self) {}

    fn on_voice_call_closed(&self, _reason: &str) {}

    fn on_voice_call_waiting(&self) {}

    fn on_voice_call_incoming(&self) {}

    fn get_rgba(&self, _display: usize) -> *const u8 {
        std::ptr::null()
    }

    fn next_rgba(&self, _display: usize) {}

    #[cfg(all(feature = "vram", feature = "flutter"))]
    fn on_texture(&self, _display: usize, _texture: *mut c_void) {}

    fn set_multiple_windows_session(&self, _sessions: Vec<WindowsSession>) {}

    fn set_current_display(&self, _display_index: i32) {}

    #[cfg(feature = "flutter")]
    fn is_multi_ui_session(&self) -> bool {
        false
    }

    fn update_record_status(&self, _start: bool) {}

    fn update_empty_dirs(&self, _response: ReadEmptyDirsResponse) {}

    fn printer_request(&self, _id: i32, _path: String) {}

    fn handle_screenshot_resp(&self, _session_id: String, _message: String) {}

    fn handle_terminal_response(&self, _response: TerminalResponse) {}
}

#[cfg(test)]
mod tests {
    use super::{HeadlessFileTransferEvent, HeadlessFileTransferHandler};
    use crate::{client::QualityStatus, ui_session_interface::InvokeUiSession};
    use hbb_common::{
        message_proto::{FileEntry, PeerInfo},
        rendezvous_proto::ConnType,
    };
    use std::sync::mpsc::{self, Receiver, TryRecvError};

    fn handler() -> (
        HeadlessFileTransferHandler,
        Receiver<HeadlessFileTransferEvent>,
    ) {
        let (event_tx, event_rx) = mpsc::channel();
        (HeadlessFileTransferHandler::new(event_tx), event_rx)
    }

    fn file_entry() -> FileEntry {
        FileEntry::default()
    }

    #[test]
    fn emits_only_file_transfer_connection_and_peer_platform() {
        let (handler, rx) = handler();
        handler.set_peer_info(&PeerInfo {
            platform: "Windows".into(),
            ..Default::default()
        });
        handler.on_connected(ConnType::FILE_TRANSFER);
        handler.on_connected(ConnType::TERMINAL);

        assert_eq!(
            rx.recv().unwrap(),
            HeadlessFileTransferEvent::PeerPlatform("Windows".into())
        );
        assert_eq!(rx.recv().unwrap(), HeadlessFileTransferEvent::Connected);
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn maps_files_conflict_progress_completion_and_failure() {
        let (handler, rx) = handler();
        handler.update_folder_files(7, &vec![file_entry()], "remote".into(), false, false);
        handler.override_file_confirm(7, 0, "target".into(), false, true);
        handler.job_progress(7, 0, 1024.0, 42.0);
        handler.file_transfer_job_completed(
            r#"{"id":7,"fileNum":1,"totalSize":42,"finishedSize":42,"done":true,"error":""}"#,
        );
        handler.job_error(7, "permission denied".into(), 0);

        assert!(matches!(
            rx.recv().unwrap(),
            HeadlessFileTransferEvent::Files { .. }
        ));
        assert!(matches!(
            rx.recv().unwrap(),
            HeadlessFileTransferEvent::Conflict { .. }
        ));
        assert!(matches!(
            rx.recv().unwrap(),
            HeadlessFileTransferEvent::Progress { .. }
        ));
        assert!(matches!(
            rx.recv().unwrap(),
            HeadlessFileTransferEvent::Completed(_)
        ));
        assert!(matches!(
            rx.recv().unwrap(),
            HeadlessFileTransferEvent::JobFailed { .. }
        ));
    }

    #[test]
    fn maps_authentication_and_error_message_boxes() {
        let (handler, rx) = handler();
        handler.msgbox("input-password", "", "", "", false);
        handler.msgbox("re-input-password", "", "", "", false);
        handler.msgbox("input-2fa", "", "", "", false);
        handler.msgbox("insecure-connection", "", "", "", false);
        handler.msgbox("connection-error", "title", "message", "", false);

        assert!(matches!(
            rx.recv().unwrap(),
            HeadlessFileTransferEvent::Auth(_)
        ));
        assert!(matches!(
            rx.recv().unwrap(),
            HeadlessFileTransferEvent::Auth(_)
        ));
        assert!(matches!(
            rx.recv().unwrap(),
            HeadlessFileTransferEvent::Auth(_)
        ));
        assert!(matches!(
            rx.recv().unwrap(),
            HeadlessFileTransferEvent::Auth(_)
        ));
        assert_eq!(
            rx.recv().unwrap(),
            HeadlessFileTransferEvent::ConnectionFailed("message".into())
        );
    }

    #[test]
    fn sanitizes_malformed_completion_and_invalid_progress_values() {
        let (handler, rx) = handler();
        handler.job_progress(7, 0, f64::INFINITY, -1.0);
        handler.job_progress(7, 0, u64::MAX as f64, u64::MAX as f64);
        handler.file_transfer_job_completed(r#"{"id":0,"fileNum":1}"#);

        assert_eq!(
            rx.recv().unwrap(),
            HeadlessFileTransferEvent::Progress {
                id: 7,
                file_num: 0,
                speed: 0,
                finished_size: 0,
            }
        );
        assert_eq!(
            rx.recv().unwrap(),
            HeadlessFileTransferEvent::Progress {
                id: 7,
                file_num: 0,
                speed: u64::MAX,
                finished_size: u64::MAX,
            }
        );
        assert_eq!(
            rx.recv().unwrap(),
            HeadlessFileTransferEvent::ProtocolFailed(
                "invalid file-transfer completion event".into()
            )
        );
    }

    #[test]
    fn unrelated_ui_callbacks_do_not_emit_events() {
        let (handler, rx) = handler();
        handler.set_cursor_id("cursor-1".into());
        handler.update_quality_status(QualityStatus::default());
        handler.next_rgba(0);
        handler.set_connection_type(false, false, "relay");
        handler.set_fingerprint("fingerprint".into());
        handler.job_done(7, 0);
        handler.clear_all_jobs();

        assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));
    }
}
