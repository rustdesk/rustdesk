use crate::platform::windows as platform;
use hbb_common::{anyhow::Context, bail, config, tokio::time::sleep, ResultType};
use parity_tokio_ipc::{ConnectionClient, SecurityAttributes};
use std::{
    ffi::c_void,
    fs::OpenOptions,
    io,
    os::windows::{
        fs::OpenOptionsExt,
        io::{AsRawHandle, IntoRawHandle},
    },
    time::Duration,
};
use winapi::um::winnt::ACCESS_ALLOWED_ACE_TYPE;
use windows::{
    core::PWSTR,
    Win32::{
        Foundation::{LocalFree, ERROR_PIPE_BUSY, HANDLE, HLOCAL},
        Security::{
            Authorization::{ConvertSidToStringSidW, GetSecurityInfo, SE_FILE_OBJECT},
            EqualSid, GetAce, IsWellKnownSid, WinLocalSystemSid, ACCESS_ALLOWED_ACE, ACE_HEADER,
            ACL, DACL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID,
        },
        Storage::FileSystem::{
            FILE_FLAG_OVERLAPPED, FILE_GENERIC_READ, FILE_WRITE_DATA, SECURITY_IDENTIFICATION,
        },
        System::Pipes::{GetNamedPipeServerProcessId, GetNamedPipeServerSessionId},
    },
};

// FILE_GENERIC_WRITE includes FILE_CREATE_PIPE_INSTANCE, so clients request only data writes.
const CLIENT_ACCESS: u32 = FILE_GENERIC_READ.0 | FILE_WRITE_DATA.0;
const PIPE_BUSY_RETRY: Duration = Duration::from_millis(50);
const AUTH_LOG_INTERVAL: Duration = Duration::from_secs(5);
const SYSTEM_SID: &str = "S-1-5-18";

struct LocalAllocation(*mut c_void);

impl Drop for LocalAllocation {
    fn drop(&mut self) {
        unsafe { LocalFree(Some(HLOCAL(self.0))) };
    }
}

pub(super) fn listener_security_attributes() -> io::Result<SecurityAttributes> {
    let owner = platform::current_process_user_sid_string()
        .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
    let sddl = format!("O:{owner}D:P(A;;GA;;;SY)(A;;GA;;;{owner})(A;;0x{CLIENT_ACCESS:x};;;WD)");
    SecurityAttributes::from_sddl(&sddl)
}

fn open_pipe(path: &str) -> io::Result<ConnectionClient> {
    let file = OpenOptions::new()
        .access_mode(CLIENT_ACCESS)
        .share_mode(0)
        .custom_flags(FILE_FLAG_OVERLAPPED.0)
        .security_qos_flags(SECURITY_IDENTIFICATION.0)
        .open(path)?;
    // Tokio takes ownership of the handle, including when registration fails.
    unsafe { ConnectionClient::from_raw_handle(file.into_raw_handle()) }
}

pub(super) async fn connect(path: &str, postfix: &str) -> ResultType<ConnectionClient> {
    let client = loop {
        match open_pipe(path) {
            Ok(client) => break client,
            Err(error) if error.raw_os_error() == Some(ERROR_PIPE_BUSY.0 as i32) => {
                sleep(PIPE_BUSY_RETRY).await;
            }
            Err(error) => return Err(error.into()),
        }
    };
    // The UI can own the main pipe while an installed service is unavailable.
    if let Err(error) = ensure_server(&client, config::is_service_ipc_postfix(postfix)) {
        hbb_common::throttled_log!(
            AUTH_LOG_INTERVAL,
            warn,
            "Rejected untrusted Windows IPC server at '{}': {}",
            path,
            error
        );
        return Err(error);
    }
    Ok(client)
}

fn ensure_server(client: &ConnectionClient, require_system: bool) -> ResultType<()> {
    let pipe = HANDLE(client.as_raw_handle());
    let mut pid = 0;
    unsafe { GetNamedPipeServerProcessId(pipe, &mut pid) }
        .context("Failed to resolve IPC server process")?;
    if pid == 0 {
        bail!("IPC server process is unavailable");
    }
    if ensure_pipe_security(pipe, require_system)? {
        return Ok(());
    }
    let mut session = 0;
    unsafe { GetNamedPipeServerSessionId(pipe, &mut session) }
        .context("Failed to resolve IPC server session")?;
    if platform::get_current_process_session_id() != Some(session) {
        bail!("IPC server belongs to a different session");
    }
    super::ipc_auth::ensure_peer_executable_matches_current_by_pid_opt(Some(pid), "")?;
    if platform::is_elevated(None)? && !platform::is_elevated(Some(pid))? {
        bail!("IPC server is less privileged than the client");
    }
    Ok(())
}

fn ensure_pipe_security(pipe: HANDLE, require_system: bool) -> ResultType<bool> {
    let mut owner = PSID::default();
    let mut dacl = std::ptr::null_mut();
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    unsafe {
        GetSecurityInfo(
            pipe,
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            Some(&mut owner),
            None,
            Some(&mut dacl),
            None,
            Some(&mut descriptor),
        )
        .ok()
        .context("Failed to read IPC server security descriptor")?;
    }
    let _descriptor = LocalAllocation(descriptor.0);
    if owner.0.is_null() || dacl.is_null() {
        bail!("IPC server has no owner or restricted DACL");
    }
    let owner_string = sid_string(owner)?;
    let system = owner_string == SYSTEM_SID;
    if !system && (require_system || owner_string != platform::current_process_user_sid_string()?) {
        bail!("IPC server is not owned by the expected identity");
    }
    // SYSTEM ownership alone is insufficient: a legacy DACL allows rogue extra instances.
    // Together these checks authenticate installed pipes without opening SYSTEM process tokens.
    ensure_restricted_dacl(dacl, owner)?;
    Ok(system)
}

fn ensure_restricted_dacl(dacl: *const ACL, owner: PSID) -> ResultType<()> {
    for index in 0..unsafe { (*dacl).AceCount } {
        let mut ace = std::ptr::null_mut();
        unsafe { GetAce(dacl, index.into(), &mut ace) }
            .context("Failed to read IPC server access rule")?;
        let header = unsafe { &*(ace as *const ACE_HEADER) };
        if header.AceType != ACCESS_ALLOWED_ACE_TYPE
            || usize::from(header.AceSize) < std::mem::size_of::<ACCESS_ALLOWED_ACE>()
        {
            bail!("Unsupported IPC server access rule");
        }
        let ace = unsafe { &*(ace as *const ACCESS_ALLOWED_ACE) };
        let sid = PSID(std::ptr::addr_of!(ace.SidStart) as *mut c_void);
        let trusted = unsafe {
            EqualSid(sid, owner).is_ok() || IsWellKnownSid(sid, WinLocalSystemSid).as_bool()
        };
        if !trusted && ace.Mask & !CLIENT_ACCESS != 0 {
            bail!("IPC server grants untrusted users more than client access");
        }
    }
    Ok(())
}

fn sid_string(sid: PSID) -> ResultType<String> {
    let mut string = PWSTR::null();
    unsafe { ConvertSidToStringSidW(sid, &mut string) }?;
    let _string = LocalAllocation(string.0.cast());
    Ok(unsafe { string.to_string() }?)
}

#[cfg(test)]
#[path = "windows_tests.rs"]
mod tests;
