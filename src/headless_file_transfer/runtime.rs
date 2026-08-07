use std::{
    io::{self, Write},
    path::Path,
    sync::{
        atomic::AtomicUsize,
        mpsc::{self, Receiver, Sender},
        Arc, RwLock,
    },
};

use hbb_common::{fs, rendezvous_proto::ConnType};

use crate::{
    client::FileManager,
    headless_auth::{prompt_confirmation, prompt_line, prompt_secret, stdin_is_tty},
    ui_session_interface::{io_loop, Session},
};

use super::{
    handler::{HeadlessFileTransferEvent, HeadlessFileTransferHandler},
    inspect_pull_destination, inspect_push_source,
    signals::spawn_signal_forwarder,
    verify_source_unchanged, FileSnapshot, HeadlessFileTransferArgs, HeadlessFileTransferError,
    RuntimeEvent, TransferAction, TransferBackend, TransferCoordinator, TransferDirection,
};

trait TransferSession {
    fn send_files(
        &self,
        id: i32,
        job_type: i32,
        source: String,
        destination: String,
        file_num: i32,
        include_hidden: bool,
        is_remote: bool,
    );
    fn set_confirm_override_file(
        &self,
        id: i32,
        file_num: i32,
        overwrite: bool,
        remember: bool,
        is_upload: bool,
    );
    fn read_remote_dir(&self, path: String, include_hidden: bool);
    fn cancel_job(&self, id: i32);
    fn close(&self);
    fn continue_insecure_connection(&self, continue_insecure: bool);
}

fn map_action<S: TransferSession>(session: &S, action: TransferAction) {
    match action {
        TransferAction::StartJob {
            id,
            source,
            destination,
            is_remote,
        } => session.send_files(
            id,
            hbb_common::fs::JobType::Generic as i32,
            source,
            destination,
            0,
            false,
            is_remote,
        ),
        TransferAction::ConfirmOverwrite {
            id,
            file_num,
            overwrite,
            is_upload,
        } => session.set_confirm_override_file(id, file_num, overwrite, false, is_upload),
        TransferAction::ReadRemoteDir {
            path,
            include_hidden,
        } => session.read_remote_dir(path, include_hidden),
        TransferAction::CancelJob { id } => session.cancel_job(id),
        TransferAction::CloseTransport => session.close(),
        TransferAction::RejectInsecureConnection => session.continue_insecure_connection(false),
    }
}

impl TransferSession for Session<HeadlessFileTransferHandler> {
    fn send_files(
        &self,
        id: i32,
        job_type: i32,
        source: String,
        destination: String,
        file_num: i32,
        include_hidden: bool,
        is_remote: bool,
    ) {
        FileManager::send_files(
            self,
            id,
            job_type,
            source,
            destination,
            file_num,
            include_hidden,
            is_remote,
        );
    }

    fn set_confirm_override_file(
        &self,
        id: i32,
        file_num: i32,
        overwrite: bool,
        remember: bool,
        is_upload: bool,
    ) {
        FileManager::set_confirm_override_file(self, id, file_num, overwrite, remember, is_upload);
    }

    fn read_remote_dir(&self, path: String, include_hidden: bool) {
        FileManager::read_remote_dir(self, path, include_hidden);
    }

    fn cancel_job(&self, id: i32) {
        FileManager::cancel_job(self, id);
    }

    fn close(&self) {
        Session::close(self);
    }

    fn continue_insecure_connection(&self, continue_insecure: bool) {
        Session::continue_insecure_connection(self, continue_insecure);
    }
}

pub(crate) struct SystemTransferBackend {
    session: Session<HeadlessFileTransferHandler>,
    source_snapshot: Option<FileSnapshot>,
    destination: String,
}

impl SystemTransferBackend {
    fn new(
        session: Session<HeadlessFileTransferHandler>,
        source_snapshot: Option<FileSnapshot>,
        destination: String,
    ) -> Self {
        Self {
            session,
            source_snapshot,
            destination,
        }
    }
}

impl TransferBackend for SystemTransferBackend {
    fn stdin_is_tty(&self) -> bool {
        stdin_is_tty()
    }

    fn action(&mut self, action: TransferAction) {
        map_action(&self.session, action);
    }

    fn verify_push_source(&mut self) -> Result<(), HeadlessFileTransferError> {
        let snapshot = self.source_snapshot.as_ref().ok_or_else(|| {
            HeadlessFileTransferError::Internal("push source snapshot is unavailable".into())
        })?;
        verify_source_unchanged(snapshot)
    }

    fn verify_pull_destination(
        &mut self,
        expected_size: u64,
    ) -> Result<(), HeadlessFileTransferError> {
        verify_pull_destination_size(Path::new(&self.destination), expected_size)
    }

    fn prompt_secret(&mut self) -> Result<Option<String>, HeadlessFileTransferError> {
        prompt_secret("Password: ").map_err(|error| {
            HeadlessFileTransferError::Authentication(format!(
                "failed to read password securely: {error}"
            ))
        })
    }

    fn prompt_confirmation(&mut self) -> Result<Option<bool>, HeadlessFileTransferError> {
        prompt_confirmation("Save password for this peer? [y/N] ").map_err(|error| {
            HeadlessFileTransferError::Authentication(format!(
                "failed to read password-save confirmation: {error}"
            ))
        })
    }

    fn prompt_line(&mut self) -> Result<Option<String>, HeadlessFileTransferError> {
        prompt_line("2FA code: ").map_err(|error| {
            HeadlessFileTransferError::Authentication(format!("failed to read 2FA code: {error}"))
        })
    }

    fn login(&mut self, password: String, remember: bool) {
        self.session
            .login(String::new(), String::new(), password, remember);
    }

    fn send_two_factor(&mut self, code: String) {
        self.session.send2fa(code, false);
    }

    fn write_stdout(&mut self, destination: &str) -> Result<(), HeadlessFileTransferError> {
        let mut stdout = io::stdout().lock();
        writeln!(stdout, "{destination}")
            .and_then(|()| stdout.flush())
            .map_err(|error| {
                HeadlessFileTransferError::Internal(format!("failed to write stdout: {error}"))
            })
    }

    fn write_stderr(&mut self, message: &str) {
        let mut stderr = io::stderr().lock();
        let _ = writeln!(stderr, "{message}").and_then(|()| stderr.flush());
    }
}

fn verify_pull_destination_size(
    destination: &Path,
    expected_size: u64,
) -> Result<(), HeadlessFileTransferError> {
    let metadata = std::fs::symlink_metadata(destination).map_err(|error| {
        HeadlessFileTransferError::Transfer(format!(
            "cannot inspect completed destination {}: {error}",
            destination.display()
        ))
    })?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(HeadlessFileTransferError::Transfer(format!(
            "completed destination is not a regular non-symlink file: {}",
            destination.display()
        )));
    }
    if metadata.len() != expected_size {
        return Err(HeadlessFileTransferError::Transfer(format!(
            "completed destination size mismatch: expected {expected_size}, got {}",
            metadata.len()
        )));
    }
    Ok(())
}

pub(crate) fn run(args: HeadlessFileTransferArgs) -> i32 {
    match run_inner(args) {
        Ok(status) => status,
        Err(error) => {
            eprintln!("{}", error_message(&error));
            error.status()
        }
    }
}

fn run_inner(args: HeadlessFileTransferArgs) -> Result<i32, HeadlessFileTransferError> {
    let source_snapshot = preflight(&args)?;

    let (session_event_tx, session_event_rx) = mpsc::channel();
    let (runtime_tx, runtime_rx) = mpsc::channel();
    let session: Session<HeadlessFileTransferHandler> = Session {
        password: String::new(),
        ui_handler: HeadlessFileTransferHandler::new(session_event_tx),
        server_keyboard_enabled: Arc::new(RwLock::new(true)),
        server_file_transfer_enabled: Arc::new(RwLock::new(true)),
        server_clipboard_enabled: Arc::new(RwLock::new(true)),
        reconnect_count: Arc::new(AtomicUsize::new(0)),
        ..Default::default()
    };
    {
        let mut config = session.lc.write().map_err(|_| {
            HeadlessFileTransferError::Internal("session config lock is poisoned".into())
        })?;
        config.initialize(
            args.peer_id.clone(),
            ConnType::FILE_TRANSFER,
            None,
            args.force_relay,
            None,
            None,
            None,
        );
    }

    let signal_tx = runtime_tx.clone();
    let _signal_forwarder = spawn_signal_forwarder(move |signal| {
        let _ = signal_tx.send(RuntimeEvent::Signal(signal));
    })
    .map_err(|error| {
        HeadlessFileTransferError::Internal(format!("failed to start signal handling: {error}"))
    })?;
    spawn_session_event_adapter(session_event_rx, runtime_tx.clone());
    spawn_network_thread(session.clone(), runtime_tx.clone())?;

    let expected_job_id = fs::get_next_job_id();
    let mut coordinator =
        TransferCoordinator::new(args.clone(), source_snapshot.clone(), expected_job_id);
    let mut backend =
        SystemTransferBackend::new(session, source_snapshot, args.destination.clone());
    Ok(run_event_channel(
        runtime_rx,
        &mut coordinator,
        &mut backend,
    ))
}

fn preflight(
    args: &HeadlessFileTransferArgs,
) -> Result<Option<FileSnapshot>, HeadlessFileTransferError> {
    match args.direction {
        TransferDirection::Push => Ok(Some(inspect_push_source(Path::new(&args.source))?)),
        TransferDirection::Pull => {
            inspect_pull_destination(Path::new(&args.destination), args.overwrite)?;
            Ok(None)
        }
    }
}

fn run_event_channel(
    runtime_rx: Receiver<RuntimeEvent>,
    coordinator: &mut TransferCoordinator,
    backend: &mut SystemTransferBackend,
) -> i32 {
    loop {
        let event = runtime_rx.recv().unwrap_or(RuntimeEvent::TransportClosed);
        if let Some(status) = coordinator.handle(event, backend) {
            return status;
        }
    }
}

fn spawn_session_event_adapter(
    session_event_rx: Receiver<HeadlessFileTransferEvent>,
    runtime_tx: Sender<RuntimeEvent>,
) {
    std::thread::spawn(move || {
        while let Ok(event) = session_event_rx.recv() {
            if runtime_tx.send(RuntimeEvent::Session(event)).is_err() {
                break;
            }
        }
    });
}

fn spawn_network_thread(
    session: Session<HeadlessFileTransferHandler>,
    runtime_tx: Sender<RuntimeEvent>,
) -> Result<(), HeadlessFileTransferError> {
    let round = session
        .connection_round_state
        .lock()
        .map_err(|_| {
            HeadlessFileTransferError::Internal("connection round lock is poisoned".into())
        })?
        .new_round();
    std::thread::spawn(move || {
        io_loop(session, round);
        let _ = runtime_tx.send(RuntimeEvent::TransportClosed);
    });
    Ok(())
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
    use std::{
        path::{Path, PathBuf},
        sync::{
            atomic::{AtomicU64, Ordering},
            Mutex,
        },
        time::{SystemTime, UNIX_EPOCH},
    };

    use hbb_common::fs::JobType;

    use super::*;

    static NEXT_TEST_DIRECTORY_ID: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let sequence = NEXT_TEST_DIRECTORY_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "rdh-hft-runtime-{}-{unique}-{sequence}",
                std::process::id()
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn transfer_args(
        direction: TransferDirection,
        source: String,
        destination: String,
        overwrite: bool,
    ) -> HeadlessFileTransferArgs {
        HeadlessFileTransferArgs {
            peer_id: "175116438".into(),
            direction,
            source,
            destination,
            force_relay: false,
            overwrite,
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    enum SessionCall {
        SendFiles {
            id: i32,
            job_type: i32,
            source: String,
            destination: String,
            file_num: i32,
            include_hidden: bool,
            is_remote: bool,
        },
        ConfirmOverwrite {
            id: i32,
            file_num: i32,
            overwrite: bool,
            remember: bool,
            is_upload: bool,
        },
        ReadRemoteDir {
            path: String,
            include_hidden: bool,
        },
        CancelJob(i32),
        Close,
        ContinueInsecure(bool),
    }

    #[derive(Default)]
    struct FakeSession {
        calls: Mutex<Vec<SessionCall>>,
    }

    impl FakeSession {
        fn calls(&self) -> Vec<SessionCall> {
            self.calls.lock().unwrap().clone()
        }

        fn record(&self, call: SessionCall) {
            self.calls.lock().unwrap().push(call);
        }
    }

    impl TransferSession for FakeSession {
        fn send_files(
            &self,
            id: i32,
            job_type: i32,
            source: String,
            destination: String,
            file_num: i32,
            include_hidden: bool,
            is_remote: bool,
        ) {
            self.record(SessionCall::SendFiles {
                id,
                job_type,
                source,
                destination,
                file_num,
                include_hidden,
                is_remote,
            });
        }

        fn set_confirm_override_file(
            &self,
            id: i32,
            file_num: i32,
            overwrite: bool,
            remember: bool,
            is_upload: bool,
        ) {
            self.record(SessionCall::ConfirmOverwrite {
                id,
                file_num,
                overwrite,
                remember,
                is_upload,
            });
        }

        fn read_remote_dir(&self, path: String, include_hidden: bool) {
            self.record(SessionCall::ReadRemoteDir {
                path,
                include_hidden,
            });
        }

        fn cancel_job(&self, id: i32) {
            self.record(SessionCall::CancelJob(id));
        }

        fn close(&self) {
            self.record(SessionCall::Close);
        }

        fn continue_insecure_connection(&self, continue_insecure: bool) {
            self.record(SessionCall::ContinueInsecure(continue_insecure));
        }
    }

    #[test]
    fn maps_every_transfer_action_to_the_exact_native_session_call() {
        let session = FakeSession::default();

        map_action(
            &session,
            TransferAction::StartJob {
                id: 7,
                source: "/tmp/source.bin".into(),
                destination: r"C:\Users\82520\target.bin".into(),
                is_remote: false,
            },
        );
        map_action(
            &session,
            TransferAction::ConfirmOverwrite {
                id: 7,
                file_num: 0,
                overwrite: true,
                is_upload: true,
            },
        );
        map_action(
            &session,
            TransferAction::ReadRemoteDir {
                path: r"C:\Users\82520".into(),
                include_hidden: true,
            },
        );
        map_action(&session, TransferAction::CancelJob { id: 7 });
        map_action(&session, TransferAction::CloseTransport);
        map_action(&session, TransferAction::RejectInsecureConnection);

        assert_eq!(
            session.calls(),
            vec![
                SessionCall::SendFiles {
                    id: 7,
                    job_type: JobType::Generic as i32,
                    source: "/tmp/source.bin".into(),
                    destination: r"C:\Users\82520\target.bin".into(),
                    file_num: 0,
                    include_hidden: false,
                    is_remote: false,
                },
                SessionCall::ConfirmOverwrite {
                    id: 7,
                    file_num: 0,
                    overwrite: true,
                    remember: false,
                    is_upload: true,
                },
                SessionCall::ReadRemoteDir {
                    path: r"C:\Users\82520".into(),
                    include_hidden: true,
                },
                SessionCall::CancelJob(7),
                SessionCall::Close,
                SessionCall::ContinueInsecure(false),
            ]
        );
    }

    #[test]
    fn preflight_rejects_invalid_push_and_existing_pull_before_networking() {
        let temp = TestDirectory::new();
        let push_error = preflight(&transfer_args(
            TransferDirection::Push,
            temp.path().to_string_lossy().into_owned(),
            "remote.bin".into(),
            false,
        ))
        .unwrap_err();
        assert_eq!(push_error.status(), 3);

        let destination = temp.path().join("existing.bin");
        std::fs::write(&destination, b"old").unwrap();
        let pull_error = preflight(&transfer_args(
            TransferDirection::Pull,
            "remote.bin".into(),
            destination.to_string_lossy().into_owned(),
            false,
        ))
        .unwrap_err();
        assert_eq!(pull_error.status(), 7);
    }

    #[test]
    fn pull_postflight_requires_exact_regular_file_size() {
        let temp = TestDirectory::new();
        let destination = temp.path().join("pulled.bin");
        std::fs::write(&destination, b"abcd").unwrap();

        verify_pull_destination_size(&destination, 4).unwrap();
        assert_eq!(
            verify_pull_destination_size(&destination, 5)
                .unwrap_err()
                .status(),
            6
        );

        let link = temp.path().join("pulled-link.bin");
        std::os::unix::fs::symlink(&destination, &link).unwrap();
        assert_eq!(
            verify_pull_destination_size(&link, 4).unwrap_err().status(),
            6
        );
    }
}
