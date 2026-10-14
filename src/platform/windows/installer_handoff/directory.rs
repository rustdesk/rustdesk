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

pub(super) fn path() -> ResultType<PathBuf> {
    let system = get_system_executable("")?;
    let windows = system
        .parent()
        .ok_or_else(|| anyhow!("System directory has no parent"))?;
    let temporary = windows.join("Temp");
    for parent in temporary.ancestors() {
        let metadata = fs::symlink_metadata(parent)?;
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
