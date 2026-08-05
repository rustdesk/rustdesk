use super::{
    handler::{AuthPrompt, HeadlessEvent, HeadlessTerminalHandler},
    tty::{
        prompt_confirmation, prompt_line, prompt_secret, spawn_signal_forwarder, split_input,
        InputChunk, LocalTtyGuard, SignalEvent, SignalForwarder, SystemTtyBackend, TtyBackend,
        TtySize,
    },
    HeadlessTerminalArgs,
};
use crate::{
    client::LoginConfigHandler,
    ui_session_interface::{io_loop, Session},
};
use hbb_common::rendezvous_proto::ConnType;
use std::{
    fmt,
    io::{self, Read, Write},
    sync::{
        atomic::AtomicUsize,
        mpsc::{self, Receiver, Sender},
        Arc, RwLock,
    },
};

const TERMINAL_ID: i32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Authenticating,
    Opening,
    Active,
    Closing,
    Closed,
    Failed,
}

struct RuntimeState {
    phase: Phase,
    persistent: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SessionAction {
    OpenTerminal {
        terminal_id: i32,
        rows: u32,
        cols: u32,
    },
    SendInput {
        terminal_id: i32,
        data: Vec<u8>,
    },
    ResizeTerminal {
        terminal_id: i32,
        rows: u32,
        cols: u32,
    },
    CloseTerminal {
        terminal_id: i32,
    },
    CloseTransport,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum HeadlessTerminalError {
    // Usage is classified by the desktop-safe parent module before macOS runtime entry.
    #[allow(dead_code)]
    Usage(String),
    Tty(String),
    Authentication(String),
    Connection(String),
    Protocol(String),
}

impl HeadlessTerminalError {
    fn status(&self) -> i32 {
        match self {
            Self::Usage(_) => 2,
            Self::Tty(_) => 3,
            Self::Authentication(_) => 4,
            Self::Connection(_) | Self::Protocol(_) => 5,
        }
    }
}

impl fmt::Display for HeadlessTerminalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (kind, message) = match self {
            Self::Usage(message) => ("usage error", message),
            Self::Tty(message) => ("local TTY error", message),
            Self::Authentication(message) => ("authentication error", message),
            Self::Connection(message) => ("connection error", message),
            Self::Protocol(message) => ("protocol error", message),
        };
        write!(formatter, "RDH headless terminal {kind}: {message}")
    }
}

impl RuntimeState {
    fn new(persistent: bool) -> Self {
        Self {
            phase: Phase::Authenticating,
            persistent,
        }
    }

    fn connected(&mut self) -> Result<(), HeadlessTerminalError> {
        if self.phase != Phase::Authenticating {
            return self.protocol_failure("unexpected connected event");
        }
        self.phase = Phase::Opening;
        Ok(())
    }

    fn opened(&mut self, terminal_id: i32, success: bool) -> Result<(), HeadlessTerminalError> {
        if self.phase != Phase::Opening {
            return self.protocol_failure("unexpected terminal opened event");
        }
        if terminal_id != TERMINAL_ID {
            return self.protocol_failure("terminal opened event used an unexpected terminal ID");
        }
        if !success {
            self.phase = Phase::Failed;
            return Err(HeadlessTerminalError::Connection(
                "remote terminal did not open".to_owned(),
            ));
        }
        self.phase = Phase::Active;
        Ok(())
    }

    fn output(&mut self, terminal_id: i32) -> Result<(), HeadlessTerminalError> {
        if self.phase != Phase::Active {
            return self.protocol_failure("terminal output arrived before the terminal opened");
        }
        if terminal_id != TERMINAL_ID {
            return self.protocol_failure("terminal output used an unexpected terminal ID");
        }
        Ok(())
    }

    fn begin_close(&mut self) -> Result<(), HeadlessTerminalError> {
        if self.phase != Phase::Active {
            return self.protocol_failure("local close requested outside an active terminal");
        }
        self.phase = Phase::Closing;
        Ok(())
    }

    fn closed(&mut self, terminal_id: i32) -> Result<(), HeadlessTerminalError> {
        if !matches!(self.phase, Phase::Active | Phase::Closing) {
            return self.protocol_failure("unexpected terminal closed event");
        }
        if terminal_id != TERMINAL_ID {
            return self.protocol_failure("terminal closed event used an unexpected terminal ID");
        }
        self.phase = Phase::Closed;
        Ok(())
    }

    fn fail(&mut self) -> Result<(), HeadlessTerminalError> {
        if matches!(self.phase, Phase::Closed | Phase::Failed) {
            return Err(HeadlessTerminalError::Protocol(
                "duplicate terminal failure event".to_owned(),
            ));
        }
        self.phase = Phase::Failed;
        Ok(())
    }

    fn protocol_failure<T>(&mut self, message: &str) -> Result<T, HeadlessTerminalError> {
        if self.phase != Phase::Closed {
            self.phase = Phase::Failed;
        }
        Err(HeadlessTerminalError::Protocol(message.to_owned()))
    }
}

fn detach_actions(persistent: bool) -> Vec<SessionAction> {
    let mut actions = Vec::with_capacity(if persistent { 1 } else { 2 });
    if !persistent {
        actions.push(SessionAction::CloseTerminal {
            terminal_id: TERMINAL_ID,
        });
    }
    actions.push(SessionAction::CloseTransport);
    actions
}

fn local_exit_status(remote_status: i32) -> i32 {
    if (0..=125).contains(&remote_status) {
        remote_status
    } else {
        1
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RuntimeEvent {
    Remote(HeadlessEvent),
    Input(InputChunk),
    Signal(SignalEvent),
    StdinClosed(Option<String>),
    TransportClosed,
}

trait RuntimeBackend {
    fn tty_size(&mut self) -> Result<TtySize, HeadlessTerminalError>;
    fn enter_raw(&mut self) -> Result<(), HeadlessTerminalError>;
    fn restore_tty(&mut self) -> Result<(), HeadlessTerminalError>;
    fn start_active_io(&mut self) -> Result<(), HeadlessTerminalError>;
    fn session_action(&mut self, action: SessionAction);
    fn write_stdout(&mut self, data: &[u8]) -> Result<(), HeadlessTerminalError>;
    fn write_stderr(&mut self, message: &str);
    fn prompt_secret(&mut self) -> Result<Option<String>, HeadlessTerminalError>;
    fn prompt_confirmation(&mut self) -> Result<Option<bool>, HeadlessTerminalError>;
    fn prompt_line(&mut self) -> Result<Option<String>, HeadlessTerminalError>;
    fn login(&mut self, password: String, remember: bool);
    fn send_two_factor(&mut self, code: String);
    fn reject_insecure_connection(&mut self);
}

struct RuntimeCoordinator {
    state: RuntimeState,
    password_submission_pending: bool,
    last_size: Option<TtySize>,
    local_result: i32,
}

impl RuntimeCoordinator {
    fn new(persistent: bool) -> Self {
        Self {
            state: RuntimeState::new(persistent),
            password_submission_pending: false,
            last_size: None,
            local_result: 0,
        }
    }

    fn handle<B: RuntimeBackend>(&mut self, event: RuntimeEvent, backend: &mut B) -> Option<i32> {
        match self.handle_event(event, backend) {
            Ok(status) => status,
            Err(error) => Some(self.finish_error(backend, error)),
        }
    }

    fn handle_event<B: RuntimeBackend>(
        &mut self,
        event: RuntimeEvent,
        backend: &mut B,
    ) -> Result<Option<i32>, HeadlessTerminalError> {
        match event {
            RuntimeEvent::Remote(event) => self.handle_remote(event, backend),
            RuntimeEvent::Input(InputChunk::Data(data)) => {
                if self.state.phase != Phase::Active {
                    return self
                        .state
                        .protocol_failure("local terminal input arrived outside the active phase");
                }
                backend.session_action(SessionAction::SendInput {
                    terminal_id: TERMINAL_ID,
                    data,
                });
                Ok(None)
            }
            RuntimeEvent::Input(InputChunk::Detach)
            | RuntimeEvent::Signal(SignalEvent::Shutdown)
            | RuntimeEvent::StdinClosed(None) => self.begin_local_close(backend),
            RuntimeEvent::Signal(SignalEvent::Resize) => self.resize(backend),
            RuntimeEvent::StdinClosed(Some(error)) => {
                backend.session_action(SessionAction::CloseTransport);
                Err(HeadlessTerminalError::Tty(format!(
                    "failed to read stdin: {error}"
                )))
            }
            RuntimeEvent::TransportClosed => {
                if matches!(self.state.phase, Phase::Closing | Phase::Closed) {
                    Ok(Some(self.local_result))
                } else {
                    Err(HeadlessTerminalError::Connection(
                        "transport closed before the terminal lifecycle completed".to_owned(),
                    ))
                }
            }
        }
    }

    fn handle_remote<B: RuntimeBackend>(
        &mut self,
        event: HeadlessEvent,
        backend: &mut B,
    ) -> Result<Option<i32>, HeadlessTerminalError> {
        match event {
            HeadlessEvent::Connected => {
                self.state.connected()?;
                self.password_submission_pending = false;
                let size = backend.tty_size()?;
                self.last_size = Some(size);
                backend.session_action(SessionAction::OpenTerminal {
                    terminal_id: TERMINAL_ID,
                    rows: size.rows.into(),
                    cols: size.cols.into(),
                });
                Ok(None)
            }
            HeadlessEvent::Auth(prompt) => self.handle_auth(prompt, backend),
            HeadlessEvent::Opened {
                terminal_id,
                success,
                message,
                pid,
                service_id,
                persistent_sessions,
                replay_terminal_output,
            } => {
                if let Err(error) = self.state.opened(terminal_id, success) {
                    return if success || message.is_empty() {
                        Err(error)
                    } else {
                        Err(HeadlessTerminalError::Connection(format!(
                            "remote terminal did not open: {message}"
                        )))
                    };
                }

                let escaped_service_id: String =
                    service_id.chars().flat_map(char::escape_default).collect();
                backend.write_stderr(&format!(
                    "RDH headless terminal opened: pid={pid} service_id={escaped_service_id} replay_terminal_output={replay_terminal_output}"
                ));
                let _ = persistent_sessions;
                backend.enter_raw()?;
                backend.start_active_io()?;
                Ok(None)
            }
            HeadlessEvent::Output { terminal_id, data } => {
                self.state.output(terminal_id)?;
                backend.write_stdout(&data)?;
                Ok(None)
            }
            HeadlessEvent::Closed {
                terminal_id,
                exit_code,
            } => {
                self.state.closed(terminal_id)?;
                self.local_result = local_exit_status(exit_code);
                backend.restore_tty()?;
                backend.session_action(SessionAction::CloseTransport);
                Ok(None)
            }
            HeadlessEvent::Failed {
                terminal_id,
                message,
            } => {
                if terminal_id != TERMINAL_ID {
                    return self
                        .state
                        .protocol_failure("terminal failure used an unexpected terminal ID");
                }
                self.state.fail()?;
                Err(HeadlessTerminalError::Connection(format!(
                    "remote terminal failed: {message}"
                )))
            }
            HeadlessEvent::ConnectionFailed(message) => {
                self.state.fail()?;
                Err(HeadlessTerminalError::Connection(message))
            }
        }
    }

    fn handle_auth<B: RuntimeBackend>(
        &mut self,
        prompt: AuthPrompt,
        backend: &mut B,
    ) -> Result<Option<i32>, HeadlessTerminalError> {
        if self.state.phase != Phase::Authenticating {
            return self
                .state
                .protocol_failure("authentication prompt arrived after authentication completed");
        }

        match prompt {
            AuthPrompt::Password { retry } => {
                if retry {
                    self.password_submission_pending = false;
                } else if self.password_submission_pending {
                    return Ok(None);
                }

                let Some(password) = backend.prompt_secret()? else {
                    backend.session_action(SessionAction::CloseTransport);
                    return Err(HeadlessTerminalError::Authentication(
                        "password prompt reached EOF".to_owned(),
                    ));
                };
                let Some(remember) = backend.prompt_confirmation()? else {
                    backend.session_action(SessionAction::CloseTransport);
                    return Err(HeadlessTerminalError::Authentication(
                        "confirmation prompt reached EOF".to_owned(),
                    ));
                };
                backend.login(password, remember);
                self.password_submission_pending = true;
            }
            AuthPrompt::TwoFactor => {
                let Some(code) = backend.prompt_line()? else {
                    backend.session_action(SessionAction::CloseTransport);
                    return Err(HeadlessTerminalError::Authentication(
                        "2FA prompt reached EOF".to_owned(),
                    ));
                };
                backend.send_two_factor(code);
            }
            AuthPrompt::InsecureConnection => backend.reject_insecure_connection(),
        }
        Ok(None)
    }

    fn begin_local_close<B: RuntimeBackend>(
        &mut self,
        backend: &mut B,
    ) -> Result<Option<i32>, HeadlessTerminalError> {
        if matches!(self.state.phase, Phase::Closing | Phase::Closed) {
            return Ok(None);
        }
        backend.restore_tty()?;
        self.state.begin_close()?;
        for action in detach_actions(self.state.persistent) {
            backend.session_action(action);
        }
        Ok(None)
    }

    fn resize<B: RuntimeBackend>(
        &mut self,
        backend: &mut B,
    ) -> Result<Option<i32>, HeadlessTerminalError> {
        if self.state.phase != Phase::Active {
            return Ok(None);
        }
        let size = backend.tty_size()?;
        if self.last_size != Some(size) {
            self.last_size = Some(size);
            backend.session_action(SessionAction::ResizeTerminal {
                terminal_id: TERMINAL_ID,
                rows: size.rows.into(),
                cols: size.cols.into(),
            });
        }
        Ok(None)
    }

    fn finish_error<B: RuntimeBackend>(
        &mut self,
        backend: &mut B,
        error: HeadlessTerminalError,
    ) -> i32 {
        let _ = self.state.fail();
        if let Err(restore_error) = backend.restore_tty() {
            backend.write_stderr(&restore_error.to_string());
            return restore_error.status();
        }
        backend.write_stderr(&error.to_string());
        error.status()
    }
}

fn validate_tty<B: TtyBackend>(backend: &B) -> Result<TtySize, HeadlessTerminalError> {
    if !backend.stdin_is_tty() {
        return Err(HeadlessTerminalError::Tty(
            "stdin is not an interactive TTY".to_owned(),
        ));
    }
    if !backend.stdout_is_tty() {
        return Err(HeadlessTerminalError::Tty(
            "stdout is not an interactive TTY".to_owned(),
        ));
    }
    let size = backend
        .size()
        .map_err(|error| HeadlessTerminalError::Tty(error.to_string()))?;
    if size.rows == 0 || size.cols == 0 {
        return Err(HeadlessTerminalError::Tty(
            "local TTY reported zero rows or columns".to_owned(),
        ));
    }
    Ok(size)
}

fn apply_cli_persistence(config: &mut LoginConfigHandler, persistent: bool) {
    config.get_config().terminal_persistent.v = persistent;
}

struct SystemRuntimeBackend {
    session: Session<HeadlessTerminalHandler>,
    tty_backend: Arc<SystemTtyBackend>,
    runtime_tx: Sender<RuntimeEvent>,
    tty_guard: Option<LocalTtyGuard<SystemTtyBackend>>,
    signal_forwarder: Option<SignalForwarder>,
    input_started: bool,
}

impl SystemRuntimeBackend {
    fn new(
        session: Session<HeadlessTerminalHandler>,
        tty_backend: Arc<SystemTtyBackend>,
        runtime_tx: Sender<RuntimeEvent>,
    ) -> Self {
        Self {
            session,
            tty_backend,
            runtime_tx,
            tty_guard: None,
            signal_forwarder: None,
            input_started: false,
        }
    }
}

impl RuntimeBackend for SystemRuntimeBackend {
    fn tty_size(&mut self) -> Result<TtySize, HeadlessTerminalError> {
        self.tty_backend
            .size()
            .map_err(|error| HeadlessTerminalError::Tty(error.to_string()))
    }

    fn enter_raw(&mut self) -> Result<(), HeadlessTerminalError> {
        if self.tty_guard.is_some() {
            return Err(HeadlessTerminalError::Protocol(
                "local TTY raw mode was entered more than once".to_owned(),
            ));
        }
        let guard = LocalTtyGuard::enter(self.tty_backend.clone())
            .map_err(|error| HeadlessTerminalError::Tty(error.to_string()))?;
        self.tty_guard = Some(guard);
        Ok(())
    }

    fn restore_tty(&mut self) -> Result<(), HeadlessTerminalError> {
        if let Some(guard) = self.tty_guard.as_mut() {
            guard
                .restore()
                .map_err(|error| HeadlessTerminalError::Tty(error.to_string()))?;
        }
        self.tty_guard.take();
        Ok(())
    }

    fn start_active_io(&mut self) -> Result<(), HeadlessTerminalError> {
        if self.input_started || self.signal_forwarder.is_some() {
            return Err(HeadlessTerminalError::Protocol(
                "local terminal input was started more than once".to_owned(),
            ));
        }

        let signal_tx = self.runtime_tx.clone();
        self.signal_forwarder = Some(
            spawn_signal_forwarder(move |signal| {
                let _ = signal_tx.send(RuntimeEvent::Signal(signal));
            })
            .map_err(|error| HeadlessTerminalError::Tty(error.to_string()))?,
        );

        self.input_started = true;
        let input_tx = self.runtime_tx.clone();
        std::thread::spawn(move || read_stdin(input_tx));
        Ok(())
    }

    fn session_action(&mut self, action: SessionAction) {
        match action {
            SessionAction::OpenTerminal {
                terminal_id,
                rows,
                cols,
            } => self.session.open_terminal(terminal_id, rows, cols),
            SessionAction::SendInput { terminal_id, data } => {
                self.session.send_terminal_input_bytes(terminal_id, data)
            }
            SessionAction::ResizeTerminal {
                terminal_id,
                rows,
                cols,
            } => self.session.resize_terminal(terminal_id, rows, cols),
            SessionAction::CloseTerminal { terminal_id } => {
                self.session.close_terminal(terminal_id)
            }
            SessionAction::CloseTransport => self.session.close(),
        }
    }

    fn write_stdout(&mut self, data: &[u8]) -> Result<(), HeadlessTerminalError> {
        let mut stdout = io::stdout().lock();
        stdout
            .write_all(data)
            .and_then(|()| stdout.flush())
            .map_err(|error| HeadlessTerminalError::Tty(format!("failed to write stdout: {error}")))
    }

    fn write_stderr(&mut self, message: &str) {
        let mut stderr = io::stderr().lock();
        let _ = writeln!(stderr, "{message}").and_then(|()| stderr.flush());
    }

    fn prompt_secret(&mut self) -> Result<Option<String>, HeadlessTerminalError> {
        prompt_secret("Password: ").map_err(|error| HeadlessTerminalError::Tty(error.to_string()))
    }

    fn prompt_confirmation(&mut self) -> Result<Option<bool>, HeadlessTerminalError> {
        prompt_confirmation("Save password for this peer? [y/N] ")
            .map_err(|error| HeadlessTerminalError::Tty(error.to_string()))
    }

    fn prompt_line(&mut self) -> Result<Option<String>, HeadlessTerminalError> {
        prompt_line("2FA code: ").map_err(|error| HeadlessTerminalError::Tty(error.to_string()))
    }

    fn login(&mut self, password: String, remember: bool) {
        self.session
            .login(String::new(), String::new(), password, remember);
    }

    fn send_two_factor(&mut self, code: String) {
        self.session.send2fa(code, false);
    }

    fn reject_insecure_connection(&mut self) {
        self.session.continue_insecure_connection(false);
    }
}

fn read_stdin(runtime_tx: Sender<RuntimeEvent>) {
    let stdin = io::stdin();
    let mut stdin = stdin.lock();
    let mut buffer = [0_u8; 8192];
    loop {
        match stdin.read(&mut buffer) {
            Ok(0) => {
                let _ = runtime_tx.send(RuntimeEvent::StdinClosed(None));
                return;
            }
            Ok(read) => {
                for chunk in split_input(&buffer[..read]) {
                    let detached = chunk == InputChunk::Detach;
                    if runtime_tx.send(RuntimeEvent::Input(chunk)).is_err() || detached {
                        return;
                    }
                }
            }
            Err(error) => {
                let _ = runtime_tx.send(RuntimeEvent::StdinClosed(Some(error.to_string())));
                return;
            }
        }
    }
}

pub(crate) fn run(args: HeadlessTerminalArgs) -> i32 {
    match run_inner(args) {
        Ok(status) => status,
        Err(error) => {
            eprintln!("{error}");
            error.status()
        }
    }
}

fn run_inner(args: HeadlessTerminalArgs) -> Result<i32, HeadlessTerminalError> {
    let tty_backend = Arc::new(SystemTtyBackend);
    validate_tty(tty_backend.as_ref())?;

    let (event_tx, event_rx) = mpsc::channel();
    let (runtime_tx, runtime_rx) = mpsc::channel();
    let session: Session<HeadlessTerminalHandler> = Session {
        password: String::new(),
        ui_handler: HeadlessTerminalHandler::new(event_tx),
        server_keyboard_enabled: Arc::new(RwLock::new(true)),
        server_file_transfer_enabled: Arc::new(RwLock::new(true)),
        server_clipboard_enabled: Arc::new(RwLock::new(true)),
        reconnect_count: Arc::new(AtomicUsize::new(0)),
        ..Default::default()
    };
    {
        let mut config = session.lc.write().unwrap();
        config.initialize(
            args.peer_id,
            ConnType::TERMINAL,
            None,
            args.force_relay,
            None,
            None,
            None,
        );
        apply_cli_persistence(&mut config, args.persistent);
    }

    spawn_remote_event_adapter(event_rx, runtime_tx.clone());
    spawn_network_thread(session.clone(), runtime_tx.clone());

    let mut coordinator = RuntimeCoordinator::new(args.persistent);
    let mut backend = SystemRuntimeBackend::new(session, tty_backend, runtime_tx);
    Ok(run_event_channel(
        runtime_rx,
        &mut coordinator,
        &mut backend,
    ))
}

fn run_event_channel<B: RuntimeBackend>(
    runtime_rx: Receiver<RuntimeEvent>,
    coordinator: &mut RuntimeCoordinator,
    backend: &mut B,
) -> i32 {
    loop {
        let event = runtime_rx.recv().unwrap_or(RuntimeEvent::TransportClosed);
        if let Some(status) = coordinator.handle(event, backend) {
            return status;
        }
    }
}

fn spawn_remote_event_adapter(event_rx: Receiver<HeadlessEvent>, runtime_tx: Sender<RuntimeEvent>) {
    std::thread::spawn(move || {
        while let Ok(event) = event_rx.recv() {
            if runtime_tx.send(RuntimeEvent::Remote(event)).is_err() {
                break;
            }
        }
    });
}

fn spawn_network_thread(
    session: Session<HeadlessTerminalHandler>,
    runtime_tx: Sender<RuntimeEvent>,
) {
    let round = session.connection_round_state.lock().unwrap().new_round();
    std::thread::spawn(move || {
        io_loop(session, round);
        let _ = runtime_tx.send(RuntimeEvent::TransportClosed);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::headless_terminal::{
        handler::{AuthPrompt, HeadlessEvent},
        tty::{InputChunk, SignalEvent, TtyBackend, TtySize},
    };
    use std::{
        collections::VecDeque,
        io,
        sync::atomic::{AtomicUsize, Ordering},
    };

    #[derive(Clone, Debug, Eq, PartialEq)]
    enum Observation {
        Session(SessionAction),
        EnterRaw,
        RestoreTty,
        Stdout(Vec<u8>),
        Stderr(String),
        PromptSecret,
        PromptRemember,
        Login { password: String, remember: bool },
        PromptLine,
        SendTwoFactor(String, bool),
        RejectInsecureConnection,
        Exit(i32),
    }

    struct FakeRuntimeBackend {
        sizes: VecDeque<Result<TtySize, HeadlessTerminalError>>,
        secrets: VecDeque<Option<String>>,
        confirmations: VecDeque<Option<bool>>,
        lines: VecDeque<Option<String>>,
        observations: Vec<Observation>,
    }

    impl FakeRuntimeBackend {
        fn new() -> Self {
            Self {
                sizes: VecDeque::from([Ok(TtySize { rows: 24, cols: 80 })]),
                secrets: VecDeque::new(),
                confirmations: VecDeque::new(),
                lines: VecDeque::new(),
                observations: Vec::new(),
            }
        }
    }

    impl RuntimeBackend for FakeRuntimeBackend {
        fn tty_size(&mut self) -> Result<TtySize, HeadlessTerminalError> {
            self.sizes
                .pop_front()
                .unwrap_or(Ok(TtySize { rows: 24, cols: 80 }))
        }

        fn enter_raw(&mut self) -> Result<(), HeadlessTerminalError> {
            self.observations.push(Observation::EnterRaw);
            Ok(())
        }

        fn restore_tty(&mut self) -> Result<(), HeadlessTerminalError> {
            self.observations.push(Observation::RestoreTty);
            Ok(())
        }

        fn start_active_io(&mut self) -> Result<(), HeadlessTerminalError> {
            Ok(())
        }

        fn session_action(&mut self, action: SessionAction) {
            self.observations.push(Observation::Session(action));
        }

        fn write_stdout(&mut self, data: &[u8]) -> Result<(), HeadlessTerminalError> {
            self.observations.push(Observation::Stdout(data.to_vec()));
            Ok(())
        }

        fn write_stderr(&mut self, message: &str) {
            self.observations
                .push(Observation::Stderr(message.to_owned()));
        }

        fn prompt_secret(&mut self) -> Result<Option<String>, HeadlessTerminalError> {
            self.observations.push(Observation::PromptSecret);
            Ok(self.secrets.pop_front().flatten())
        }

        fn prompt_confirmation(&mut self) -> Result<Option<bool>, HeadlessTerminalError> {
            self.observations.push(Observation::PromptRemember);
            Ok(self.confirmations.pop_front().unwrap_or(Some(false)))
        }

        fn prompt_line(&mut self) -> Result<Option<String>, HeadlessTerminalError> {
            self.observations.push(Observation::PromptLine);
            Ok(self.lines.pop_front().flatten())
        }

        fn login(&mut self, password: String, remember: bool) {
            self.observations
                .push(Observation::Login { password, remember });
        }

        fn send_two_factor(&mut self, code: String) {
            self.observations
                .push(Observation::SendTwoFactor(code, false));
        }

        fn reject_insecure_connection(&mut self) {
            self.observations
                .push(Observation::RejectInsecureConnection);
        }
    }

    fn connected_event() -> RuntimeEvent {
        RuntimeEvent::Remote(HeadlessEvent::Connected)
    }

    fn opened_event() -> RuntimeEvent {
        RuntimeEvent::Remote(HeadlessEvent::Opened {
            terminal_id: TERMINAL_ID,
            success: true,
            message: String::new(),
            pid: 4815,
            service_id: "service-alpha".to_owned(),
            persistent_sessions: vec![3, 8],
            replay_terminal_output: true,
        })
    }

    fn active_runtime(persistent: bool) -> (RuntimeCoordinator, FakeRuntimeBackend) {
        let mut coordinator = RuntimeCoordinator::new(persistent);
        let mut backend = FakeRuntimeBackend::new();
        assert_eq!(coordinator.handle(connected_event(), &mut backend), None);
        assert_eq!(coordinator.handle(opened_event(), &mut backend), None);
        backend.observations.clear();
        (coordinator, backend)
    }

    fn handle_and_record_exit(
        coordinator: &mut RuntimeCoordinator,
        backend: &mut FakeRuntimeBackend,
        event: RuntimeEvent,
    ) -> Option<i32> {
        let exit = coordinator.handle(event, backend);
        if let Some(status) = exit {
            backend.observations.push(Observation::Exit(status));
        }
        exit
    }

    struct FakeTtyBackend {
        stdin_is_tty: bool,
        stdout_is_tty: bool,
        size: Option<TtySize>,
        size_count: AtomicUsize,
    }

    impl TtyBackend for FakeTtyBackend {
        type Snapshot = ();

        fn stdin_is_tty(&self) -> bool {
            self.stdin_is_tty
        }

        fn stdout_is_tty(&self) -> bool {
            self.stdout_is_tty
        }

        fn capture(&self) -> io::Result<Self::Snapshot> {
            Ok(())
        }

        fn enter_raw(&self, _snapshot: &Self::Snapshot) -> io::Result<()> {
            Ok(())
        }

        fn restore(&self, _snapshot: &Self::Snapshot) -> io::Result<()> {
            Ok(())
        }

        fn size(&self) -> io::Result<TtySize> {
            self.size_count.fetch_add(1, Ordering::SeqCst);
            self.size
                .ok_or_else(|| io::Error::other("injected size syscall failure"))
        }
    }

    fn fake_tty(stdin_is_tty: bool, stdout_is_tty: bool, size: Option<TtySize>) -> FakeTtyBackend {
        FakeTtyBackend {
            stdin_is_tty,
            stdout_is_tty,
            size,
            size_count: AtomicUsize::new(0),
        }
    }

    #[test]
    fn successful_flow_is_authenticating_opening_active_closed() {
        let mut state = RuntimeState::new(false);
        assert_eq!(state.phase, Phase::Authenticating);
        state.connected().unwrap();
        assert_eq!(state.phase, Phase::Opening);
        state.opened(TERMINAL_ID, true).unwrap();
        assert_eq!(state.phase, Phase::Active);
        state.closed(TERMINAL_ID).unwrap();
        assert_eq!(state.phase, Phase::Closed);
    }

    #[test]
    fn data_before_opened_is_a_protocol_failure() {
        let mut state = RuntimeState::new(false);
        assert!(state.output(TERMINAL_ID).is_err());
        assert_eq!(state.phase, Phase::Failed);
    }

    #[test]
    fn wrong_terminal_id_is_a_protocol_failure() {
        let mut state = RuntimeState::new(false);
        state.connected().unwrap();
        assert!(state.opened(2, true).is_err());
        assert_eq!(state.phase, Phase::Failed);
    }

    #[test]
    fn duplicate_opened_or_closed_is_rejected() {
        let mut state = RuntimeState::new(false);
        state.connected().unwrap();
        state.opened(TERMINAL_ID, true).unwrap();
        assert!(state.opened(TERMINAL_ID, true).is_err());

        let mut state = RuntimeState::new(false);
        state.connected().unwrap();
        state.opened(TERMINAL_ID, true).unwrap();
        state.closed(TERMINAL_ID).unwrap();
        assert!(state.closed(TERMINAL_ID).is_err());
    }

    #[test]
    fn detach_closes_terminal_only_when_not_persistent() {
        assert_eq!(
            detach_actions(false),
            vec![
                SessionAction::CloseTerminal {
                    terminal_id: TERMINAL_ID,
                },
                SessionAction::CloseTransport,
            ]
        );
        assert_eq!(detach_actions(true), vec![SessionAction::CloseTransport]);
    }

    #[test]
    fn remote_exit_status_maps_to_local_contract() {
        assert_eq!(local_exit_status(0), 0);
        assert_eq!(local_exit_status(1), 1);
        assert_eq!(local_exit_status(125), 125);
        assert_eq!(local_exit_status(126), 1);
        assert_eq!(local_exit_status(-1), 1);
    }

    #[test]
    fn preflight_rejects_non_tty_stdin_before_reading_size() {
        let backend = fake_tty(false, true, Some(TtySize { rows: 24, cols: 80 }));
        assert_eq!(validate_tty(&backend).unwrap_err().status(), 3);
        assert_eq!(backend.size_count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn preflight_rejects_non_tty_stdout_before_reading_size() {
        let backend = fake_tty(true, false, Some(TtySize { rows: 24, cols: 80 }));
        assert_eq!(validate_tty(&backend).unwrap_err().status(), 3);
        assert_eq!(backend.size_count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn preflight_rejects_zero_rows_or_columns() {
        for size in [TtySize { rows: 0, cols: 80 }, TtySize { rows: 24, cols: 0 }] {
            let backend = fake_tty(true, true, Some(size));
            assert_eq!(validate_tty(&backend).unwrap_err().status(), 3);
        }
    }

    #[test]
    fn preflight_maps_size_syscall_failure_to_tty_status() {
        let backend = fake_tty(true, true, None);
        assert_eq!(validate_tty(&backend).unwrap_err().status(), 3);
    }

    #[test]
    fn connected_opens_terminal_one_with_current_size() {
        let mut coordinator = RuntimeCoordinator::new(false);
        let mut backend = FakeRuntimeBackend::new();
        assert_eq!(coordinator.handle(connected_event(), &mut backend), None);
        assert_eq!(
            backend.observations,
            vec![Observation::Session(SessionAction::OpenTerminal {
                terminal_id: TERMINAL_ID,
                rows: 24,
                cols: 80,
            })]
        );
    }

    #[test]
    fn channel_driven_flow_preserves_event_order_and_exit_status() {
        let (event_tx, event_rx) = mpsc::channel();
        event_tx.send(connected_event()).unwrap();
        event_tx.send(opened_event()).unwrap();
        event_tx
            .send(RuntimeEvent::Remote(HeadlessEvent::Output {
                terminal_id: TERMINAL_ID,
                data: vec![0x1b, b'[', b'2', b'J', 0xff],
            }))
            .unwrap();
        event_tx
            .send(RuntimeEvent::Remote(HeadlessEvent::Closed {
                terminal_id: TERMINAL_ID,
                exit_code: 7,
            }))
            .unwrap();
        event_tx.send(RuntimeEvent::TransportClosed).unwrap();
        drop(event_tx);

        let mut coordinator = RuntimeCoordinator::new(false);
        let mut backend = FakeRuntimeBackend::new();
        assert_eq!(
            run_event_channel(event_rx, &mut coordinator, &mut backend),
            7
        );
        assert!(matches!(
            backend.observations.as_slice(),
            [
                Observation::Session(SessionAction::OpenTerminal { .. }),
                Observation::Stderr(_),
                Observation::EnterRaw,
                Observation::Stdout(data),
                Observation::RestoreTty,
                Observation::Session(SessionAction::CloseTransport),
            ] if data == &[0x1b, b'[', b'2', b'J', 0xff]
        ));
    }

    #[test]
    fn opened_reports_metadata_and_enters_raw_mode() {
        let mut coordinator = RuntimeCoordinator::new(false);
        let mut backend = FakeRuntimeBackend::new();
        coordinator.handle(connected_event(), &mut backend);
        backend.observations.clear();

        assert_eq!(coordinator.handle(opened_event(), &mut backend), None);

        assert!(matches!(
            backend.observations.as_slice(),
            [Observation::Stderr(message), Observation::EnterRaw]
                if message.contains("pid=4815")
                    && message.contains("service_id=service-alpha")
                    && message.contains("replay_terminal_output=true")
        ));
    }

    #[test]
    fn remote_output_preserves_invalid_utf8_and_ansi_bytes_on_stdout_only() {
        let (mut coordinator, mut backend) = active_runtime(false);
        let output = vec![0x00, 0x1b, b'[', b'3', b'1', b'm', 0xff, b'\n'];

        assert_eq!(
            coordinator.handle(
                RuntimeEvent::Remote(HeadlessEvent::Output {
                    terminal_id: TERMINAL_ID,
                    data: output.clone(),
                }),
                &mut backend,
            ),
            None
        );

        assert_eq!(backend.observations, vec![Observation::Stdout(output)]);
        assert!(!backend
            .observations
            .iter()
            .any(|action| matches!(action, Observation::Stderr(_))));
    }

    #[test]
    fn resize_is_sent_only_after_the_terminal_size_changes() {
        let (mut coordinator, mut backend) = active_runtime(false);
        backend.sizes = VecDeque::from([
            Ok(TtySize { rows: 24, cols: 80 }),
            Ok(TtySize {
                rows: 40,
                cols: 120,
            }),
        ]);

        coordinator.handle(RuntimeEvent::Signal(SignalEvent::Resize), &mut backend);
        coordinator.handle(RuntimeEvent::Signal(SignalEvent::Resize), &mut backend);

        assert_eq!(
            backend.observations,
            vec![Observation::Session(SessionAction::ResizeTerminal {
                terminal_id: TERMINAL_ID,
                rows: 40,
                cols: 120,
            })]
        );
    }

    #[test]
    fn local_detach_closes_default_terminal_then_transport() {
        let (mut coordinator, mut backend) = active_runtime(false);
        coordinator.handle(RuntimeEvent::Input(InputChunk::Detach), &mut backend);
        assert_eq!(
            backend.observations,
            vec![
                Observation::RestoreTty,
                Observation::Session(SessionAction::CloseTerminal {
                    terminal_id: TERMINAL_ID,
                }),
                Observation::Session(SessionAction::CloseTransport),
            ]
        );
    }

    #[test]
    fn local_detach_preserves_persistent_terminal() {
        let (mut coordinator, mut backend) = active_runtime(true);
        coordinator.handle(RuntimeEvent::Input(InputChunk::Detach), &mut backend);
        assert_eq!(
            backend.observations,
            vec![
                Observation::RestoreTty,
                Observation::Session(SessionAction::CloseTransport),
            ]
        );
    }

    #[test]
    fn local_input_forwards_bytes_with_terminal_id_one() {
        let (mut coordinator, mut backend) = active_runtime(false);
        coordinator.handle(
            RuntimeEvent::Input(InputChunk::Data(vec![0x00, 0x03, 0xff])),
            &mut backend,
        );
        assert_eq!(
            backend.observations,
            vec![Observation::Session(SessionAction::SendInput {
                terminal_id: TERMINAL_ID,
                data: vec![0x00, 0x03, 0xff],
            })]
        );
    }

    #[test]
    fn closed_statuses_restore_tty_close_transport_and_map_exit() {
        for (remote_status, local_status) in [(0, 0), (7, 7), (125, 125), (126, 1)] {
            let (mut coordinator, mut backend) = active_runtime(false);
            assert_eq!(
                coordinator.handle(
                    RuntimeEvent::Remote(HeadlessEvent::Closed {
                        terminal_id: TERMINAL_ID,
                        exit_code: remote_status,
                    }),
                    &mut backend,
                ),
                None
            );
            assert_eq!(
                handle_and_record_exit(
                    &mut coordinator,
                    &mut backend,
                    RuntimeEvent::TransportClosed,
                ),
                Some(local_status)
            );
            assert_eq!(
                backend.observations,
                vec![
                    Observation::RestoreTty,
                    Observation::Session(SessionAction::CloseTransport),
                    Observation::Exit(local_status),
                ]
            );
        }
    }

    #[test]
    fn transport_close_before_opened_is_a_connection_failure() {
        let mut coordinator = RuntimeCoordinator::new(false);
        let mut backend = FakeRuntimeBackend::new();
        coordinator.handle(connected_event(), &mut backend);
        backend.observations.clear();

        assert_eq!(
            handle_and_record_exit(
                &mut coordinator,
                &mut backend,
                RuntimeEvent::TransportClosed,
            ),
            Some(5)
        );
        assert!(matches!(
            backend.observations.as_slice(),
            [
                Observation::RestoreTty,
                Observation::Stderr(_),
                Observation::Exit(5)
            ]
        ));
    }

    #[test]
    fn transport_close_while_active_is_a_connection_failure() {
        let (mut coordinator, mut backend) = active_runtime(false);
        assert_eq!(
            handle_and_record_exit(
                &mut coordinator,
                &mut backend,
                RuntimeEvent::TransportClosed,
            ),
            Some(5)
        );
        assert!(matches!(
            backend.observations.as_slice(),
            [
                Observation::RestoreTty,
                Observation::Stderr(_),
                Observation::Exit(5)
            ]
        ));
    }

    #[test]
    fn connection_failure_restores_and_exits_in_opening_or_active() {
        for active in [false, true] {
            let mut coordinator = RuntimeCoordinator::new(false);
            let mut backend = FakeRuntimeBackend::new();
            coordinator.handle(connected_event(), &mut backend);
            if active {
                coordinator.handle(opened_event(), &mut backend);
            }
            backend.observations.clear();

            assert_eq!(
                handle_and_record_exit(
                    &mut coordinator,
                    &mut backend,
                    RuntimeEvent::Remote(HeadlessEvent::ConnectionFailed(
                        "injected connection failure".to_owned(),
                    )),
                ),
                Some(5)
            );
            assert!(matches!(
                backend.observations.as_slice(),
                [
                    Observation::RestoreTty,
                    Observation::Stderr(_),
                    Observation::Exit(5)
                ]
            ));
        }
    }

    #[test]
    fn saved_credential_success_opens_without_prompting() {
        let mut coordinator = RuntimeCoordinator::new(false);
        let mut backend = FakeRuntimeBackend::new();
        coordinator.handle(connected_event(), &mut backend);
        assert_eq!(
            backend.observations,
            vec![Observation::Session(SessionAction::OpenTerminal {
                terminal_id: TERMINAL_ID,
                rows: 24,
                cols: 80,
            })]
        );
    }

    #[test]
    fn password_prompt_submits_once_while_pending_and_retries_on_request() {
        let mut coordinator = RuntimeCoordinator::new(false);
        let mut backend = FakeRuntimeBackend::new();
        backend.secrets = VecDeque::from([
            Some("first-password".to_owned()),
            Some("retry-password".to_owned()),
        ]);
        backend.confirmations = VecDeque::from([Some(true), Some(false)]);
        let first =
            RuntimeEvent::Remote(HeadlessEvent::Auth(AuthPrompt::Password { retry: false }));
        let duplicate = first.clone();
        let retry = RuntimeEvent::Remote(HeadlessEvent::Auth(AuthPrompt::Password { retry: true }));

        coordinator.handle(first, &mut backend);
        coordinator.handle(duplicate, &mut backend);
        coordinator.handle(retry, &mut backend);

        assert_eq!(
            backend.observations,
            vec![
                Observation::PromptSecret,
                Observation::PromptRemember,
                Observation::Login {
                    password: "first-password".to_owned(),
                    remember: true,
                },
                Observation::PromptSecret,
                Observation::PromptRemember,
                Observation::Login {
                    password: "retry-password".to_owned(),
                    remember: false,
                },
            ]
        );
    }

    #[test]
    fn two_factor_prompts_then_submits_without_trust() {
        let mut coordinator = RuntimeCoordinator::new(false);
        let mut backend = FakeRuntimeBackend::new();
        backend.lines = VecDeque::from([Some("246810".to_owned())]);

        coordinator.handle(
            RuntimeEvent::Remote(HeadlessEvent::Auth(AuthPrompt::TwoFactor)),
            &mut backend,
        );

        assert_eq!(
            backend.observations,
            vec![
                Observation::PromptLine,
                Observation::SendTwoFactor("246810".to_owned(), false),
            ]
        );
    }

    #[test]
    fn insecure_connection_is_rejected_without_a_bypass() {
        let mut coordinator = RuntimeCoordinator::new(false);
        let mut backend = FakeRuntimeBackend::new();
        coordinator.handle(
            RuntimeEvent::Remote(HeadlessEvent::Auth(AuthPrompt::InsecureConnection)),
            &mut backend,
        );
        assert_eq!(
            backend.observations,
            vec![Observation::RejectInsecureConnection]
        );
    }

    #[test]
    fn authentication_prompt_eof_closes_transport_and_exits_four() {
        let mut coordinator = RuntimeCoordinator::new(false);
        let mut backend = FakeRuntimeBackend::new();
        assert_eq!(
            handle_and_record_exit(
                &mut coordinator,
                &mut backend,
                RuntimeEvent::Remote(HeadlessEvent::Auth(AuthPrompt::Password { retry: false })),
            ),
            Some(4)
        );
        assert!(matches!(
            backend.observations.as_slice(),
            [
                Observation::PromptSecret,
                Observation::Session(SessionAction::CloseTransport),
                Observation::RestoreTty,
                Observation::Stderr(_),
                Observation::Exit(4)
            ]
        ));
    }

    #[test]
    fn confirmation_prompt_eof_closes_transport_before_status_four() {
        let mut coordinator = RuntimeCoordinator::new(false);
        let mut backend = FakeRuntimeBackend::new();
        backend.secrets = VecDeque::from([Some("password".to_owned())]);
        backend.confirmations = VecDeque::from([None]);

        assert_eq!(
            handle_and_record_exit(
                &mut coordinator,
                &mut backend,
                RuntimeEvent::Remote(HeadlessEvent::Auth(AuthPrompt::Password { retry: false })),
            ),
            Some(4)
        );
        assert!(matches!(
            backend.observations.as_slice(),
            [
                Observation::PromptSecret,
                Observation::PromptRemember,
                Observation::Session(SessionAction::CloseTransport),
                Observation::RestoreTty,
                Observation::Stderr(_),
                Observation::Exit(4)
            ]
        ));
    }

    #[test]
    fn cli_persistence_overrides_only_the_in_memory_peer_config() {
        let mut without_persistence = crate::client::LoginConfigHandler::default();
        without_persistence.get_config().terminal_persistent.v = true;
        apply_cli_persistence(&mut without_persistence, false);
        assert!(!without_persistence.get_config().terminal_persistent.v);

        let mut with_persistence = crate::client::LoginConfigHandler::default();
        with_persistence.get_config().terminal_persistent.v = false;
        apply_cli_persistence(&mut with_persistence, true);
        assert!(with_persistence.get_config().terminal_persistent.v);
    }
}
