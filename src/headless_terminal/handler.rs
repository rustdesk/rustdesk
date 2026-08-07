use crate::{
    client::QualityStatus, headless_auth::AuthPrompt, ui_session_interface::InvokeUiSession,
};
use hbb_common::{log, message_proto::*, rendezvous_proto::ConnType};
#[cfg(all(feature = "vram", feature = "flutter"))]
use std::ffi::c_void;
use std::sync::{mpsc::Sender, Arc, Mutex};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HeadlessEvent {
    Connected,
    Auth(AuthPrompt),
    Opened {
        terminal_id: i32,
        success: bool,
        message: String,
        pid: u32,
        service_id: String,
        persistent_sessions: Vec<i32>,
        replay_terminal_output: bool,
    },
    Output {
        terminal_id: i32,
        data: Vec<u8>,
    },
    Closed {
        terminal_id: i32,
        exit_code: i32,
    },
    Failed {
        terminal_id: i32,
        message: String,
    },
    ConnectionFailed(String),
}

#[derive(Clone, Default)]
pub(crate) struct HeadlessTerminalHandler {
    event_tx: Arc<Mutex<Option<Sender<HeadlessEvent>>>>,
}

impl HeadlessTerminalHandler {
    pub(crate) fn new(event_tx: Sender<HeadlessEvent>) -> Self {
        Self {
            event_tx: Arc::new(Mutex::new(Some(event_tx))),
        }
    }

    fn emit(&self, event: HeadlessEvent) -> bool {
        let event_tx = match self.event_tx.lock() {
            Ok(event_tx) => event_tx,
            Err(_) => {
                log::error!("Headless terminal event sender lock is poisoned");
                return false;
            }
        };
        let Some(event_tx) = event_tx.as_ref() else {
            log::error!("Headless terminal event sender is unavailable");
            return false;
        };
        if event_tx.send(event).is_err() {
            log::error!("Failed to emit headless terminal event: receiver is closed");
            return false;
        }
        true
    }
}

impl InvokeUiSession for HeadlessTerminalHandler {
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

    fn set_peer_info(&self, _peer_info: &PeerInfo) {}

    fn set_displays(&self, _displays: &Vec<DisplayInfo>) {}

    fn set_platform_additions(&self, _data: &str) {}

    fn on_connected(&self, conn_type: ConnType) {
        if conn_type == ConnType::TERMINAL {
            let _ = self.emit(HeadlessEvent::Connected);
        }
    }

    fn update_privacy_mode(&self) {}

    fn set_permission(&self, _name: &str, _value: bool) {}

    fn close_success(&self) {}

    fn update_quality_status(&self, _quality_status: QualityStatus) {}

    fn set_connection_type(&self, _is_secured: bool, _direct: bool, _stream_type: &str) {}

    fn set_fingerprint(&self, _fingerprint: String) {}

    fn job_error(&self, _id: i32, _error: String, _file_num: i32) {}

    fn job_done(&self, _id: i32, _file_num: i32) {}

    fn clear_all_jobs(&self) {}

    fn new_message(&self, _message: String) {}

    fn update_transfer_list(&self) {}

    fn load_last_job(&self, _count: i32, _job_json: &str, _auto_start: bool) {}

    fn update_folder_files(
        &self,
        _id: i32,
        _entries: &Vec<FileEntry>,
        _path: String,
        _is_local: bool,
        _only_count: bool,
    ) {
    }

    fn confirm_delete_files(&self, _id: i32, _file_num: i32, _name: String) {}

    fn override_file_confirm(
        &self,
        _id: i32,
        _file_num: i32,
        _destination: String,
        _is_upload: bool,
        _is_identical: bool,
    ) {
    }

    fn update_block_input_state(&self, _on: bool) {}

    fn job_progress(&self, _id: i32, _file_num: i32, _speed: f64, _finished_size: f64) {}

    fn adapt_size(&self) {}

    fn on_rgba(&self, _display: usize, _rgba: &mut scrap::ImageRgb) {}

    fn msgbox(&self, msgtype: &str, title: &str, text: &str, _link: &str, _retry: bool) {
        let event = match msgtype {
            "input-password" => Some(HeadlessEvent::Auth(AuthPrompt::Password { retry: false })),
            "re-input-password" => Some(HeadlessEvent::Auth(AuthPrompt::Password { retry: true })),
            "input-2fa" => Some(HeadlessEvent::Auth(AuthPrompt::TwoFactor)),
            _ if msgtype.contains("insecure-connection") => {
                Some(HeadlessEvent::Auth(AuthPrompt::InsecureConnection))
            }
            _ if msgtype.contains("error") => {
                let message = if !text.is_empty() {
                    text
                } else if !title.is_empty() {
                    title
                } else {
                    msgtype
                };
                Some(HeadlessEvent::ConnectionFailed(message.to_owned()))
            }
            _ => None,
        };

        if let Some(event) = event {
            let _ = self.emit(event);
        }
    }

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

    fn handle_terminal_response(&self, response: TerminalResponse) {
        use hbb_common::message_proto::terminal_response::Union;

        let event = match response.union {
            Some(Union::Opened(opened)) => Some(HeadlessEvent::Opened {
                terminal_id: opened.terminal_id,
                success: opened.success,
                message: opened.message,
                pid: opened.pid,
                service_id: opened.service_id,
                persistent_sessions: opened.persistent_sessions,
                replay_terminal_output: opened.replay_terminal_output,
            }),
            Some(Union::Data(data)) => {
                let output = if data.compressed {
                    hbb_common::compress::decompress(&data.data)
                } else {
                    data.data.to_vec()
                };
                Some(HeadlessEvent::Output {
                    terminal_id: data.terminal_id,
                    data: output,
                })
            }
            Some(Union::Closed(closed)) => Some(HeadlessEvent::Closed {
                terminal_id: closed.terminal_id,
                exit_code: closed.exit_code,
            }),
            Some(Union::Error(error)) => Some(HeadlessEvent::Failed {
                terminal_id: error.terminal_id,
                message: error.message,
            }),
            None => None,
            Some(_) => {
                log::warn!("Unhandled headless terminal response type");
                None
            }
        };

        if let Some(event) = event {
            let _ = self.emit(event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{client::QualityStatus, ui_session_interface::InvokeUiSession};
    use hbb_common::{
        message_proto::{
            terminal_response::Union, TerminalClosed, TerminalData, TerminalError, TerminalOpened,
            TerminalResponse,
        },
        rendezvous_proto::ConnType,
    };
    use std::{
        sync::mpsc::{self, Receiver, TryRecvError},
        time::Duration,
    };

    fn handler_and_receiver() -> (HeadlessTerminalHandler, Receiver<HeadlessEvent>) {
        let (event_tx, event_rx) = mpsc::channel();
        (HeadlessTerminalHandler::new(event_tx), event_rx)
    }

    fn receive_event(event_rx: &Receiver<HeadlessEvent>) -> HeadlessEvent {
        event_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("handler should emit an event")
    }

    #[test]
    fn maps_opened_response_with_persistence_metadata() {
        let (handler, event_rx) = handler_and_receiver();
        let response = TerminalResponse {
            union: Some(Union::Opened(TerminalOpened {
                terminal_id: 17,
                success: true,
                message: "restored".to_owned(),
                pid: 4815,
                service_id: "service-alpha".to_owned(),
                persistent_sessions: vec![3, 8, 13],
                replay_terminal_output: true,
                ..Default::default()
            })),
            ..Default::default()
        };

        handler.handle_terminal_response(response);

        assert_eq!(
            receive_event(&event_rx),
            HeadlessEvent::Opened {
                terminal_id: 17,
                success: true,
                message: "restored".to_owned(),
                pid: 4815,
                service_id: "service-alpha".to_owned(),
                persistent_sessions: vec![3, 8, 13],
                replay_terminal_output: true,
            }
        );
    }

    #[test]
    fn maps_uncompressed_output_without_changing_bytes() {
        let (handler, event_rx) = handler_and_receiver();
        let output = vec![0x00, 0x1b, b'[', b'3', b'1', b'm', 0xff, b'\n'];
        let response = TerminalResponse {
            union: Some(Union::Data(TerminalData {
                terminal_id: 21,
                data: output.clone().into(),
                compressed: false,
                ..Default::default()
            })),
            ..Default::default()
        };

        handler.handle_terminal_response(response);

        assert_eq!(
            receive_event(&event_rx),
            HeadlessEvent::Output {
                terminal_id: 21,
                data: output,
            }
        );
    }

    #[test]
    fn decompresses_output_without_changing_terminal_bytes() {
        let (handler, event_rx) = handler_and_receiver();
        let output = vec![0x1b, b'[', b'2', b'J', 0x00, 0xff, b'\r', b'\n'];
        let response = TerminalResponse {
            union: Some(Union::Data(TerminalData {
                terminal_id: 34,
                data: hbb_common::compress::compress(&output).into(),
                compressed: true,
                ..Default::default()
            })),
            ..Default::default()
        };

        handler.handle_terminal_response(response);

        assert_eq!(
            receive_event(&event_rx),
            HeadlessEvent::Output {
                terminal_id: 34,
                data: output,
            }
        );
    }

    #[test]
    fn maps_closed_response() {
        let (handler, event_rx) = handler_and_receiver();
        let response = TerminalResponse {
            union: Some(Union::Closed(TerminalClosed {
                terminal_id: 55,
                exit_code: 143,
                ..Default::default()
            })),
            ..Default::default()
        };

        handler.handle_terminal_response(response);

        assert_eq!(
            receive_event(&event_rx),
            HeadlessEvent::Closed {
                terminal_id: 55,
                exit_code: 143,
            }
        );
    }

    #[test]
    fn maps_error_response() {
        let (handler, event_rx) = handler_and_receiver();
        let response = TerminalResponse {
            union: Some(Union::Error(TerminalError {
                terminal_id: 89,
                message: "remote terminal failed".to_owned(),
                ..Default::default()
            })),
            ..Default::default()
        };

        handler.handle_terminal_response(response);

        assert_eq!(
            receive_event(&event_rx),
            HeadlessEvent::Failed {
                terminal_id: 89,
                message: "remote terminal failed".to_owned(),
            }
        );
    }

    #[test]
    fn maps_terminal_connection_notification_only() {
        let (handler, event_rx) = handler_and_receiver();

        handler.on_connected(ConnType::DEFAULT_CONN);
        assert_eq!(event_rx.try_recv(), Err(TryRecvError::Empty));

        handler.on_connected(ConnType::TERMINAL);
        assert_eq!(receive_event(&event_rx), HeadlessEvent::Connected);
    }

    #[test]
    fn maps_authentication_prompts() {
        let cases = [
            ("input-password", AuthPrompt::Password { retry: false }),
            ("re-input-password", AuthPrompt::Password { retry: true }),
            ("input-2fa", AuthPrompt::TwoFactor),
            ("insecure-connection", AuthPrompt::InsecureConnection),
            (
                "insecure-connection-nocancel-hasclose",
                AuthPrompt::InsecureConnection,
            ),
        ];

        for (msgtype, expected_prompt) in cases {
            let (handler, event_rx) = handler_and_receiver();

            handler.msgbox(msgtype, "title", "text", "link", false);

            assert_eq!(
                receive_event(&event_rx),
                HeadlessEvent::Auth(expected_prompt),
                "message type {msgtype}"
            );
        }
    }

    #[test]
    fn maps_error_message_boxes_using_non_empty_fallback_priority() {
        let cases = [
            ("connection-error", "title", "text", "text"),
            ("connection-error", "title", "", "title"),
            ("connection-error", "", "", "connection-error"),
        ];

        for (msgtype, title, text, expected_message) in cases {
            let (handler, event_rx) = handler_and_receiver();

            handler.msgbox(msgtype, title, text, "", false);

            assert_eq!(
                receive_event(&event_rx),
                HeadlessEvent::ConnectionFailed(expected_message.to_owned())
            );
        }
    }

    #[test]
    fn unrelated_ui_callbacks_do_not_emit_events() {
        let (handler, event_rx) = handler_and_receiver();

        handler.set_cursor_id("cursor-1".to_owned());
        handler.update_quality_status(QualityStatus::default());
        handler.next_rgba(0);
        handler.set_connection_type(false, false, "relay");
        handler.set_fingerprint("fingerprint".to_owned());

        assert_eq!(event_rx.try_recv(), Err(TryRecvError::Empty));
    }
}
