// https://learn.microsoft.com/en-us/windows/win32/secgloss/security-glossary

use super::{read_token_user_buffer, wide_string, ResultType};
use hbb_common::{anyhow::anyhow, bail};
use std::{fs, io, os::windows::ffi::OsStrExt, path::Path};
use windows::{
    core::{PCWSTR, PWSTR},
    Win32::{
        Foundation::{CloseHandle, LocalFree, HANDLE, HLOCAL},
        Security::{
            Authorization::{
                ConvertSidToStringSidW, ConvertStringSidToSidW, GetNamedSecurityInfoW,
                SetEntriesInAclW, SetNamedSecurityInfoW, EXPLICIT_ACCESS_W, SET_ACCESS,
                SE_FILE_OBJECT, TRUSTEE_IS_GROUP, TRUSTEE_IS_SID, TRUSTEE_IS_USER, TRUSTEE_W,
            },
            ACE_FLAGS, ACL, CONTAINER_INHERIT_ACE, DACL_SECURITY_INFORMATION, NO_INHERITANCE,
            OBJECT_INHERIT_ACE, PROTECTED_DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID,
            TOKEN_QUERY, TOKEN_USER,
        },
        Storage::FileSystem::{FILE_ALL_ACCESS, FILE_GENERIC_WRITE},
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    },
};

fn apply_grant_sid_allow_ace_to_path(
    path: &Path,
    sid_ptr: *mut std::ffi::c_void,
    access_mask: u32,
    is_group: bool,
    is_dir: bool,
) -> ResultType<()> {
    // Merge mode: read existing DACL and append/replace ACE via SetEntriesInAclW.
    // https://learn.microsoft.com/en-us/windows/win32/secauthz/modifying-the-acls-of-an-object-in-c--
    let mut old_dacl: *mut ACL = std::ptr::null_mut();
    let mut security_descriptor = PSECURITY_DESCRIPTOR::default();
    let path_utf16: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let get_named_result = unsafe {
        GetNamedSecurityInfoW(
            PCWSTR::from_raw(path_utf16.as_ptr()),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            Some(&mut old_dacl),
            None,
            &mut security_descriptor,
        )
    };
    if get_named_result.0 != 0 {
        bail!(
            "GetNamedSecurityInfoW failed for '{}': win32_error={}",
            path.display(),
            get_named_result.0
        );
    }
    let _sd_guard = LocalAllocGuard(security_descriptor.0);

    let inherit_flags = if is_dir {
        ACE_FLAGS(OBJECT_INHERIT_ACE.0 | CONTAINER_INHERIT_ACE.0)
    } else {
        NO_INHERITANCE
    };
    let explicit_access = [make_sid_trustee_entry(
        sid_ptr,
        access_mask,
        inherit_flags,
        is_group,
    )];
    let old_acl_option = if old_dacl.is_null() {
        None
    } else {
        Some(old_dacl as *const ACL)
    };
    let mut new_acl: *mut ACL = std::ptr::null_mut();
    let set_entries_result = unsafe {
        SetEntriesInAclW(
            Some(explicit_access.as_slice()),
            old_acl_option,
            &mut new_acl,
        )
    };
    if set_entries_result.0 != 0 {
        bail!(
            "SetEntriesInAclW failed for '{}': win32_error={}",
            path.display(),
            set_entries_result.0
        );
    }
    if new_acl.is_null() {
        bail!(
            "SetEntriesInAclW returned null ACL for '{}'",
            path.display()
        );
    }
    let _acl_guard = LocalAllocGuard(new_acl as *mut std::ffi::c_void);

    let set_named_result = unsafe {
        SetNamedSecurityInfoW(
            PCWSTR::from_raw(path_utf16.as_ptr()),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            Some(new_acl),
            None,
        )
    };
    if set_named_result.0 != 0 {
        bail!(
            "SetNamedSecurityInfoW failed for '{}': win32_error={}",
            path.display(),
            set_named_result.0
        );
    }
    Ok(())
}

/// Grants `Everyone` on `dir` recursively for helper/runtime files that must be
/// readable/executable across user contexts.
///
/// `access_mask` is the Win32 file access mask to grant recursively.
pub fn set_path_permission(dir: &Path, access_mask: u32) -> ResultType<()> {
    let metadata = fs::symlink_metadata(dir).map_err(|e| {
        anyhow!(
            "Failed to inspect ACL target directory '{}': {}",
            dir.display(),
            e
        )
    })?;
    if !metadata.file_type().is_dir() {
        bail!("ACL target is not a directory: '{}'", dir.display());
    }

    let everyone_sid = sid_string_to_local_alloc_guard("S-1-1-0")?;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(path) = stack.pop() {
        let metadata = fs::symlink_metadata(&path)
            .map_err(|e| anyhow!("Failed to inspect ACL target '{}': {}", path.display(), e))?;
        let is_dir = metadata.file_type().is_dir();
        apply_grant_sid_allow_ace_to_path(
            &path,
            everyone_sid.as_sid_ptr(),
            access_mask,
            true,
            is_dir,
        )?;
        if !is_dir {
            continue;
        }
        for entry in fs::read_dir(&path)
            .map_err(|e| anyhow!("Failed to list ACL target dir '{}': {}", path.display(), e))?
        {
            let entry = entry.map_err(|e| {
                anyhow!(
                    "Failed to read ACL target dir entry under '{}': {}",
                    path.display(),
                    e
                )
            })?;
            stack.push(entry.path());
        }
    }
    Ok(())
}

/// Returns the current process user SID as a standard SID string
/// (for example: `S-1-5-18`).
///
/// Source:
/// - Official SID-to-string API (`ConvertSidToStringSidW`):
///   https://learn.microsoft.com/en-us/windows/win32/api/sddl/nf-sddl-convertsidtostringsidw
pub(crate) fn current_process_user_sid_string() -> ResultType<String> {
    let mut token = HANDLE::default();
    let result = (|| -> ResultType<String> {
        unsafe {
            OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)
                .map_err(|e| anyhow!("Failed to open current process token: {}", e))?;
        }

        let buffer = unsafe { read_token_user_buffer(token, "current process")? };
        let token_user: TOKEN_USER =
            unsafe { std::ptr::read_unaligned(buffer.as_ptr() as *const TOKEN_USER) };
        if token_user.User.Sid.0.is_null() {
            bail!("Token SID is null");
        }

        let mut sid_string_ptr = PWSTR::null();
        unsafe {
            ConvertSidToStringSidW(token_user.User.Sid, &mut sid_string_ptr).map_err(|e| {
                anyhow!(
                    "ConvertSidToStringSidW failed for current process token SID: {}",
                    e
                )
            })?;
        }
        if sid_string_ptr.is_null() {
            bail!("ConvertSidToStringSidW returned null SID string pointer");
        }
        let _sid_string_guard = LocalAllocGuard(sid_string_ptr.0 as *mut std::ffi::c_void);
        unsafe {
            sid_string_ptr
                .to_string()
                .map_err(|e| anyhow!("Failed to decode SID string as UTF-16: {}", e))
        }
    })();

    if !token.is_invalid() {
        unsafe {
            let _ = CloseHandle(token);
        }
    }
    result
}

/// Hardens ACLs for portable-service shared-memory path (directory or file).
///
/// Why:
/// - Shared memory used by portable service carries runtime control/data and must not inherit
///   broad/default ACLs.
/// - We explicitly grant only trusted principals and remove broad groups to reduce local
///   privilege-boundary bypass risk.
///
/// ACL policy applied via Win32 ACL APIs (`SetEntriesInAclW` + `SetNamedSecurityInfoW`):
/// - common (directory + file):
///   - `S-1-5-18` (LocalSystem): full control
///   - `S-1-5-32-544` (Built-in Administrators): full control
///   - `current_process_user_sid_string()` result: full control
/// - directory (`portable_service_shmem` parent):
///   - keep `Authenticated Users` directory-level write so other local accounts can
///     create their own runtime shmem files after account switching
///   - `FILE_GENERIC_WRITE + NO_INHERITANCE` means write/create on this directory itself;
///     it is intentionally not inherited by children.
///     Reference:
///     - File access rights:
///       https://learn.microsoft.com/en-us/windows/win32/fileio/file-access-rights-constants
///     - ACE inheritance rules:
///       https://learn.microsoft.com/en-us/windows/win32/secauthz/ace-inheritance-rules
///   - remove `Everyone` and `Users` grants
/// - file (`shared_memory*` flink):
///   - remove broad grants:
///     - `S-1-1-0` (Everyone)
///     - `S-1-5-11` (Authenticated Users)
///     - `S-1-5-32-545` (Users)
///
/// https://learn.microsoft.com/en-us/windows/win32/secauthz/well-known-sids
pub fn set_path_permission_for_portable_service_shmem_dir(path: &Path) -> ResultType<()> {
    set_path_permission_for_portable_service_shmem_impl(path, true)
}

#[inline]
pub fn set_path_permission_for_portable_service_shmem_file(path: &Path) -> ResultType<()> {
    set_path_permission_for_portable_service_shmem_impl(path, false)
}

#[derive(Debug)]
pub(super) struct LocalAllocGuard(*mut std::ffi::c_void);

impl LocalAllocGuard {
    #[inline]
    pub(super) fn as_sid_ptr(&self) -> *mut std::ffi::c_void {
        self.0
    }
}

impl Drop for LocalAllocGuard {
    fn drop(&mut self) {
        if self.0.is_null() {
            return;
        }
        // Buffers returned by ConvertStringSidToSidW / SetEntriesInAclW /
        // ConvertSidToStringSidW are LocalAlloc-owned and must be LocalFree'ed.
        unsafe {
            let _ = LocalFree(Some(HLOCAL(self.0)));
        }
    }
}

#[inline]
pub(super) fn sid_string_to_local_alloc_guard(sid: &str) -> ResultType<LocalAllocGuard> {
    let sid_utf16 = wide_string(sid);
    let mut sid_ptr = PSID::default();
    unsafe {
        ConvertStringSidToSidW(PCWSTR::from_raw(sid_utf16.as_ptr()), &mut sid_ptr)
            .map_err(|e| anyhow!("ConvertStringSidToSidW failed for '{}': {}", sid, e))?;
    }
    if sid_ptr.0.is_null() {
        bail!("ConvertStringSidToSidW returned null SID for '{}'", sid);
    }
    Ok(LocalAllocGuard(sid_ptr.0))
}

#[inline]
fn make_sid_trustee_entry(
    sid_ptr: *mut std::ffi::c_void,
    access_permissions: u32,
    inheritance: ACE_FLAGS,
    is_group: bool,
) -> EXPLICIT_ACCESS_W {
    // `is_group` is explicitly provided by the caller from the concrete SID semantic
    // (e.g. Administrators/Authenticated Users => group, LocalSystem/current user => user).
    EXPLICIT_ACCESS_W {
        grfAccessPermissions: access_permissions,
        grfAccessMode: SET_ACCESS,
        grfInheritance: inheritance,
        Trustee: TRUSTEE_W {
            pMultipleTrustee: std::ptr::null_mut(),
            MultipleTrusteeOperation: Default::default(),
            TrusteeForm: TRUSTEE_IS_SID,
            TrusteeType: if is_group {
                TRUSTEE_IS_GROUP
            } else {
                TRUSTEE_IS_USER
            },
            // SAFETY: With TrusteeForm=TRUSTEE_IS_SID, ptstrName is interpreted as PSID.
            ptstrName: PWSTR::from_raw(sid_ptr as *mut u16),
        },
    }
}

fn set_path_permission_for_portable_service_shmem_impl(
    path: &Path,
    expect_dir: bool,
) -> ResultType<()> {
    let metadata_result = fs::symlink_metadata(path);
    if expect_dir {
        let metadata = metadata_result.map_err(|e| {
            anyhow!(
                "Failed to inspect portable service shared-memory ACL directory '{}': {}",
                path.display(),
                e
            )
        })?;
        if !metadata.file_type().is_dir() {
            bail!(
                "Portable service shared-memory ACL target is not a directory: '{}'",
                path.display()
            );
        }
    } else {
        match metadata_result {
            Ok(metadata) => {
                if metadata.file_type().is_dir() {
                    bail!(
                        "Portable service shared-memory ACL target is a directory, expected file-like path: '{}'",
                        path.display()
                    );
                }
            }
            Err(e)
                if e.kind() == io::ErrorKind::NotFound
                    || e.kind() == io::ErrorKind::PermissionDenied =>
            {
                // Keep going and let Win32 ACL APIs return the final OS error.
                // `Path::exists()/is_file()` and metadata can collapse ACL-denied paths into
                // a false "not found" signal under restricted directory ACLs.
            }
            Err(e) => {
                bail!(
                    "Failed to inspect portable service shared-memory ACL target '{}': {}",
                    path.display(),
                    e
                );
            }
        }
    }

    let user_sid = current_process_user_sid_string()?;
    let local_system_sid = sid_string_to_local_alloc_guard("S-1-5-18")?;
    let administrators_sid = sid_string_to_local_alloc_guard("S-1-5-32-544")?;
    let current_user_sid = sid_string_to_local_alloc_guard(&user_sid)?;
    let authenticated_users_sid = if expect_dir {
        Some(sid_string_to_local_alloc_guard("S-1-5-11")?)
    } else {
        None
    };

    let inherit_flags = if expect_dir {
        ACE_FLAGS(OBJECT_INHERIT_ACE.0 | CONTAINER_INHERIT_ACE.0)
    } else {
        NO_INHERITANCE
    };
    let mut entries = vec![
        make_sid_trustee_entry(
            local_system_sid.as_sid_ptr(),
            FILE_ALL_ACCESS.0,
            inherit_flags,
            false,
        ),
        make_sid_trustee_entry(
            administrators_sid.as_sid_ptr(),
            FILE_ALL_ACCESS.0,
            inherit_flags,
            true,
        ),
        make_sid_trustee_entry(
            current_user_sid.as_sid_ptr(),
            FILE_ALL_ACCESS.0,
            inherit_flags,
            false,
        ),
    ];
    if let Some(auth_sid) = authenticated_users_sid.as_ref() {
        // Keep the shared parent directory multi-user writable at directory level.
        entries.push(make_sid_trustee_entry(
            auth_sid.as_sid_ptr(),
            FILE_GENERIC_WRITE.0,
            NO_INHERITANCE,
            true,
        ));
    }

    // Rebuild mode: build a fresh DACL (old ACL not merged) and apply as protected.
    // This avoids carrying over broad legacy ACEs from inherited/default ACLs.
    // Reference:
    // - SetEntriesInAclW:
    //   https://learn.microsoft.com/en-us/windows/win32/api/aclapi/nf-aclapi-setentriesinaclw
    // - SetNamedSecurityInfoW (PROTECTED_DACL_SECURITY_INFORMATION):
    //   https://learn.microsoft.com/en-us/windows/win32/api/aclapi/nf-aclapi-setnamedsecurityinfow
    let mut new_acl: *mut ACL = std::ptr::null_mut();
    let set_entries_result =
        unsafe { SetEntriesInAclW(Some(entries.as_slice()), None, &mut new_acl) };
    if set_entries_result.0 != 0 {
        bail!(
            "SetEntriesInAclW failed for '{}': win32_error={}",
            path.display(),
            set_entries_result.0
        );
    }
    if new_acl.is_null() {
        bail!(
            "SetEntriesInAclW returned null ACL for '{}'",
            path.display()
        );
    }
    let _acl_guard = LocalAllocGuard(new_acl as *mut std::ffi::c_void);

    let path_utf16: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let security_info = DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION;
    let set_named_result = unsafe {
        SetNamedSecurityInfoW(
            PCWSTR::from_raw(path_utf16.as_ptr()),
            SE_FILE_OBJECT,
            security_info,
            None,
            None,
            Some(new_acl),
            None,
        )
    };
    if set_named_result.0 != 0 {
        bail!(
            "SetNamedSecurityInfoW failed for '{}': win32_error={}",
            path.display(),
            set_named_result.0
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        current_process_user_sid_string, set_path_permission,
        set_path_permission_for_portable_service_shmem_dir,
        set_path_permission_for_portable_service_shmem_file, sid_string_to_local_alloc_guard,
        LocalAllocGuard, ResultType,
    };
    use hbb_common::bail;
    use std::{
        fs,
        os::windows::ffi::OsStrExt,
        path::{Path, PathBuf},
    };
    use windows::{
        core::PCWSTR,
        Win32::{
            Security::{
                AclSizeInformation,
                Authorization::{GetNamedSecurityInfoW, SE_FILE_OBJECT},
                EqualSid as WinEqualSid, GetAce, GetAclInformation, GetSecurityDescriptorControl,
                ACCESS_ALLOWED_ACE, ACE_HEADER, ACL, ACL_SIZE_INFORMATION,
                DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID, SE_DACL_PROTECTED,
            },
            Storage::FileSystem::{
                FILE_ALL_ACCESS, FILE_GENERIC_EXECUTE, FILE_GENERIC_READ, FILE_GENERIC_WRITE,
            },
        },
    };

    const ACCESS_ALLOWED_ACE_TYPE_U8: u8 = 0;

    fn unique_acl_test_path(prefix: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "rustdesk_acl_{}_{}_{}",
            prefix,
            std::process::id(),
            hbb_common::rand::random::<u32>()
        ))
    }

    fn get_file_dacl(path: &Path) -> ResultType<(*mut ACL, LocalAllocGuard)> {
        let mut dacl: *mut ACL = std::ptr::null_mut();
        let mut sd = PSECURITY_DESCRIPTOR::default();
        let path_utf16: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let result = unsafe {
            GetNamedSecurityInfoW(
                PCWSTR::from_raw(path_utf16.as_ptr()),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                None,
                None,
                Some(&mut dacl),
                None,
                &mut sd,
            )
        };
        if result.0 != 0 {
            bail!(
                "GetNamedSecurityInfoW failed for '{}': win32_error={}",
                path.display(),
                result.0
            );
        }
        if dacl.is_null() || sd.0.is_null() {
            bail!("DACL/security descriptor missing for '{}'", path.display());
        }
        Ok((dacl, LocalAllocGuard(sd.0)))
    }

    fn has_allow_ace_with_mask(
        dacl: *const ACL,
        sid_ptr: *mut std::ffi::c_void,
        mask: u32,
    ) -> bool {
        let mut info = ACL_SIZE_INFORMATION::default();
        if unsafe {
            GetAclInformation(
                dacl,
                &mut info as *mut _ as *mut std::ffi::c_void,
                std::mem::size_of::<ACL_SIZE_INFORMATION>() as u32,
                AclSizeInformation,
            )
        }
        .is_err()
        {
            return false;
        }
        for index in 0..info.AceCount {
            let mut ace_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
            if unsafe { GetAce(dacl, index, &mut ace_ptr) }.is_err() || ace_ptr.is_null() {
                continue;
            }
            let header = unsafe { &*(ace_ptr as *const ACE_HEADER) };
            if header.AceType != ACCESS_ALLOWED_ACE_TYPE_U8 {
                continue;
            }
            let allowed = unsafe { &*(ace_ptr as *const ACCESS_ALLOWED_ACE) };
            let ace_sid = PSID((&allowed.SidStart as *const u32) as *mut std::ffi::c_void);
            if unsafe { WinEqualSid(PSID(sid_ptr), ace_sid) }.is_ok()
                && (allowed.Mask & mask) == mask
            {
                return true;
            }
        }
        false
    }

    fn has_any_allow_ace_for_sid(dacl: *const ACL, sid_ptr: *mut std::ffi::c_void) -> bool {
        has_allow_ace_with_mask(dacl, sid_ptr, 0)
    }

    fn is_dacl_protected(sd: PSECURITY_DESCRIPTOR) -> bool {
        let mut control: u16 = 0;
        let mut revision: u32 = 0;
        if unsafe { GetSecurityDescriptorControl(sd, &mut control, &mut revision) }.is_err() {
            return false;
        }
        (control & SE_DACL_PROTECTED.0) != 0
    }

    #[test]
    fn test_portable_service_shmem_dir_acl_policy() {
        let dir = unique_acl_test_path("dir");
        fs::create_dir_all(&dir).unwrap();
        set_path_permission_for_portable_service_shmem_dir(&dir).unwrap();

        let (dacl, sd_guard) = get_file_dacl(&dir).unwrap();
        let current_user_sid =
            sid_string_to_local_alloc_guard(&current_process_user_sid_string().unwrap()).unwrap();
        let system_sid = sid_string_to_local_alloc_guard("S-1-5-18").unwrap();
        let admin_sid = sid_string_to_local_alloc_guard("S-1-5-32-544").unwrap();
        let auth_users_sid = sid_string_to_local_alloc_guard("S-1-5-11").unwrap();
        let everyone_sid = sid_string_to_local_alloc_guard("S-1-1-0").unwrap();
        let users_sid = sid_string_to_local_alloc_guard("S-1-5-32-545").unwrap();

        assert!(has_allow_ace_with_mask(
            dacl,
            system_sid.as_sid_ptr(),
            FILE_ALL_ACCESS.0
        ));
        assert!(has_allow_ace_with_mask(
            dacl,
            admin_sid.as_sid_ptr(),
            FILE_ALL_ACCESS.0
        ));
        assert!(has_allow_ace_with_mask(
            dacl,
            current_user_sid.as_sid_ptr(),
            FILE_ALL_ACCESS.0
        ));
        assert!(has_allow_ace_with_mask(
            dacl,
            auth_users_sid.as_sid_ptr(),
            FILE_GENERIC_WRITE.0
        ));
        assert!(!has_any_allow_ace_for_sid(dacl, everyone_sid.as_sid_ptr()));
        assert!(!has_any_allow_ace_for_sid(dacl, users_sid.as_sid_ptr()));
        assert!(is_dacl_protected(PSECURITY_DESCRIPTOR(
            sd_guard.as_sid_ptr()
        )));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_portable_service_shmem_file_acl_policy() {
        let dir = unique_acl_test_path("file");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("shared_memory_portable_service_test");
        fs::write(&file, b"x").unwrap();
        set_path_permission_for_portable_service_shmem_file(&file).unwrap();

        let (dacl, sd_guard) = get_file_dacl(&file).unwrap();
        let current_user_sid =
            sid_string_to_local_alloc_guard(&current_process_user_sid_string().unwrap()).unwrap();
        let system_sid = sid_string_to_local_alloc_guard("S-1-5-18").unwrap();
        let admin_sid = sid_string_to_local_alloc_guard("S-1-5-32-544").unwrap();
        let auth_users_sid = sid_string_to_local_alloc_guard("S-1-5-11").unwrap();
        let everyone_sid = sid_string_to_local_alloc_guard("S-1-1-0").unwrap();
        let users_sid = sid_string_to_local_alloc_guard("S-1-5-32-545").unwrap();

        assert!(has_allow_ace_with_mask(
            dacl,
            system_sid.as_sid_ptr(),
            FILE_ALL_ACCESS.0
        ));
        assert!(has_allow_ace_with_mask(
            dacl,
            admin_sid.as_sid_ptr(),
            FILE_ALL_ACCESS.0
        ));
        assert!(has_allow_ace_with_mask(
            dacl,
            current_user_sid.as_sid_ptr(),
            FILE_ALL_ACCESS.0
        ));
        assert!(!has_any_allow_ace_for_sid(
            dacl,
            auth_users_sid.as_sid_ptr()
        ));
        assert!(!has_any_allow_ace_for_sid(dacl, everyone_sid.as_sid_ptr()));
        assert!(!has_any_allow_ace_for_sid(dacl, users_sid.as_sid_ptr()));
        assert!(is_dacl_protected(PSECURITY_DESCRIPTOR(
            sd_guard.as_sid_ptr()
        )));

        let _ = fs::remove_file(&file);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_set_path_permission_rx_applies_recursively() {
        let root = unique_acl_test_path("set_path_permission");
        let child_dir = root.join("child");
        let child_file = child_dir.join("helper.exe");
        fs::create_dir_all(&child_dir).unwrap();
        fs::write(&child_file, b"x").unwrap();

        if let Err(err) = set_path_permission(&root, FILE_GENERIC_READ.0 | FILE_GENERIC_EXECUTE.0) {
            let text = err.to_string();
            let _ = fs::remove_file(&child_file);
            let _ = fs::remove_dir_all(&root);
            if text.contains("win32_error=5") || text.contains("Access is denied") {
                eprintln!(
                    "skip test_set_path_permission_rx_applies_recursively: insufficient WRITE_DAC in current environment: {}",
                    text
                );
                return;
            }
            panic!("set_path_permission failed unexpectedly: {}", text);
        }

        let everyone_sid = sid_string_to_local_alloc_guard("S-1-1-0").unwrap();
        let rx_mask = FILE_GENERIC_READ.0 | FILE_GENERIC_EXECUTE.0;
        for target in [&root, &child_dir, &child_file] {
            let (dacl, _sd_guard) = get_file_dacl(target).unwrap();
            assert!(
                has_allow_ace_with_mask(dacl, everyone_sid.as_sid_ptr(), rx_mask),
                "Everyone RX grant missing on '{}'",
                target.display()
            );
        }

        let _ = fs::remove_file(&child_file);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_portable_service_shmem_dir_acl_rejects_file_target() {
        let dir = unique_acl_test_path("dir_target_file");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("target.txt");
        fs::write(&file, b"x").unwrap();
        let result = set_path_permission_for_portable_service_shmem_dir(&file);
        assert!(result.is_err());
        let _ = fs::remove_file(&file);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_portable_service_shmem_file_acl_rejects_dir_target() {
        let dir = unique_acl_test_path("file_target_dir");
        fs::create_dir_all(&dir).unwrap();
        let result = set_path_permission_for_portable_service_shmem_file(&dir);
        assert!(result.is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_portable_service_shmem_file_acl_rejects_missing_target() {
        let path = unique_acl_test_path("missing").join("shared_memory_missing");
        let result = set_path_permission_for_portable_service_shmem_file(&path);
        assert!(result.is_err());
    }
}
