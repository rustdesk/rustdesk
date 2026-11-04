use super::*;
use crate::platform::windows::{
    installer_files::ProtectedInstallFile,
    installer_shell::{get_system_executable, path_for_cmd_environment},
    wide_string,
};
use std::{fs, os::windows::process::CommandExt, process::Command};
use windows::{
    core::{BOOL, PCWSTR},
    Win32::{
        Foundation::ERROR_SHARING_VIOLATION,
        Security::{
            Authorization::{
                ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
            },
            GetSecurityDescriptorDacl, GetSecurityDescriptorOwner,
        },
        Storage::FileSystem::FILE_ADD_FILE,
        System::Threading::CREATE_NO_WINDOW,
    },
};

#[test]
fn installer_directory_guard_blocks_reparse_writers() {
    let root = std::env::temp_dir().join(format!("rustdesk-guard-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let open_writer = || {
        OpenOptions::new()
            .access_mode(FILE_ADD_FILE.0)
            .share_mode((FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE).0)
            .custom_flags((FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT).0)
            .open(&root)
    };
    let guard = open_guarded_directory(&root).unwrap();
    let writer_error = open_writer().err().and_then(|error| error.raw_os_error());
    let staged = ProtectedInstallFile::new(&root.join("staged.tmp"), b"installer").unwrap();
    let contents = fs::read(staged.path()).unwrap();
    drop(staged);
    drop(guard);
    let writer = open_writer().unwrap();
    let guard_error = open_guarded_directory(&root)
        .err()
        .and_then(|error| error.raw_os_error());
    drop(writer);
    fs::remove_dir(&root).unwrap();
    assert_eq!(writer_error, Some(ERROR_SHARING_VIOLATION.0 as i32));
    assert_eq!(guard_error, Some(ERROR_SHARING_VIOLATION.0 as i32));
    assert_eq!(contents, b"installer");
}

#[test]
fn selected_installer_directory_supports_protected_staging() {
    let staged_path = super::super::path().unwrap().with_extension("tmp");
    let staged = ProtectedInstallFile::new(&staged_path, b"installer").unwrap();
    assert_eq!(fs::read(staged.path()).unwrap(), b"installer");
    drop(staged);
    assert!(!staged_path.exists());
}

#[test]
fn junction_targets_require_protected_ownership_and_permissions() {
    let root = std::env::temp_dir().join(format!("rustdesk-junction-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let trusted = get_system_executable("").unwrap();
    for (name, target, accepted) in [
        ("trusted", trusted.as_path(), true),
        ("unsafe", root.as_path(), false),
    ] {
        let link = root.join(name);
        let output = Command::new(get_system_executable("cmd.exe").unwrap())
            .args(["/D", "/C", "mklink", "/J"])
            .arg(&link)
            .arg(target)
            .creation_flags(CREATE_NO_WINDOW.0)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let result = super::super::candidate_parent(&link);
        fs::remove_dir(&link).unwrap();
        if accepted {
            let resolved = result.unwrap().unwrap();
            assert_eq!(resolved, fs::canonicalize(target).unwrap());
            assert!(!path_for_cmd_environment(&resolved)
                .unwrap()
                .starts_with(r"\\?\"));
        } else {
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("Unsafe installer directory"));
        }
    }
    fs::remove_dir(root).unwrap();
}

#[test]
fn installer_acl_rejects_unprivileged_control_of_staged_files() {
    for (sddl, accepted) in [
        (
            "O:SYG:SYD:P(A;;FA;;;SY)(A;;FA;;;BA)(A;;0x120026;;;BU)",
            true,
        ),
        (
            "O:BUG:SYD:P(A;;FA;;;SY)(A;;FA;;;BA)(A;;0x120026;;;BU)",
            false,
        ),
        ("O:SYG:SYD:P(A;;FA;;;SY)(A;;0x120066;;;BU)", false),
        ("O:SYG:SYD:P(A;;FA;;;SY)(A;;GW;;;BU)", false),
    ] {
        let text = wide_string(sddl);
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(text.as_ptr()),
                SDDL_REVISION_1,
                &mut descriptor,
                None,
            )
            .unwrap();
        }
        let _descriptor = unsafe { Owned::new(HLOCAL(descriptor.0)) };
        let mut owner = PSID::default();
        let mut dacl = std::ptr::null_mut();
        let mut present = BOOL::default();
        let mut defaulted = BOOL::default();
        unsafe {
            GetSecurityDescriptorOwner(descriptor, &mut owner, &mut defaulted).unwrap();
            GetSecurityDescriptorDacl(descriptor, &mut present, &mut dacl, &mut defaulted).unwrap();
        }
        assert_eq!(validate_acl(owner, dacl).is_ok(), accepted, "{sddl}");
    }
}
