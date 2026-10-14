use super::super::installer_shell::{embedded_shortcut_commands, shortcut_bytes};
use super::*;
use ::windows::Win32::{
    Storage::FileSystem::{FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_READ, FILE_SHARE_WRITE},
    System::Threading,
};
use std::os::windows::{fs::OpenOptionsExt, process::CommandExt};

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
    assert_native_handoff_structure(&script, &shortcut_commands, &runner_dir);
    let output = run_install_bootstrap_for_test(&bootstrap);
    assert!(
        output.status.success(),
        "unchanged script failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(marker.exists(), "verified install script must execute");
    std::fs::remove_file(&marker).expect("test marker should be removed");
    assert!(
        !runner_dir.exists(),
        "protected runner directory must be removed"
    );
    std::fs::create_dir(&runner_dir).expect("runner directory should be created");
    assert_replaced_install_script_is_rejected(&script, &runner_dir, &marker);
    assert!(
        !runner_dir.exists(),
        "failed handoff directory must be removed"
    );
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

fn assert_native_handoff_structure(
    script: &InstallCommandScript,
    shortcut_commands: &str,
    runner_dir: &Path,
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
    let parameters = verified_install_parameters(script, runner_dir)
        .expect("elevated parameters should be generated");
    assert!(parameters.contains(&format!("/R /I /X /C:\"{win7_hash_pattern}\"")));
    assert!(parameters.contains("certutil.exe"));
    assert!(parameters.contains("findstr.exe"));
    assert!(!parameters.to_ascii_lowercase().contains("powershell"));
    assert!(parameters.encode_utf16().count() < WIN7_SHELL_EXECUTE_MAX_PARAMETER_CHARS);
}

#[test]
fn cleanup_failure_preserves_installer_exit_code() {
    const INSTALLER_FAILURE: i32 = 77;
    for exit_code in [0, INSTALLER_FAILURE] {
        let runner_dir = std::env::temp_dir().join(format!(
            "rustdesk_install_locked_{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir(&runner_dir).expect("runner directory should be created");
        let locked_directory = fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS.0)
            .open(&runner_dir)
            .expect("cleanup should be blocked by an open directory");
        let script = write_install_script(format!("exit /b {exit_code}"))
            .expect("install script should be created");
        let bootstrap = verified_install_bootstrap(&script, &runner_dir)
            .expect("verifier bootstrap should be generated");
        let output = run_install_bootstrap_for_test(&bootstrap);
        let directory_remains = runner_dir.exists();
        drop(locked_directory);
        fs::remove_dir_all(&runner_dir).expect("test directory should be removed");
        assert!(
            directory_remains,
            "open handle must prevent directory cleanup"
        );
        assert_eq!(
            output.status.code(),
            Some(exit_code),
            "cleanup must preserve the installer's result: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
