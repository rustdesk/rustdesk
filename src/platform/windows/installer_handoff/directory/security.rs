use hbb_common::{anyhow::anyhow, bail, ResultType};
use std::{
    fs::{File, OpenOptions},
    os::windows::{
        fs::{MetadataExt, OpenOptionsExt},
        io::AsRawHandle,
    },
    path::{Component, Path, Prefix},
};
use winapi::um::winnt::{ACCESS_ALLOWED_ACE_TYPE, ACCESS_DENIED_ACE_TYPE, FILE_PERSISTENT_ACLS};
use windows::{
    core::{w, Owned, PCWSTR},
    Win32::{
        Foundation::{HANDLE, HLOCAL},
        Security::{
            Authorization::{ConvertStringSidToSidW, GetSecurityInfo, SE_FILE_OBJECT},
            EqualSid, GetAce, IsWellKnownSid, MapGenericMask, WinBuiltinAdministratorsSid,
            WinCreatorOwnerRightsSid, WinLocalSystemSid, ACCESS_ALLOWED_ACE, ACE_HEADER, ACL,
            DACL_SECURITY_INFORMATION, GENERIC_MAPPING, INHERIT_ONLY_ACE,
            OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID,
        },
        Storage::FileSystem::{
            GetVolumeInformationByHandleW, DELETE, FILE_ALL_ACCESS, FILE_ATTRIBUTE_REPARSE_POINT,
            FILE_DELETE_CHILD, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
            FILE_GENERIC_EXECUTE, FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_READ_ATTRIBUTES,
            FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_TRAVERSE,
            FILE_WRITE_ATTRIBUTES, FILE_WRITE_EA, READ_CONTROL, WRITE_DAC, WRITE_OWNER,
        },
    },
};

const TRUSTED_INSTALLER_SID: PCWSTR =
    w!("S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464");
const UNSAFE_DIRECTORY_ACCESS: u32 = DELETE.0
    | FILE_DELETE_CHILD.0
    | WRITE_DAC.0
    | WRITE_OWNER.0
    | FILE_WRITE_ATTRIBUTES.0
    | FILE_WRITE_EA.0;
const FILE_MAPPING: GENERIC_MAPPING = GENERIC_MAPPING {
    GenericRead: FILE_GENERIC_READ.0,
    GenericWrite: FILE_GENERIC_WRITE.0,
    GenericExecute: FILE_GENERIC_EXECUTE.0,
    GenericAll: FILE_ALL_ACCESS.0,
};

pub(super) fn supports_acls(path: &Path) -> ResultType<bool> {
    let directory = OpenOptions::new()
        .access_mode(FILE_READ_ATTRIBUTES.0)
        .share_mode((FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE).0)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS.0)
        .open(path)
        .map_err(|error| {
            anyhow!(
                "Failed to inspect installer volume {}: {error}",
                path.display()
            )
        })?;
    file_supports_acls(&directory)
}

fn file_supports_acls(directory: &File) -> ResultType<bool> {
    let mut flags = 0;
    unsafe {
        GetVolumeInformationByHandleW(
            HANDLE(directory.as_raw_handle()),
            None,
            None,
            None,
            Some(&mut flags),
            None,
        )?;
    }
    Ok(flags & FILE_PERSISTENT_ACLS != 0)
}

pub(super) fn is_resolved(path: &Path) -> bool {
    matches!(path.components().next(), Some(Component::Prefix(prefix))
        if matches!(prefix.kind(), Prefix::VerbatimDisk(_)))
}

pub(super) fn guard(path: &Path) -> ResultType<Vec<File>> {
    if !is_resolved(path) {
        return Ok(Vec::new());
    }
    // Keep an empty target from becoming a junction until the protected file exists.
    let mut directories = Vec::new();
    for parent in path.ancestors().collect::<Vec<_>>().into_iter().rev() {
        let directory = inspect_directory(parent).map_err(|error| {
            anyhow!("Unsafe installer directory {}: {error:#}", parent.display())
        })?;
        directories.push(directory);
    }
    Ok(directories)
}

fn open_guarded_directory(path: &Path) -> std::io::Result<File> {
    // Metadata-only handles do not enforce sharing exclusions.
    OpenOptions::new()
        .access_mode(READ_CONTROL.0 | FILE_READ_ATTRIBUTES.0 | FILE_TRAVERSE.0)
        .share_mode(FILE_SHARE_READ.0)
        .custom_flags((FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT).0)
        .open(path)
}

fn inspect_directory(path: &Path) -> ResultType<File> {
    let directory = open_guarded_directory(path)?;
    let metadata = directory.metadata()?;
    if !metadata.is_dir() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0 {
        bail!("Resolved installer parent is not a regular directory");
    }
    if !file_supports_acls(&directory)? {
        bail!("Resolved installer volume does not support persistent ACLs");
    }
    let mut owner = PSID::default();
    let mut dacl = std::ptr::null_mut();
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    unsafe {
        GetSecurityInfo(
            HANDLE(directory.as_raw_handle()),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            Some(&mut owner),
            None,
            Some(&mut dacl),
            None,
            Some(&mut descriptor),
        )
        .ok()?;
    }
    let _descriptor = unsafe { Owned::new(HLOCAL(descriptor.0)) };
    validate_acl(owner, dacl)?;
    Ok(directory)
}

fn validate_acl(owner: PSID, dacl: *const ACL) -> ResultType<()> {
    let mut installer_sid = PSID::default();
    unsafe {
        ConvertStringSidToSidW(TRUSTED_INSTALLER_SID, &mut installer_sid)?;
    }
    let _installer_sid = unsafe { Owned::new(HLOCAL(installer_sid.0)) };
    if owner.0.is_null() || !trusted_sid(owner, installer_sid) {
        bail!("Installer parent must be owned by SYSTEM, Administrators, or TrustedInstaller");
    }
    if dacl.is_null() {
        bail!("Installer parent has no restrictive DACL");
    }
    for index in 0..unsafe { (*dacl).AceCount } {
        let mut ace = std::ptr::null_mut();
        unsafe { GetAce(dacl, u32::from(index), &mut ace)? };
        if ace.is_null() {
            bail!("Installer parent contains an invalid ACE");
        }
        validate_ace(ace.cast(), owner, installer_sid)?;
    }
    Ok(())
}

fn validate_ace(ace: *const ACE_HEADER, owner: PSID, installer_sid: PSID) -> ResultType<()> {
    let header = unsafe { &*ace };
    if u32::from(header.AceFlags) & INHERIT_ONLY_ACE.0 != 0
        || header.AceType == ACCESS_DENIED_ACE_TYPE
    {
        return Ok(());
    }
    if header.AceType != ACCESS_ALLOWED_ACE_TYPE {
        bail!("Cannot verify installer parent ACE type {}", header.AceType);
    }
    let allowed = unsafe { &*ace.cast::<ACCESS_ALLOWED_ACE>() };
    let sid = PSID(std::ptr::addr_of!(allowed.SidStart).cast_mut().cast());
    let sid = if unsafe { IsWellKnownSid(sid, WinCreatorOwnerRightsSid) }.as_bool() {
        owner
    } else {
        sid
    };
    let mut access = allowed.Mask;
    unsafe { MapGenericMask(&mut access, &FILE_MAPPING) };
    if access & UNSAFE_DIRECTORY_ACCESS != 0 && !trusted_sid(sid, installer_sid) {
        bail!("Installer parent grants unprivileged modification or deletion rights");
    }
    Ok(())
}

fn trusted_sid(sid: PSID, installer_sid: PSID) -> bool {
    unsafe {
        IsWellKnownSid(sid, WinLocalSystemSid).as_bool()
            || IsWellKnownSid(sid, WinBuiltinAdministratorsSid).as_bool()
            || EqualSid(sid, installer_sid).is_ok()
    }
}

#[cfg(test)]
mod tests;
