use super::super::{
    installer_shell::{get_system_executable, path_for_cmd_environment},
    wide_string,
};
use hbb_common::{anyhow::anyhow, bail, ResultType};
use std::{
    fs, mem,
    os::windows::fs::MetadataExt,
    path::{Path, PathBuf},
};
use windows::{
    core::{w, PCWSTR},
    Win32::{
        Foundation::{LocalFree, HLOCAL},
        Security::{
            Authorization::{
                ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
            },
            PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES,
        },
        Storage::FileSystem::{CreateDirectoryW, FILE_ATTRIBUTE_REPARSE_POINT},
    },
};

pub(in crate::platform::windows) fn path() -> ResultType<PathBuf> {
    let system = get_system_executable("")?;
    let windows = system
        .parent()
        .ok_or_else(|| anyhow!("System directory has no parent"))?;
    let temporary = windows.join("Temp");
    for parent in temporary.ancestors() {
        let metadata = fs::symlink_metadata(parent).map_err(|err| {
            anyhow!(
                "Failed to inspect installer temporary parent {}: {err}",
                parent.display()
            )
        })?;
        if !metadata.is_dir() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0 {
            bail!(
                "Installer temporary parent is not a regular directory: {}",
                parent.display()
            );
        }
    }
    Ok(temporary.join(format!(
        "rustdesk-install-{}",
        uuid::Uuid::new_v4().simple()
    )))
}

pub(super) fn create(path: &Path) -> ResultType<()> {
    let wide_path = wide_string(path_for_cmd_environment(path)?);
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    // OWNER RIGHTS removes the ordinary creator's implicit WRITE_DAC permission.
    // The owner can inspect the ACL, but only Administrators/SYSTEM can modify contents.
    // https://learn.microsoft.com/windows-server/identity/ad-ds/manage/understand-special-identities-groups#owner-rights
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            w!("D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;RC;;;OW)"),
            SDDL_REVISION_1,
            &mut descriptor,
            None,
        )?;
    }
    let attributes = SECURITY_ATTRIBUTES {
        nLength: mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: false.into(),
    };
    let result = unsafe { CreateDirectoryW(PCWSTR(wide_path.as_ptr()), Some(&attributes)) };
    unsafe { LocalFree(Some(HLOCAL(descriptor.0))) };
    result.map_err(|err| {
        anyhow!(
            "Failed to create protected installer directory {}: {err}",
            path.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use winapi::um::winnt::MAXIMUM_ALLOWED;
    use windows::{
        core::{Owned, BOOL},
        Win32::{
            Foundation::HANDLE,
            Security::{
                AccessCheck,
                Authorization::{GetNamedSecurityInfoW, SE_FILE_OBJECT},
                DuplicateToken, SecurityImpersonation, DACL_SECURITY_INFORMATION, GENERIC_MAPPING,
                GROUP_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION, PRIVILEGE_SET,
                TOKEN_DUPLICATE, TOKEN_QUERY,
            },
            Storage::FileSystem::{
                FILE_ALL_ACCESS, FILE_GENERIC_EXECUTE, FILE_GENERIC_READ, FILE_GENERIC_WRITE,
                READ_CONTROL,
            },
            System::Threading::{GetCurrentProcess, OpenProcessToken},
        },
    };

    #[test]
    fn protected_directory_enforces_creator_permissions() {
        let elevated =
            super::super::super::is_elevated(None).expect("creator elevation should be available");
        // The user-owned parent allows cleanup through DELETE_CHILD. AccessCheck
        // below checks the child's DACL, not that separate parent permission.
        let directory = std::env::temp_dir().join(format!(
            "rustdesk_installer_acl_{}",
            uuid::Uuid::new_v4().simple()
        ));
        create(&directory).expect("production directory creation should succeed");
        let access = creator_directory_access(&directory);
        fs::remove_dir(&directory).expect("test directory should be removed");
        let expected = if elevated {
            FILE_ALL_ACCESS
        } else {
            READ_CONTROL
        };
        assert_eq!(
            access.expect("the created directory's access should be checked"),
            (true, expected.0),
            "only elevated administrators may modify the protected directory"
        );
    }

    fn creator_directory_access(path: &Path) -> ResultType<(bool, u32)> {
        let wide_path = wide_string(path_for_cmd_environment(path)?);
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        unsafe {
            GetNamedSecurityInfoW(
                PCWSTR(wide_path.as_ptr()),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION | GROUP_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                None,
                None,
                None,
                None,
                &mut descriptor,
            )
            .ok()?;
        }
        let _descriptor = unsafe { Owned::new(HLOCAL(descriptor.0)) };
        let token = creator_impersonation_token()?;
        let mapping = GENERIC_MAPPING {
            GenericRead: FILE_GENERIC_READ.0,
            GenericWrite: FILE_GENERIC_WRITE.0,
            GenericExecute: FILE_GENERIC_EXECUTE.0,
            GenericAll: FILE_ALL_ACCESS.0,
        };
        let mut privileges = PRIVILEGE_SET::default();
        let mut privilege_size = mem::size_of::<PRIVILEGE_SET>() as u32;
        let mut granted = 0;
        let mut allowed = BOOL::default();
        unsafe {
            AccessCheck(
                descriptor,
                *token,
                MAXIMUM_ALLOWED,
                &mapping,
                Some(&mut privileges),
                &mut privilege_size,
                &mut granted,
                &mut allowed,
            )?;
        }
        Ok((allowed.as_bool(), granted))
    }

    fn creator_impersonation_token() -> ResultType<Owned<HANDLE>> {
        let mut token = HANDLE::default();
        unsafe {
            OpenProcessToken(
                GetCurrentProcess(),
                TOKEN_QUERY | TOKEN_DUPLICATE,
                &mut token,
            )?
        };
        let token = unsafe { Owned::new(token) };
        let mut impersonation = HANDLE::default();
        unsafe { DuplicateToken(*token, SecurityImpersonation, &mut impersonation)? };
        Ok(unsafe { Owned::new(impersonation) })
    }
}
