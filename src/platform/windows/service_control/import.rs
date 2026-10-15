use super::{super::read_token_user_buffer, ResultType};
use hbb_common::{anyhow::Context, bail};
use std::{
    ffi::OsStr, fmt, os::windows::ffi::OsStrExt, path::Path, process::Command, time::Duration,
};
use windows::{
    core::{Error, Owned, PCWSTR, PWSTR},
    Win32::{
        Foundation::{
            GetLastError, ERROR_NOT_ALL_ASSIGNED, ERROR_TIMEOUT, HANDLE, WAIT_OBJECT_0,
            WAIT_TIMEOUT,
        },
        Security::{
            AdjustTokenPrivileges, IsWellKnownSid, LookupPrivilegeValueW, WinLocalSystemSid,
            LUID_AND_ATTRIBUTES, SE_DEBUG_NAME, SE_PRIVILEGE_ENABLED, TOKEN_ADJUST_PRIVILEGES,
            TOKEN_ASSIGN_PRIMARY, TOKEN_DUPLICATE, TOKEN_PRIVILEGES, TOKEN_QUERY, TOKEN_USER,
        },
        System::Threading::{
            CreateProcessWithTokenW, GetCurrentProcess, GetExitCodeProcess, OpenProcess,
            OpenProcessToken, TerminateProcess, WaitForSingleObject, CREATE_NO_WINDOW,
            LOGON_WITH_PROFILE, PROCESS_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION,
            STARTUPINFOW,
        },
    },
};

#[derive(Debug)]
pub(super) struct ImportExitUnconfirmed;

impl fmt::Display for ImportExitUnconfirmed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Configuration importer exit is unconfirmed; service remains stopped")
    }
}

impl std::error::Error for ImportExitUnconfirmed {}

pub(super) fn run(config_path: &Path) -> ResultType<()> {
    let mut command = super::system_import_command()?;
    command.arg(config_path);
    let token = system_token()?;
    let process = spawn(&command, *token)?;
    wait_for_import(*process)
}

fn wait_for_import(process: HANDLE) -> ResultType<()> {
    let timeout = super::SERVICE_WAIT_TIMEOUT.as_millis() as u32;
    match unsafe { WaitForSingleObject(process, timeout) } {
        WAIT_OBJECT_0 => {}
        WAIT_TIMEOUT => {
            unsafe { TerminateProcess(process, ERROR_TIMEOUT.0) }.context(ImportExitUnconfirmed)?;
            wait_for_termination(process, super::SERVICE_WAIT_TIMEOUT)?;
            bail!("Configuration import timed out");
        }
        _ => return Err(Error::from_win32()).context(ImportExitUnconfirmed),
    }
    let mut exit_code = 0;
    unsafe { GetExitCodeProcess(process, &mut exit_code) }?;
    if exit_code != 0 {
        bail!("Configuration importer failed with exit code {exit_code}");
    }
    Ok(())
}

fn wait_for_termination(process: HANDLE, timeout: Duration) -> ResultType<()> {
    match unsafe { WaitForSingleObject(process, timeout.as_millis() as u32) } {
        WAIT_OBJECT_0 => Ok(()),
        WAIT_TIMEOUT => Err(ImportExitUnconfirmed.into()),
        _ => Err(Error::from_win32()).context(ImportExitUnconfirmed),
    }
}

fn spawn(command: &Command, token: HANDLE) -> ResultType<Owned<HANDLE>> {
    let executable = wide(command.get_program());
    let directory = wide(
        command
            .get_current_dir()
            .context("Missing import directory")?
            .as_os_str(),
    );
    let mut command_line = std::iter::once(command.get_program())
        .chain(command.get_args())
        .map(quoted)
        .collect::<Result<Vec<_>, _>>()?
        .join(" ")
        .encode_utf16()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let startup = STARTUPINFOW {
        cb: std::mem::size_of::<STARTUPINFOW>() as u32,
        ..Default::default()
    };
    let mut info = PROCESS_INFORMATION::default();
    // A null environment with LOGON_WITH_PROFILE uses the SYSTEM profile,
    // rather than inheriting the elevated user's configuration directory.
    unsafe {
        CreateProcessWithTokenW(
            token,
            LOGON_WITH_PROFILE,
            PCWSTR(executable.as_ptr()),
            Some(PWSTR(command_line.as_mut_ptr())),
            CREATE_NO_WINDOW,
            None,
            PCWSTR(directory.as_ptr()),
            &startup,
            &mut info,
        )?;
    }
    let process = unsafe { Owned::new(info.hProcess) };
    let _thread = unsafe { Owned::new(info.hThread) };
    Ok(process)
}

fn system_token() -> ResultType<Owned<HANDLE>> {
    enable_debug_privilege()?;
    let pid = impersonate_system::get_process_pid("winlogon.exe", false)
        .map_err(|_| hbb_common::anyhow::anyhow!("Cannot find the SYSTEM logon process"))?;
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }?;
    let process = unsafe { Owned::new(process) };
    let mut token = HANDLE::default();
    unsafe {
        OpenProcessToken(
            *process,
            TOKEN_QUERY | TOKEN_DUPLICATE | TOKEN_ASSIGN_PRIMARY,
            &mut token,
        )?;
    }
    let token = unsafe { Owned::new(token) };
    let buffer = unsafe { read_token_user_buffer(*token, "configuration importer") }?;
    let user = unsafe { std::ptr::read_unaligned(buffer.as_ptr().cast::<TOKEN_USER>()) };
    if !unsafe { IsWellKnownSid(user.User.Sid, WinLocalSystemSid) }.as_bool() {
        bail!("Configuration import requires a LocalSystem token");
    }
    Ok(token)
}

fn enable_debug_privilege() -> ResultType<()> {
    let mut token = HANDLE::default();
    unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES, &mut token) }?;
    let token = unsafe { Owned::new(token) };
    let mut privilege = LUID_AND_ATTRIBUTES {
        Attributes: SE_PRIVILEGE_ENABLED,
        ..Default::default()
    };
    unsafe { LookupPrivilegeValueW(None, SE_DEBUG_NAME, &mut privilege.Luid) }?;
    let privileges = TOKEN_PRIVILEGES {
        PrivilegeCount: 1,
        Privileges: [privilege],
    };
    unsafe { AdjustTokenPrivileges(*token, false, Some(&privileges), 0, None, None) }?;
    if unsafe { GetLastError() } == ERROR_NOT_ALL_ASSIGNED {
        bail!("Administrator token does not grant SeDebugPrivilege for configuration import");
    }
    Ok(())
}

fn quoted(value: &OsStr) -> ResultType<String> {
    let value = value
        .to_str()
        .context("Import argument is not valid Unicode")?;
    if value.contains(['\0', '"']) || value.ends_with('\\') {
        bail!("Invalid configuration import argument");
    }
    Ok(format!("\"{value}\""))
}

fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(Some(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        os::windows::{io::AsRawHandle, process::CommandExt},
        process::Stdio,
        thread,
    };

    #[test]
    fn termination_wait_reports_timeout_while_child_is_running() {
        const CHILD_EXIT_DELAY: Duration = Duration::from_millis(250);
        let shell = super::super::super::installer_shell::get_system_executable("cmd.exe").unwrap();
        let mut child = Command::new(shell)
            .args(["/d", "/c", "set /p input="])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .creation_flags(winapi::um::winbase::CREATE_NO_WINDOW)
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let release_input = thread::spawn(move || {
            thread::sleep(CHILD_EXIT_DELAY);
            drop(input);
        });
        let result = wait_for_termination(HANDLE(child.as_raw_handle()), Duration::ZERO);
        release_input.join().unwrap();
        child.wait().unwrap();
        assert!(
            result.unwrap_err().is::<ImportExitUnconfirmed>(),
            "A live importer must time out instead of allowing service startup"
        );
        assert!(wait_for_termination(HANDLE(child.as_raw_handle()), Duration::ZERO).is_ok());
    }

    #[test]
    fn importer_failure_is_reported_after_the_child_exits() {
        let shell = super::super::super::installer_shell::get_system_executable("cmd.exe").unwrap();
        for exit_code in [0, 1] {
            let mut child = Command::new(&shell)
                .args(["/d", "/c", &format!("exit /b {exit_code}")])
                .creation_flags(winapi::um::winbase::CREATE_NO_WINDOW)
                .spawn()
                .unwrap();
            let result = wait_for_import(HANDLE(child.as_raw_handle()));
            assert_eq!(result.is_ok(), exit_code == 0);
            assert_eq!(child.try_wait().unwrap().unwrap().code(), Some(exit_code));
        }
    }
}
