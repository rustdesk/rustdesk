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

// Windows-specific imports from terminal_helper module
#[cfg(target_os = "windows")]
use super::terminal_helper::{
    create_named_pipe_server, encode_helper_message, encode_resize_message,
    is_helper_process_running, launch_terminal_helper_with_token, wait_for_pipe_connection,
    HelperProcessGuard, OwnedHandle, SendableHandle, WinCloseHandle, WinTerminateProcess,
    WinWaitForSingleObject, MSG_TYPE_DATA, PIPE_CONNECTION_TIMEOUT_MS, WIN_WAIT_OBJECT_0,
};

const MAX_OUTPUT_BUFFER_SIZE: usize = 1024 * 1024; // 1MB per terminal
const MAX_BUFFER_LINES: usize = 10000;
const MAX_SERVICES: usize = 100; // Maximum number of persistent terminal services
const SERVICE_IDLE_TIMEOUT: Duration = Duration::from_secs(3600); // 1 hour idle timeout
const CHANNEL_BUFFER_SIZE: usize = 500; // Channel buffer size. Max per-message size ~4KB (reader buffer), so worst case ~500*4KB ≈ 2MB/terminal. Increased from 100 to reduce data loss during disconnects.
const COMPRESS_THRESHOLD: usize = 512; // Compress terminal data larger than this
                                       // Default max bytes for reconnection buffer replay.
const DEFAULT_RECONNECT_BUFFER_BYTES: usize = 8 * 1024;
const MAX_SIGWINCH_PHASE_ATTEMPTS: u8 = 3; // Max attempts per SIGWINCH phase before giving up

/// Two-phase SIGWINCH trigger for TUI app redraw on reconnection.
///
/// Why two phases? A single resize-then-restore done back-to-back is too fast:
/// by the time the TUI app handles the asynchronous SIGWINCH signal and calls
/// `ioctl(TIOCGWINSZ)`, the PTY size has already been restored to the original.
/// ncurses sees no size change and skips the full redraw.
///
/// Splitting across two `read_outputs()` calls (~30ms apart) ensures the app
/// sees a real size change on each SIGWINCH, forcing a complete redraw.
#[derive(Debug, Clone)]
enum SigwinchPhase {
    /// No SIGWINCH needed.
    Idle,
    /// Phase 1: Resize PTY to temp dimensions (rows±1). The app handles SIGWINCH
    /// and redraws at the temporary size.
    TempResize { retries: u8 },
    /// Phase 2: Restore PTY to correct dimensions. The app handles SIGWINCH,
    /// detects the size change, and performs a full redraw at the correct size.
    Restore { retries: u8 },
}

/// Which resize to perform in the two-phase SIGWINCH sequence.
enum SigwinchAction {
    /// Phase 1: resize to temp dimensions (rows±1) to trigger SIGWINCH with a visible size change.
    TempResize,
    /// Phase 2: restore to correct dimensions to trigger SIGWINCH and force full redraw.
    Restore,
}

/// Session state machine for terminal streaming.
#[derive(Debug)]
enum SessionState {
    /// Session is closed, not streaming data to client.
    Closed,
    /// Session is active, streaming data to client.
    /// pending_buffer: historical buffer to send before real-time data (set on reconnection).
    /// sigwinch: two-phase SIGWINCH trigger state for TUI app redraw.
    Active {
        pending_buffer: Option<Vec<u8>>,
        sigwinch: SigwinchPhase,
    },
}

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
        // Use shared implementation from terminal_helper
        super::terminal_helper::get_default_shell()
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

/// User token wrapper for cross-module use.
///
/// # Design Note
/// On Windows, this type is defined in terminal_helper.rs and re-exported here.
/// On non-Windows platforms, it's defined here directly.
/// This design avoids circular dependencies while keeping the API consistent.
/// Both definitions MUST have identical public API (new, as_raw methods).
#[cfg(not(target_os = "windows"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserToken(pub usize);

#[cfg(not(target_os = "windows"))]
impl UserToken {
    pub fn new(handle: usize) -> Self {
        Self(handle)
    }

    pub fn as_raw(&self) -> usize {
        self.0
    }
}

#[cfg(target_os = "windows")]
pub use super::terminal_helper::UserToken;

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
        if max_bytes == 0 {
            return Vec::new();
        }
        let mut chunks: Vec<&[u8]> = Vec::new();
        let mut size = 0;

        // Collect whole chunks from newest to oldest, preserving chronological continuity.
        // If the newest chunk alone exceeds max_bytes, take its tail (truncation may split
        // an ANSI escape, but the terminal will self-correct on subsequent output).
        for line in self.lines.iter().rev() {
            if size + line.len() > max_bytes {
                if size == 0 && line.len() > max_bytes {
                    // Single oversized chunk: take the tail to preserve the most recent content.
                    // Align offset forward to a UTF-8 char boundary so that downstream
                    // clients (e.g. Dart) that decode the payload as UTF-8 text don't
                    // encounter split code points. The protobuf bytes field itself allows
                    // arbitrary bytes; this is a best-effort mitigation for client-side decoding.
                    let mut offset = line.len() - max_bytes;
                    // Skip at most 3 continuation bytes (UTF-8 max 4-byte sequence).
                    // Prevents runaway skipping on non-UTF-8 binary data.
                    let mut skipped = 0u8;
                    while skipped < 3
                        && offset < line.len()
                        && (line[offset] & 0b1100_0000) == 0b1000_0000
                    {
                        offset += 1;
                        skipped += 1;
                    }
                    // If we skipped past all remaining bytes (degenerate data), drop the
                    // chunk entirely rather than emitting a slice that decodes poorly on the client.
                    if offset < line.len() {
                        chunks.push(&line[offset..]);
                        size = line.len() - offset;
                    }
                }
                break;
            }
            size += line.len();
            chunks.push(line);
        }

        // Reverse to restore chronological order and concatenate
        chunks.reverse();
        let mut result = Vec::with_capacity(size);
        for chunk in chunks {
            result.extend_from_slice(chunk);
        }

        result
    }
}

/// Try to send data through the output channel with rate-limited drop logging.
/// Returns `true` if the caller should break out of the read loop (channel disconnected).
fn try_send_output(
    output_tx: &mpsc::SyncSender<Vec<u8>>,
    data: Vec<u8>,
    terminal_id: i32,
    label: &str,
    drop_count: &mut u64,
    last_drop_warn: &mut Instant,
) -> bool {
    match output_tx.try_send(data) {
        Ok(_) => {
            if *drop_count > 0 {
                log::trace!(
                    "Terminal {}{} output channel recovered, dropped {} chunks since last report",
                    terminal_id,
                    label,
                    *drop_count
                );
                *drop_count = 0;
            }
            false
        }
        Err(mpsc::TrySendError::Full(_)) => {
            *drop_count += 1;
            if last_drop_warn.elapsed() >= Duration::from_secs(5) {
                log::trace!(
                    "Terminal {}{} output channel full, dropped {} chunks in last {:?}",
                    terminal_id,
                    label,
                    *drop_count,
                    last_drop_warn.elapsed()
                );
                *drop_count = 0;
                *last_drop_warn = Instant::now();
            }
            false
        }
        Err(mpsc::TrySendError::Disconnected(_)) => {
            log::debug!("Terminal {}{} output channel disconnected", terminal_id, label);
            true
        }
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
    // Session state machine for reconnection handling
    state: SessionState,
    // Helper mode: PTY is managed by helper process, communication via message protocol
    #[cfg(target_os = "windows")]
    is_helper_mode: bool,
    // Handle to helper process for termination when session closes
    #[cfg(target_os = "windows")]
    helper_process_handle: Option<SendableHandle>,
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
            state: SessionState::Closed,
            #[cfg(target_os = "windows")]
            is_helper_mode: false,
            #[cfg(target_os = "windows")]
            helper_process_handle: None,
        }
    }

    fn update_activity(&mut self) {
        self.last_activity = Instant::now();
    }

    // This helper function is to ensure that the threads are joined before the child process is dropped.
    // Though this is not strictly necessary on macOS.
    fn stop(&mut self) {
        self.state = SessionState::Closed;
        self.exiting.store(true, Ordering::SeqCst);

        // Drop the input channel to signal writer thread to exit
        if let Some(input_tx) = self.input_tx.take() {
            // Send a final newline to ensure the reader can read some data, and then exit.
            // This is required on Windows and Linux.
            // Although `self.pty_pair = None;` is called below, we can still send a final newline here.
            #[cfg(target_os = "windows")]
            let final_msg = if self.is_helper_mode {
                encode_helper_message(MSG_TYPE_DATA, b"\r\n")
            } else {
                b"\r\n".to_vec()
            };
            #[cfg(not(target_os = "windows"))]
            let final_msg = b"\r\n".to_vec();

            if let Err(e) = input_tx.send(final_msg) {
                log::warn!("Failed to send final newline to the terminal: {}", e);
            }
            drop(input_tx);
        }
        self.output_rx = None;

        // CRITICAL: In helper mode, we must terminate the helper process BEFORE joining threads!
        // The reader thread is blocking on output_pipe.read(), which only returns EOF when
        // the helper process exits. If we try to join the reader thread first, we deadlock.
        //
        // Sequence for helper mode:
        // 1. Signal exiting and close input channel (done above)
        // 2. Terminate helper process (causes output pipe EOF)
        // 3. Join reader thread (now unblocked due to EOF)
        // 4. Join writer thread
        #[cfg(target_os = "windows")]
        if self.is_helper_mode {
            if let Some(helper_handle) = self.helper_process_handle.take() {
                let handle = helper_handle.as_raw();
                log::debug!("Helper mode: terminating helper process before joining threads...");

                // Give helper a very short time to exit gracefully (it should detect pipe close)
                // But don't wait too long - we need to unblock the reader thread
                let wait_result = unsafe { WinWaitForSingleObject(handle, 100) };

                if wait_result == WIN_WAIT_OBJECT_0 {
                    log::debug!("Helper process exited gracefully");
                } else {
                    // Force terminate to unblock reader thread
                    log::debug!("Force terminating helper process to unblock reader thread");
                    unsafe {
                        let _ = WinTerminateProcess(handle, 0);
                    }
                }

                unsafe {
                    let _ = WinCloseHandle(handle);
                }
            }
        }

        // 1. Windows (non-helper mode)
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
                session
                    .output_buffer
                    .get_recent(DEFAULT_RECONNECT_BUFFER_BYTES),
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
            session.state = SessionState::Closed;
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

        // When the client requests a terminal_id that doesn't exist but there are
        // surviving persistent sessions, remap the lowest-ID session to the requested
        // terminal_id. This handles the case where _nextTerminalId resets to 1 on
        // reconnect but the server-side sessions have non-contiguous IDs (e.g. {2: htop}).
        //
        // The client's requested terminal_id may not match any surviving session ID
        // (e.g. _nextTerminalId incremented beyond the surviving IDs). This remap is a
        // one-time handle reassignment — only the first reconnect triggers it because
        // needs_session_sync is cleared afterward. Remaining sessions are communicated
        // back via `persistent_sessions` with their original server-side IDs.
        if !service.sessions.contains_key(&open.terminal_id)
            && service.needs_session_sync
            && !service.sessions.is_empty()
        {
            if let Some(&lowest_id) = service.sessions.keys().min() {
                log::info!(
                    "Remapping persistent session {} -> {} for reconnection",
                    lowest_id,
                    open.terminal_id
                );
                if let Some(session_arc) = service.sessions.remove(&lowest_id) {
                    service.sessions.insert(open.terminal_id, session_arc);
                }
            }
        }

        // Check if terminal already exists
        if let Some(session_arc) = service.sessions.get(&open.terminal_id) {
            // Reconnect to existing terminal
            let mut session = session_arc.lock().unwrap();
            // Directly enter Active state with pending buffer for immediate streaming.
            // Historical buffer is sent first by read_outputs(), then real-time data follows.
            // No overlap: pending_buffer comes from output_buffer (pre-disconnect history),
            // while received_data in read_outputs() comes from the channel (post-reconnect).
            // During disconnect, the run loop (sp.ok()) exits so read_outputs() stops being
            // called; output_buffer is not updated, and channel data may be lost if it fills up.
            let buffer = session
                .output_buffer
                .get_recent(DEFAULT_RECONNECT_BUFFER_BYTES);
            let has_pending = !buffer.is_empty();
            session.state = SessionState::Active {
                pending_buffer: if has_pending { Some(buffer) } else { None },
                // Always trigger two-phase SIGWINCH on reconnect to force TUI app redraw,
                // regardless of whether there's pending buffer data. This avoids edge cases
                // where buffer is empty but a TUI app (top/htop) still needs a full redraw.
                sigwinch: SigwinchPhase::TempResize {
                    retries: MAX_SIGWINCH_PHASE_ATTEMPTS,
                },
            };
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

            return Ok(Some(response));
        }

        // Windows with user_token: use helper process to run shell as the logged-in user
        // This solves the ConPTY + CreateProcessAsUserW incompatibility issue where
        // vim, Claude Code, and other TUI applications hang when ConPTY is created
        // by SYSTEM service but shell runs as user via CreateProcessAsUserW.
        #[cfg(target_os = "windows")]
        if self.user_token.is_some() {
            return self.handle_open_with_helper(service, open);
        }

        // Create new terminal session
        log::info!(
            "Creating new terminal {} for service {}",
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

        // macOS-specific terminal configuration
        // 1. Use login shell (-l) to load user's shell profile (~/.zprofile, ~/.bash_profile)
        //    This ensures PATH includes Homebrew paths (/opt/homebrew/bin, /usr/local/bin)
        // 2. Set TERM environment variable for proper terminal behavior
        //    This fixes issues with control sequences (e.g., Delete/Backspace keys)
        //    macOS terminfo uses hex naming: '78' = 'x' for xterm entries
        // Note: For Linux, `TERM` is set in src/platform/linux.rs try_start_server_()
        #[cfg(target_os = "macos")]
        {
            // Start as login shell to load user environment (PATH, etc.)
            cmd.arg("-l");
            log::debug!("Added -l flag for macOS login shell");

            let term = if std::path::Path::new("/usr/share/terminfo/78/xterm-256color").exists() {
                "xterm-256color"
            } else {
                "xterm"
            };
            cmd.env("TERM", term);
            log::debug!("Set TERM={} for macOS PTY", term);
        }

        // Note: On Windows with user_token, we use helper mode (handle_open_with_helper)
        // which is dispatched earlier in this function. This code path is only reached
        // when user_token is None (e.g., running directly as user, not as SYSTEM service).

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
            let mut drop_count: u64 = 0;
            // Initialize to > 5s ago so the first drop triggers a warning immediately.
            let mut last_drop_warn = Instant::now() - Duration::from_secs(6);
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
                        // Use try_send to avoid blocking the reader thread when channel is full.
                        // During disconnect, the run loop (sp.ok()) stops and read_outputs() is
                        // no longer called, so the channel won't be drained. Blocking send would
                        // deadlock the reader thread in that case.
                        // Note: data produced during disconnect may be lost if channel fills up,
                        // since output_buffer is only updated in read_outputs(). The buffer will
                        // contain history from before the disconnect, not data produced after it.
                        if try_send_output(
                            &output_tx,
                            data,
                            terminal_id,
                            "",
                            &mut drop_count,
                            &mut last_drop_warn,
                        ) {
                            break;
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
        session.state = SessionState::Active {
            pending_buffer: None,
            sigwinch: SigwinchPhase::Idle,
        };

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

    /// Windows-only: Open terminal using helper process pattern
    /// This solves the ConPTY + CreateProcessAsUserW incompatibility issue.
    /// The helper process runs as the logged-in user and creates ConPTY + shell,
    /// communicating with this service via named pipes.
    #[cfg(target_os = "windows")]
    fn handle_open_with_helper(
        &self,
        service: &mut PersistentTerminalService,
        open: &OpenTerminal,
    ) -> Result<Option<TerminalResponse>> {
        let mut response = TerminalResponse::new();

        log::info!(
            "Creating new terminal {} using helper process for service: {}",
            open.terminal_id,
            service.service_id
        );

        let mut session =
            TerminalSession::new(open.terminal_id, open.rows as u16, open.cols as u16);

        // Generate unique pipe names for this terminal
        let pipe_id = uuid::Uuid::new_v4();
        let input_pipe_name = format!(r"\\.\pipe\rustdesk_term_in_{}", pipe_id);
        let output_pipe_name = format!(r"\\.\pipe\rustdesk_term_out_{}", pipe_id);

        log::debug!(
            "Creating pipes: input={}, output={}",
            input_pipe_name,
            output_pipe_name
        );

        // Get user_token early - needed for both DACL creation and helper launch
        let user_token = self
            .user_token
            .ok_or_else(|| anyhow!("user_token is required for helper mode"))?;

        // Create pipes (server side, don't wait for connection yet)
        // input_pipe: service WRITES to this, helper READS from this
        // output_pipe: service READS from this, helper WRITES to this
        // Using OwnedHandle for RAII - handles are automatically closed on error
        // Pass user_token to create restricted DACL (only SYSTEM + user can access)
        let input_pipe_handle = OwnedHandle::new(create_named_pipe_server(
            &input_pipe_name,
            false,
            user_token,
        )?);
        let output_pipe_handle = OwnedHandle::new(create_named_pipe_server(
            &output_pipe_name,
            true,
            user_token,
        )?);

        let helper_process_info = launch_terminal_helper_with_token(
            user_token,
            &input_pipe_name,
            &output_pipe_name,
            open.terminal_id,
            open.rows as u16,
            open.cols as u16,
        )?;

        // Use HelperProcessGuard for RAII cleanup - terminates process on error
        // Unlike OwnedHandle which only closes the handle, this guard ensures
        // the helper process is terminated if pipe connection fails or other errors occur.
        let helper_process_guard =
            HelperProcessGuard::new(helper_process_info.handle, helper_process_info.pid);
        let helper_pid = helper_process_guard.pid();

        // Wait for helper to connect to pipes
        // If this fails, HelperProcessGuard will terminate the helper process
        let mut input_pipe = wait_for_pipe_connection(
            input_pipe_handle,
            &input_pipe_name,
            PIPE_CONNECTION_TIMEOUT_MS,
        )?;
        let mut output_pipe = wait_for_pipe_connection(
            output_pipe_handle,
            &output_pipe_name,
            PIPE_CONNECTION_TIMEOUT_MS,
        )?;

        // Check if helper process is still running after pipe connection
        // This provides early detection if helper crashed during startup
        if !is_helper_process_running(helper_process_guard.as_raw()) {
            return Err(anyhow!(
                "Helper process (PID {}) exited unexpectedly after pipe connection",
                helper_pid
            ));
        }

        // Disarm the guard and transfer ownership to session
        // From this point, the session is responsible for terminating the helper
        let helper_raw_handle = helper_process_guard.disarm();

        // Use helper process PID for session tracking
        // Note: This is the helper process PID, not the actual shell PID.
        // The real shell runs inside the helper process but its PID is not exposed here.
        // For process management (termination, status), the helper PID is what we need.
        session.pid = helper_pid;

        // Create channels for input/output (same as direct PTY mode)
        let (input_tx, input_rx) = mpsc::sync_channel::<Vec<u8>>(CHANNEL_BUFFER_SIZE);
        let (output_tx, output_rx) = mpsc::sync_channel::<Vec<u8>>(CHANNEL_BUFFER_SIZE);

        // Spawn writer thread: reads from channel, writes to input pipe
        let terminal_id = open.terminal_id;
        let writer_thread = thread::spawn(move || {
            while let Ok(data) = input_rx.recv() {
                if let Err(e) = input_pipe.write_all(&data) {
                    log::error!("Terminal {} pipe write error: {}", terminal_id, e);
                    break;
                }
                if let Err(e) = input_pipe.flush() {
                    log::error!("Terminal {} pipe flush error: {}", terminal_id, e);
                }
            }
            log::debug!(
                "Terminal {} writer thread (helper mode) exiting",
                terminal_id
            );
        });

        // Spawn reader thread: reads from output pipe, sends to channel
        // Note: The output pipe was created with FILE_FLAG_OVERLAPPED for timeout support
        // during ConnectNamedPipe. However, once converted to a File handle, reads are
        // performed synchronously. The WouldBlock handling below is defensive but may
        // not be triggered in practice since File::read() blocks until data is available.
        let exiting = session.exiting.clone();
        let terminal_id = open.terminal_id;
        let reader_thread = thread::spawn(move || {
            let mut buf = vec![0u8; 4096];
            let mut drop_count: u64 = 0;
            // Initialize to > 5s ago so the first drop triggers a warning immediately.
            let mut last_drop_warn = Instant::now() - Duration::from_secs(6);
            loop {
                match output_pipe.read(&mut buf) {
                    Ok(0) => {
                        // EOF - helper process exited
                        log::debug!("Terminal {} helper output EOF", terminal_id);
                        break;
                    }
                    Ok(n) => {
                        if exiting.load(Ordering::SeqCst) {
                            break;
                        }
                        let data = buf[..n].to_vec();
                        // Use try_send to avoid blocking the reader thread (same as direct PTY mode)
                        if try_send_output(
                            &output_tx,
                            data,
                            terminal_id,
                            " (helper)",
                            &mut drop_count,
                            &mut last_drop_warn,
                        ) {
                            break;
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        // Defensive: WouldBlock is unlikely with synchronous File::read(),
                        // but handle it gracefully just in case.
                        if exiting.load(Ordering::SeqCst) {
                            break;
                        }
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(e) => {
                        log::error!("Terminal {} pipe read error: {}", terminal_id, e);
                        break;
                    }
                }
            }
            log::debug!(
                "Terminal {} reader thread (helper mode) exiting",
                terminal_id
            );
        });

        // In helper mode, we don't have pty_pair or child - helper manages those
        session.pty_pair = None;
        session.child = None;
        session.input_tx = Some(input_tx);
        session.output_rx = Some(output_rx);
        session.reader_thread = Some(reader_thread);
        session.writer_thread = Some(writer_thread);
        session.state = SessionState::Active {
            pending_buffer: None,
            sigwinch: SigwinchPhase::Idle,
        };
        session.is_helper_mode = true;
        session.helper_process_handle = Some(SendableHandle::new(helper_raw_handle));

        let mut opened = TerminalOpened::new();
        opened.terminal_id = open.terminal_id;
        opened.success = true;
        opened.message = "Terminal opened (helper mode)".to_string();
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
            "Terminal {} opened successfully using helper process (PID {})",
            open.terminal_id,
            session.pid
        );

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

            // Note: we do NOT clear the sigwinch phase here. The server-side two-phase
            // SIGWINCH mechanism in read_outputs() is self-contained (temp resize → restore
            // across two polling cycles), so client resize is purely a dimension sync and
            // doesn't affect it.

            // Windows: handle helper mode vs direct PTY mode
            #[cfg(target_os = "windows")]
            {
                if session.is_helper_mode {
                    // Helper mode: send resize command via message protocol
                    if let Some(input_tx) = &session.input_tx {
                        let msg = encode_resize_message(resize.rows as u16, resize.cols as u16);
                        if let Err(e) = input_tx.send(msg) {
                            log::error!("Failed to send resize to helper: {}", e);
                        }
                    } else {
                        log::warn!(
                            "Terminal {} is in helper mode but input_tx is None, cannot send resize",
                            resize.terminal_id
                        );
                    }
                } else {
                    // Direct PTY mode
                    Self::resize_pty(&session, resize)?;
                }
            }

            // Non-Windows: always direct PTY mode
            #[cfg(not(target_os = "windows"))]
            {
                Self::resize_pty(&session, resize)?;
            }
        }
        Ok(None)
    }

    /// Resize PTY directly (used for non-helper mode)
    fn resize_pty(session: &TerminalSession, resize: &ResizeTerminal) -> Result<()> {
        if let Some(pty_pair) = &session.pty_pair {
            pty_pair.master.resize(PtySize {
                rows: resize.rows as u16,
                cols: resize.cols as u16,
                pixel_width: 0,
                pixel_height: 0,
            })?;
        }
        Ok(())
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
                // Encode data for helper mode or send raw for direct PTY mode
                #[cfg(target_os = "windows")]
                let msg = if session.is_helper_mode {
                    encode_helper_message(MSG_TYPE_DATA, &data.data)
                } else {
                    data.data.to_vec()
                };
                #[cfg(not(target_os = "windows"))]
                let msg = data.data.to_vec();

                // Send data to writer thread
                if let Err(e) = input_tx.send(msg) {
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

    /// Perform a single PTY resize as part of the two-phase SIGWINCH sequence.
    /// Returns true if the resize succeeded.
    ///
    /// Takes individual field references to avoid borrowing the entire TerminalSession,
    /// which would conflict with the mutable borrow of session.state in read_outputs().
    fn do_sigwinch_resize(
        terminal_id: i32,
        rows: u16,
        cols: u16,
        pty_pair: &Option<portable_pty::PtyPair>,
        input_tx: &Option<SyncSender<Vec<u8>>>,
        _is_helper_mode: bool,
        action: &SigwinchAction,
    ) -> bool {
        // Skip if dimensions are not initialized (shouldn't happen on reconnect,
        // but guard against it to avoid resizing to nonsensical values).
        if rows == 0 || cols == 0 {
            return false;
        }

        let target_rows = match action {
            SigwinchAction::TempResize => {
                // For very small terminals (≤2 rows), subtracting 1 would result in an unusable
                // size (0 or 1 row), so we add 1 instead. Either direction triggers SIGWINCH.
                if rows > 2 {
                    rows.saturating_sub(1)
                } else {
                    rows.saturating_add(1)
                }
            }
            SigwinchAction::Restore => rows,
        };

        let phase_name = match action {
            SigwinchAction::TempResize => "temp resize",
            SigwinchAction::Restore => "restore",
        };

        #[cfg(target_os = "windows")]
        let use_helper = _is_helper_mode;
        #[cfg(not(target_os = "windows"))]
        let use_helper = false;

        if use_helper {
            #[cfg(target_os = "windows")]
            {
                let input_tx = match input_tx {
                    Some(tx) => tx,
                    None => return false,
                };
                let msg = encode_resize_message(target_rows, cols);
                if let Err(e) = input_tx.try_send(msg) {
                    log::warn!(
                        "Terminal {} SIGWINCH {} via helper failed: {}",
                        terminal_id,
                        phase_name,
                        e
                    );
                    return false;
                }
                true
            }
            #[cfg(not(target_os = "windows"))]
            {
                let _ = (input_tx, phase_name);
                false
            }
        } else if let Some(pty_pair) = pty_pair {
            if let Err(e) = pty_pair.master.resize(PtySize {
                rows: target_rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            }) {
                log::warn!(
                    "Terminal {} SIGWINCH {} failed: {}",
                    terminal_id,
                    phase_name,
                    e
                );
                return false;
            }
            true
        } else {
            false
        }
    }

    /// Helper to create a TerminalResponse with optional compression.
    fn create_terminal_data_response(terminal_id: i32, data: Vec<u8>) -> TerminalResponse {
        let mut response = TerminalResponse::new();
        let mut terminal_data = TerminalData::new();
        terminal_data.terminal_id = terminal_id;

        if data.len() > COMPRESS_THRESHOLD {
            let compressed = compress::compress(&data);
            if compressed.len() < data.len() {
                terminal_data.data = bytes::Bytes::from(compressed);
                terminal_data.compressed = true;
            } else {
                terminal_data.data = bytes::Bytes::from(data);
            }
        } else {
            terminal_data.data = bytes::Bytes::from(data);
        }

        response.set_data(terminal_data);
        response
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

                // Always drain the output channel regardless of session state.
                // When Active: data is sent to client. When Closed (within the same
                // connection): data is buffered in output_buffer for reconnection replay.
                // Note: during actual disconnect, the run loop exits and read_outputs()
                // is not called, so channel data produced after disconnect may be lost.
                let mut has_activity = false;
                let mut received_data = Vec::new();
                if let Some(output_rx) = &session.output_rx {
                    // Try to read all available data
                    while let Ok(data) = output_rx.try_recv() {
                        has_activity = true;
                        received_data.push(data);
                    }
                }

                if has_activity {
                    session.update_activity();
                }

                // Update buffer (always buffer for reconnection support)
                for data in &received_data {
                    session.output_buffer.append(data);
                }

                // Skip sending responses if session is not Active.
                // Data is already buffered above and will be sent on next reconnection.
                // Use a scoped block to limit the mutable borrow of session.state,
                // so we can immutably borrow other session fields afterwards.
                let sigwinch_action = {
                    let (pending_buffer, sigwinch) = match &mut session.state {
                        SessionState::Active {
                            pending_buffer,
                            sigwinch,
                        } => (pending_buffer, sigwinch),
                        _ => continue,
                    };

                    // Send pending buffer response first (set on reconnection in handle_open).
                    // This ensures historical buffer is sent before any real-time data.
                    if let Some(buffer) = pending_buffer.take() {
                        if !buffer.is_empty() {
                            responses
                                .push(Self::create_terminal_data_response(terminal_id, buffer));
                        }
                    }

                    // Two-phase SIGWINCH: see SigwinchPhase doc comments for rationale.
                    // Each phase is a single PTY resize, spaced ~30ms apart by the polling
                    // interval, ensuring the TUI app sees a real size change on each signal.
                    match sigwinch {
                        SigwinchPhase::TempResize { retries } => {
                            if *retries == 0 {
                                log::warn!(
                                    "Terminal {} SIGWINCH phase 1 (temp resize) failed after {} attempts, giving up",
                                    terminal_id, MAX_SIGWINCH_PHASE_ATTEMPTS
                                );
                                *sigwinch = SigwinchPhase::Idle;
                                None
                            } else {
                                *retries -= 1;
                                Some(SigwinchAction::TempResize)
                            }
                        }
                        SigwinchPhase::Restore { retries } => {
                            if *retries == 0 {
                                log::warn!(
                                    "Terminal {} SIGWINCH phase 2 (restore) failed after {} attempts, giving up",
                                    terminal_id, MAX_SIGWINCH_PHASE_ATTEMPTS
                                );
                                *sigwinch = SigwinchPhase::Idle;
                                None
                            } else {
                                *retries -= 1;
                                Some(SigwinchAction::Restore)
                            }
                        }
                        SigwinchPhase::Idle => None,
                    }
                };

                // Execute SIGWINCH resize outside the mutable borrow scope of session.state.
                if let Some(action) = sigwinch_action {
                    #[cfg(target_os = "windows")]
                    let is_helper = session.is_helper_mode;
                    #[cfg(not(target_os = "windows"))]
                    let is_helper = false;
                    let resize_ok = Self::do_sigwinch_resize(
                        terminal_id,
                        session.rows,
                        session.cols,
                        &session.pty_pair,
                        &session.input_tx,
                        is_helper,
                        &action,
                    );
                    if let SessionState::Active { sigwinch, .. } = &mut session.state {
                        match action {
                            SigwinchAction::TempResize => {
                                if resize_ok {
                                    // Phase 1 succeeded — advance to phase 2 (restore).
                                    *sigwinch = SigwinchPhase::Restore {
                                        retries: MAX_SIGWINCH_PHASE_ATTEMPTS,
                                    };
                                }
                                // If failed, retries already decremented; will retry phase 1.
                            }
                            SigwinchAction::Restore => {
                                if resize_ok {
                                    // Phase 2 succeeded — SIGWINCH sequence complete.
                                    *sigwinch = SigwinchPhase::Idle;
                                }
                                // If failed, retries already decremented; will retry phase 2.
                            }
                        }
                    }
                }

                // Send real-time data after historical buffer
                for data in received_data {
                    responses.push(Self::create_terminal_data_response(terminal_id, data));
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
