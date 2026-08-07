use super::{
    handler::HeadlessFileTransferEvent, single_regular_file_size, split_remote_file_path,
    FileSnapshot, HeadlessFileTransferArgs, HeadlessFileTransferError, RemoteFilePath,
    TransferDirection,
};
use crate::headless_auth::AuthPrompt;
use hbb_common::message_proto::FileType;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TransferSignal {
    Interrupt,
    Terminate,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum RuntimeEvent {
    Session(HeadlessFileTransferEvent),
    Signal(TransferSignal),
    TransportClosed,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum TransferAction {
    StartJob {
        id: i32,
        source: String,
        destination: String,
        is_remote: bool,
    },
    ConfirmOverwrite {
        id: i32,
        file_num: i32,
        overwrite: bool,
        is_upload: bool,
    },
    ReadRemoteDir {
        path: String,
        include_hidden: bool,
    },
    CancelJob {
        id: i32,
    },
    CloseTransport,
    RejectInsecureConnection,
}

pub(crate) trait TransferBackend {
    fn stdin_is_tty(&self) -> bool;
    fn action(&mut self, action: TransferAction);
    fn verify_push_source(&mut self) -> Result<(), HeadlessFileTransferError>;
    fn verify_pull_destination(
        &mut self,
        expected_size: u64,
    ) -> Result<(), HeadlessFileTransferError>;
    fn prompt_secret(&mut self) -> Result<Option<String>, HeadlessFileTransferError>;
    fn prompt_confirmation(&mut self) -> Result<Option<bool>, HeadlessFileTransferError>;
    fn prompt_line(&mut self) -> Result<Option<String>, HeadlessFileTransferError>;
    fn login(&mut self, password: String, remember: bool);
    fn send_two_factor(&mut self, code: String);
    fn write_stdout(&mut self, destination: &str) -> Result<(), HeadlessFileTransferError>;
    fn write_stderr(&mut self, message: &str);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TransferPhase {
    Authenticating,
    Transferring,
    FinalizingPush,
    Closing,
    Closed,
    Failed,
}

pub(crate) struct TransferCoordinator {
    args: HeadlessFileTransferArgs,
    expected_job_id: i32,
    phase: TransferPhase,
    peer_platform: Option<String>,
    expected_size: Option<u64>,
    maximum_finished_size: u64,
    conflict_selected: bool,
    password_submission_pending: bool,
    final_status: Option<i32>,
    push_postflight_path: Option<RemoteFilePath>,
}

impl TransferCoordinator {
    pub(crate) fn new(
        args: HeadlessFileTransferArgs,
        push_snapshot: Option<FileSnapshot>,
        expected_job_id: i32,
    ) -> Self {
        let expected_size = match args.direction {
            TransferDirection::Push => push_snapshot.map(|snapshot| snapshot.size),
            TransferDirection::Pull => None,
        };
        Self {
            args,
            expected_job_id,
            phase: TransferPhase::Authenticating,
            peer_platform: None,
            expected_size,
            maximum_finished_size: 0,
            conflict_selected: false,
            password_submission_pending: false,
            final_status: None,
            push_postflight_path: None,
        }
    }

    pub(crate) fn handle(
        &mut self,
        event: RuntimeEvent,
        backend: &mut impl TransferBackend,
    ) -> Option<i32> {
        match event {
            RuntimeEvent::TransportClosed => self.handle_transport_closed(backend),
            RuntimeEvent::Signal(signal) => self.handle_signal(signal, backend),
            RuntimeEvent::Session(event) => self.handle_session(event, backend),
        }
    }

    fn handle_transport_closed(&mut self, backend: &mut impl TransferBackend) -> Option<i32> {
        match self.phase {
            TransferPhase::Closing => {
                self.phase = TransferPhase::Closed;
                Some(self.final_status.unwrap_or(5))
            }
            TransferPhase::Closed | TransferPhase::Failed => Some(self.final_status.unwrap_or(5)),
            TransferPhase::Authenticating
            | TransferPhase::Transferring
            | TransferPhase::FinalizingPush => {
                backend.write_stderr("transfer interrupted; a partial file may remain");
                self.phase = TransferPhase::Failed;
                self.final_status = Some(5);
                Some(5)
            }
        }
    }

    fn handle_signal(
        &mut self,
        signal: TransferSignal,
        backend: &mut impl TransferBackend,
    ) -> Option<i32> {
        if matches!(
            self.phase,
            TransferPhase::Closing | TransferPhase::Closed | TransferPhase::Failed
        ) {
            return None;
        }
        let status = match signal {
            TransferSignal::Interrupt => 130,
            TransferSignal::Terminate => 143,
        };
        backend.action(TransferAction::CancelJob {
            id: self.expected_job_id,
        });
        self.begin_close(status, backend);
        None
    }

    fn handle_session(
        &mut self,
        event: HeadlessFileTransferEvent,
        backend: &mut impl TransferBackend,
    ) -> Option<i32> {
        if matches!(
            self.phase,
            TransferPhase::Closing | TransferPhase::Closed | TransferPhase::Failed
        ) {
            return None;
        }

        match event {
            HeadlessFileTransferEvent::PeerPlatform(platform) => {
                if self.phase != TransferPhase::Authenticating
                    || self.peer_platform.is_some()
                    || platform.is_empty()
                {
                    self.protocol_failure("unexpected peer platform event", backend);
                } else {
                    self.peer_platform = Some(platform);
                }
            }
            HeadlessFileTransferEvent::Connected => self.handle_connected(backend),
            HeadlessFileTransferEvent::Auth(prompt) => self.handle_auth(prompt, backend),
            HeadlessFileTransferEvent::Files {
                id,
                entries,
                path,
                is_local,
                only_count,
            } => self.handle_files(id, entries, path, is_local, only_count, backend),
            HeadlessFileTransferEvent::Conflict {
                id,
                file_num,
                destination: _,
                is_upload,
                is_identical: _,
            } => self.handle_conflict(id, file_num, is_upload, backend),
            HeadlessFileTransferEvent::Progress {
                id,
                file_num,
                speed,
                finished_size,
            } => self.handle_progress(id, file_num, speed, finished_size, backend),
            HeadlessFileTransferEvent::Completed(completion) => {
                self.handle_completion(completion, backend)
            }
            HeadlessFileTransferEvent::JobFailed {
                id,
                file_num: _,
                message,
            } => {
                if id != self.expected_job_id
                    || !matches!(
                        self.phase,
                        TransferPhase::Transferring | TransferPhase::FinalizingPush
                    )
                {
                    self.protocol_failure(
                        "job failure used an unexpected job ID or phase",
                        backend,
                    );
                } else {
                    self.fail(
                        HeadlessFileTransferError::Transfer(if message.is_empty() {
                            "file transfer job failed".into()
                        } else {
                            message
                        }),
                        backend,
                    );
                }
            }
            HeadlessFileTransferEvent::ProtocolFailed(message) => {
                self.fail(HeadlessFileTransferError::Protocol(message), backend);
            }
            HeadlessFileTransferEvent::ConnectionFailed(message) => {
                self.fail(HeadlessFileTransferError::Connection(message), backend);
            }
        }
        None
    }

    fn handle_connected(&mut self, backend: &mut impl TransferBackend) {
        if self.phase != TransferPhase::Authenticating || self.peer_platform.is_none() {
            self.protocol_failure("connected event arrived outside authentication", backend);
            return;
        }
        if self.args.direction == TransferDirection::Push && self.expected_size.is_none() {
            self.fail(
                HeadlessFileTransferError::LocalPrecondition(
                    "push source snapshot is unavailable".into(),
                ),
                backend,
            );
            return;
        }

        self.phase = TransferPhase::Transferring;
        self.password_submission_pending = false;
        backend.action(TransferAction::StartJob {
            id: self.expected_job_id,
            source: self.args.source.clone(),
            destination: self.args.destination.clone(),
            is_remote: self.args.direction == TransferDirection::Pull,
        });
    }

    fn handle_auth(&mut self, prompt: AuthPrompt, backend: &mut impl TransferBackend) {
        if self.phase != TransferPhase::Authenticating {
            self.protocol_failure(
                "authentication prompt arrived after transfer started",
                backend,
            );
            return;
        }
        if prompt == AuthPrompt::InsecureConnection {
            backend.action(TransferAction::RejectInsecureConnection);
            return;
        }
        if !backend.stdin_is_tty() {
            self.fail(
                HeadlessFileTransferError::Authentication(
                    "authentication prompt requires an interactive TTY".into(),
                ),
                backend,
            );
            return;
        }

        match prompt {
            AuthPrompt::Password { retry } => {
                if retry {
                    self.password_submission_pending = false;
                } else if self.password_submission_pending {
                    return;
                }
                let password = match backend.prompt_secret() {
                    Ok(Some(password)) => password,
                    Ok(None) => {
                        self.fail(
                            HeadlessFileTransferError::Authentication(
                                "password prompt reached EOF".into(),
                            ),
                            backend,
                        );
                        return;
                    }
                    Err(error) => {
                        self.fail(error, backend);
                        return;
                    }
                };
                let remember = match backend.prompt_confirmation() {
                    Ok(Some(remember)) => remember,
                    Ok(None) => {
                        self.fail(
                            HeadlessFileTransferError::Authentication(
                                "confirmation prompt reached EOF".into(),
                            ),
                            backend,
                        );
                        return;
                    }
                    Err(error) => {
                        self.fail(error, backend);
                        return;
                    }
                };
                backend.login(password, remember);
                self.password_submission_pending = true;
            }
            AuthPrompt::TwoFactor => {
                let code = match backend.prompt_line() {
                    Ok(Some(code)) => code,
                    Ok(None) => {
                        self.fail(
                            HeadlessFileTransferError::Authentication(
                                "2FA prompt reached EOF".into(),
                            ),
                            backend,
                        );
                        return;
                    }
                    Err(error) => {
                        self.fail(error, backend);
                        return;
                    }
                };
                backend.send_two_factor(code);
            }
            AuthPrompt::InsecureConnection => {}
        }
    }

    fn handle_files(
        &mut self,
        id: i32,
        entries: Vec<hbb_common::message_proto::FileEntry>,
        path: String,
        is_local: bool,
        only_count: bool,
        backend: &mut impl TransferBackend,
    ) {
        match self.phase {
            TransferPhase::Transferring => {
                if id != self.expected_job_id {
                    self.protocol_failure("file metadata used an unexpected job ID", backend);
                    return;
                }
                let size = match single_regular_file_size(&entries) {
                    Ok(size) => size,
                    Err(error) => {
                        self.fail(error, backend);
                        return;
                    }
                };
                if let Some(expected_size) = self.expected_size {
                    if size != expected_size {
                        self.protocol_failure("file metadata size changed", backend);
                    }
                } else {
                    self.expected_size = Some(size);
                }
            }
            TransferPhase::FinalizingPush => {
                let Some(postflight) = self.push_postflight_path.as_ref() else {
                    self.protocol_failure("push postflight path is unavailable", backend);
                    return;
                };
                let Some(expected_size) = self.expected_size else {
                    self.protocol_failure("push postflight size is unavailable", backend);
                    return;
                };
                let matches_destination = id == 0
                    && path == postflight.parent
                    && !is_local
                    && !only_count
                    && entries.iter().any(|entry| {
                        entry.name == postflight.name
                            && entry.entry_type.enum_value() == Ok(FileType::File)
                            && entry.size == expected_size
                    });
                if !matches_destination {
                    self.protocol_failure(
                        "remote destination did not match push postflight metadata",
                        backend,
                    );
                    return;
                }
                if let Err(error) = backend.write_stdout(&self.args.destination) {
                    self.fail(error, backend);
                    return;
                }
                self.begin_close(0, backend);
            }
            _ => self.protocol_failure("file metadata arrived in an unexpected phase", backend),
        }
    }

    fn handle_conflict(
        &mut self,
        id: i32,
        file_num: i32,
        is_upload: bool,
        backend: &mut impl TransferBackend,
    ) {
        let expected_upload = self.args.direction == TransferDirection::Push;
        if self.phase != TransferPhase::Transferring
            || id != self.expected_job_id
            || file_num != 0
            || is_upload != expected_upload
        {
            self.protocol_failure("overwrite conflict used an unexpected job shape", backend);
            return;
        }
        if !self.args.overwrite {
            self.conflict_selected = true;
        }
        backend.action(TransferAction::ConfirmOverwrite {
            id,
            file_num: 0,
            overwrite: self.args.overwrite,
            is_upload,
        });
    }

    fn handle_progress(
        &mut self,
        id: i32,
        file_num: i32,
        speed: u64,
        finished_size: u64,
        backend: &mut impl TransferBackend,
    ) {
        if self.phase != TransferPhase::Transferring || id != self.expected_job_id || file_num != 0
        {
            self.protocol_failure("progress used an unexpected job shape", backend);
            return;
        }
        let Some(total) = self.expected_size else {
            self.protocol_failure("progress arrived before file metadata", backend);
            return;
        };
        let bounded_finished = finished_size.min(total);
        self.maximum_finished_size = self.maximum_finished_size.max(bounded_finished);
        let percent = if total == 0 {
            100.0
        } else {
            self.maximum_finished_size as f64 * 100.0 / total as f64
        };
        let direction = match self.args.direction {
            TransferDirection::Push => "push",
            TransferDirection::Pull => "pull",
        };
        backend.write_stderr(&format!(
            "direction={direction} transferred={} total={total} percent={percent:.2} speed_bps={speed}",
            self.maximum_finished_size
        ));
    }

    fn handle_completion(
        &mut self,
        completion: super::completion::TransferCompletion,
        backend: &mut impl TransferBackend,
    ) {
        if self.phase != TransferPhase::Transferring
            || completion.id != self.expected_job_id
            || completion.file_num != 1
            || !completion.done
            || !completion.error.is_empty()
            || completion.finished_size != completion.total_size
            || self.expected_size != Some(completion.total_size)
        {
            self.protocol_failure("transfer completion was incomplete or unexpected", backend);
            return;
        }
        if self.conflict_selected {
            self.begin_close(7, backend);
            return;
        }

        match self.args.direction {
            TransferDirection::Pull => {
                if let Err(error) = backend.verify_pull_destination(completion.total_size) {
                    self.fail(error, backend);
                    return;
                }
                if let Err(error) = backend.write_stdout(&self.args.destination) {
                    self.fail(error, backend);
                    return;
                }
                self.begin_close(0, backend);
            }
            TransferDirection::Push => {
                if let Err(error) = backend.verify_push_source() {
                    self.fail(error, backend);
                    return;
                }
                let Some(peer_platform) = self.peer_platform.as_deref() else {
                    self.protocol_failure("peer platform is unavailable", backend);
                    return;
                };
                let postflight = match split_remote_file_path(&self.args.destination, peer_platform)
                {
                    Ok(postflight) => postflight,
                    Err(error) => {
                        self.fail(error, backend);
                        return;
                    }
                };
                backend.action(TransferAction::ReadRemoteDir {
                    path: postflight.parent.clone(),
                    include_hidden: true,
                });
                self.push_postflight_path = Some(postflight);
                self.phase = TransferPhase::FinalizingPush;
            }
        }
    }

    fn protocol_failure(&mut self, message: &str, backend: &mut impl TransferBackend) {
        self.fail(
            HeadlessFileTransferError::Protocol(message.to_owned()),
            backend,
        );
    }

    fn fail(&mut self, error: HeadlessFileTransferError, backend: &mut impl TransferBackend) {
        backend.write_stderr(error_message(&error));
        let status = error.status();
        if self.phase != TransferPhase::Closing {
            self.begin_close(status, backend);
        } else {
            self.final_status = Some(status);
        }
    }

    fn begin_close(&mut self, status: i32, backend: &mut impl TransferBackend) {
        self.phase = TransferPhase::Closing;
        self.final_status = Some(status);
        backend.action(TransferAction::CloseTransport);
    }
}

fn error_message(error: &HeadlessFileTransferError) -> &str {
    match error {
        HeadlessFileTransferError::Internal(message)
        | HeadlessFileTransferError::Usage(message)
        | HeadlessFileTransferError::LocalPrecondition(message)
        | HeadlessFileTransferError::Authentication(message)
        | HeadlessFileTransferError::Connection(message)
        | HeadlessFileTransferError::Transfer(message)
        | HeadlessFileTransferError::DestinationExists(message)
        | HeadlessFileTransferError::Protocol(message) => message,
        HeadlessFileTransferError::Interrupted => "transfer interrupted",
        HeadlessFileTransferError::Terminated => "transfer terminated",
    }
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, time::UNIX_EPOCH};

    use crate::headless_auth::AuthPrompt;
    use hbb_common::message_proto::{FileEntry, FileType};

    use super::*;
    use crate::headless_file_transfer::{completion::TransferCompletion, TransferDirection};

    #[derive(Default)]
    struct FakeBackend {
        actions: Vec<TransferAction>,
        stdout: Vec<String>,
        stderr: Vec<String>,
        prompts: Vec<&'static str>,
        stdin_is_tty: bool,
        push_source_valid: bool,
        pull_destination_sizes: Vec<u64>,
        secret: Option<String>,
        confirmation: Option<bool>,
        line: Option<String>,
        logins: Vec<(String, bool)>,
        two_factor_codes: Vec<String>,
    }

    impl TransferBackend for FakeBackend {
        fn stdin_is_tty(&self) -> bool {
            self.stdin_is_tty
        }

        fn action(&mut self, action: TransferAction) {
            self.actions.push(action);
        }

        fn verify_push_source(&mut self) -> Result<(), HeadlessFileTransferError> {
            if self.push_source_valid {
                Ok(())
            } else {
                Err(HeadlessFileTransferError::LocalPrecondition(
                    "push source changed".into(),
                ))
            }
        }

        fn verify_pull_destination(
            &mut self,
            expected_size: u64,
        ) -> Result<(), HeadlessFileTransferError> {
            self.pull_destination_sizes.push(expected_size);
            Ok(())
        }

        fn prompt_secret(&mut self) -> Result<Option<String>, HeadlessFileTransferError> {
            self.prompts.push("secret");
            Ok(self.secret.clone())
        }

        fn prompt_confirmation(&mut self) -> Result<Option<bool>, HeadlessFileTransferError> {
            self.prompts.push("confirmation");
            Ok(self.confirmation)
        }

        fn prompt_line(&mut self) -> Result<Option<String>, HeadlessFileTransferError> {
            self.prompts.push("line");
            Ok(self.line.clone())
        }

        fn login(&mut self, password: String, remember: bool) {
            self.logins.push((password, remember));
        }

        fn send_two_factor(&mut self, code: String) {
            self.two_factor_codes.push(code);
        }

        fn write_stdout(&mut self, destination: &str) -> Result<(), HeadlessFileTransferError> {
            self.stdout.push(destination.into());
            Ok(())
        }

        fn write_stderr(&mut self, message: &str) {
            self.stderr.push(message.into());
        }
    }

    fn args(direction: TransferDirection, overwrite: bool) -> HeadlessFileTransferArgs {
        let (source, destination) = match direction {
            TransferDirection::Push => (
                "/tmp/source.bin".into(),
                r"C:\Users\82520\target.bin".into(),
            ),
            TransferDirection::Pull => (
                r"C:\Users\82520\source.bin".into(),
                "/tmp/target.bin".into(),
            ),
        };
        HeadlessFileTransferArgs {
            peer_id: "175116438".into(),
            direction,
            source,
            destination,
            force_relay: false,
            overwrite,
        }
    }

    fn push_coordinator(overwrite: bool, id: i32, size: u64) -> TransferCoordinator {
        TransferCoordinator::new(
            args(TransferDirection::Push, overwrite),
            Some(FileSnapshot {
                path: PathBuf::from("/tmp/source.bin"),
                size,
                modified: UNIX_EPOCH,
            }),
            id,
        )
    }

    fn pull_coordinator(overwrite: bool, id: i32) -> TransferCoordinator {
        TransferCoordinator::new(args(TransferDirection::Pull, overwrite), None, id)
    }

    fn peer_platform(platform: &str) -> RuntimeEvent {
        RuntimeEvent::Session(HeadlessFileTransferEvent::PeerPlatform(platform.into()))
    }

    fn connected() -> RuntimeEvent {
        RuntimeEvent::Session(HeadlessFileTransferEvent::Connected)
    }

    fn conflict(id: i32, file_num: i32, is_upload: bool) -> RuntimeEvent {
        RuntimeEvent::Session(HeadlessFileTransferEvent::Conflict {
            id,
            file_num,
            destination: if is_upload {
                r"C:\Users\82520\target.bin".into()
            } else {
                "/tmp/target.bin".into()
            },
            is_upload,
            is_identical: false,
        })
    }

    fn completion(id: i32, size: u64) -> RuntimeEvent {
        RuntimeEvent::Session(HeadlessFileTransferEvent::Completed(TransferCompletion {
            id,
            file_num: 1,
            total_size: size,
            finished_size: size,
            done: true,
            error: String::new(),
        }))
    }

    fn regular_file(name: &str, size: u64) -> FileEntry {
        FileEntry {
            entry_type: FileType::File.into(),
            name: name.into(),
            size,
            ..Default::default()
        }
    }

    fn remote_files(id: i32, path: &str, entries: Vec<FileEntry>) -> RuntimeEvent {
        RuntimeEvent::Session(HeadlessFileTransferEvent::Files {
            id,
            entries,
            path: path.into(),
            is_local: false,
            only_count: false,
        })
    }

    fn pull_metadata(id: i32, size: u64) -> RuntimeEvent {
        remote_files(
            id,
            r"C:\Users\82520\source.bin",
            vec![regular_file("source.bin", size)],
        )
    }

    fn job_failed(id: i32) -> RuntimeEvent {
        RuntimeEvent::Session(HeadlessFileTransferEvent::JobFailed {
            id,
            file_num: 0,
            message: "permission denied".into(),
        })
    }

    fn start_pull(coordinator: &mut TransferCoordinator, backend: &mut FakeBackend) {
        assert_eq!(coordinator.handle(peer_platform("Windows"), backend), None);
        assert_eq!(coordinator.handle(connected(), backend), None);
    }

    fn close_status(coordinator: &mut TransferCoordinator, backend: &mut FakeBackend) -> i32 {
        coordinator
            .handle(RuntimeEvent::TransportClosed, backend)
            .unwrap()
    }

    #[test]
    fn connected_starts_exactly_one_push_job() {
        let mut coordinator = push_coordinator(false, 7, 42);
        let mut backend = FakeBackend::default();

        assert_eq!(
            coordinator.handle(peer_platform("Windows"), &mut backend),
            None
        );
        assert_eq!(coordinator.handle(connected(), &mut backend), None);
        assert_eq!(
            backend.actions,
            vec![TransferAction::StartJob {
                id: 7,
                source: "/tmp/source.bin".into(),
                destination: r"C:\Users\82520\target.bin".into(),
                is_remote: false,
            }]
        );
    }

    #[test]
    fn connected_starts_exactly_one_pull_job() {
        let mut coordinator = pull_coordinator(false, 7);
        let mut backend = FakeBackend::default();

        start_pull(&mut coordinator, &mut backend);
        assert_eq!(coordinator.handle(connected(), &mut backend), None);
        assert_eq!(
            backend
                .actions
                .iter()
                .filter(|action| matches!(action, TransferAction::StartJob { .. }))
                .count(),
            1
        );
        assert_eq!(
            backend.actions.first(),
            Some(&TransferAction::StartJob {
                id: 7,
                source: r"C:\Users\82520\source.bin".into(),
                destination: "/tmp/target.bin".into(),
                is_remote: true,
            })
        );
        assert_eq!(
            backend.actions.last(),
            Some(&TransferAction::CloseTransport)
        );
        assert_eq!(close_status(&mut coordinator, &mut backend), 5);
    }

    #[test]
    fn saved_credentials_need_no_tty_but_prompt_without_tty_exits_four() {
        let mut saved = pull_coordinator(false, 7);
        let mut saved_backend = FakeBackend::default();
        start_pull(&mut saved, &mut saved_backend);
        assert!(matches!(
            saved_backend.actions.as_slice(),
            [TransferAction::StartJob { .. }]
        ));

        let mut prompted = pull_coordinator(false, 7);
        let mut prompted_backend = FakeBackend::default();
        prompted.handle(peer_platform("Windows"), &mut prompted_backend);
        assert_eq!(
            prompted.handle(
                RuntimeEvent::Session(HeadlessFileTransferEvent::Auth(AuthPrompt::Password {
                    retry: false,
                })),
                &mut prompted_backend,
            ),
            None
        );
        assert!(prompted_backend.prompts.is_empty());
        assert_eq!(
            prompted_backend.actions,
            vec![TransferAction::CloseTransport]
        );
        assert_eq!(close_status(&mut prompted, &mut prompted_backend), 4);
    }

    #[test]
    fn authentication_prompts_submit_once_and_eof_exits_four() {
        let mut coordinator = pull_coordinator(false, 7);
        let mut backend = FakeBackend {
            stdin_is_tty: true,
            secret: Some("password".into()),
            confirmation: Some(true),
            line: Some("123456".into()),
            ..Default::default()
        };
        coordinator.handle(peer_platform("Windows"), &mut backend);
        let password =
            RuntimeEvent::Session(HeadlessFileTransferEvent::Auth(AuthPrompt::Password {
                retry: false,
            }));
        assert_eq!(coordinator.handle(password.clone(), &mut backend), None);
        assert_eq!(coordinator.handle(password, &mut backend), None);
        assert_eq!(backend.prompts, vec!["secret", "confirmation"]);
        assert_eq!(backend.logins, vec![("password".into(), true)]);
        assert_eq!(
            coordinator.handle(
                RuntimeEvent::Session(HeadlessFileTransferEvent::Auth(AuthPrompt::TwoFactor)),
                &mut backend,
            ),
            None
        );
        assert_eq!(backend.two_factor_codes, vec!["123456"]);

        let mut eof = pull_coordinator(false, 7);
        let mut eof_backend = FakeBackend {
            stdin_is_tty: true,
            ..Default::default()
        };
        eof.handle(peer_platform("Windows"), &mut eof_backend);
        eof.handle(
            RuntimeEvent::Session(HeadlessFileTransferEvent::Auth(AuthPrompt::Password {
                retry: false,
            })),
            &mut eof_backend,
        );
        assert_eq!(eof_backend.actions, vec![TransferAction::CloseTransport]);
        assert_eq!(close_status(&mut eof, &mut eof_backend), 4);
    }

    #[test]
    fn insecure_authentication_is_rejected_without_prompting() {
        let mut coordinator = pull_coordinator(false, 7);
        let mut backend = FakeBackend::default();
        coordinator.handle(peer_platform("Windows"), &mut backend);

        assert_eq!(
            coordinator.handle(
                RuntimeEvent::Session(HeadlessFileTransferEvent::Auth(
                    AuthPrompt::InsecureConnection,
                )),
                &mut backend,
            ),
            None
        );
        assert_eq!(
            backend.actions,
            vec![TransferAction::RejectInsecureConnection]
        );
        assert!(backend.prompts.is_empty());
    }

    #[test]
    fn save_confirmation_and_two_factor_eof_exit_four() {
        let mut confirmation = pull_coordinator(false, 7);
        let mut confirmation_backend = FakeBackend {
            stdin_is_tty: true,
            secret: Some("password".into()),
            confirmation: None,
            ..Default::default()
        };
        confirmation.handle(peer_platform("Windows"), &mut confirmation_backend);
        confirmation.handle(
            RuntimeEvent::Session(HeadlessFileTransferEvent::Auth(AuthPrompt::Password {
                retry: false,
            })),
            &mut confirmation_backend,
        );
        assert_eq!(confirmation_backend.prompts, vec!["secret", "confirmation"]);
        assert_eq!(
            confirmation_backend.actions,
            vec![TransferAction::CloseTransport]
        );
        assert_eq!(
            close_status(&mut confirmation, &mut confirmation_backend),
            4
        );

        let mut two_factor = pull_coordinator(false, 7);
        let mut two_factor_backend = FakeBackend {
            stdin_is_tty: true,
            line: None,
            ..Default::default()
        };
        two_factor.handle(peer_platform("Windows"), &mut two_factor_backend);
        two_factor.handle(
            RuntimeEvent::Session(HeadlessFileTransferEvent::Auth(AuthPrompt::TwoFactor)),
            &mut two_factor_backend,
        );
        assert_eq!(two_factor_backend.prompts, vec!["line"]);
        assert_eq!(
            two_factor_backend.actions,
            vec![TransferAction::CloseTransport]
        );
        assert_eq!(close_status(&mut two_factor, &mut two_factor_backend), 4);
    }

    #[test]
    fn conflict_defaults_to_skip_and_finishes_with_status_seven() {
        let mut coordinator = push_coordinator(false, 7, 42);
        let mut backend = FakeBackend::default();
        coordinator.handle(peer_platform("Windows"), &mut backend);
        coordinator.handle(connected(), &mut backend);

        assert_eq!(coordinator.handle(conflict(7, 0, true), &mut backend), None);
        assert_eq!(
            backend.actions.last(),
            Some(&TransferAction::ConfirmOverwrite {
                id: 7,
                file_num: 0,
                overwrite: false,
                is_upload: true,
            })
        );
        assert_eq!(coordinator.handle(completion(7, 42), &mut backend), None);
        assert_eq!(
            backend.actions.last(),
            Some(&TransferAction::CloseTransport)
        );
        assert_eq!(close_status(&mut coordinator, &mut backend), 7);
        assert!(backend.stdout.is_empty());
    }

    #[test]
    fn overwrite_confirms_offset_zero_and_does_not_resume() {
        let mut push = push_coordinator(true, 7, 42);
        let mut push_backend = FakeBackend::default();
        push.handle(peer_platform("Windows"), &mut push_backend);
        push.handle(connected(), &mut push_backend);
        push.handle(conflict(7, 0, true), &mut push_backend);
        assert_eq!(
            push_backend.actions.last(),
            Some(&TransferAction::ConfirmOverwrite {
                id: 7,
                file_num: 0,
                overwrite: true,
                is_upload: true,
            })
        );

        let mut pull = pull_coordinator(true, 7);
        let mut pull_backend = FakeBackend::default();
        start_pull(&mut pull, &mut pull_backend);
        pull.handle(conflict(7, 0, false), &mut pull_backend);
        assert_eq!(
            pull_backend.actions.last(),
            Some(&TransferAction::ConfirmOverwrite {
                id: 7,
                file_num: 0,
                overwrite: true,
                is_upload: false,
            })
        );
        assert_eq!(push_backend.actions.len(), 2);
        assert_eq!(pull_backend.actions.len(), 2);
    }

    #[test]
    fn pull_success_requires_regular_metadata_and_complete_local_write() {
        let mut coordinator = pull_coordinator(false, 7);
        let mut backend = FakeBackend::default();
        start_pull(&mut coordinator, &mut backend);

        assert_eq!(coordinator.handle(pull_metadata(7, 42), &mut backend), None);
        assert_eq!(coordinator.handle(completion(7, 42), &mut backend), None);
        assert_eq!(backend.pull_destination_sizes, vec![42]);
        assert_eq!(backend.stdout, vec!["/tmp/target.bin"]);
        assert_eq!(
            backend.actions.last(),
            Some(&TransferAction::CloseTransport)
        );
        assert_eq!(close_status(&mut coordinator, &mut backend), 0);
    }

    #[test]
    fn push_success_requires_source_unchanged_then_remote_read_dir_match() {
        let mut coordinator = push_coordinator(false, 7, 42);
        let mut backend = FakeBackend {
            push_source_valid: true,
            ..Default::default()
        };
        coordinator.handle(peer_platform("Windows"), &mut backend);
        coordinator.handle(connected(), &mut backend);

        assert_eq!(coordinator.handle(completion(7, 42), &mut backend), None);
        assert_eq!(
            backend.actions.last(),
            Some(&TransferAction::ReadRemoteDir {
                path: r"C:\Users\82520".into(),
                include_hidden: true,
            })
        );
        assert_eq!(
            coordinator.handle(
                remote_files(0, r"C:\Users\82520", vec![regular_file("target.bin", 42)]),
                &mut backend,
            ),
            None
        );
        assert_eq!(backend.stdout, vec![r"C:\Users\82520\target.bin"]);
        assert_eq!(
            backend.actions.last(),
            Some(&TransferAction::CloseTransport)
        );
        assert_eq!(close_status(&mut coordinator, &mut backend), 0);
    }

    #[test]
    fn push_source_and_remote_postflight_mismatches_never_succeed() {
        let mut changed_source = push_coordinator(false, 7, 42);
        let mut changed_source_backend = FakeBackend::default();
        changed_source.handle(peer_platform("Windows"), &mut changed_source_backend);
        changed_source.handle(connected(), &mut changed_source_backend);
        changed_source.handle(completion(7, 42), &mut changed_source_backend);
        assert_eq!(
            changed_source_backend.actions.last(),
            Some(&TransferAction::CloseTransport)
        );
        assert!(changed_source_backend.stdout.is_empty());
        assert_eq!(
            close_status(&mut changed_source, &mut changed_source_backend),
            3
        );

        let wrong_directory = vec![
            (1, r"C:\Users\82520", regular_file("target.bin", 42)),
            (0, r"C:\Users", regular_file("target.bin", 42)),
            (0, r"C:\Users\82520", regular_file("other.bin", 42)),
            (
                0,
                r"C:\Users\82520",
                FileEntry {
                    entry_type: FileType::Dir.into(),
                    name: "target.bin".into(),
                    size: 42,
                    ..Default::default()
                },
            ),
            (0, r"C:\Users\82520", regular_file("target.bin", 41)),
        ];
        for (id, path, entry) in wrong_directory {
            let mut coordinator = push_coordinator(false, 7, 42);
            let mut backend = FakeBackend {
                push_source_valid: true,
                ..Default::default()
            };
            coordinator.handle(peer_platform("Windows"), &mut backend);
            coordinator.handle(connected(), &mut backend);
            coordinator.handle(completion(7, 42), &mut backend);
            coordinator.handle(remote_files(id, path, vec![entry]), &mut backend);

            assert!(backend.stdout.is_empty());
            assert_eq!(
                backend.actions.last(),
                Some(&TransferAction::CloseTransport)
            );
            assert_eq!(close_status(&mut coordinator, &mut backend), 5);
        }
    }

    #[test]
    fn incomplete_or_wrong_job_completion_is_protocol_status_five() {
        let cases = [
            TransferCompletion {
                id: 8,
                file_num: 1,
                total_size: 42,
                finished_size: 42,
                done: true,
                error: String::new(),
            },
            TransferCompletion {
                id: 7,
                file_num: 1,
                total_size: 42,
                finished_size: 42,
                done: false,
                error: String::new(),
            },
            TransferCompletion {
                id: 7,
                file_num: 1,
                total_size: 42,
                finished_size: 41,
                done: true,
                error: String::new(),
            },
        ];

        for completion in cases {
            let mut coordinator = push_coordinator(false, 7, 42);
            let mut backend = FakeBackend::default();
            coordinator.handle(peer_platform("Windows"), &mut backend);
            coordinator.handle(connected(), &mut backend);
            coordinator.handle(
                RuntimeEvent::Session(HeadlessFileTransferEvent::Completed(completion)),
                &mut backend,
            );

            assert_eq!(
                backend
                    .actions
                    .iter()
                    .filter(|action| **action == TransferAction::CloseTransport)
                    .count(),
                1
            );
            assert!(backend.stdout.is_empty());
            assert_eq!(close_status(&mut coordinator, &mut backend), 5);
        }
    }

    #[test]
    fn job_error_maps_to_six_and_connection_loss_maps_to_five() {
        let mut failed = pull_coordinator(false, 7);
        let mut failed_backend = FakeBackend::default();
        start_pull(&mut failed, &mut failed_backend);
        assert_eq!(failed.handle(job_failed(7), &mut failed_backend), None);
        assert_eq!(close_status(&mut failed, &mut failed_backend), 6);
        assert_eq!(
            failed_backend
                .actions
                .iter()
                .filter(|action| matches!(action, TransferAction::StartJob { .. }))
                .count(),
            1
        );
        assert!(failed_backend.stdout.is_empty());

        let mut interrupted = pull_coordinator(false, 7);
        let mut interrupted_backend = FakeBackend::default();
        start_pull(&mut interrupted, &mut interrupted_backend);
        assert_eq!(
            interrupted.handle(RuntimeEvent::TransportClosed, &mut interrupted_backend),
            Some(5)
        );
        assert_eq!(
            interrupted_backend.stderr,
            vec!["transfer interrupted; a partial file may remain"]
        );
        assert_eq!(
            interrupted_backend
                .actions
                .iter()
                .filter(|action| matches!(action, TransferAction::StartJob { .. }))
                .count(),
            1
        );
        assert!(interrupted_backend.stdout.is_empty());
    }

    #[test]
    fn interrupt_cancels_before_close_and_returns_130() {
        let mut coordinator = pull_coordinator(false, 7);
        let mut backend = FakeBackend::default();
        start_pull(&mut coordinator, &mut backend);

        assert_eq!(
            coordinator.handle(
                RuntimeEvent::Signal(TransferSignal::Interrupt),
                &mut backend
            ),
            None
        );
        assert_eq!(
            &backend.actions[1..],
            &[
                TransferAction::CancelJob { id: 7 },
                TransferAction::CloseTransport
            ]
        );
        assert_eq!(close_status(&mut coordinator, &mut backend), 130);
    }

    #[test]
    fn terminate_cancels_before_close_and_returns_143() {
        let mut coordinator = pull_coordinator(false, 7);
        let mut backend = FakeBackend::default();
        start_pull(&mut coordinator, &mut backend);

        assert_eq!(
            coordinator.handle(
                RuntimeEvent::Signal(TransferSignal::Terminate),
                &mut backend
            ),
            None
        );
        assert_eq!(
            &backend.actions[1..],
            &[
                TransferAction::CancelJob { id: 7 },
                TransferAction::CloseTransport
            ]
        );
        assert_eq!(close_status(&mut coordinator, &mut backend), 143);
    }

    #[test]
    fn success_outputs_only_destination_and_failure_outputs_nothing() {
        let mut success = pull_coordinator(false, 7);
        let mut success_backend = FakeBackend::default();
        start_pull(&mut success, &mut success_backend);
        success.handle(pull_metadata(7, 42), &mut success_backend);
        success.handle(completion(7, 42), &mut success_backend);
        assert_eq!(success_backend.stdout, vec!["/tmp/target.bin"]);

        let mut failure = pull_coordinator(false, 7);
        let mut failure_backend = FakeBackend::default();
        start_pull(&mut failure, &mut failure_backend);
        failure.handle(job_failed(7), &mut failure_backend);
        assert!(failure_backend.stdout.is_empty());
        assert_eq!(failure_backend.stderr, vec!["permission denied"]);
    }

    #[test]
    fn committed_success_ignores_late_session_events_while_closing() {
        let mut coordinator = pull_coordinator(false, 7);
        let mut backend = FakeBackend::default();
        start_pull(&mut coordinator, &mut backend);
        coordinator.handle(pull_metadata(7, 42), &mut backend);
        coordinator.handle(completion(7, 42), &mut backend);

        assert_eq!(
            coordinator.handle(
                RuntimeEvent::Session(HeadlessFileTransferEvent::Progress {
                    id: 7,
                    file_num: 0,
                    speed: 1,
                    finished_size: 42,
                }),
                &mut backend,
            ),
            None
        );
        assert_eq!(backend.stdout, vec!["/tmp/target.bin"]);
        assert_eq!(close_status(&mut coordinator, &mut backend), 0);
    }

    #[test]
    fn progress_is_monotonic_and_bounded_by_expected_size() {
        let mut coordinator = push_coordinator(false, 7, 46_964_366);
        let mut backend = FakeBackend::default();
        coordinator.handle(peer_platform("Windows"), &mut backend);
        coordinator.handle(connected(), &mut backend);
        for finished_size in [1_048_576, u64::MAX, 1] {
            coordinator.handle(
                RuntimeEvent::Session(HeadlessFileTransferEvent::Progress {
                    id: 7,
                    file_num: 0,
                    speed: 1_048_576,
                    finished_size,
                }),
                &mut backend,
            );
        }

        assert_eq!(
            backend.stderr,
            vec![
                "direction=push transferred=1048576 total=46964366 percent=2.23 speed_bps=1048576",
                "direction=push transferred=46964366 total=46964366 percent=100.00 speed_bps=1048576",
                "direction=push transferred=46964366 total=46964366 percent=100.00 speed_bps=1048576",
            ]
        );
    }

    #[test]
    fn unexpected_ids_and_events_close_with_protocol_status_five() {
        let mut wrong_id = pull_coordinator(false, 7);
        let mut wrong_id_backend = FakeBackend::default();
        start_pull(&mut wrong_id, &mut wrong_id_backend);
        wrong_id.handle(
            RuntimeEvent::Session(HeadlessFileTransferEvent::Progress {
                id: 8,
                file_num: 0,
                speed: 1,
                finished_size: 1,
            }),
            &mut wrong_id_backend,
        );
        assert_eq!(close_status(&mut wrong_id, &mut wrong_id_backend), 5);
        assert!(wrong_id_backend.stdout.is_empty());

        let mut missing_platform = pull_coordinator(false, 7);
        let mut missing_platform_backend = FakeBackend::default();
        missing_platform.handle(connected(), &mut missing_platform_backend);
        assert_eq!(
            close_status(&mut missing_platform, &mut missing_platform_backend),
            5
        );
        assert!(missing_platform_backend.stdout.is_empty());
    }
}
