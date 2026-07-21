use super::{
    installer_shell::{
        get_system_executable, path_for_cmd_assignment, path_for_cmd_environment,
        run_elevated_and_wait, trusted_install_environment,
        BATCH_SHORTCUT_DECODE_FAILURE_EXIT_CODE, CMD_RELATIVE_PATH,
    },
    ResultType,
};
use hbb_common::{
    bail, log,
    sha2::{Digest, Sha256},
};
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

const CERTUTIL_RELATIVE_PATH: &str = "certutil.exe";
const CHCP_RELATIVE_PATH: &str = "chcp.com";
const FINDSTR_RELATIVE_PATH: &str = "findstr.exe";
const UTF8_CODE_PAGE: u32 = 65001;
const INSTALL_HANDOFF_RUNNER_EXISTS_EXIT_CODE: u32 = 0x5253_0001;
const INSTALL_HANDOFF_COPY_FAILURE_EXIT_CODE: u32 = 0x5253_0002;
const INSTALL_HANDOFF_HASH_FAILURE_EXIT_CODE: u32 = 0x5253_0003;
pub(super) const INSTALL_HANDOFF_HASH_MISMATCH_EXIT_CODE: u32 = 0x5253_0004;
const BATCH_CODE_PAGE_FAILURE_EXIT_CODE: u32 = 0x5253_0005;
const BATCH_OUTPUT_DIRECTORY_EXISTS_EXIT_CODE: u32 = 0x5253_0006;
const BATCH_OUTPUT_DIRECTORY_CREATE_FAILURE_EXIT_CODE: u32 = 0x5253_0007;
const SHA256_HASH_LENGTH: usize = 32;

pub(super) type BatchHash = [u8; SHA256_HASH_LENGTH];

pub(super) struct InstallCommandScript {
    pub(super) path: PathBuf,
    pub(super) expected_hash: BatchHash,
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
    Ok(format!(
        "@echo off\r\nsetlocal EnableExtensions DisableDelayedExpansion\r\n\
         \"{chcp}\" {UTF8_CODE_PAGE} > nul || exit /b \
         {BATCH_CODE_PAGE_FAILURE_EXIT_CODE}\r\n\
         {}\r\n\
         if exist \"%~f0.dir\" exit /b {BATCH_OUTPUT_DIRECTORY_EXISTS_EXIT_CODE}\r\n\
         md \"%~f0.dir\" || exit /b {BATCH_OUTPUT_DIRECTORY_CREATE_FAILURE_EXIT_CODE}\r\n\
         set \"RUSTDESK_OUTPUT_DIR=%~f0.dir\"\r\n{commands}\r\nexit /b 0\r\n",
        trusted_install_environment()?
    ))
}

pub(super) fn write_install_script(cmds: String) -> ResultType<InstallCommandScript> {
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

fn install_hash_pattern(hash: &BatchHash) -> String {
    hash.iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" *")
}

pub(super) fn verified_install_bootstrap(
    script: &InstallCommandScript,
    runner_directory: &Path,
) -> ResultType<String> {
    let source = path_for_cmd_assignment(&script.path)?;
    let runner = runner_directory.join(format!(
        "rustdesk_install_{}.bat",
        uuid::Uuid::new_v4().simple()
    ));
    let runner = path_for_cmd_assignment(&runner)?;
    let cmd_path = get_system_executable(CMD_RELATIVE_PATH)?;
    let certutil_path = get_system_executable(CERTUTIL_RELATIVE_PATH)?;
    let findstr_path = get_system_executable(FINDSTR_RELATIVE_PATH)?;
    let cmd = path_for_cmd_assignment(&cmd_path)?;
    let certutil = path_for_cmd_assignment(&certutil_path)?;
    let findstr = path_for_cmd_assignment(&findstr_path)?;
    Ok(format!(
        "setlocal DisableDelayedExpansion & set \"S={source}\" & set \"R={runner}\" & \
         set \"Q={cmd}\" & set \"H={certutil}\" & set \"F={findstr}\" & \
         set \"C=0\" & set \"E=0\" & setlocal EnableDelayedExpansion & \
         if exist \"!R!\" (set \"E={INSTALL_HANDOFF_RUNNER_EXISTS_EXIT_CODE}\") else (\
         set \"C=1\" & copy /Y \"!S!\" \"!R!\" > nul || \
         (set \"E={INSTALL_HANDOFF_COPY_FAILURE_EXIT_CODE}\") & \
         if \"!E!\"==\"0\" (\"!H!\" -hashfile \"!R!\" SHA256 > \"!R!.hash\" || \
         set \"E={INSTALL_HANDOFF_HASH_FAILURE_EXIT_CODE}\") & \
         if \"!E!\"==\"0\" (\"!F!\" /R /I /X /C:\"{}\" \"!R!.hash\" > nul || \
         set \"E={INSTALL_HANDOFF_HASH_MISMATCH_EXIT_CODE}\") & \
         if \"!E!\"==\"0\" (\"!Q!\" /D /E:ON /V:OFF /C \"\"!R!\"\" & \
         set \"E=!errorlevel!\")) & \
         if \"!C!\"==\"1\" (rd /s /q \"!R!.dir\" > nul 2>&1 & \
         del /f /q \"!R!\" \"!R!.*\" > nul 2>&1) & exit /b !E!",
        install_hash_pattern(&script.expected_hash),
    ))
}

pub(super) fn verified_install_parameters(script: &InstallCommandScript) -> ResultType<String> {
    let system_directory = get_system_executable("")?;
    Ok(format!(
        "/D /E:ON /V:ON /C {}",
        verified_install_bootstrap(script, &system_directory)?
    ))
}

pub(super) fn run_cmds(cmds: String, show: bool, tip: &str) -> ResultType<()> {
    let script = write_install_script(cmds)?;
    let cmd_path = get_system_executable(CMD_RELATIVE_PATH)?;
    let parameters = verified_install_parameters(&script)?;
    let exit_code = run_elevated_and_wait(&cmd_path, &parameters, show)?;
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
        INSTALL_HANDOFF_HASH_FAILURE_EXIT_CODE => "failed to hash protected runner",
        INSTALL_HANDOFF_HASH_MISMATCH_EXIT_CODE => "protected runner hash mismatch",
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
    use super::super::installer_shell::{
        embedded_shortcut_commands, shortcut_bytes, trusted_install_environment_from_paths,
        WIN7_SHELL_EXECUTE_MAX_PARAMETER_CHARS,
    };
    use super::*;
    use ::windows::Win32::System::Threading;
    use std::os::windows::process::CommandExt;

    #[test]
    fn protected_batch_environment_preserves_carets() {
        let environment = trusted_install_environment_from_paths(
            Path::new(r"C:\Win^Root\System32"),
            Path::new(r"C:\Program^Data"),
            Path::new(r"C:\Users\Pub^lic"),
        )
        .expect("protected batch environment should be generated");
        assert!(environment.contains(r#"set "PATH=C:\Win^Root\System32""#));
    }

    #[test]
    fn native_install_handoff_verifies_before_execution() {
        let marker = std::env::temp_dir().join(format!(
            "rustdesk_install_marker_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let runner_dir = std::env::temp_dir().join(format!(
            "rustdesk_install_!RUSTDESK_HANDOFF_EXPAND!&^@()runner_{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir(&runner_dir).expect("runner directory should be created");
        let shortcut_commands = embedded_shortcut_commands(
            shortcut_bytes(r"C:\RustDesk.exe", None, None)
                .expect("native shortcut should be generated"),
            "test.lnk",
            "test",
        );
        assert!(shortcut_commands.contains("certutil"));
        assert!(shortcut_commands.contains("-decode"));
        assert!(!shortcut_commands.to_ascii_lowercase().contains("cscript"));
        assert!(!shortcut_commands
            .to_ascii_lowercase()
            .contains("powershell"));
        let script = write_install_script(format!(
            "if \"%PROGRAMDATA%\"==\"rustdesk_untrusted\" exit /b 77\r\n\
             if \"%PUBLIC%\"==\"rustdesk_untrusted\" exit /b 77\r\n\
             {shortcut_commands}\r\n\
             > \"{}\" echo verified",
            marker.display()
        ))
        .expect("install script should be created");
        let bootstrap = verified_install_bootstrap(&script, &runner_dir)
            .expect("native verifier bootstrap should be generated");
        let win7_hash_pattern = script
            .expected_hash
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<Vec<_>>()
            .join(" *");
        assert!(bootstrap.contains(&format!("/R /I /X /C:\"{win7_hash_pattern}\"")));
        let parameters =
            verified_install_parameters(&script).expect("elevated parameters should be generated");
        assert!(bootstrap.contains("certutil.exe"));
        assert!(bootstrap.contains("findstr.exe"));
        assert!(!bootstrap.to_ascii_lowercase().contains("powershell"));
        assert!(parameters.encode_utf16().count() < WIN7_SHELL_EXECUTE_MAX_PARAMETER_CHARS);

        let output = run_install_bootstrap_for_test(&bootstrap);
        assert!(
            output.status.success(),
            "unchanged script failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(marker.exists(), "verified install script must execute");
        std::fs::remove_file(&marker).expect("test marker should be removed");
        assert_replaced_install_script_is_rejected(&script, &runner_dir, &marker);
        std::fs::remove_dir(runner_dir).expect("runner directory should be empty");
    }

    fn assert_replaced_install_script_is_rejected(
        script: &InstallCommandScript,
        runner_dir: &Path,
        marker: &Path,
    ) {
        std::fs::write(
            &script.path,
            format!("> \"{}\" echo hijacked\r\n", marker.display()),
        )
        .expect("install script should be replaceable");
        let replaced = verified_install_bootstrap(&script, &runner_dir)
            .expect("replacement verifier should be generated");
        let output = run_install_bootstrap_for_test(&replaced);
        assert!(
            !output.status.success(),
            "replaced script unexpectedly passed verification"
        );
        assert_eq!(
            output.status.code(),
            Some(INSTALL_HANDOFF_HASH_MISMATCH_EXIT_CODE as i32)
        );
        assert!(!marker.exists(), "replaced script must not execute");
    }

    fn run_install_bootstrap_for_test(bootstrap: &str) -> std::process::Output {
        let cmd = get_system_executable(CMD_RELATIVE_PATH).expect("system cmd.exe should resolve");
        let mut command = std::process::Command::new(cmd);
        command
            .env("PROGRAMDATA", "rustdesk_untrusted")
            .env("PUBLIC", "rustdesk_untrusted")
            .env("RUSTDESK_HANDOFF_EXPAND", "expanded");
        command.raw_arg(format!("/D /E:ON /V:ON /C {bootstrap}"));
        command
            .creation_flags(Threading::CREATE_NO_WINDOW.0)
            .output()
            .expect("native verifier should run")
    }
}
