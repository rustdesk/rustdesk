use super::*;
use hbb_common::{
    anyhow::{anyhow, Context, Result},
    compress,
};
use portable_pty::{Child, CommandBuilder, PtySize};
use std::{
    collections::{HashMap, VecDeque},
    io::{Read, Write},
    ops::{Deref, DerefMut},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

const MAX_OUTPUT_BUFFER_SIZE: usize = 1024 * 1024; // 1MB per terminal
const MAX_BUFFER_LINES: usize = 10000;
const MAX_SERVICES: usize = 100; // Maximum number of persistent terminal services
const SERVICE_IDLE_TIMEOUT: Duration = Duration::from_secs(3600); // 1 hour idle timeout
const CHANNEL_BUFFER_SIZE: usize = 100; // Number of messages to buffer in channel
const COMPRESS_THRESHOLD: usize = 512; // Compress terminal data larger than this

lazy_static::lazy_static! {
    // Global registry of persistent terminal services indexed by service_id
    static ref TERMINAL_SERVICES: Arc<Mutex<HashMap<String, Arc<Mutex<PersistentTerminalService>>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    // Cleanup task handle
    static ref CLEANUP_TASK: Arc<Mutex<Option<std::thread::JoinHandle<()>>>> = Arc::new(Mutex::new(None));

    // List of terminal child processes to check for zombies
    static ref TERMINAL_TASKS: Arc<Mutex<Vec<Box<dyn Child + Send + Sync>>>> = Arc::new(Mutex::new(Vec::new()));
}

/// Service metadata that is sent to clients
#[derive(Clone, Debug)]
pub struct ServiceMetadata {
    pub service_id: String,
    pub created_at: Instant,
    pub terminal_count: usize,
    pub is_persistent: bool,
}

/// Generate a new persistent service ID
pub fn generate_service_id() -> String {
    format!("ts_{}", uuid::Uuid::new_v4())
}

fn get_default_shell() -> String {
    #[cfg(target_os = "windows")]
    {
        // Try PowerShell Core first (cross-platform version)
        // Common installation paths for PowerShell Core
        let pwsh_paths = [
            "pwsh.exe",
            r"C:\Program Files\PowerShell\7\pwsh.exe",
            r"C:\Program Files\PowerShell\6\pwsh.exe",
        ];

        for path in &pwsh_paths {
            if std::path::Path::new(path).exists() {
                return path.to_string();
            }
        }

        // Try Windows PowerShell (should be available on all Windows systems)
        let powershell_path = r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe";
        if std::path::Path::new(powershell_path).exists() {
            return powershell_path.to_string();
        }

        // Final fallback to cmd.exe
        std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string())
    }
    #[cfg(not(target_os = "windows"))]
    {
        // First try the SHELL environment variable
        if let Ok(shell) = std::env::var("SHELL") {
            if !shell.is_empty() {
                return shell;
            }
        }

        // Check for common shells in order of preference
        let shells = ["/bin/bash", "/bin/zsh", "/bin/sh"];
        for shell in &shells {
            if std::path::Path::new(shell).exists() {
                return shell.to_string();
            }
        }

        // Final fallback to /bin/sh which should exist on all POSIX systems
        "/bin/sh".to_string()
    }
}

pub fn is_service_specified_user(service_id: &str) -> Option<bool> {
    get_service(service_id).map(|s| s.lock().unwrap().is_specified_user)
}

/// Get or create a persistent terminal service
fn get_or_create_service(
    service_id: String,
    is_persistent: bool,
    is_specified_user: bool,
) -> Result<Arc<Mutex<PersistentTerminalService>>> {
    let mut services = TERMINAL_SERVICES.lock().unwrap();

    // Check service limit
    if !services.contains_key(&service_id) && services.len() >= MAX_SERVICES {
        return Err(anyhow!(
            "Maximum number of terminal services ({}) reached",
            MAX_SERVICES
        ));
    }

    let service = services
        .entry(service_id.clone())
        .or_insert_with(|| {
            log::info!(
                "Creating new terminal service: {} (persistent: {})",
                service_id,
                is_persistent
            );
            Arc::new(Mutex::new(PersistentTerminalService::new(
                service_id.clone(),
                is_persistent,
                is_specified_user,
            )))
        })
        .clone();

    // Ensure cleanup task is running
    ensure_cleanup_task();

    service.lock().unwrap().reset_status(is_persistent);

    Ok(service)
}

/// Remove a service from the global registry
fn remove_service(service_id: &str) {
    let mut services = TERMINAL_SERVICES.lock().unwrap();
    if let Some(service) = services.remove(service_id) {
        log::info!("Removed service: {}", service_id);
        // Close all terminals in the service
        let sessions = service.lock().unwrap().sessions.clone();
        for (_, session) in sessions.iter() {
            let mut session = session.lock().unwrap();
            session.stop();
        }
    }
}

/// List all active terminal services
pub fn list_services() -> Vec<ServiceMetadata> {
    let services = TERMINAL_SERVICES.lock().unwrap();
    services
        .iter()
        .filter_map(|(id, service)| {
            service.lock().ok().map(|svc| ServiceMetadata {
                service_id: id.clone(),
                created_at: svc.created_at,
                terminal_count: svc.sessions.len(),
                is_persistent: svc.is_persistent,
            })
        })
        .collect()
}

/// Get service by ID
pub fn get_service(service_id: &str) -> Option<Arc<Mutex<PersistentTerminalService>>> {
    let services = TERMINAL_SERVICES.lock().unwrap();
    services.get(service_id).cloned()
}

/// Clean up inactive services
pub fn cleanup_inactive_services() {
    let services = TERMINAL_SERVICES.lock().unwrap();
    let now = Instant::now();
    let mut to_remove = Vec::new();

    for (service_id, service) in services.iter() {
        if let Ok(svc) = service.lock() {
            // Remove non-persistent services after idle timeout
            if !svc.is_persistent && now.duration_since(svc.last_activity) > SERVICE_IDLE_TIMEOUT {
                to_remove.push(service_id.clone());
                log::info!("Cleaning up idle non-persistent service: {}", service_id);
            }
            // Remove persistent services with no active terminals after longer timeout
            else if svc.is_persistent
                && svc.sessions.is_empty()
                && now.duration_since(svc.last_activity) > SERVICE_IDLE_TIMEOUT * 2
            {
                to_remove.push(service_id.clone());
                log::info!("Cleaning up empty persistent service: {}", service_id);
            }
        }
    }

    // Remove outside of iteration to avoid deadlock
    drop(services);
    for id in to_remove {
        remove_service(&id);
    }
}

/// Add a child process to the zombie reaper
fn add_to_reaper(child: Box<dyn Child + Send + Sync>) {
    if let Ok(mut tasks) = TERMINAL_TASKS.lock() {
        tasks.push(child);
    }
}

/// Check and reap zombie terminal processes
fn check_zombie_terminals() {
    let mut tasks = match TERMINAL_TASKS.lock() {
        Ok(t) => t,
        Err(_) => return,
    };

    let mut i = 0;
    while i < tasks.len() {
        match tasks[i].try_wait() {
            Ok(Some(_)) => {
                // Process has exited, remove it
                log::info!("Process exited: {:?}", tasks[i].process_id());
                tasks.remove(i);
            }
            Ok(None) => {
                // Still running
                i += 1;
            }
            Err(err) => {
                // Error checking status, remove it
                log::info!(
                    "Process exited with error: {:?}, err: {err}",
                    tasks[i].process_id()
                );
                tasks.remove(i);
            }
        }
    }
}

/// Ensure the cleanup task is running
fn ensure_cleanup_task() {
    let mut task_handle = CLEANUP_TASK.lock().unwrap();
    if task_handle.is_none() {
        let handle = std::thread::spawn(|| {
            log::info!("Started cleanup task");
            let mut last_service_cleanup = Instant::now();
            loop {
                // Check for zombie processes every 100ms
                check_zombie_terminals();

                // Check for inactive services every 5 minutes
                if last_service_cleanup.elapsed() > Duration::from_secs(300) {
                    cleanup_inactive_services();
                    last_service_cleanup = Instant::now();
                }

                std::thread::sleep(Duration::from_millis(100));
            }
        });
        *task_handle = Some(handle);
    }
}

#[cfg(target_os = "linux")]
pub fn get_terminal_session_count(include_zombie_tasks: bool) -> usize {
    let mut c = TERMINAL_SERVICES.lock().unwrap().len();
    if include_zombie_tasks {
        c += TERMINAL_TASKS.lock().unwrap().len();
    }
    c
}

pub type UserToken = u64;

#[derive(Clone)]
pub struct TerminalService {
    sp: GenericService,
    user_token: Option<UserToken>,
}

impl Deref for TerminalService {
    type Target = ServiceTmpl<ConnInner>;

    fn deref(&self) -> &Self::Target {
        &self.sp
    }
}

impl DerefMut for TerminalService {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.sp
    }
}

pub fn get_service_name(source: VideoSource, idx: usize) -> String {
    format!("{}{}", source.service_name_prefix(), idx)
}

pub fn new(
    service_id: String,
    is_persistent: bool,
    user_token: Option<UserToken>,
) -> GenericService {
    // Create the service with initial persistence setting
    allow_err!(get_or_create_service(
        service_id.clone(),
        is_persistent,
        user_token.is_some()
    ));
    let svc = TerminalService {
        sp: GenericService::new(service_id.clone(), false),
        user_token,
    };
    GenericService::run(&svc.clone(), move |sp| run(sp, service_id.clone()));
    svc.sp
}

fn run(sp: TerminalService, service_id: String) -> ResultType<()> {
    while sp.ok() {
        let responses = TerminalServiceProxy::new(service_id.clone(), None, sp.user_token.clone())
            .read_outputs();
        for response in responses {
            let mut msg_out = Message::new();
            msg_out.set_terminal_response(response);
            sp.send(msg_out);
        }

        thread::sleep(Duration::from_millis(30)); // Read at ~33fps for responsive terminal
    }

    // Clean up non-persistent service when loop exits
    if let Some(service) = get_service(&service_id) {
        let should_remove = !service.lock().unwrap().is_persistent;
        if should_remove {
            remove_service(&service_id);
        }
    }

    Ok(())
}

/// Output buffer for terminal session
struct OutputBuffer {
    lines: VecDeque<Vec<u8>>,
    total_size: usize,
    last_line_incomplete: bool,
}

impl OutputBuffer {
    fn new() -> Self {
        Self {
            lines: VecDeque::new(),
            total_size: 0,
            last_line_incomplete: false,
        }
    }

    fn append(&mut self, data: &[u8]) {
        if data.is_empty() {
            return;
        }

        // Handle incomplete lines
        let mut start = 0;
        if self.last_line_incomplete {
            if let Some(last_line) = self.lines.back_mut() {
                // Find first newline in new data
                if let Some(newline_pos) = data.iter().position(|&b| b == b'\n') {
                    last_line.extend_from_slice(&data[..=newline_pos]);
                    start = newline_pos + 1;
                    self.last_line_incomplete = false;
                } else {
                    // Still no newline, append all
                    last_line.extend_from_slice(data);
                    self.total_size += data.len();
                    return;
                }
            }
        }

        // Process remaining data
        let remaining = &data[start..];
        let ends_with_newline = remaining.last() == Some(&b'\n');

        // Split by lines
        let lines: Vec<&[u8]> = remaining.split(|&b| b == b'\n').collect();

        for (i, line) in lines.iter().enumerate() {
            if i == lines.len() - 1 && !ends_with_newline && !line.is_empty() {
                // Last line without newline
                self.last_line_incomplete = true;
            }

            if !line.is_empty() || i < lines.len() - 1 {
                let mut line_data = line.to_vec();
                if i < lines.len() - 1 || ends_with_newline {
                    line_data.push(b'\n');
                }

                self.total_size += line_data.len();
                self.lines.push_back(line_data);
            }
        }

        // Trim old data if buffer is too large
        while self.total_size > MAX_OUTPUT_BUFFER_SIZE || self.lines.len() > MAX_BUFFER_LINES {
            if let Some(removed) = self.lines.pop_front() {
                self.total_size -= removed.len();
            }
        }
    }

    fn get_recent(&self, max_bytes: usize) -> Vec<u8> {
        let mut result = Vec::new();
        let mut size = 0;

        // Get recent lines up to max_bytes
        for line in self.lines.iter().rev() {
            if size + line.len() > max_bytes {
                break;
            }
            size += line.len();
            result.splice(0..0, line.iter().cloned());
        }

        result
    }
}

pub struct TerminalSession {
    pub created_at: Instant,
    last_activity: Instant,
    pty_pair: Option<portable_pty::PtyPair>,
    child: Option<Box<dyn Child + std::marker::Send + Sync>>,
    // Channel for sending input to the writer thread
    input_tx: Option<SyncSender<Vec<u8>>>,
    // Channel for receiving output from the reader thread
    output_rx: Option<Receiver<Vec<u8>>>,
    exiting: Arc<AtomicBool>,
    // Thread handles
    reader_thread: Option<thread::JoinHandle<()>>,
    writer_thread: Option<thread::JoinHandle<()>>,
    output_buffer: OutputBuffer,
    title: String,
    pid: u32,
    rows: u16,
    cols: u16,
    // Track if we've already sent the closed message
    closed_message_sent: bool,
    is_opened: bool,
}

impl TerminalSession {
    fn new(terminal_id: i32, rows: u16, cols: u16) -> Self {
        Self {
            created_at: Instant::now(),
            last_activity: Instant::now(),
            pty_pair: None,
            child: None,
            input_tx: None,
            output_rx: None,
            exiting: Arc::new(AtomicBool::new(false)),
            reader_thread: None,
            writer_thread: None,
            output_buffer: OutputBuffer::new(),
            title: format!("Terminal {}", terminal_id),
            pid: 0,
            rows,
            cols,
            closed_message_sent: false,
            is_opened: false,
        }
    }

    fn update_activity(&mut self) {
        self.last_activity = Instant::now();
    }

    // This helper function is to ensure that the threads are joined before the child process is dropped.
    // Though this is not strictly necessary on macOS.
    fn stop(&mut self) {
        self.is_opened = false;
        self.exiting.store(true, Ordering::SeqCst);

        // Drop the input channel to signal writer thread to exit
        if let Some(input_tx) = self.input_tx.take() {
            // Send a final newline to ensure the reader can read some data, and then exit.
            // This is required on Windows and Linux.
            // Although `self.pty_pair = None;` is called below, we can still send a final newline here.
            if let Err(e) = input_tx.send(b"\r\n".to_vec()) {
                log::warn!("Failed to send final newline to the terminal: {}", e);
            }
            drop(input_tx);
        }
        self.output_rx = None;

        // 1. Windows
        //    `pty_pair` uses pipe. https://github.com/rustdesk-org/wezterm/blob/80174f8009f41565f0fa8c66dab90d4f9211ae16/pty/src/win/conpty.rs#L16
        //     `read()` may stuck at https://github.com/rustdesk-org/wezterm/blob/80174f8009f41565f0fa8c66dab90d4f9211ae16/filedescriptor/src/windows.rs#L345
        //     We can close the pipe to signal the reader thread to exit.
        //     After https://github.com/rustdesk-org/wezterm/blob/80174f8009f41565f0fa8c66dab90d4f9211ae16/pty/src/win/psuedocon.rs#L86, the reader reads `[27, 91, 63, 57, 48, 48, 49, 108, 27, 91, 63, 49, 48, 48, 52, 108]` in my tests.
        // 2. Linux
        //    `pty_pair` uses `libc::openpty`. https://github.com/rustdesk-org/wezterm/blob/80174f8009f41565f0fa8c66dab90d4f9211ae16/pty/src/unix.rs#L32
        //    We can also call the drop method first. https://github.com/rustdesk-org/wezterm/blob/80174f8009f41565f0fa8c66dab90d4f9211ae16/pty/src/unix.rs#L352
        //    The reader will get [13, 10] after dropping the `pty_pair`.
        // 3. macOS
        //    No stuck cases have been found so far, more testing is needed.
        #[cfg(any(target_os = "windows", target_os = "linux"))]
        {
            self.pty_pair = None;
        }

        // Wait for threads to finish
        // The reader thread should join before the writer thread on Windows.
        if let Some(reader_thread) = self.reader_thread.take() {
            let _ = reader_thread.join();
        }

        // The read can read the last "\r\n" after the writer thread (not the child process) exits
        // on Linux in my tests.
        // But we still send "\r\n" to the writer thread and let the reader thread exit first for safety.
        if let Some(writer_thread) = self.writer_thread.take() {
            let _ = writer_thread.join();
        }

        if let Some(mut child) = self.child.take() {
            // Kill the process
            let _ = child.kill();
            add_to_reaper(child);
        }
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        // Ensure child process is properly handled when session is dropped
        self.stop();
    }
}

/// Persistent terminal service that can survive connection drops
pub struct PersistentTerminalService {
    service_id: String,
    sessions: HashMap<i32, Arc<Mutex<TerminalSession>>>,
    pub created_at: Instant,
    last_activity: Instant,
    pub is_persistent: bool,
    needs_session_sync: bool,
    is_specified_user: bool,
}

impl PersistentTerminalService {
    pub fn new(service_id: String, is_persistent: bool, is_specified_user: bool) -> Self {
        Self {
            service_id,
            sessions: HashMap::new(),
            created_at: Instant::now(),
            last_activity: Instant::now(),
            is_persistent,
            needs_session_sync: false,
            is_specified_user,
        }
    }

    fn update_activity(&mut self) {
        self.last_activity = Instant::now();
    }

    /// Get list of terminal metadata
    pub fn list_terminals(&self) -> Vec<(i32, String, u32, Instant)> {
        self.sessions
            .iter()
            .map(|(id, session)| {
                let s = session.lock().unwrap();
                (*id, s.title.clone(), s.pid, s.created_at)
            })
            .collect()
    }

    /// Get buffered output for a terminal
    pub fn get_terminal_buffer(&self, terminal_id: i32, max_bytes: usize) -> Option<Vec<u8>> {
        self.sessions.get(&terminal_id).map(|session| {
            let session = session.lock().unwrap();
            session.output_buffer.get_recent(max_bytes)
        })
    }

    /// Get terminal info for recovery
    pub fn get_terminal_info(&self, terminal_id: i32) -> Option<(u16, u16, Vec<u8>)> {
        self.sessions.get(&terminal_id).map(|session| {
            let session = session.lock().unwrap();
            (
                session.rows,
                session.cols,
                session.output_buffer.get_recent(4096),
            )
        })
    }

    /// Check if service has active terminals
    pub fn has_active_terminals(&self) -> bool {
        !self.sessions.is_empty()
    }

    fn reset_status(&mut self, is_persistent: bool) {
        self.is_persistent = is_persistent;
        self.needs_session_sync = true;
        for session in self.sessions.values() {
            let mut session = session.lock().unwrap();
            session.is_opened = false;
        }
    }
}

pub struct TerminalServiceProxy {
    service_id: String,
    is_persistent: bool,
    #[cfg(target_os = "windows")]
    user_token: Option<UserToken>,
}

pub fn set_persistent(service_id: &str, is_persistent: bool) -> Result<()> {
    if let Some(service) = get_service(service_id) {
        service.lock().unwrap().is_persistent = is_persistent;
        Ok(())
    } else {
        Err(anyhow!("Service {} not found", service_id))
    }
}

impl TerminalServiceProxy {
    pub fn new(
        service_id: String,
        is_persistent: Option<bool>,
        _user_token: Option<UserToken>,
    ) -> Self {
        // Get persistence from the service if it exists
        let is_persistent =
            is_persistent.unwrap_or(if let Some(service) = get_service(&service_id) {
                service.lock().unwrap().is_persistent
            } else {
                false
            });
        TerminalServiceProxy {
            service_id,
            is_persistent,
            #[cfg(target_os = "windows")]
            user_token: _user_token,
        }
    }

    pub fn get_service_id(&self) -> &str {
        &self.service_id
    }

    pub fn handle_action(&mut self, action: &TerminalAction) -> Result<Option<TerminalResponse>> {
        let service = match get_service(&self.service_id) {
            Some(s) => s,
            None => {
                let mut response = TerminalResponse::new();
                let mut error = TerminalError::new();
                error.message = format!("Terminal service {} not found", self.service_id);
                response.set_error(error);
                return Ok(Some(response));
            }
        };
        service.lock().unwrap().update_activity();
        match &action.union {
            Some(terminal_action::Union::Open(open)) => {
                self.handle_open(&mut service.lock().unwrap(), open)
            }
            Some(terminal_action::Union::Resize(resize)) => {
                let session = service
                    .lock()
                    .unwrap()
                    .sessions
                    .get(&resize.terminal_id)
                    .cloned();
                self.handle_resize(session, resize)
            }
            Some(terminal_action::Union::Data(data)) => {
                let session = service
                    .lock()
                    .unwrap()
                    .sessions
                    .get(&data.terminal_id)
                    .cloned();
                self.handle_data(session, data)
            }
            Some(terminal_action::Union::Close(close)) => {
                self.handle_close(&mut service.lock().unwrap(), close)
            }
            _ => Ok(None),
        }
    }

    fn handle_open(
        &self,
        service: &mut PersistentTerminalService,
        open: &OpenTerminal,
    ) -> Result<Option<TerminalResponse>> {
        let mut response = TerminalResponse::new();

        // Check if terminal already exists
        if let Some(session_arc) = service.sessions.get(&open.terminal_id) {
            // Reconnect to existing terminal
            let mut session = session_arc.lock().unwrap();
            session.is_opened = true;
            let mut opened = TerminalOpened::new();
            opened.terminal_id = open.terminal_id;
            opened.success = true;
            opened.message = "Reconnected to existing terminal".to_string();
            opened.pid = session.pid;
            opened.service_id = self.service_id.clone();
            if service.needs_session_sync {
                if service.sessions.len() > 1 {
                    // No need to include the current terminal in the list.
                    // Because the `persistent_sessions` is used to restore the other sessions.
                    opened.persistent_sessions = service
                        .sessions
                        .keys()
                        .filter(|&id| *id != open.terminal_id)
                        .cloned()
                        .collect();
                }
                service.needs_session_sync = false;
            }
            response.set_opened(opened);

            // Send buffered output
            let buffer = session.output_buffer.get_recent(4096);
            if !buffer.is_empty() {
                // We'll need to send this separately or extend the protocol
                // For now, just acknowledge the reconnection
            }

            return Ok(Some(response));
        }

        // Create new terminal session
        log::info!(
            "Creating new terminal {} for service: {}",
            open.terminal_id,
            service.service_id
        );
        let mut session =
            TerminalSession::new(open.terminal_id, open.rows as u16, open.cols as u16);

        let pty_size = PtySize {
            rows: open.rows as u16,
            cols: open.cols as u16,
            pixel_width: 0,
            pixel_height: 0,
        };

        log::debug!("Opening PTY with size: {}x{}", open.rows, open.cols);
        let pty_system = portable_pty::native_pty_system();
        let pty_pair = pty_system.openpty(pty_size).context("Failed to open PTY")?;

        // Use default shell for the platform
        let shell = get_default_shell();
        log::debug!("Using shell: {}", shell);

        #[allow(unused_mut)]
        let mut cmd = CommandBuilder::new(&shell);

        #[cfg(target_os = "windows")]
        if let Some(token) = &self.user_token {
            cmd.set_user_token(*token as _);
        }

        log::debug!("Spawning shell process...");
        let child = pty_pair
            .slave
            .spawn_command(cmd)
            .context("Failed to spawn command")?;

        let writer = pty_pair
            .master
            .take_writer()
            .context("Failed to get writer")?;

        let reader = pty_pair
            .master
            .try_clone_reader()
            .context("Failed to get reader")?;

        session.pid = child.process_id().unwrap_or(0) as u32;

        // Create channels for input/output
        let (input_tx, input_rx) = mpsc::sync_channel::<Vec<u8>>(CHANNEL_BUFFER_SIZE);
        let (output_tx, output_rx) = mpsc::sync_channel::<Vec<u8>>(CHANNEL_BUFFER_SIZE);

        // Spawn writer thread
        let terminal_id = open.terminal_id;
        let writer_thread = thread::spawn(move || {
            let mut writer = writer;
            // Write initial carriage return:
            // 1. Windows requires at least one carriage return for `drop()` to work properly.
            //    Without this, the reader may fail to read the buffer after `input_tx.send(b"\r\n".to_vec()).ok();`.
            // 2. This also refreshes the terminal interface on the controlling side (workaround for blank content on connect).
            if let Err(e) = writer.write_all(b"\r") {
                log::error!("Terminal {} initial write error: {}", terminal_id, e);
            } else {
                if let Err(e) = writer.flush() {
                    log::error!("Terminal {} initial flush error: {}", terminal_id, e);
                }
            }
            while let Ok(data) = input_rx.recv() {
                if let Err(e) = writer.write_all(&data) {
                    log::error!("Terminal {} write error: {}", terminal_id, e);
                    break;
                }
                if let Err(e) = writer.flush() {
                    log::error!("Terminal {} flush error: {}", terminal_id, e);
                }
            }
            log::debug!("Terminal {} writer thread exiting", terminal_id);
        });

        let exiting = session.exiting.clone();
        // Spawn reader thread
        let terminal_id = open.terminal_id;
        let reader_thread = thread::spawn(move || {
            let mut reader = reader;
            let mut buf = vec![0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        // EOF
                        // This branch can be reached when the child process exits on macOS.
                        // But not on Linux and Windows in my tests.
                        break;
                    }
                    Ok(n) => {
                        if exiting.load(Ordering::SeqCst) {
                            break;
                        }
                        let data = buf[..n].to_vec();
                        // Try to send, if channel is full, drop the data
                        match output_tx.try_send(data) {
                            Ok(_) => {}
                            Err(mpsc::TrySendError::Full(_)) => {
                                log::debug!(
                                    "Terminal {} output channel full, dropping data",
                                    terminal_id
                                );
                            }
                            Err(mpsc::TrySendError::Disconnected(_)) => {
                                log::debug!("Terminal {} output channel disconnected", terminal_id);
                                break;
                            }
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        // This branch is not reached in my tests, but we still add `exiting` check to ensure we can exit.
                        if exiting.load(Ordering::SeqCst) {
                            break;
                        }
                        // For non-blocking I/O, sleep briefly
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(e) => {
                        log::error!("Terminal {} read error: {}", terminal_id, e);
                        break;
                    }
                }
            }
            log::debug!("Terminal {} reader thread exiting", terminal_id);
        });

        session.pty_pair = Some(pty_pair);
        session.child = Some(child);
        session.input_tx = Some(input_tx);
        session.output_rx = Some(output_rx);
        session.reader_thread = Some(reader_thread);
        session.writer_thread = Some(writer_thread);
        session.is_opened = true;

        let mut opened = TerminalOpened::new();
        opened.terminal_id = open.terminal_id;
        opened.success = true;
        opened.message = "Terminal opened".to_string();
        opened.pid = session.pid;
        opened.service_id = service.service_id.clone();
        if service.needs_session_sync {
            if !service.sessions.is_empty() {
                opened.persistent_sessions = service.sessions.keys().cloned().collect();
            }
            service.needs_session_sync = false;
        }
        response.set_opened(opened);

        log::info!(
            "Terminal {} opened successfully with PID {}",
            open.terminal_id,
            session.pid
        );

        // Store the session
        service
            .sessions
            .insert(open.terminal_id, Arc::new(Mutex::new(session)));

        Ok(Some(response))
    }

    fn handle_resize(
        &self,
        session: Option<Arc<Mutex<TerminalSession>>>,
        resize: &ResizeTerminal,
    ) -> Result<Option<TerminalResponse>> {
        if let Some(session_arc) = session {
            let mut session = session_arc.lock().unwrap();
            session.update_activity();
            session.rows = resize.rows as u16;
            session.cols = resize.cols as u16;

            if let Some(pty_pair) = &session.pty_pair {
                pty_pair.master.resize(PtySize {
                    rows: resize.rows as u16,
                    cols: resize.cols as u16,
                    pixel_width: 0,
                    pixel_height: 0,
                })?;
            }
        }
        Ok(None)
    }

    fn handle_data(
        &self,
        session: Option<Arc<Mutex<TerminalSession>>>,
        data: &TerminalData,
    ) -> Result<Option<TerminalResponse>> {
        if let Some(session_arc) = session {
            let mut session = session_arc.lock().unwrap();
            session.update_activity();
            if let Some(input_tx) = &session.input_tx {
                // Send data to writer thread
                if let Err(e) = input_tx.send(data.data.to_vec()) {
                    log::error!(
                        "Failed to send data to terminal {}: {}",
                        data.terminal_id,
                        e
                    );
                }
            }
        }

        Ok(None)
    }

    fn handle_close(
        &self,
        service: &mut PersistentTerminalService,
        close: &CloseTerminal,
    ) -> Result<Option<TerminalResponse>> {
        let mut response = TerminalResponse::new();

        // Always close and remove the terminal
        if let Some(session_arc) = service.sessions.remove(&close.terminal_id) {
            let mut session = session_arc.lock().unwrap();
            let exit_code = if let Some(mut child) = session.child.take() {
                child.kill()?;
                add_to_reaper(child);
                -1 // -1 indicates forced termination
            } else {
                0
            };

            let mut closed = TerminalClosed::new();
            closed.terminal_id = close.terminal_id;
            closed.exit_code = exit_code;
            response.set_closed(closed);
            Ok(Some(response))
        } else {
            Ok(None)
        }
    }

    pub fn read_outputs(&self) -> Vec<TerminalResponse> {
        let service = match get_service(&self.service_id) {
            Some(s) => s,
            None => {
                return vec![];
            }
        };

        // Get session references with minimal service lock time
        let sessions: Vec<(i32, Arc<Mutex<TerminalSession>>)> = {
            let service = service.lock().unwrap();
            service
                .sessions
                .iter()
                .map(|(id, session)| (*id, session.clone()))
                .collect()
        };

        let mut responses = Vec::new();
        let mut closed_terminals = Vec::new();

        // Process each session with its own lock
        for (terminal_id, session_arc) in sessions {
            if let Ok(mut session) = session_arc.try_lock() {
                // Check if reader thread is still alive and we haven't sent closed message yet
                let mut should_send_closed = false;
                if !session.closed_message_sent {
                    if let Some(thread) = &session.reader_thread {
                        if thread.is_finished() {
                            should_send_closed = true;
                            session.closed_message_sent = true;
                        }
                    }
                }
                // It's Ok to put the closed message here.
                // Because the `reader_thread` is joined in `stop()`,
                // and `stop()` is called before the session is dropped.
                if should_send_closed {
                    closed_terminals.push(terminal_id);
                }

                if !session.is_opened {
                    // Skip the session if it is not opened.
                    continue;
                }

                // Read from output channel
                let mut has_activity = false;
                let mut received_data = Vec::new();
                if let Some(output_rx) = &session.output_rx {
                    // Try to read all available data
                    while let Ok(data) = output_rx.try_recv() {
                        has_activity = true;
                        received_data.push(data);
                    }
                }

                // Update buffer after reading
                for data in &received_data {
                    session.output_buffer.append(data);
                }

                // Process received data for responses
                for data in received_data {
                    let mut response = TerminalResponse::new();
                    let mut terminal_data = TerminalData::new();
                    terminal_data.terminal_id = terminal_id;

                    // Compress data if it exceeds threshold
                    if data.len() > COMPRESS_THRESHOLD {
                        let compressed = compress::compress(&data);
                        if compressed.len() < data.len() {
                            terminal_data.data = bytes::Bytes::from(compressed);
                            terminal_data.compressed = true;
                        } else {
                            // Compression didn't help, send uncompressed
                            terminal_data.data = bytes::Bytes::from(data);
                        }
                    } else {
                        terminal_data.data = bytes::Bytes::from(data);
                    }

                    response.set_data(terminal_data);
                    responses.push(response);
                }

                if has_activity {
                    session.update_activity();
                }
            }
        }

        // Clean up closed terminals (requires service lock briefly)
        if !closed_terminals.is_empty() {
            let mut sessions = service.lock().unwrap().sessions.clone();
            for terminal_id in closed_terminals {
                let mut exit_code = 0;

                if !self.is_persistent {
                    if let Some(session_arc) = sessions.remove(&terminal_id) {
                        service.lock().unwrap().sessions.remove(&terminal_id);
                        let mut session = session_arc.lock().unwrap();
                        // Take the child and add to zombie reaper
                        if let Some(mut child) = session.child.take() {
                            // Try to get exit code if available
                            if let Ok(Some(status)) = child.try_wait() {
                                exit_code = status.exit_code() as i32;
                            }
                            add_to_reaper(child);
                        }
                    }
                } else {
                    // For persistent sessions, just clear the child reference
                    if let Some(session_arc) = sessions.get(&terminal_id) {
                        let mut session = session_arc.lock().unwrap();
                        if let Some(mut child) = session.child.take() {
                            // Try to get exit code if available
                            if let Ok(Some(status)) = child.try_wait() {
                                exit_code = status.exit_code() as i32;
                            }
                            add_to_reaper(child);
                        }
                    }
                }

                let mut response = TerminalResponse::new();
                let mut closed = TerminalClosed::new();
                closed.terminal_id = terminal_id;
                closed.exit_code = exit_code;
                response.set_closed(closed);
                responses.push(response);
            }
        }

        responses
    }

    /// Cleanup when connection drops
    pub fn on_disconnect(&self) {
        if !self.is_persistent {
            // Remove non-persistent service
            remove_service(&self.service_id);
        }
    }
}
