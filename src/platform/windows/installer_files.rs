use super::{installer_handoff::directory, wide_string};
use hbb_common::{anyhow::anyhow, log, ResultType};
use std::{
    fs::{self, File},
    io::{self, Read},
    os::windows::io::{AsRawHandle, FromRawHandle},
    path::{Path, PathBuf},
};
use windows::{
    core::{w, Owned, PCWSTR},
    Win32::{
        Foundation::{DuplicateHandle, GENERIC_READ, GENERIC_WRITE, HANDLE, HLOCAL},
        Security::{
            Authorization::{
                ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
            },
            PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES,
        },
        Storage::FileSystem::{
            CreateFileW, FileDispositionInfo, SetFileInformationByHandle, CREATE_NEW, DELETE,
            FILE_ATTRIBUTE_NORMAL, FILE_DISPOSITION_INFO, FILE_SHARE_READ,
        },
        System::Threading::GetCurrentProcess,
    },
};

// Stage data before elevation instead of relaunching a verifier from a portable
// directory whose executable or DLLs could be replaced while UAC is pending.
pub(super) struct ProtectedInstallFile {
    path: PathBuf,
    cleanup_handle: Option<Owned<HANDLE>>,
}

impl ProtectedInstallFile {
    pub(super) fn new(path: &Path, bytes: &[u8]) -> ResultType<Self> {
        Self::create(path, &mut io::Cursor::new(bytes), None)
    }

    pub(super) fn copy_verified(source: &Path, expected_sha256: &str) -> ResultType<Self> {
        let mut source = File::open(source)?;
        let path = directory::path()?.with_extension("tmp");
        Self::create(&path, &mut source, Some(expected_sha256))
    }

    fn create(
        path: &Path,
        source: &mut impl Read,
        expected_sha256: Option<&str>,
    ) -> ResultType<Self> {
        let mut file = create_protected_file(path)?;
        let result = (|| -> ResultType<Owned<HANDLE>> {
            io::copy(source, &mut file)?;
            file.sync_all()?;
            if let Some(expected) = expected_sha256 {
                crate::update_hash::verify_sha256_reader(&mut file, expected).map_err(|error| {
                    anyhow!(
                        "Failed to verify staged installer file {}: {error:?}",
                        path.display()
                    )
                })?;
            }
            duplicate_cleanup_handle(&file)
        })();
        let cleanup_handle = result.map_err(|error| {
            // The initial handle can remove an incomplete file without reopening its protected path.
            if let Err(cleanup_error) = delete_staged_file(HANDLE(file.as_raw_handle())) {
                log::warn!(
                    "Failed to remove incomplete installer file {}: {cleanup_error}",
                    path.display()
                );
            }
            error
        })?;
        Ok(Self {
            path: path.to_owned(),
            cleanup_handle: Some(cleanup_handle),
        })
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn release(&mut self) {
        // cmd.exe cannot read/copy these files while a DELETE handle is open.
        // The handoff gate keeps it waiting until every staged handle is closed.
        drop(self.cleanup_handle.take());
    }
}

impl Drop for ProtectedInstallFile {
    fn drop(&mut self) {
        // Cancellation cannot reopen the protected path with DELETE permission.
        let result = match &self.cleanup_handle {
            Some(handle) => delete_staged_file(**handle),
            None => match fs::remove_file(&self.path) {
                Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error.into()),
                _ => Ok(()),
            },
        };
        if let Err(error) = result {
            log::warn!(
                "Protected installer file may remain at {}: {error}",
                self.path.display()
            );
        }
    }
}

fn create_protected_file(path: &Path) -> ResultType<File> {
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    // Restrict OWNER RIGHTS at creation so the unelevated creator cannot reopen
    // the file for writing or grant itself that permission after staging.
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            w!("D:P(A;;FA;;;SY)(A;;FA;;;BA)(A;;FRFX;;;OW)"),
            SDDL_REVISION_1,
            &mut descriptor,
            None,
        )?;
    }
    let _descriptor = unsafe { Owned::new(HLOCAL(descriptor.0)) };
    let attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: false.into(),
    };
    let path_string = path
        .to_str()
        .ok_or_else(|| anyhow!("Invalid protected installer path"))?;
    let path_string = wide_string(path_string);
    let handle = unsafe {
        CreateFileW(
            PCWSTR(path_string.as_ptr()),
            GENERIC_READ.0 | GENERIC_WRITE.0 | DELETE.0,
            FILE_SHARE_READ,
            Some(&attributes),
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
    }
    .map_err(|error| {
        anyhow!(
            "Failed to create protected installer file {}: {error}",
            path.display()
        )
    })?;
    Ok(unsafe { File::from_raw_handle(handle.0) })
}

fn duplicate_cleanup_handle(file: &File) -> ResultType<Owned<HANDLE>> {
    let mut handle = HANDLE::default();
    // Retain deletion capability, not a writable handle, while waiting for UAC.
    unsafe {
        let process = GetCurrentProcess();
        DuplicateHandle(
            process,
            HANDLE(file.as_raw_handle()),
            process,
            &mut handle,
            DELETE.0,
            false,
            Default::default(),
        )?;
        Ok(Owned::new(handle))
    }
}

fn delete_staged_file(handle: HANDLE) -> ResultType<()> {
    let disposition = FILE_DISPOSITION_INFO {
        DeleteFile: true.into(),
    };
    unsafe {
        SetFileInformationByHandle(
            handle,
            FileDispositionInfo,
            &disposition as *const _ as *const _,
            std::mem::size_of_val(&disposition) as u32,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
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
                GENERIC_MAPPING, GROUP_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION,
                PRIVILEGE_SET, PSID, SECURITY_MAX_SID_SIZE, SID_AND_ATTRIBUTES, TOKEN_DUPLICATE,
                TOKEN_QUERY,
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
        // Omit DELETE_CHILD as in Windows Temp, but retain fixture ACL control.
        set_parent_acl(&root, w!("D:P(A;;0x1600af;;;OW)"));
        unsafe {
            let token = ordinary_owner_token();
            ImpersonateLoggedOnUser(*token).unwrap();
            drop(file);
            RevertToSelf().unwrap();
        }
        set_parent_acl(&root, w!("D:P(A;;FA;;;OW)"));
        let remaining = fs::symlink_metadata(&path);
        fs::remove_dir_all(&root).unwrap();
        assert!(
            matches!(remaining, Err(error) if error.kind() == io::ErrorKind::NotFound),
            "Cancelled staging must remove the protected package"
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
            ProtectedInstallFile::create(&path, &mut io::Cursor::new(b"rustdesk"), Some(HASH))
                .unwrap();
        assert_eq!(
            ordinary_owner_access(file.path()),
            (FILE_GENERIC_READ | FILE_GENERIC_EXECUTE).0
        );
        assert_eq!(fs::read(file.path()).unwrap(), b"rustdesk");
        assert!(ProtectedInstallFile::new(&path, b"replacement").is_err());
        let rejected = root.join("rejected.tmp");
        assert!(ProtectedInstallFile::create(
            &rejected,
            &mut io::Cursor::new(b"changed"),
            Some(HASH)
        )
        .is_err());
        assert!(!rejected.exists());
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
}
