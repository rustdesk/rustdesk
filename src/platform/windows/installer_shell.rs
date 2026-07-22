use super::{wide_string, ResultType};
use hbb_common::{
    anyhow::anyhow,
    bail,
    base64::{engine::general_purpose::STANDARD, Engine as _},
    log,
};
use std::{
    ffi::OsString,
    io, mem,
    os::windows::ffi::OsStringExt,
    path::{Path, PathBuf},
};
use windows::{
    core::{Interface, PCWSTR},
    Win32::{
        Foundation::{self, CloseHandle, HANDLE},
        System::{Com, SystemInformation, Threading},
        UI::{
            Shell::{
                self, FOLDERID_ProgramData, FOLDERID_Public, SHGetKnownFolderPath, KF_FLAG_DEFAULT,
            },
            WindowsAndMessaging,
        },
    },
};

pub(super) const CMD_RELATIVE_PATH: &str = "cmd.exe";
pub(super) const BATCH_SHORTCUT_DECODE_FAILURE_EXIT_CODE: u32 = 0x5253_0008;
pub(super) const WIN7_SHELL_EXECUTE_MAX_PARAMETER_CHARS: usize = 2048;
const SHORTCUT_ICON_INDEX: i32 = 0;

pub(super) fn shortcut_bytes(
    target_path: &str,
    arguments: Option<&str>,
    icon_location: Option<&str>,
) -> ResultType<Vec<u8>> {
    let _com = initialize_shell_com()?;
    let link: Shell::IShellLinkW =
        unsafe { Com::CoCreateInstance(&Shell::ShellLink, None, Com::CLSCTX_INPROC_SERVER) }?;
    let target_path = wide_string(target_path);
    unsafe { link.SetPath(PCWSTR(target_path.as_ptr())) }?;
    if let Some(arguments) = arguments {
        let arguments = wide_string(arguments);
        unsafe { link.SetArguments(PCWSTR(arguments.as_ptr())) }?;
    }
    if let Some(icon_location) = icon_location {
        let icon_location = wide_string(icon_location);
        unsafe { link.SetIconLocation(PCWSTR(icon_location.as_ptr()), SHORTCUT_ICON_INDEX) }?;
    }

    let stream = unsafe { Shell::SHCreateMemStream(None) }
        .ok_or_else(|| anyhow!("Failed to create shortcut memory stream"))?;
    let persist: Com::IPersistStream = link.cast()?;
    unsafe { persist.Save(&stream, true) }?;
    let mut stat = Com::STATSTG::default();
    unsafe { stream.Stat(&mut stat, Com::STATFLAG_NONAME) }?;
    let size = usize::try_from(stat.cbSize).map_err(|_| anyhow!("Shortcut data is too large"))?;
    let read_size = u32::try_from(size).map_err(|_| anyhow!("Shortcut data is too large"))?;
    let mut bytes = vec![0; size];
    let mut bytes_read = 0;
    unsafe {
        stream.Seek(0, Com::STREAM_SEEK_SET, None)?;
        stream
            .Read(bytes.as_mut_ptr().cast(), read_size, Some(&mut bytes_read))
            .ok()?;
    }
    if bytes_read != read_size {
        bail!("Failed to read complete shortcut data");
    }
    Ok(bytes)
}

pub(super) fn embedded_shortcut_commands(bytes: Vec<u8>, filename: &str, name: &str) -> String {
    let encoded = STANDARD.encode(bytes);
    let encoded_path = format!("%~f0.{name}.b64");
    format!(
        "> \"{encoded_path}\" echo {encoded}\r\n\
         certutil -f -decode \"{encoded_path}\" \"%RUSTDESK_OUTPUT_DIR%\\{filename}\" > nul || exit /b {BATCH_SHORTCUT_DECODE_FAILURE_EXIT_CODE}"
    )
}

pub(super) fn embedded_tray_shortcut_commands(
    app_name: &str,
    exe: &str,
    icon_location: Option<&str>,
) -> ResultType<String> {
    let filename = format!("{app_name} Tray.lnk");
    Ok(embedded_shortcut_commands(
        shortcut_bytes(exe, Some("--tray"), icon_location)?,
        &filename,
        "tray_shortcut",
    ))
}

pub(super) fn validate_install_value(value: &str) -> ResultType<()> {
    if value.contains(['\0', '"', '%', '\r', '\n', '|', '<', '>']) {
        bail!("Installer path or name contains characters unsafe for cmd.exe");
    }
    Ok(())
}

pub(super) fn get_system_executable(relative_path: &str) -> ResultType<PathBuf> {
    let mut buffer = vec![0u16; Foundation::MAX_PATH as usize];
    let len = unsafe { SystemInformation::GetSystemDirectoryW(Some(&mut buffer)) } as usize;
    if len == 0 {
        return Err(io::Error::last_os_error().into());
    }
    if len >= buffer.len() {
        bail!("Windows system directory path is too long");
    }
    buffer.truncate(len);
    let mut path = PathBuf::from(OsString::from_wide(&buffer));
    path.push(relative_path);
    Ok(path)
}

fn get_known_folder(id: &windows::core::GUID) -> ResultType<PathBuf> {
    let value = unsafe { SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None) }?;
    let path = unsafe { value.to_string() };
    unsafe { Com::CoTaskMemFree(Some(value.0.cast())) };
    Ok(PathBuf::from(path?))
}

// `%VAR%` is expanded before cmd.exe executes even inside a quoted `set` assignment.
// Reject all `%` so a handoff path cannot alter the elevated bootstrap before hash verification.
// https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/set_1
pub(super) fn path_for_cmd_environment(path: &Path) -> ResultType<&str> {
    let value = path
        .to_str()
        .ok_or_else(|| anyhow!("Path is not valid Unicode: {:?}", path))?;
    if value.contains(['\0', '"', '%', '\r', '\n']) {
        bail!("Path is unsafe for an elevated cmd.exe handoff: {:?}", path);
    }
    Ok(value)
}

// Bootstrap assignments are parsed before DisableDelayedExpansion takes effect.
// Escape carets first so `^!` preserves each literal exclamation mark.
pub(super) fn path_for_cmd_assignment(path: &Path) -> ResultType<String> {
    Ok(path_for_cmd_environment(path)?
        .replace('^', "^^")
        .replace('!', "^!"))
}

pub(super) fn trusted_install_environment() -> ResultType<String> {
    let system = get_system_executable("")?;
    let program_data = get_known_folder(&FOLDERID_ProgramData)?;
    let public = get_known_folder(&FOLDERID_Public)?;
    trusted_install_environment_from_paths(&system, &program_data, &public)
}

fn trusted_install_environment_from_paths(
    system: &Path,
    program_data: &Path,
    public: &Path,
) -> ResultType<String> {
    let windows = system
        .parent()
        .ok_or_else(|| anyhow!("System directory has no parent"))?;
    let cmd = system.join(CMD_RELATIVE_PATH);
    // These paths are parsed once from the protected BAT, with delayed expansion disabled.
    let system = path_for_cmd_environment(system)?;
    let windows = path_for_cmd_environment(windows)?;
    let cmd = path_for_cmd_environment(&cmd)?;
    let program_data = path_for_cmd_environment(program_data)?;
    let public = path_for_cmd_environment(public)?;
    Ok(format!(
        "set \"ComSpec={cmd}\" & set \"PATH={system}\" & \
         set \"SystemRoot={windows}\" & set \"WINDIR={windows}\" & \
         set \"ProgramData={program_data}\" & set \"PUBLIC={public}\" & \
         set \"PATHEXT=.COM;.EXE;.BAT;.CMD\" & \
         set \"NoDefaultCurrentDirectoryInExePath=1\""
    ))
}

struct ShellComGuard;

impl Drop for ShellComGuard {
    fn drop(&mut self) {
        unsafe { Com::CoUninitialize() };
    }
}

fn initialize_shell_com() -> ResultType<Option<ShellComGuard>> {
    let result = unsafe {
        Com::CoInitializeEx(
            None,
            Com::COINIT_APARTMENTTHREADED | Com::COINIT_DISABLE_OLE1DDE,
        )
    };
    if result == Foundation::RPC_E_CHANGED_MODE {
        return Ok(None);
    }
    if result.is_err() {
        bail!(
            "Failed to initialize COM: HRESULT 0x{:08X}",
            result.0 as u32
        );
    }
    Ok(Some(ShellComGuard))
}

struct ProcessHandle(HANDLE);

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        if let Err(err) = unsafe { CloseHandle(self.0) } {
            log::warn!("Failed to close elevated process handle: {err}");
        }
    }
}

fn elevated_working_directory(executable: &Path) -> ResultType<&Path> {
    executable
        .parent()
        .ok_or_else(|| anyhow!("Elevated executable has no parent directory"))
}

pub(super) fn run_elevated_and_wait(
    executable: &Path,
    parameters: &str,
    show: bool,
) -> ResultType<u32> {
    let parameter_chars = parameters.encode_utf16().count();
    if parameter_chars >= WIN7_SHELL_EXECUTE_MAX_PARAMETER_CHARS {
        bail!("Elevated command is too long: {parameter_chars} UTF-16 characters");
    }
    let _com = initialize_shell_com()?;
    let verb = wide_string("runas");
    let working_directory = wide_string(path_for_cmd_environment(elevated_working_directory(
        executable,
    )?)?);
    let executable = wide_string(path_for_cmd_environment(executable)?);
    let parameters = wide_string(parameters);
    let mut info = Shell::SHELLEXECUTEINFOW::default();
    info.cbSize = mem::size_of::<Shell::SHELLEXECUTEINFOW>() as u32;
    info.fMask = Shell::SEE_MASK_NOCLOSEPROCESS | Shell::SEE_MASK_NOASYNC;
    info.lpVerb = PCWSTR(verb.as_ptr());
    info.lpFile = PCWSTR(executable.as_ptr());
    info.lpParameters = PCWSTR(parameters.as_ptr());
    info.lpDirectory = PCWSTR(working_directory.as_ptr());
    info.nShow = if show {
        WindowsAndMessaging::SW_SHOWNORMAL.0
    } else {
        WindowsAndMessaging::SW_HIDE.0
    };
    unsafe { Shell::ShellExecuteExW(&mut info) }?;
    if info.hProcess.0.is_null() {
        bail!("Windows did not return an elevated process handle");
    }
    let process = ProcessHandle(info.hProcess);
    let wait_result = unsafe { Threading::WaitForSingleObject(process.0, Threading::INFINITE) };
    if wait_result == Foundation::WAIT_FAILED {
        return Err(io::Error::last_os_error().into());
    }
    if wait_result != Foundation::WAIT_OBJECT_0 {
        bail!("Unexpected elevated process wait result: {}", wait_result.0);
    }
    let mut exit_code = 0;
    unsafe { Threading::GetExitCodeProcess(process.0, &mut exit_code) }?;
    Ok(exit_code)
}

// Escape `^` before using it to escape `&` so both survive nested cmd.exe parsing.
// https://learn.microsoft.com/en-us/previous-versions/windows/it-pro/windows-server-2012-r2-and-2012/cc754250(v=ws.11)
pub(super) fn escape_nested_cmd_ampersands(value: &str) -> String {
    value.replace('^', "^^").replace('&', "^&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_values_enforce_command_safety() {
        assert!(validate_install_value(r"C:\safe ! path").is_ok());
        assert!(validate_install_value(r"C:\Program Files (x86)\RustDesk").is_ok());
        assert!(validate_install_value(r"C:\Users\R&D\RustDesk.exe").is_ok());
        assert!(validate_install_value(r"C:\A&^ B\RustDesk.exe").is_ok());
        for character in ['\0', '"', '%', '\r', '\n', '|', '<', '>'] {
            let value = format!(r"C:\unsafe{character}path");
            assert!(
                validate_install_value(&value).is_err(),
                "cmd.exe control character was accepted: {character:?}"
            );
        }
    }

    #[test]
    fn nested_commands_escape_while_protected_environment_preserves_carets() {
        assert_eq!(
            escape_nested_cmd_ampersands(r"C:\A&^ B\RustDesk.exe"),
            r"C:\A^&^^ B\RustDesk.exe"
        );
        let path = Path::new(r"C:\Win^Root\System32");
        let environment = trusted_install_environment_from_paths(path, path, path).unwrap();
        assert!(environment.contains(r#"set "PATH=C:\Win^Root\System32""#));
    }
}
