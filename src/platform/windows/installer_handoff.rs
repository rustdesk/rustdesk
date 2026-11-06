pub(super) mod directory;

use super::{
    installer_files::ProtectedInstallFile,
    installer_shell::{
        get_system_executable, launch_elevated, path_for_cmd_environment,
        trusted_install_environment, BATCH_SHORTCUT_COPY_FAILURE_EXIT_CODE, CMD_RELATIVE_PATH,
        WIN7_SHELL_EXECUTE_MAX_PARAMETER_CHARS,
    },
    validate_install_app_name, ResultType,
};
use hbb_common::{bail, log};
use std::{fs, io, path::Path};

const CHCP_RELATIVE_PATH: &str = "chcp.com";
const UTF8_CODE_PAGE: u32 = 65001;
const BATCH_CODE_PAGE_FAILURE_EXIT_CODE: u32 = 0x5253_0005;

pub(super) struct InstallCommands {
    pub script: String,
    pub files: Vec<ProtectedInstallFile>,
}

impl From<String> for InstallCommands {
    fn from(script: String) -> Self {
        Self {
            script,
            files: Vec::new(),
        }
    }
}

struct InstallCommandScript {
    file: ProtectedInstallFile,
    _files: Vec<ProtectedInstallFile>,
}

impl InstallCommandScript {
    fn release_files(&mut self) {
        self.file.release();
        for file in &mut self._files {
            file.release();
        }
    }
}

fn prepare_install_commands(cmds: &InstallCommands) -> ResultType<String> {
    let commands = cmds.script.replace("\r\n", "\n").replace('\n', "\r\n");
    let chcp_path = get_system_executable(CHCP_RELATIVE_PATH)?;
    let chcp = path_for_cmd_environment(&chcp_path)?;
    let files = cmds
        .files
        .iter()
        .map(|file| path_for_cmd_environment(file.path()).map(|path| format!("\"{path}\"")))
        .collect::<ResultType<Vec<_>>>()?
        .join(" ");
    // Call a fixed label so early EXIT /B still reaches cleanup, without reparsing a path.
    // Exit the dedicated cmd after self-deletion; EXIT /B would reopen the missing BAT.
    Ok(format!(
        "@echo off\r\nsetlocal EnableExtensions DisableDelayedExpansion\r\n\
         set \"ERRORLEVEL=\"\r\n\
         call :rustdesk_install\r\n\
         set \"RUSTDESK_INSTALL_EXIT_CODE=%ERRORLEVEL%\"\r\n\
         rd /s /q \"%~f0.dir\" > nul 2>&1\r\n\
         (del /q \"%~f0\" {files} > nul 2>&1 & exit %RUSTDESK_INSTALL_EXIT_CODE%)\r\n\
         :rustdesk_install\r\n\
         \"{chcp}\" {UTF8_CODE_PAGE} > nul || exit /b \
         {BATCH_CODE_PAGE_FAILURE_EXIT_CODE}\r\n\
         {}\r\n\
         set \"RUSTDESK_OUTPUT_DIR=%~f0.dir\"\r\n{commands}\r\nexit /b 0\r\n",
        trusted_install_environment()?
    ))
}

fn write_install_script(cmds: InstallCommands, path: &Path) -> ResultType<InstallCommandScript> {
    let commands = prepare_install_commands(&cmds)?;
    Ok(InstallCommandScript {
        file: ProtectedInstallFile::new(path, commands.as_bytes())?,
        _files: cmds.files,
    })
}

fn protected_install_parameters(script: &InstallCommandScript) -> ResultType<String> {
    let path = path_for_cmd_environment(script.file.path())?;
    let parameters = format!("/D /E:ON /V:OFF /C \"\"{path}\"\"");
    let length = parameters.encode_utf16().count();
    if length >= WIN7_SHELL_EXECUTE_MAX_PARAMETER_CHARS {
        bail!("Elevated command is too long: {length} UTF-16 characters");
    }
    Ok(parameters)
}

pub(super) fn run_cmds(cmds: impl Into<InstallCommands>, show: bool, tip: &str) -> ResultType<()> {
    validate_install_app_name(&crate::get_app_name())?;
    let path = directory::path()?.with_extension("bat");
    let directory = path.with_extension("bat.dir");
    let mut script = write_install_script(cmds.into(), &path)?;
    let cmd_path = get_system_executable(CMD_RELATIVE_PATH)?;
    let parameters = protected_install_parameters(&script)?;
    directory::create(&directory)?;
    // cmd cannot open the BAT or copy its inputs while a DELETE-capable handle is open, so
    // close them before launching it; a cancelled UAC prompt then leaves the files behind.
    // Holding them until cmd read a release signal from a named pipe failed under F-Secure's
    // behavior detection, which closed cmd's end first (#16840, #16842). The file DACLs and
    // the parent directory's permissions still keep unelevated users from changing them.
    script.release_files();
    let process = launch_elevated(&cmd_path, &parameters, show).map_err(|err| {
        log::debug!("Failed to launch elevated installer: {err:#}");
        err.context(format!(
            "Installer handoff failed; protected directory may remain at {}",
            directory.display()
        ))
    })?;
    let exit_code = process.wait()?;
    // Cleanup must not change the result of an installation that already ran.
    match fs::symlink_metadata(&directory) {
        Ok(_) => log::warn!(
            "Protected installer directory may remain at {}",
            directory.display()
        ),
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => log::warn!(
            "Protected installer directory may remain at {}: {err}",
            directory.display()
        ),
    }
    if exit_code != 0 {
        bail!(
            "{tip} failed with elevated exit code {exit_code}: {}",
            elevated_install_failure_reason(exit_code)
        );
    }
    Ok(())
}

fn elevated_install_failure_reason(exit_code: u32) -> &'static str {
    match exit_code {
        BATCH_CODE_PAGE_FAILURE_EXIT_CODE => "failed to set the installer code page",
        BATCH_SHORTCUT_COPY_FAILURE_EXIT_CODE => "failed to copy a prepared shortcut",
        _ => "installer command failed",
    }
}

#[cfg(test)]
mod tests {
    use super::super::installer_shell::shortcut_bytes;
    use super::*;
    use std::{os::windows::process::CommandExt, process::Stdio};

    #[test]
    fn protected_handoff_preserves_shortcuts_failures_and_cleanup() {
        const COMMAND_FAILURE: u32 = 77;
        for exit_code in [0, COMMAND_FAILURE] {
            let root =
                std::env::temp_dir().join(format!("rustdesk-handoff-{}", uuid::Uuid::new_v4()));
            let source = root.join("source !&^@().bat");
            let runner = source.with_extension("bat.dir");
            fs::create_dir_all(&runner).unwrap();
            let bytes = shortcut_bytes(r"C:\RustDesk.exe", Some("--tray"), None).unwrap();
            let shortcut =
                ProtectedInstallFile::new(&root.join("shortcut !&^@().tmp"), &bytes).unwrap();
            let marker = root.join("copied.lnk");
            let commands = InstallCommands {
                script: format!(
                    "copy /B /Y \"{}\" \"{}\" > nul\r\nexit /b {exit_code}",
                    shortcut.path().display(),
                    marker.display()
                ),
                files: vec![shortcut],
            };
            let mut script = write_install_script(commands, &source).unwrap();
            // The DACL, not a held handle, keeps the unelevated owner from editing the BAT.
            script.release_files();
            if !super::super::is_elevated(None).unwrap() {
                assert!(fs::write(script.file.path(), "exit /b 0\r\n").is_err());
            }
            let output = run_script(&mut script);
            let copied = fs::read(&marker).unwrap();
            let cleaned = !runner.exists()
                && !script.file.path().exists()
                && script._files.iter().all(|file| !file.path().exists());
            fs::remove_dir_all(&root).unwrap();
            assert_eq!(
                output.status.code().map(|code| code as u32),
                Some(exit_code),
                "{output:?}"
            );
            assert_eq!(copied, bytes);
            assert!(cleaned, "The runner and protected inputs must be removed");
        }
    }

    // Mirrors run_cmds without UAC: the files are released before cmd starts and nothing
    // reaches cmd's stdin.
    fn run_script(script: &mut InstallCommandScript) -> std::process::Output {
        let parameters = protected_install_parameters(script).unwrap();
        script.release_files();
        std::process::Command::new(get_system_executable(CMD_RELATIVE_PATH).unwrap())
            .raw_arg(parameters)
            .creation_flags(::windows::Win32::System::Threading::CREATE_NO_WINDOW.0)
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }
}
