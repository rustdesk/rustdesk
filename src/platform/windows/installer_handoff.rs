pub(super) mod directory;
mod gate;

use super::{
    installer_files::ProtectedInstallFile,
    installer_shell::{
        get_system_executable, launch_elevated, path_for_cmd_environment,
        trusted_install_environment, BATCH_SHORTCUT_COPY_FAILURE_EXIT_CODE, CMD_RELATIVE_PATH,
        WIN7_SHELL_EXECUTE_MAX_PARAMETER_CHARS,
    },
    validate_install_app_name, ResultType, UPDATE_APP_EXIT_QUERY_FAILURE_EXIT_CODE,
    UPDATE_APP_EXIT_TIMEOUT_EXIT_CODE, UPDATE_SERVICE_RESTORE_FAILURE_EXIT_CODE,
};
use gate::InstallGate;
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

fn protected_install_parameters(
    script: &InstallCommandScript,
    gate: &InstallGate,
) -> ResultType<String> {
    let path = path_for_cmd_environment(script.file.path())?;
    let pipe = gate.name();
    let parameters =
        format!("/D /E:ON /V:OFF /C \"set /p RUSTDESK_HANDOFF=<\"{pipe}\" >nul && \"{path}\"\"");
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
    let gate = InstallGate::new()?;
    let parameters = protected_install_parameters(&script, &gate)?;
    directory::create(&directory)?;
    let process = launch_elevated(&cmd_path, &parameters, show).map_err(|err| {
        log::debug!("Failed to launch elevated installer: {err:#}");
        err.context(format!(
            "Installer handoff failed; protected directory may remain at {}",
            directory.display()
        ))
    })?;
    gate.wait_for_client(process.handle())?;
    // Keep DELETE-capable handles through UAC cancellation, but close them before
    // cmd opens the BAT or copies its inputs. The protected DACL stays unchanged.
    script.release_files();
    gate.release(process.handle())?;
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
        UPDATE_APP_EXIT_TIMEOUT_EXIT_CODE => "timed out waiting for the app processes to exit",
        UPDATE_APP_EXIT_QUERY_FAILURE_EXIT_CODE => "failed to query the app processes",
        UPDATE_SERVICE_RESTORE_FAILURE_EXIT_CODE => "failed to restart the stopped service",
        _ => "installer command failed",
    }
}

#[cfg(test)]
mod tests {
    use super::super::installer_shell::shortcut_bytes;
    use super::*;
    use std::{
        os::windows::{io::AsRawHandle, process::CommandExt},
        path::PathBuf,
        process::Stdio,
    };
    use windows::Win32::Foundation::HANDLE;

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
            if !super::super::is_elevated(None).unwrap() {
                assert!(fs::write(script.file.path(), "exit /b 0\r\n").is_err());
            }
            let output = run_script(&mut script, true);
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

    #[test]
    fn interrupted_handoff_does_not_execute_the_batch() {
        let root =
            std::env::temp_dir().join(format!("rustdesk-interrupted-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let source = root.join("source.bat");
        let marker = root.join("executed");
        let commands = format!("echo unexpected > \"{}\"", marker.display());
        let mut script = write_install_script(commands.into(), &source).unwrap();
        let output = run_script(&mut script, false);
        assert!(!output.status.success(), "{output:?}");
        assert!(!marker.exists());
        drop(script);
        assert!(!source.exists());
        fs::remove_dir(&root).unwrap();

        // A child exiting without connecting must also retire the pending pipe I/O.
        let gate = InstallGate::new().unwrap();
        let mut child =
            std::process::Command::new(get_system_executable(CMD_RELATIVE_PATH).unwrap())
                .args(["/D", "/C", "exit", "0"])
                .creation_flags(::windows::Win32::System::Threading::CREATE_NO_WINDOW.0)
                .spawn()
                .unwrap();
        assert!(gate.wait_for_client(HANDLE(child.as_raw_handle())).is_err());
        assert!(child.wait().unwrap().success());
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

    // Runs `wait` through the protected handoff, followed by a copy marker in
    // `dir`. Returns the exit code and whether the copy was reached.
    fn run_update_script_for_test(dir: &Path, wait: &str) -> (Option<i32>, bool) {
        let copied = dir.join("copied");
        let commands = format!("{wait}\r\n> \"{}\" echo copied", copied.display());
        // The script removes itself and its `.dir` on the way out, so both stay
        // apart from the markers in `dir`.
        let source = dir.join("update.bat");
        fs::create_dir(source.with_extension("bat.dir"))
            .expect("script directory should be created");
        let mut script = write_install_script(commands.into(), &source)
            .expect("install script should be created");
        let output = run_script(&mut script, true);
        (output.status.code(), copied.exists())
    }

    fn run_script(script: &mut InstallCommandScript, release: bool) -> std::process::Output {
        let gate = InstallGate::new().unwrap();
        let parameters = protected_install_parameters(script, &gate).unwrap();
        let mut child =
            std::process::Command::new(get_system_executable(CMD_RELATIVE_PATH).unwrap())
                .raw_arg(parameters)
                .creation_flags(::windows::Win32::System::Threading::CREATE_NO_WINDOW.0)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
        let process = HANDLE(child.as_raw_handle());
        gate.wait_for_client(process).unwrap();
        assert!(child.try_wait().unwrap().is_none());
        if release {
            script.release_files();
            gate.release(process).unwrap();
        } else {
            drop(gate);
        }
        child.wait_with_output().unwrap()
    }
}
