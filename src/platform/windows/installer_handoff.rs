mod directory;

use super::{
    installer_files::{self, VERIFY_FILE_ARGUMENT},
    installer_shell::{
        get_system_executable, path_for_cmd_assignment, path_for_cmd_environment,
        run_elevated_and_wait, trusted_install_environment,
        BATCH_SHORTCUT_DECODE_FAILURE_EXIT_CODE, CMD_RELATIVE_PATH,
        WIN7_SHELL_EXECUTE_MAX_PARAMETER_CHARS,
    },
    validate_install_app_name, ResultType,
};
use hbb_common::{
    anyhow::anyhow,
    bail, log,
    sha2::{Digest, Sha256},
};
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

const CHCP_RELATIVE_PATH: &str = "chcp.com";
const UTF8_CODE_PAGE: u32 = 65001;
const INSTALL_HANDOFF_RUNNER_EXISTS_EXIT_CODE: u32 = 0x5253_0001;
const INSTALL_HANDOFF_COPY_FAILURE_EXIT_CODE: u32 = 0x5253_0002;
const INSTALL_HANDOFF_HASH_FAILURE_EXIT_CODE: u32 = 0x5253_0003;
const BATCH_CODE_PAGE_FAILURE_EXIT_CODE: u32 = 0x5253_0005;
const BATCH_OUTPUT_DIRECTORY_EXISTS_EXIT_CODE: u32 = 0x5253_0006;
const BATCH_OUTPUT_DIRECTORY_CREATE_FAILURE_EXIT_CODE: u32 = 0x5253_0007;
const SHA256_HASH_LENGTH: usize = 32;

type BatchHash = [u8; SHA256_HASH_LENGTH];

struct InstallCommandScript {
    path: PathBuf,
    expected_hash: BatchHash,
}

impl Drop for InstallCommandScript {
    fn drop(&mut self) {
        if let Err(err) = fs::remove_file(&self.path) {
            if err.kind() != io::ErrorKind::NotFound {
                log::warn!(
                    "Failed to remove temporary installer file {:?}: {err}",
                    self.path
                );
            }
        }
    }
}

fn prepare_install_commands(commands: &str) -> ResultType<String> {
    let commands = commands.replace("\r\n", "\n").replace('\n', "\r\n");
    let chcp_path = get_system_executable(CHCP_RELATIVE_PATH)?;
    let chcp = path_for_cmd_environment(&chcp_path)?;
    let helper = installer_files::helper_path()?.replace('%', "%%");
    Ok(format!(
        "@echo off\r\nsetlocal EnableExtensions DisableDelayedExpansion\r\n\
         \"{chcp}\" {UTF8_CODE_PAGE} > nul || exit /b \
         {BATCH_CODE_PAGE_FAILURE_EXIT_CODE}\r\n\
         {}\r\n\
         set \"RUSTDESK_INSTALL_HELPER={helper}\"\r\n\
         if exist \"%~f0.dir\" exit /b {BATCH_OUTPUT_DIRECTORY_EXISTS_EXIT_CODE}\r\n\
         md \"%~f0.dir\" || exit /b {BATCH_OUTPUT_DIRECTORY_CREATE_FAILURE_EXIT_CODE}\r\n\
         set \"RUSTDESK_OUTPUT_DIR=%~f0.dir\"\r\n{commands}\r\nexit /b 0\r\n",
        trusted_install_environment()?
    ))
}

fn write_install_script(cmds: String) -> ResultType<InstallCommandScript> {
    let directory = std::env::temp_dir();
    path_for_cmd_environment(&directory)?;
    let commands = prepare_install_commands(&cmds)?;
    let expected_hash = Sha256::digest(commands.as_bytes()).into();
    let path = directory.join(format!(
        "rustdesk_install_{}.bat",
        uuid::Uuid::new_v4().simple()
    ));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    let script = InstallCommandScript {
        path,
        expected_hash,
    };
    file.write_all(commands.as_bytes())?;
    file.sync_all()?;
    Ok(script)
}

fn verified_install_bootstrap(
    script: &InstallCommandScript,
    runner_directory: &Path,
) -> ResultType<String> {
    let source = path_for_cmd_assignment(&script.path)?;
    let directory = path_for_cmd_assignment(runner_directory)?;
    let runner = runner_directory.join("install.bat");
    let runner = path_for_cmd_assignment(&runner)?;
    let helper = installer_files::helper_path()?
        .replace('^', "^^")
        .replace('!', "^!")
        .replace('%', "!P!");
    // Short names preserve headroom under the Windows 7 ShellExecuteExW 2,048
    // UTF-16-character parameter limit. Paths are stored with delayed expansion
    // disabled, then expanded indirectly so literal `!` survives: S=source,
    // R=runner, H=native file helper, D=directory, E=exit code, P=literal percent.
    // Insert percent signs with delayed expansion to avoid expanding environment variables.
    // Execute the BAT directly in this cmd process, without CALL's extra parsing.
    Ok(format!(
        "setlocal DisableDelayedExpansion & set \"S={source}\" & set \"R={runner}\" & \
         set \"ERRORLEVEL=\" & set \"D={directory}\" & set \"E=0\" & set \"P=%\" & \
         setlocal EnableDelayedExpansion & set \"H={helper}\" & \
         if exist \"!R!\" (set \"E={INSTALL_HANDOFF_RUNNER_EXISTS_EXIT_CODE}\") else (\
         copy /Y \"!S!\" \"!R!\" > nul || \
         (set \"E={INSTALL_HANDOFF_COPY_FAILURE_EXIT_CODE}\") & \
         if \"!E!\"==\"0\" (start \"\" /b /wait \"!H!\" {VERIFY_FILE_ARGUMENT} \
         \"!R!\" {expected_hash} \"!R!.verified\" || \
         set \"E={INSTALL_HANDOFF_HASH_FAILURE_EXIT_CODE}\") & \
         if \"!E!\"==\"0\" (if not exist \"!R!.verified\" \
         set \"E={INSTALL_HANDOFF_HASH_FAILURE_EXIT_CODE}\") & \
         if \"!E!\"==\"0\" (\"!R!\" & \
         set \"E=!errorlevel!\")) & \
         rd /s /q \"!D!\" > nul 2>&1 & exit /b !E!",
        expected_hash = hex::encode(script.expected_hash),
    ))
}

fn verified_install_parameters(
    script: &InstallCommandScript,
    directory: &Path,
) -> ResultType<String> {
    let parameters = format!(
        "/D /E:ON /V:ON /C {}",
        verified_install_bootstrap(script, directory)?
    );
    let length = parameters.encode_utf16().count();
    if length >= WIN7_SHELL_EXECUTE_MAX_PARAMETER_CHARS {
        bail!("Elevated command is too long: {length} UTF-16 characters");
    }
    Ok(parameters)
}

pub(super) fn run_cmds(cmds: String, show: bool, tip: &str) -> ResultType<()> {
    validate_install_app_name(&crate::get_app_name())?;
    let script = write_install_script(cmds)?;
    let cmd_path = get_system_executable(CMD_RELATIVE_PATH)?;
    let directory = directory::path()?;
    let parameters = verified_install_parameters(&script, &directory)?;
    directory::create(&directory)?;
    let exit_code = run_elevated_and_wait(&cmd_path, &parameters, show).map_err(|err| {
        anyhow!(
            "Installer handoff failed; protected directory may remain at {}: {err}",
            directory.display()
        )
    })?;
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
        INSTALL_HANDOFF_RUNNER_EXISTS_EXIT_CODE => "protected runner already exists",
        INSTALL_HANDOFF_COPY_FAILURE_EXIT_CODE => "failed to copy protected runner",
        INSTALL_HANDOFF_HASH_FAILURE_EXIT_CODE => "failed to verify protected runner",
        BATCH_CODE_PAGE_FAILURE_EXIT_CODE => "failed to set the installer code page",
        BATCH_OUTPUT_DIRECTORY_EXISTS_EXIT_CODE => "installer output directory already exists",
        BATCH_OUTPUT_DIRECTORY_CREATE_FAILURE_EXIT_CODE => {
            "failed to create the installer output directory"
        }
        BATCH_SHORTCUT_DECODE_FAILURE_EXIT_CODE => "failed to decode an embedded shortcut",
        _ => "installer command failed",
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        installer_files::{handle_command, DECODE_SHORTCUT_ARGUMENT},
        installer_shell::{embedded_shortcut_commands, shortcut_bytes},
    };
    use super::*;
    use hbb_common::base64::{engine::general_purpose::STANDARD, Engine as _};

    #[test]
    fn installer_verification_rejects_changed_files() {
        let shortcut = embedded_shortcut_commands(vec![0, 1, 2], "test.lnk", "test");
        let script = write_install_script(shortcut.clone()).unwrap();
        let runner = std::env::temp_dir().join("rustdesk !&^@() runner");
        let parameters = verified_install_parameters(&script, &runner).unwrap();
        for tool in ["certutil", "findstr", "cscript", "powershell"] {
            assert!(!parameters.to_ascii_lowercase().contains(tool));
            assert!(!shortcut.to_ascii_lowercase().contains(tool));
        }
        assert!(!parameters.to_ascii_lowercase().contains("cmd.exe"));
        assert!(parameters.encode_utf16().count() < WIN7_SHELL_EXECUTE_MAX_PARAMETER_CHARS);

        let receipt = script.path.with_extension("verified");
        let arguments = vec![
            VERIFY_FILE_ARGUMENT.into(),
            script.path.to_str().unwrap().into(),
            hex::encode(script.expected_hash),
            receipt.to_str().unwrap().into(),
        ];
        handle_command(&arguments).unwrap().unwrap();
        assert!(receipt.exists());
        fs::remove_file(&receipt).unwrap();

        fs::write(&script.path, "exit /b 77\r\n").unwrap();
        assert!(handle_command(&arguments).unwrap().is_err());
        assert!(
            !receipt.exists(),
            "Changed files must not authorize execution"
        );
    }

    #[test]
    fn native_handoff_accepts_percent_in_executable_path() {
        use std::os::windows::process::CommandExt;

        let executable = std::env::current_exe().unwrap();
        let alias = executable.with_file_name(format!(
            "rustdesk-%RUSTDESK_HELPER_PATH%-{}.exe",
            uuid::Uuid::new_v4().simple()
        ));
        fs::hard_link(&executable, &alias).unwrap();
        let output = std::process::Command::new(&alias)
            .args([
                "--exact",
                "platform::windows::installer_handoff::tests::installer_verification_rejects_changed_files",
            ])
            .env("RUSTDESK_HELPER_PATH", "expanded")
            .creation_flags(::windows::Win32::System::Threading::CREATE_NO_WINDOW.0)
            .output();
        fs::remove_file(alias).unwrap();
        let output = output.unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
    }

    #[test]
    fn installer_shortcut_decoding_preserves_link_bytes() {
        let source = std::env::temp_dir().join(format!(
            "rustdesk-shortcut-{}-!&^@().b64",
            uuid::Uuid::new_v4().simple()
        ));
        let destination = source.with_extension("lnk");
        let shortcut = shortcut_bytes(r"C:\RustDesk.exe", Some("--tray"), None).unwrap();
        fs::write(&source, format!("{}\r\n", STANDARD.encode(&shortcut))).unwrap();
        let arguments = vec![
            DECODE_SHORTCUT_ARGUMENT.into(),
            source.to_str().unwrap().into(),
            destination.to_str().unwrap().into(),
        ];
        handle_command(&arguments).unwrap().unwrap();
        assert_eq!(fs::read(&destination).unwrap(), shortcut);
        fs::remove_file(&destination).unwrap();

        fs::write(&source, "invalid base64").unwrap();
        assert!(handle_command(&arguments).unwrap().is_err());
        assert!(!destination.exists());
        fs::remove_file(source).unwrap();
    }
}
