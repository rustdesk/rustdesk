//! Terminal Helper Process
//!
//! This module implements a helper process that runs as the logged-in user and creates
//! the ConPTY + Shell. This is necessary because ConPTY has compatibility issues with
//! CreateProcessAsUserW when the ConPTY is created by a different user (SYSTEM service).
//!
//! Architecture:
//! ```
//! SYSTEM Service (terminal_service.rs)
//!     |
//!     +-- CreateProcessAsUserW --> Terminal Helper (this module, runs as user)
//!     |                                |
//!     |                                +-- CreateProcessW + ConPTY --> Shell
//!     |                                |
//!     +-- Named Pipes <----------------+
//! ```
//!
//! This module also contains Windows-specific utility functions used by terminal_service.rs:
//! - Named pipe creation and connection
//! - User token and SID handling
//! - Helper process launching

use hbb_common::{
    anyhow::{anyhow, Context, Result},
    log,
};
use portable_pty::{CommandBuilder, MasterPty, PtySize};
use std::{
    ffi::{c_void, OsStr},
    fs::File,
    io::{Read, Write},
    os::windows::{ffi::OsStrExt, io::FromRawHandle, raw::HANDLE as RawHandle},
    ptr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

use windows::{
    core::{PCWSTR, PWSTR},
    Win32::{
        Foundation::{
            CloseHandle, LocalFree, ERROR_IO_PENDING, ERROR_PIPE_CONNECTED, HANDLE, HLOCAL,
            INVALID_HANDLE_VALUE, WAIT_OBJECT_0,
        },
        Security::{
            Authorization::{
                SetEntriesInAclW, EXPLICIT_ACCESS_W, SET_ACCESS, TRUSTEE_IS_SID, TRUSTEE_IS_USER,
                TRUSTEE_W,
            },
            CreateWellKnownSid, GetLengthSid, GetTokenInformation, InitializeSecurityDescriptor,
            SetSecurityDescriptorDacl, TokenUser, WinLocalSystemSid, ACE_FLAGS, ACL,
            PSECURITY_DESCRIPTOR, PSID, SECURITY_ATTRIBUTES, TOKEN_USER,
        },
        Storage::FileSystem::{
            CreateFileW, FILE_ALL_ACCESS, FILE_FLAGS_AND_ATTRIBUTES, FILE_FLAG_OVERLAPPED,
            FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_SHARE_READ, FILE_SHARE_WRITE,
            OPEN_EXISTING,
        },
        System::{
            Environment::{CreateEnvironmentBlock, DestroyEnvironmentBlock},
            Pipes::{
                ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE, PIPE_WAIT,
            },
            Threading::{
                CreateEventW, CreateProcessAsUserW, WaitForSingleObject, CREATE_NO_WINDOW,
                CREATE_UNICODE_ENVIRONMENT, PROCESS_CREATION_FLAGS, PROCESS_INFORMATION,
                STARTUPINFOW,
            },
            IO::{GetOverlappedResult, OVERLAPPED},
        },
    },
};

// Re-export types needed by terminal_service.rs
pub use windows::Win32::{
    Foundation::{
        CloseHandle as WinCloseHandle, HANDLE as WinHANDLE, WAIT_OBJECT_0 as WIN_WAIT_OBJECT_0,
    },
    System::Threading::{
        GetExitCodeProcess as WinGetExitCodeProcess, TerminateProcess as WinTerminateProcess,
        WaitForSingleObject as WinWaitForSingleObject,
    },
};

/// User token wrapper for cross-module use.
///
/// Using newtype pattern for type safety. The inner value is `usize` to match
/// platform pointer size (32-bit on x86, 64-bit on x64).
/// Windows HANDLE is defined as `*mut c_void`, which has the same size as `usize`.
///
/// # Design Note
/// This type is defined here (terminal_helper.rs) for Windows and in
/// terminal_service.rs for non-Windows platforms. This avoids circular
/// dependencies while keeping the API consistent across platforms.
/// Both definitions MUST have identical public API (new, as_raw methods).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserToken(pub usize);

impl UserToken {
    /// Create a new UserToken from a raw handle value.
    pub fn new(handle: usize) -> Self {
        Self(handle)
    }

    /// Get the raw handle value.
    pub fn as_raw(&self) -> usize {
        self.0
    }
}

// Windows pipe access mode constants (not exported by windows crate)
const PIPE_ACCESS_INBOUND: u32 = 0x00000001;
const PIPE_ACCESS_OUTBOUND: u32 = 0x00000002;

// Named pipe configuration constants
const PIPE_BUFFER_SIZE: u32 = 65536; // 64KB for better throughput with large terminal output
const PIPE_DEFAULT_TIMEOUT_MS: u32 = 5000;
/// Timeout for waiting for helper process to connect to pipes
pub const PIPE_CONNECTION_TIMEOUT_MS: u32 = 10000;

/// Message type constants for helper protocol.
/// Used to distinguish between terminal data and control commands.
/// Note: Using non-zero values to make debugging easier (0x00 could indicate uninitialized memory).
pub const MSG_TYPE_DATA: u8 = 0x01;
pub const MSG_TYPE_RESIZE: u8 = 0x02;

/// Message header size: 1 byte type + 4 bytes length
pub const MSG_HEADER_SIZE: usize = 5;

/// Maximum payload size to prevent denial of service from malicious messages.
/// 16MB should be more than enough for any legitimate terminal data.
const MAX_PAYLOAD_SIZE: usize = 16 * 1024 * 1024;

/// Timeout in milliseconds to wait for helper process to exit gracefully before force termination.
/// Using 500ms to allow helper process enough time to clean up, especially under high system load.
pub const HELPER_GRACEFUL_EXIT_TIMEOUT_MS: u64 = 500;

/// Information about a launched helper process.
/// Contains both the process handle and PID for tracking and status checks.
#[derive(Debug)]
pub struct HelperProcessInfo {
    /// Process handle for termination and waiting
    pub handle: HANDLE,
    /// Process ID for logging and status display
    pub pid: u32,
}

/// Wrapper for Windows HANDLE that implements Send.
/// This is safe because Windows HANDLEs are valid across threads.
/// Note: We only implement Send, not Sync. The handle is protected by
/// Mutex in TerminalSession, so concurrent access is controlled there.
///
/// # Ownership and Cleanup
/// This type intentionally does NOT implement Drop. The handle is owned by
/// `TerminalSession` and explicitly closed in `TerminalSession::close_internal()`
/// after graceful shutdown logic (waiting for helper to exit, force termination if needed).
/// Implementing Drop here would interfere with that cleanup sequence.
#[derive(Debug)]
pub struct SendableHandle(HANDLE);

impl SendableHandle {
    /// Create a new SendableHandle from a raw HANDLE.
    pub fn new(handle: HANDLE) -> Self {
        Self(handle)
    }

    /// Get the raw HANDLE value.
    pub fn as_raw(&self) -> HANDLE {
        self.0
    }
}

unsafe impl Send for SendableHandle {}

/// RAII wrapper for Windows HANDLE that automatically closes the handle on drop.
/// This ensures proper resource cleanup even when errors occur or code paths diverge.
pub struct OwnedHandle(HANDLE);

impl OwnedHandle {
    /// Create a new OwnedHandle from a raw HANDLE.
    /// The handle will be closed when this OwnedHandle is dropped.
    pub fn new(handle: HANDLE) -> Self {
        Self(handle)
    }

    /// Consume the OwnedHandle and return the raw HANDLE without closing it.
    /// Use this when transferring ownership to another resource (e.g., File).
    pub fn into_raw(self) -> HANDLE {
        let handle = self.0;
        std::mem::forget(self); // Prevent Drop from closing the handle
        handle
    }

    /// Get the raw HANDLE value.
    pub fn as_raw(&self) -> HANDLE {
        self.0
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if self.0 != INVALID_HANDLE_VALUE && !self.0.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

/// RAII guard for helper process that terminates the process on drop.
/// This prevents helper process leaks when pipe connection fails or other errors occur.
///
/// Unlike OwnedHandle (which only closes the handle), this guard:
/// 1. Terminates the process using TerminateProcess
/// 2. Then closes the handle
///
/// Use `disarm()` to prevent termination when the helper is successfully handed off
/// to the terminal session for proper lifecycle management.
pub struct HelperProcessGuard {
    handle: HANDLE,
    pid: u32,
    armed: bool,
}

impl HelperProcessGuard {
    /// Create a new guard for a helper process.
    pub fn new(handle: HANDLE, pid: u32) -> Self {
        Self {
            handle,
            pid,
            armed: true,
        }
    }

    /// Get the raw process HANDLE.
    pub fn as_raw(&self) -> HANDLE {
        self.handle
    }

    /// Get the process ID.
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// Disarm the guard and return the raw HANDLE.
    /// After calling this, the guard will NOT terminate the process on drop.
    /// Use this when successfully handing off the helper to session management.
    pub fn disarm(self) -> HANDLE {
        let handle = self.handle;
        std::mem::forget(self); // Prevent Drop from running
        handle
    }
}

impl Drop for HelperProcessGuard {
    fn drop(&mut self) {
        if self.armed && self.handle != INVALID_HANDLE_VALUE && !self.handle.is_invalid() {
            log::warn!(
                "HelperProcessGuard: terminating leaked helper process (PID {})",
                self.pid
            );
            unsafe {
                // Terminate the process first
                let _ = WinTerminateProcess(self.handle, 1);
                // Then close the handle
                let _ = CloseHandle(self.handle);
            }
        }
    }
}

/// Encode a message for the helper protocol.
/// Format: [type: u8][length: u32 LE][payload: bytes]
pub fn encode_helper_message(msg_type: u8, payload: &[u8]) -> Vec<u8> {
    let mut msg = Vec::with_capacity(MSG_HEADER_SIZE + payload.len());
    msg.push(msg_type);
    msg.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    msg.extend_from_slice(payload);
    msg
}

/// Encode a resize message for the helper protocol.
/// Payload: rows (u16 LE) + cols (u16 LE)
pub fn encode_resize_message(rows: u16, cols: u16) -> Vec<u8> {
    let mut payload = Vec::with_capacity(4);
    payload.extend_from_slice(&rows.to_le_bytes());
    payload.extend_from_slice(&cols.to_le_bytes());
    encode_helper_message(MSG_TYPE_RESIZE, &payload)
}

/// Get the default shell for Windows.
pub fn get_default_shell() -> String {
    // Try PowerShell Core first (absolute paths only)
    let pwsh_paths = [
        "pwsh.exe",
        r"C:\Program Files\PowerShell\7\pwsh.exe",
        r"C:\Program Files\PowerShell\6\pwsh.exe",
    ];

    for path in &pwsh_paths {
        if std::path::Path::new(path).exists() {
            log::debug!("Found PowerShell Core: {}", path);
            return path.to_string();
        }
    }

    // Try Windows PowerShell
    let powershell_path = r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe";
    if std::path::Path::new(powershell_path).exists() {
        return powershell_path.to_string();
    }

    // Fallback to cmd.exe
    std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string())
}

/// Get the SID of the user from a token.
/// Returns a Vec<u8> containing the SID bytes.
pub fn get_user_sid_from_token(user_token: UserToken) -> Result<Vec<u8>> {
    let token_handle = HANDLE(user_token.as_raw() as _);

    // First call to get required buffer size
    let mut return_length = 0u32;
    let _ = unsafe { GetTokenInformation(token_handle, TokenUser, None, 0, &mut return_length) };

    if return_length == 0 {
        return Err(anyhow!(
            "Failed to get token information size: {}",
            std::io::Error::last_os_error()
        ));
    }

    // Allocate buffer and get token information
    let mut buffer = vec![0u8; return_length as usize];
    unsafe {
        GetTokenInformation(
            token_handle,
            TokenUser,
            Some(buffer.as_mut_ptr() as *mut c_void),
            return_length,
            &mut return_length,
        )
        .map_err(|e| anyhow!("Failed to get token information: {}", e))?;
    }

    // Extract SID from TOKEN_USER structure
    let token_user = unsafe { &*(buffer.as_ptr() as *const TOKEN_USER) };
    let sid_ptr = token_user.User.Sid;

    // Get SID length and copy to owned buffer
    let sid_length = unsafe { GetLengthSid(sid_ptr) };

    if sid_length == 0 {
        return Err(anyhow!("Invalid SID length"));
    }

    let mut sid_buffer = vec![0u8; sid_length as usize];
    unsafe {
        ptr::copy_nonoverlapping(
            sid_ptr.0 as *const u8,
            sid_buffer.as_mut_ptr(),
            sid_length as usize,
        );
    }

    Ok(sid_buffer)
}

/// Create a restricted DACL that only allows SYSTEM and a specific user.
/// Returns a pointer to the ACL that must be freed with LocalFree.
///
/// # Safety
///
/// This function is safe to call, but contains internal unsafe code that relies on
/// pointer lifetime guarantees:
///
/// - The `user_sid` slice must contain valid SID binary data.
/// - Internally, raw pointers to `system_sid_buffer` (stack-allocated) and `user_sid`
///   are stored in `TRUSTEE_W.ptstrName` fields. These pointers are only used during
///   the `SetEntriesInAclW` call, which occurs before either buffer goes out of scope.
/// - The returned ACL pointer is allocated by Windows and must be freed with `LocalFree`.
pub fn create_restricted_dacl(user_sid: &[u8]) -> Result<*mut c_void> {
    // Create SYSTEM SID (well-known SID: S-1-5-18)
    // SAFETY: This buffer must outlive the TRUSTEE_W structures that reference it
    let mut system_sid_buffer = vec![0u8; 64]; // Max SID size
    let mut system_sid_size = system_sid_buffer.len() as u32;
    unsafe {
        CreateWellKnownSid(
            WinLocalSystemSid,
            None, // No domain SID
            Some(PSID(system_sid_buffer.as_mut_ptr() as *mut c_void)),
            &mut system_sid_size,
        )
        .map_err(|e| anyhow!("Failed to create SYSTEM SID: {}", e))?;
    }

    // Build EXPLICIT_ACCESS entries for SYSTEM and user
    // SAFETY: The ptstrName pointers below reference system_sid_buffer and user_sid.
    // These buffers must remain valid until SetEntriesInAclW returns.
    let mut explicit_access: [EXPLICIT_ACCESS_W; 2] = unsafe { std::mem::zeroed() };

    // Entry 0: SYSTEM - full access
    explicit_access[0].grfAccessPermissions = FILE_ALL_ACCESS.0;
    explicit_access[0].grfAccessMode = SET_ACCESS;
    explicit_access[0].grfInheritance = ACE_FLAGS(0); // No inheritance for pipes
    explicit_access[0].Trustee = TRUSTEE_W {
        pMultipleTrustee: ptr::null_mut(),
        MultipleTrusteeOperation: Default::default(),
        TrusteeForm: TRUSTEE_IS_SID,
        TrusteeType: TRUSTEE_IS_USER,
        ptstrName: PWSTR::from_raw(system_sid_buffer.as_ptr() as *mut u16),
    };

    // Entry 1: User - full access
    explicit_access[1].grfAccessPermissions = FILE_ALL_ACCESS.0;
    explicit_access[1].grfAccessMode = SET_ACCESS;
    explicit_access[1].grfInheritance = ACE_FLAGS(0); // No inheritance for pipes
                                                      // SAFETY: When TrusteeForm is TRUSTEE_IS_SID, ptstrName is interpreted as a PSID
                                                      // pointer, not a string pointer. The Windows API reuses this field for different
                                                      // purposes based on TrusteeForm. The SID binary data in user_sid is valid for
                                                      // the duration of this function call (until SetEntriesInAclW returns).
    explicit_access[1].Trustee = TRUSTEE_W {
        pMultipleTrustee: ptr::null_mut(),
        MultipleTrusteeOperation: Default::default(),
        TrusteeForm: TRUSTEE_IS_SID,
        TrusteeType: TRUSTEE_IS_USER,
        ptstrName: PWSTR::from_raw(user_sid.as_ptr() as *mut u16),
    };

    // Create ACL from explicit access entries
    // After this call returns, system_sid_buffer and user_sid are no longer needed
    let mut new_acl: *mut ACL = ptr::null_mut();
    let result = unsafe {
        SetEntriesInAclW(
            Some(&explicit_access),
            None, // No existing ACL
            &mut new_acl,
        )
    };

    if result.0 != 0 {
        return Err(anyhow!(
            "SetEntriesInAclW failed with error code: {}",
            result.0
        ));
    }

    if new_acl.is_null() {
        return Err(anyhow!("SetEntriesInAclW returned null ACL"));
    }

    Ok(new_acl as *mut c_void)
}

/// Create a named pipe with a restricted DACL.
/// Only SYSTEM and the specified user can access the pipe.
///
/// # Arguments
/// * `pipe_name` - The name of the pipe to create
/// * `for_input` - True if service writes to this pipe (helper reads), false otherwise
/// * `user_token` - Required user token for creating restricted DACL
///
/// # Security
///
/// The restricted DACL limits pipe access to:
/// - SYSTEM account (the service)
/// - The specific user whose token was provided (the helper process)
///
/// This function requires a valid user_token and will fail if DACL creation fails,
/// rather than falling back to a less secure NULL DACL.
pub fn create_named_pipe_server(
    pipe_name: &str,
    for_input: bool,
    user_token: UserToken,
) -> Result<HANDLE> {
    // SECURITY_DESCRIPTOR minimum length is 40 bytes on x64.
    const SD_BUFFER_SIZE: usize = 64;
    const _: () = assert!(
        SD_BUFFER_SIZE >= 40,
        "SD_BUFFER_SIZE must be at least 40 bytes for SECURITY_DESCRIPTOR"
    );

    let mut sd_buffer = [0u8; SD_BUFFER_SIZE];
    let sd_ptr = PSECURITY_DESCRIPTOR(sd_buffer.as_mut_ptr() as *mut c_void);

    // Initialize security descriptor
    unsafe {
        InitializeSecurityDescriptor(sd_ptr, 1)
            .map_err(|e| anyhow!("Failed to initialize security descriptor: {}", e))?;
    }

    // Create restricted DACL - fail if this doesn't work (no NULL DACL fallback)
    let user_sid = get_user_sid_from_token(user_token)
        .context("Failed to get user SID from token for pipe DACL")?;
    let acl_ptr =
        create_restricted_dacl(&user_sid).context("Failed to create restricted DACL for pipe")?;

    log::debug!("Created restricted DACL for pipe: {}", pipe_name);

    // Set DACL on security descriptor
    unsafe {
        SetSecurityDescriptorDacl(sd_ptr, true, Some(acl_ptr as *const _ as *const _), false)
            .map_err(|e| {
                // Clean up ACL on error (ignore result - cleanup is best-effort, original error takes precedence)
                let _ = LocalFree(Some(HLOCAL(acl_ptr)));
                anyhow!("Failed to set restricted DACL: {}", e)
            })?;
    }

    let sa = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: sd_buffer.as_mut_ptr() as *mut c_void,
        bInheritHandle: false.into(),
    };

    let wide_name: Vec<u16> = OsStr::new(pipe_name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let access_mode = if for_input {
        FILE_FLAGS_AND_ATTRIBUTES(PIPE_ACCESS_INBOUND | FILE_FLAG_OVERLAPPED.0)
    } else {
        FILE_FLAGS_AND_ATTRIBUTES(PIPE_ACCESS_OUTBOUND | FILE_FLAG_OVERLAPPED.0)
    };

    log::debug!(
        "Creating named pipe: {} (for_input={}, restricted_dacl=true)",
        pipe_name,
        for_input
    );

    let handle = unsafe {
        CreateNamedPipeW(
            PCWSTR::from_raw(wide_name.as_ptr()),
            access_mode,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
            1, // max instances
            PIPE_BUFFER_SIZE,
            PIPE_BUFFER_SIZE,
            PIPE_DEFAULT_TIMEOUT_MS,
            Some(&sa),
        )
    };

    // Clean up ACL after pipe creation (security descriptor has been applied)
    // Ignore result: LocalFree failure is non-critical since the pipe is already created
    unsafe {
        let _ = LocalFree(Some(HLOCAL(acl_ptr)));
    }

    if handle == INVALID_HANDLE_VALUE {
        return Err(anyhow!(
            "Failed to create named pipe {}: {}",
            pipe_name,
            std::io::Error::last_os_error()
        ));
    }

    log::debug!("Named pipe created: {}", pipe_name);
    Ok(handle)
}

/// Wait for client to connect to named pipe with timeout.
///
/// # Ownership
/// This function **takes ownership** of the `pipe_handle` via OwnedHandle:
/// - On success: the handle is extracted and wrapped in a `File`.
/// - On failure: the handle is automatically closed when OwnedHandle drops.
pub fn wait_for_pipe_connection(
    pipe_handle: OwnedHandle,
    pipe_name: &str,
    timeout_ms: u32,
) -> Result<File> {
    log::debug!("Waiting for pipe connection: {}", pipe_name);

    // Create an event for overlapped I/O (also wrapped in OwnedHandle for RAII)
    let event = unsafe { CreateEventW(None, true, false, PCWSTR::null()) }
        .map_err(|e| anyhow!("Failed to create event for pipe connection: {}", e))?;
    let event_handle = OwnedHandle::new(event);

    let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
    overlapped.hEvent = event_handle.as_raw();

    let result = unsafe { ConnectNamedPipe(pipe_handle.as_raw(), Some(&mut overlapped)) };
    if result.is_err() {
        let err = std::io::Error::last_os_error();
        let err_code = err.raw_os_error().unwrap_or(0);

        // ERROR_PIPE_CONNECTED means client already connected, which is OK
        if err_code == ERROR_PIPE_CONNECTED.0 as i32 {
            log::debug!("Pipe already connected: {}", pipe_name);
            return Ok(unsafe { File::from_raw_handle(pipe_handle.into_raw().0 as RawHandle) });
        }

        // ERROR_IO_PENDING means we need to wait
        if err_code == ERROR_IO_PENDING.0 as i32 {
            log::debug!("Pipe connection pending, waiting with timeout...");
            let wait_result = unsafe { WaitForSingleObject(event_handle.as_raw(), timeout_ms) };

            if wait_result != WAIT_OBJECT_0 {
                log::error!("Timeout waiting for pipe connection: {}", pipe_name);
                return Err(anyhow!(
                    "Timeout waiting for pipe connection: {}",
                    pipe_name
                ));
            }

            // Check if connection was successful
            let mut bytes_transferred = 0u32;
            let overlapped_result = unsafe {
                GetOverlappedResult(
                    pipe_handle.as_raw(),
                    &overlapped,
                    &mut bytes_transferred,
                    false,
                )
            };
            if overlapped_result.is_err() {
                let err = std::io::Error::last_os_error();
                log::error!("Failed to complete pipe connection {}: {}", pipe_name, err);
                return Err(anyhow!(
                    "Failed to complete pipe connection {}: {}",
                    pipe_name,
                    err
                ));
            }

            log::debug!("Pipe connected: {}", pipe_name);
        } else {
            log::error!("Failed to connect named pipe {}: {}", pipe_name, err);
            return Err(anyhow!(
                "Failed to connect named pipe {}: {}",
                pipe_name,
                err
            ));
        }
    } else {
        log::debug!("Pipe connected immediately: {}", pipe_name);
    }

    // Success: transfer pipe ownership to File, event_handle drops
    Ok(unsafe { File::from_raw_handle(pipe_handle.into_raw().0 as RawHandle) })
}

/// Launch terminal helper process as the logged-in user using the provided token.
/// The helper process creates ConPTY and shell, communicating via named pipes.
/// This uses CreateProcessAsUserW directly with the user token, which works because
/// the helper process itself doesn't need ConPTY - it creates ConPTY internally.
///
/// Returns HelperProcessInfo containing the process handle and PID.

/// RAII guard for environment block cleanup.
/// Ensures DestroyEnvironmentBlock is called even if an error occurs.
struct EnvironmentBlockGuard {
    ptr: *mut c_void,
}

impl Drop for EnvironmentBlockGuard {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                // Ignore result: DestroyEnvironmentBlock failure is non-critical during cleanup
                let _ = DestroyEnvironmentBlock(self.ptr);
            }
        }
    }
}

pub fn launch_terminal_helper_with_token(
    user_token: UserToken,
    input_pipe_name: &str,
    output_pipe_name: &str,
    terminal_id: i32,
    rows: u16,
    cols: u16,
) -> Result<HelperProcessInfo> {
    let exe_path =
        std::env::current_exe().map_err(|e| anyhow!("Failed to get current exe path: {}", e))?;

    // Build command line arguments (without exe path to avoid escaping issues)
    // lpApplicationName will contain the exe path separately
    let cmd_args = format!(
        "--terminal-helper {} {} {} {} {}",
        input_pipe_name, output_pipe_name, rows, cols, terminal_id
    );

    log::debug!("Launching terminal helper for terminal {}", terminal_id);

    // Convert exe path to wide string for lpApplicationName
    let exe_path_wide: Vec<u16> = OsStr::new(exe_path.as_os_str())
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // Command line must include exe name as first argument per Windows convention
    let cmd_line = format!("\"{}\" {}", exe_path.display(), cmd_args);
    let mut cmd_wide: Vec<u16> = OsStr::new(&cmd_line)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let mut si: STARTUPINFOW = unsafe { std::mem::zeroed() };
    si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;

    let mut pi: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };

    // Create environment block for the user with RAII cleanup
    let mut environment: *mut c_void = ptr::null_mut();
    let env_ok = unsafe {
        CreateEnvironmentBlock(
            &mut environment,
            Some(HANDLE(user_token.as_raw() as _)),
            true,
        )
    }
    .is_ok();

    // Use RAII guard to ensure cleanup even on error paths
    let _env_guard = if env_ok && !environment.is_null() {
        Some(EnvironmentBlockGuard { ptr: environment })
    } else {
        if !env_ok {
            log::warn!("Failed to create environment block, using default");
        }
        None
    };

    let creation_flags = CREATE_NO_WINDOW
        | if env_ok {
            CREATE_UNICODE_ENVIRONMENT
        } else {
            PROCESS_CREATION_FLAGS(0)
        };

    // Use lpApplicationName to pass exe path separately from command line
    // This avoids potential issues with special characters in the exe path
    let result = unsafe {
        CreateProcessAsUserW(
            Some(HANDLE(user_token.as_raw() as _)),
            PCWSTR::from_raw(exe_path_wide.as_ptr()), // lpApplicationName: exe path
            Some(PWSTR::from_raw(cmd_wide.as_mut_ptr())), // lpCommandLine: full command
            None,
            None,
            false, // Don't inherit handles
            creation_flags,
            if env_ok { Some(environment) } else { None },
            PCWSTR::null(), // Use default current directory
            &si,
            &mut pi,
        )
    };

    // Environment block cleanup is handled by _env_guard's Drop

    if let Err(e) = result {
        log::error!("CreateProcessAsUserW failed: {}", e);
        return Err(anyhow!("Failed to launch terminal helper: {}", e));
    }

    // Close thread handle - we only need the process handle for tracking
    // Ignore result: CloseHandle failure here is non-critical since process is already launched
    unsafe {
        let _ = CloseHandle(pi.hThread);
    }

    log::info!("Terminal helper launched with PID {}", pi.dwProcessId);
    // Return process info for tracking
    Ok(HelperProcessInfo {
        handle: pi.hProcess,
        pid: pi.dwProcessId,
    })
}

/// Check if a helper process is still running.
/// Returns true if the process is running, false if it has exited.
pub fn is_helper_process_running(handle: HANDLE) -> bool {
    let wait_result = unsafe { WaitForSingleObject(handle, 0) };
    // WAIT_TIMEOUT (258) means process is still running
    // WAIT_OBJECT_0 (0) means process has exited
    wait_result != WAIT_OBJECT_0
}

/// Run terminal helper process
/// Args: --terminal-helper <input_pipe_name> <output_pipe_name> <rows> <cols> <terminal_id>
pub fn run_terminal_helper(args: &[String]) -> Result<()> {
    if args.len() < 5 {
        return Err(anyhow!(
            "Usage: --terminal-helper <input_pipe> <output_pipe> <rows> <cols> <terminal_id>"
        ));
    }

    let input_pipe_name = &args[0];
    let output_pipe_name = &args[1];
    let rows: u16 = args[2]
        .parse()
        .map_err(|e| anyhow!("Failed to parse rows '{}': {}", args[2], e))?;
    let cols: u16 = args[3]
        .parse()
        .map_err(|e| anyhow!("Failed to parse cols '{}': {}", args[3], e))?;
    let terminal_id: i32 = args[4]
        .parse()
        .map_err(|e| anyhow!("Failed to parse terminal_id '{}': {}", args[4], e))?;

    log::debug!(
        "Terminal helper starting: terminal_id={}, size={}x{}",
        terminal_id,
        cols,
        rows
    );

    // Open named pipes (created by the service)
    let mut input_pipe = open_pipe(input_pipe_name, true)?;
    let mut output_pipe = open_pipe(output_pipe_name, false)?;

    // Create ConPTY and shell
    let pty_size = PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    };

    let pty_system = portable_pty::native_pty_system();
    let pty_pair = pty_system.openpty(pty_size).context("Failed to open PTY")?;

    let shell = get_default_shell();
    log::debug!("Using shell: {}", shell);

    let cmd = CommandBuilder::new(&shell);
    let mut child = pty_pair
        .slave
        .spawn_command(cmd)
        .context("Failed to spawn shell")?;

    // Explicitly drop slave after spawning to release resources
    drop(pty_pair.slave);

    let pid = child.process_id().unwrap_or(0);
    log::debug!("Shell started with PID: {}", pid);

    let mut pty_writer = pty_pair
        .master
        .take_writer()
        .context("Failed to get PTY writer")?;

    let mut pty_reader = pty_pair
        .master
        .try_clone_reader()
        .context("Failed to get PTY reader")?;

    // Wrap pty_pair.master in Arc<Mutex> for sharing with input thread (for resize).
    let pty_master: Arc<Mutex<Box<dyn MasterPty + Send>>> = Arc::new(Mutex::new(pty_pair.master));

    let exiting = Arc::new(AtomicBool::new(false));

    // Thread: Read from input pipe, parse messages, write data to PTY or handle control commands
    let exiting_clone = exiting.clone();
    let pty_master_clone = pty_master.clone();
    let input_thread = thread::spawn(move || {
        let mut input_pipe = input_pipe;
        let mut header_buf = [0u8; MSG_HEADER_SIZE];
        let mut payload_buf = vec![0u8; 4096];

        loop {
            if exiting_clone.load(Ordering::SeqCst) {
                break;
            }

            // Read message header
            match read_exact_or_eof(&mut input_pipe, &mut header_buf) {
                Ok(false) => {
                    log::debug!("Input pipe EOF");
                    break;
                }
                Ok(true) => {}
                Err(e) => {
                    log::error!("Input pipe header read error: {}", e);
                    break;
                }
            }

            let msg_type = header_buf[0];
            let payload_len =
                u32::from_le_bytes([header_buf[1], header_buf[2], header_buf[3], header_buf[4]])
                    as usize;

            // Validate payload length to prevent denial of service
            if payload_len > MAX_PAYLOAD_SIZE {
                log::error!(
                    "Payload too large: {} bytes (max {})",
                    payload_len,
                    MAX_PAYLOAD_SIZE
                );
                break;
            }

            // Ensure payload buffer is large enough
            if payload_buf.len() < payload_len {
                payload_buf.resize(payload_len, 0);
            }

            // Read payload
            if payload_len > 0 {
                match read_exact_or_eof(&mut input_pipe, &mut payload_buf[..payload_len]) {
                    Ok(false) => {
                        log::debug!("Input pipe EOF during payload read");
                        break;
                    }
                    Ok(true) => {}
                    Err(e) => {
                        log::error!("Input pipe payload read error: {}", e);
                        break;
                    }
                }
            }

            match msg_type {
                MSG_TYPE_DATA => {
                    // Write terminal data to PTY
                    if let Err(e) = pty_writer.write_all(&payload_buf[..payload_len]) {
                        log::error!("PTY write error: {}", e);
                        break;
                    }
                    if let Err(e) = pty_writer.flush() {
                        log::error!("PTY flush error: {}", e);
                        break;
                    }
                }
                MSG_TYPE_RESIZE => {
                    if payload_len >= 4 {
                        let rows = u16::from_le_bytes([payload_buf[0], payload_buf[1]]);
                        let cols = u16::from_le_bytes([payload_buf[2], payload_buf[3]]);
                        log::debug!("Resize: {}x{}", cols, rows);
                        if let Ok(master) = pty_master_clone.lock() {
                            let _ = master.resize(PtySize {
                                rows,
                                cols,
                                pixel_width: 0,
                                pixel_height: 0,
                            });
                        }
                    }
                }
                _ => {
                    // Unknown type may indicate data corruption - stop to avoid parse errors
                    log::error!("Unknown message type: {}, terminating", msg_type);
                    break;
                }
            }
        }
        log::debug!("Input thread exiting");
    });

    // Thread: Read from PTY, write to output pipe
    let exiting_clone = exiting.clone();
    let output_thread = thread::spawn(move || {
        let mut output_pipe = output_pipe;
        let mut buf = vec![0u8; 4096];
        loop {
            if exiting_clone.load(Ordering::SeqCst) {
                break;
            }
            match pty_reader.read(&mut buf) {
                Ok(0) => {
                    log::debug!("PTY EOF");
                    break;
                }
                Ok(n) => {
                    if let Err(e) = output_pipe.write_all(&buf[..n]) {
                        log::error!("Output pipe write error: {}", e);
                        break;
                    }
                    if let Err(e) = output_pipe.flush() {
                        log::error!("Output pipe flush error: {}", e);
                        break;
                    }
                }
                Err(e) => {
                    if e.kind() != std::io::ErrorKind::WouldBlock {
                        log::error!("PTY read error: {}", e);
                        break;
                    }
                    thread::sleep(Duration::from_millis(10));
                }
            }
        }
        log::debug!("Output thread exiting");
    });

    // Wait for child process to exit
    let exit_status = child.wait();
    log::info!("Shell exited: {:?}", exit_status);

    exiting.store(true, Ordering::SeqCst);

    // Wait for threads
    let _ = input_thread.join();
    let _ = output_thread.join();

    // pty_master will be dropped here, releasing PTY resources
    drop(pty_master);

    log::info!("Terminal helper exiting");
    Ok(())
}

/// Read exactly `buf.len()` bytes from reader.
/// Returns Ok(true) if successful, Ok(false) on EOF, Err on error.
fn read_exact_or_eof<R: Read>(reader: &mut R, buf: &mut [u8]) -> std::io::Result<bool> {
    let mut pos = 0;
    while pos < buf.len() {
        match reader.read(&mut buf[pos..]) {
            Ok(0) => return Ok(false), // EOF
            Ok(n) => pos += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(true)
}

/// Open a named pipe as a client.
/// `for_read`: true for reading (input pipe), false for writing (output pipe).
fn open_pipe(pipe_name: &str, for_read: bool) -> Result<File> {
    let wide_name: Vec<u16> = OsStr::new(pipe_name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let access = if for_read {
        FILE_GENERIC_READ.0
    } else {
        FILE_GENERIC_WRITE.0
    };

    let handle = unsafe {
        CreateFileW(
            PCWSTR::from_raw(wide_name.as_ptr()),
            access,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )
    };

    match handle {
        Ok(h) => Ok(unsafe { File::from_raw_handle(h.0 as _) }),
        Err(e) => Err(anyhow!(
            "Failed to open {} pipe '{}': {}",
            if for_read { "input" } else { "output" },
            pipe_name,
            e
        )),
    }
}
