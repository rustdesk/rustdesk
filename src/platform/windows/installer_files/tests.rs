use super::*;
use windows::{
    core::BOOL,
    Win32::{
        Foundation::HANDLE,
        Security::{
            AccessCheck,
            Authorization::{GetNamedSecurityInfoW, SE_FILE_OBJECT},
            CreateRestrictedToken, CreateWellKnownSid, DuplicateToken, SecurityImpersonation,
            WinBuiltinAdministratorsSid, DACL_SECURITY_INFORMATION, DISABLE_MAX_PRIVILEGE,
            GENERIC_MAPPING, GROUP_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION, PRIVILEGE_SET,
            PSID, SECURITY_MAX_SID_SIZE, SID_AND_ATTRIBUTES, TOKEN_DUPLICATE, TOKEN_QUERY,
        },
        Storage::FileSystem::{
            FILE_ALL_ACCESS, FILE_GENERIC_EXECUTE, FILE_GENERIC_READ, FILE_GENERIC_WRITE,
        },
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    },
};

#[test]
fn cancelled_staging_removes_files_without_parent_delete_permission() {
    use windows::Win32::Security::{ImpersonateLoggedOnUser, RevertToSelf};
    let root = std::env::temp_dir().join(format!("rustdesk-cancel-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let path = root.join("package.tmp");
    let file = ProtectedInstallFile::new(&path, b"cancelled update").unwrap();
    let staged_path = file.path().to_owned();
    let staged_directory = file.directory().to_owned();
    // Omit DELETE_CHILD as in Windows Temp, but retain fixture ACL control.
    set_parent_acl(&root, w!("D:P(A;;0x1600af;;;OW)"));
    unsafe {
        let token = ordinary_owner_token();
        ImpersonateLoggedOnUser(*token).unwrap();
        drop(file);
        RevertToSelf().unwrap();
    }
    set_parent_acl(&root, w!("D:P(A;;FA;;;OW)"));
    let remaining = fs::symlink_metadata(&staged_path);
    let directory_remaining = staged_directory.exists();
    fs::remove_dir_all(&root).unwrap();
    assert!(
        matches!(remaining, Err(error) if error.kind() == io::ErrorKind::NotFound),
        "Cancelled staging must remove the protected package"
    );
    assert!(
        !directory_remaining,
        "Cancelled staging must remove its directory"
    );
}

fn set_parent_acl(path: &Path, sddl: PCWSTR) {
    use windows::Win32::Security::{SetFileSecurityW, PROTECTED_DACL_SECURITY_INFORMATION};
    let path = wide_string(path.to_str().unwrap());
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl,
            SDDL_REVISION_1,
            &mut descriptor,
            None,
        )
        .unwrap();
        let _descriptor = Owned::new(HLOCAL(descriptor.0));
        assert!(SetFileSecurityW(
            PCWSTR(path.as_ptr()),
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            descriptor
        )
        .as_bool());
    }
}

#[test]
fn staged_files_protect_contents_and_remove_unverified_data() {
    const HASH: &str = "304ca1638c5effa6832e0e15b958a8f74847efe4df9c3f3187216e921c168fed";
    let root = std::env::temp_dir().join(format!("rustdesk-staging-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let path = root.join("input.tmp");
    let file =
        ProtectedInstallFile::create(&path, &mut io::Cursor::new(b"rustdesk"), Some(HASH)).unwrap();
    assert_eq!(
        ordinary_owner_access(file.path()),
        (FILE_GENERIC_READ | FILE_GENERIC_EXECUTE).0
    );
    assert_eq!(fs::read(file.path()).unwrap(), b"rustdesk");
    assert!(ProtectedInstallFile::new(&path, b"replacement").is_err());
    let rejected = root.join("rejected.tmp");
    assert!(
        ProtectedInstallFile::create(&rejected, &mut io::Cursor::new(b"changed"), Some(HASH))
            .is_err()
    );
    assert!(!rejected.with_extension("stage").exists());
    drop(file);
    fs::remove_dir(root).unwrap();
}

fn ordinary_owner_access(path: &Path) -> u32 {
    let path = wide_string(path.to_str().unwrap());
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    unsafe {
        GetNamedSecurityInfoW(
            PCWSTR(path.as_ptr()),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | GROUP_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            None,
            None,
            None,
            None,
            &mut descriptor,
        )
        .ok()
        .unwrap();
    }
    let _descriptor = unsafe { Owned::new(HLOCAL(descriptor.0)) };
    let token = ordinary_owner_token();
    let mapping = GENERIC_MAPPING {
        GenericRead: FILE_GENERIC_READ.0,
        GenericWrite: FILE_GENERIC_WRITE.0,
        GenericExecute: FILE_GENERIC_EXECUTE.0,
        GenericAll: FILE_ALL_ACCESS.0,
    };
    let mut privileges = PRIVILEGE_SET::default();
    let mut size = std::mem::size_of::<PRIVILEGE_SET>() as u32;
    let mut granted = 0;
    let mut allowed = BOOL::default();
    unsafe {
        AccessCheck(
            descriptor,
            *token,
            winapi::um::winnt::MAXIMUM_ALLOWED,
            &mapping,
            Some(&mut privileges),
            &mut size,
            &mut granted,
            &mut allowed,
        )
        .unwrap();
    }
    assert!(allowed.as_bool());
    granted
}

fn ordinary_owner_token() -> Owned<HANDLE> {
    let mut original = HANDLE::default();
    unsafe {
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_QUERY | TOKEN_DUPLICATE,
            &mut original,
        )
        .unwrap();
    }
    let original = unsafe { Owned::new(original) };
    let mut sid = [0; SECURITY_MAX_SID_SIZE as usize];
    let sid_pointer = PSID(sid.as_mut_ptr().cast());
    let mut size = sid.len() as u32;
    unsafe {
        CreateWellKnownSid(
            WinBuiltinAdministratorsSid,
            None,
            Some(sid_pointer),
            &mut size,
        )
        .unwrap();
    }
    let disabled = [SID_AND_ATTRIBUTES {
        Sid: sid_pointer,
        Attributes: 0,
    }];
    let mut restricted = HANDLE::default();
    unsafe {
        CreateRestrictedToken(
            *original,
            DISABLE_MAX_PRIVILEGE,
            Some(&disabled),
            None,
            None,
            &mut restricted,
        )
        .unwrap();
    }
    let restricted = unsafe { Owned::new(restricted) };
    let mut impersonation = HANDLE::default();
    unsafe {
        DuplicateToken(*restricted, SecurityImpersonation, &mut impersonation).unwrap();
    }
    unsafe { Owned::new(impersonation) }
}

pub(in crate::platform::windows) fn release_copy_fixture(file: &mut ProtectedInstallFile) {
    let bytes = fs::read(file.path()).unwrap();
    delete_staged_file(**file.cleanup_handle.as_ref().unwrap()).unwrap();
    file.release();
    // Copy/cleanup tests use deletable children; file ACLs have their own regression.
    fs::write(file.path(), bytes).unwrap();
}

#[test]
fn released_input_directory_rejects_replacement_during_cmd_copy() {
    use std::{os::windows::process::CommandExt, process::Command};
    use windows::Win32::Security::{ImpersonateLoggedOnUser, RevertToSelf};

    let root = std::env::temp_dir().join(format!("rustdesk-copy-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let requested = root.join("package 中文 !&^@().tmp");
    let mut input = ProtectedInstallFile::new(&requested, b"verified installer").unwrap();
    release_copy_fixture(&mut input);
    let directory = input.path().parent().unwrap().to_owned();
    let moved = root.with_extension("moved");
    let token = ordinary_owner_token();
    unsafe { ImpersonateLoggedOnUser(*token).unwrap() };
    let renamed = fs::rename(&directory, &moved);
    unsafe { RevertToSelf().unwrap() };
    if renamed.is_ok() {
        fs::rename(&moved, &directory).unwrap();
    }
    let destination = root.join("copied.tmp");
    let output =
        Command::new(super::super::installer_shell::get_system_executable("cmd.exe").unwrap())
            .raw_arg(format!(
                "/D /E:ON /V:OFF /C copy /B /Y \"{}\" \"{}\"",
                input.path().display(),
                destination.display()
            ))
            .creation_flags(windows::Win32::System::Threading::CREATE_NO_WINDOW.0)
            .output()
            .unwrap();
    let copied = fs::read(&destination);
    fs::remove_file(input.path()).unwrap();
    drop(input);
    fs::remove_file(destination).unwrap();
    fs::remove_dir(root).unwrap();

    assert!(renamed.is_err(), "Released input directory was replaceable");
    assert!(output.status.success(), "{output:?}");
    assert_eq!(copied.unwrap(), b"verified installer");
}
