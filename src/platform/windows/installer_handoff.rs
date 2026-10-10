use super::{
    installer_shell::{
        get_system_executable, path_for_cmd_assignment, path_for_cmd_environment,
        run_elevated_and_wait, trusted_install_environment,
        BATCH_SHORTCUT_DECODE_FAILURE_EXIT_CODE, CMD_RELATIVE_PATH,
    },
    validate_install_app_name, ResultType, UPDATE_APP_EXIT_QUERY_FAILURE_EXIT_CODE,
    UPDATE_APP_EXIT_TIMEOUT_EXIT_CODE, UPDATE_SERVICE_RESTORE_FAILURE_EXIT_CODE,
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
const INSTALL_HANDOFF_HASH_MISMATCH_EXIT_CODE: u32 = 0x5253_0004;
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

fn install_hash_pattern(hash: &BatchHash) -> String {
    hash.iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" *")
}

fn verified_install_bootstrap(
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
    // Short names preserve headroom under the Windows 7 ShellExecuteExW 2,048
    // UTF-16-character parameter limit. Paths are stored with delayed expansion
    // disabled, then expanded indirectly so literal `!` survives: S=source,
    // R=runner, Q=cmd.exe, H=certutil.exe, F=findstr.exe, C=created flag, E=exit code.
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

fn verified_install_parameters(script: &InstallCommandScript) -> ResultType<String> {
    let system_directory = get_system_executable("")?;
    Ok(format!(
        "/D /E:ON /V:ON /C {}",
        verified_install_bootstrap(script, &system_directory)?
    ))
}

pub(super) fn run_cmds(cmds: String, show: bool, tip: &str) -> ResultType<()> {
    validate_install_app_name(&crate::get_app_name())?;
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
        UPDATE_APP_EXIT_TIMEOUT_EXIT_CODE => "timed out waiting for the app processes to exit",
        UPDATE_APP_EXIT_QUERY_FAILURE_EXIT_CODE => "failed to query the app processes",
        UPDATE_SERVICE_RESTORE_FAILURE_EXIT_CODE => "failed to restart the stopped service",
        _ => "installer command failed",
    }
}

#[cfg(test)]
mod tests {
    use super::super::installer_shell::{
        embedded_shortcut_commands, shortcut_bytes, WIN7_SHELL_EXECUTE_MAX_PARAMETER_CHARS,
    };
    use super::*;
    use ::windows::Win32::System::Threading;
    use std::os::windows::process::CommandExt;

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
        assert_native_handoff_structure(&script, &shortcut_commands, &bootstrap);

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

    fn assert_native_handoff_structure(
        script: &InstallCommandScript,
        shortcut_commands: &str,
        bootstrap: &str,
    ) {
        assert!(shortcut_commands.contains("certutil"));
        assert!(shortcut_commands.contains("-decode"));
        assert!(!shortcut_commands.to_ascii_lowercase().contains("cscript"));
        assert!(!shortcut_commands
            .to_ascii_lowercase()
            .contains("powershell"));
        let win7_hash_pattern = script
            .expected_hash
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<Vec<_>>()
            .join(" *");
        assert!(bootstrap.contains(&format!("/R /I /X /C:\"{win7_hash_pattern}\"")));
        let parameters =
            verified_install_parameters(script).expect("elevated parameters should be generated");
        assert!(bootstrap.contains("certutil.exe"));
        assert!(bootstrap.contains("findstr.exe"));
        assert!(!bootstrap.to_ascii_lowercase().contains("powershell"));
        assert!(parameters.encode_utf16().count() < WIN7_SHELL_EXECUTE_MAX_PARAMETER_CHARS);
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

    #[test]
    fn update_wait_for_app_exit_continues_only_after_a_clean_no_match() {
        let app_name = format!("RustDeskWaitTest{}", uuid::Uuid::new_v4().simple());
        let failed = Some(UPDATE_APP_EXIT_QUERY_FAILURE_EXIT_CODE as i32);
        // No such process: the update goes on.
        assert_eq!(run_update_wait_for_test(&app_name, "", None), (Some(0), false, true));
        // None of the following may pass for "no process left". Each must
        // restore the service, skip the copy and fail.
        // The query fails without printing a match.
        assert_eq!(
            run_update_wait_for_test(&app_name, " /FI \"RUSTDESK_INVALID eq 1\"", None),
            (failed, true, false)
        );
        // The query output cannot be written, which leaves ERRORLEVEL unchanged
        // and would make `find` report no match.
        assert_eq!(
            run_update_wait_for_test(
                &app_name,
                "",
                Some(("\\tasklist.csv", "\\missing\\tasklist.csv")),
            ),
            (failed, true, false)
        );
        // The search itself fails.
        assert_eq!(
            run_update_wait_for_test(
                &app_name,
                "",
                Some(("\nfind /I ", "\nfind /RUSTDESK_INVALID /I ")),
            ),
            (failed, true, false)
        );
    }

    // Stands in for sc.exe: the old instance still reports RUNNING, then the
    // service is stopping, then stopped. The start is refused until it has
    // stopped, as the SCM does, and the service runs once started.
    const FAKE_SC_STOPPING_SERVICE: &str = r#"@echo off
if "%~1"=="start" goto start
if exist "%~dp0running" goto running
if exist "%~dp0query2" (echo         STATE              : 1  STOPPED& exit /b 0)
if exist "%~dp0query1" (type nul > "%~dp0query2" & goto stopping)
type nul > "%~dp0query1"
:running
echo         STATE              : 4  RUNNING
exit /b 0
:stopping
echo         STATE              : 3  STOP_PENDING
exit /b 0
:start
if not exist "%~dp0query2" (type nul > "%~dp0refused" & exit /b 1056)
type nul > "%~dp0running"
exit /b 0
"#;

    // Stands in for sc.exe when the service cannot be queried.
    const FAKE_SC_QUERY_FAILS: &str = "@echo off\nexit /b 1060\n";

    #[test]
    fn update_abort_restarts_the_service_only_once_it_has_stopped() {
        // A failed process query aborts the update while the old instance still
        // reports RUNNING and the service is then still stopping. It must not
        // count as restored before it has stopped, must be started once it
        // has, and must run again before the updater exits, which still
        // reports the original failure.
        let (code, copied, dir) = run_update_abort_with_fake_sc_for_test(FAKE_SC_STOPPING_SERVICE);
        assert_eq!(code, Some(UPDATE_APP_EXIT_QUERY_FAILURE_EXIT_CODE as i32));
        assert!(!copied);
        assert!(dir.join("running").exists(), "the service must run again");
        assert!(!dir.join("refused").exists(), "no start while still stopping");
        std::fs::remove_dir_all(&dir).expect("test directory should be removed");

        // A service that cannot be restored is reported instead of the
        // original failure.
        let (code, copied, dir) = run_update_abort_with_fake_sc_for_test(FAKE_SC_QUERY_FAILS);
        assert_eq!(code, Some(UPDATE_SERVICE_RESTORE_FAILURE_EXIT_CODE as i32));
        assert!(!copied);
        std::fs::remove_dir_all(&dir).expect("test directory should be removed");
    }

    // Aborts the update's wait with a failing process query, restoring the
    // service through `fake_sc` in place of sc.exe. Returns the exit code,
    // whether the copy was reached and the test directory holding the fake's
    // state, which the caller removes.
    fn run_update_abort_with_fake_sc_for_test(fake_sc: &str) -> (Option<i32>, bool, PathBuf) {
        let app_name = format!("RustDeskWaitTest{}", uuid::Uuid::new_v4().simple());
        let dir = create_test_dir_for_update();
        let fake = dir.join("fake_sc.bat");
        std::fs::write(&fake, fake_sc.replace('\n', "\r\n")).expect("fake sc should be written");
        let restore = super::super::restore_service_after_abort_cmd(&app_name);
        for command in ["query", "start"] {
            assert!(restore.contains(&format!("sc {command} {app_name} ")));
        }
        let fake_path = fake.display();
        let restore = restore
            .replace(&format!("sc query {app_name} "), &format!("call \"{fake_path}\" query "))
            .replace(&format!("sc start {app_name} "), &format!("call \"{fake_path}\" start "));
        let wait = super::super::wait_for_app_exit_cmd(
            &app_name,
            " /FI \"RUSTDESK_INVALID eq 1\"",
            &restore,
        );
        let (code, copied) = run_update_script_for_test(&dir, &wait);
        (code, copied, dir)
    }

    // Runs the update's wait for the app to exit through the verified handoff,
    // followed by a copy marker. Returns the exit code and whether the service
    // restore and the copy were reached. `inject` replaces part of the lines.
    fn run_update_wait_for_test(
        app_name: &str,
        filter: &str,
        inject: Option<(&str, &str)>,
    ) -> (Option<i32>, bool, bool) {
        let dir = create_test_dir_for_update();
        let restored = dir.join("restored");
        let restore_service_cmd = format!("> \"{}\" echo restored", restored.display());
        let mut wait = super::super::wait_for_app_exit_cmd(app_name, filter, &restore_service_cmd);
        if let Some((from, to)) = inject {
            assert!(wait.contains(from), "nothing to inject into");
            wait = wait.replace(from, to);
        }
        let (code, copied) = run_update_script_for_test(&dir, &wait);
        let result = (code, restored.exists(), copied);
        std::fs::remove_dir_all(&dir).expect("test directory should be removed");
        result
    }

    fn create_test_dir_for_update() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rustdesk_update_wait_{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir(&dir).expect("test directory should be created");
        dir
    }

    // Runs `wait` through the verified handoff in `dir`, followed by a copy
    // marker. Returns the exit code and whether the copy was reached.
    fn run_update_script_for_test(dir: &Path, wait: &str) -> (Option<i32>, bool) {
        let copied = dir.join("copied");
        let script = write_install_script(format!(
            "{wait}\r\n> \"{}\" echo copied",
            copied.display()
        ))
        .expect("install script should be created");
        let bootstrap = verified_install_bootstrap(&script, dir)
            .expect("native verifier bootstrap should be generated");
        let output = run_install_bootstrap_for_test(&bootstrap);
        (output.status.code(), copied.exists())
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
