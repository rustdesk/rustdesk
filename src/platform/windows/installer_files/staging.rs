use super::super::{
    installer_files::delete_staged_file, installer_shell::path_for_cmd_environment, wide_string,
};
use hbb_common::{anyhow::anyhow, log, ResultType};
use std::path::{Path, PathBuf};
use windows::{
    core::{w, Owned, PCWSTR},
    Wdk::{
        Foundation::OBJECT_ATTRIBUTES,
        Storage::FileSystem::{
            NtCreateFile, RtlDosPathNameToNtPathName_U_WithStatus, FILE_CREATE,
            FILE_DIRECTORY_FILE, FILE_OPEN_REPARSE_POINT, FILE_SYNCHRONOUS_IO_NONALERT,
        },
    },
    Win32::{
        Foundation::{HANDLE, HLOCAL, OBJ_CASE_INSENSITIVE, UNICODE_STRING},
        Security::{
            Authorization::{
                ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
            },
            PSECURITY_DESCRIPTOR,
        },
        Storage::FileSystem::{
            DELETE, FILE_ATTRIBUTE_DIRECTORY, FILE_GENERIC_READ, FILE_SHARE_READ,
        },
        System::{WindowsProgramming::RtlFreeUnicodeString, IO::IO_STATUS_BLOCK},
    },
};

// The owner can read and create children, but cannot delete or change permissions.
const STAGING_DIRECTORY_SDDL: PCWSTR = w!("D:P(A;;FA;;;SY)(A;;FA;;;BA)(A;;0x1200af;;;OW)");

pub(super) struct StagingDirectory {
    path: PathBuf,
    guard: Owned<HANDLE>,
}

impl StagingDirectory {
    pub(super) fn new(path: &Path) -> ResultType<Self> {
        let descriptor = security_descriptor()?;
        let native_path = NativePath::new(path)?;
        let attributes = OBJECT_ATTRIBUTES {
            Length: std::mem::size_of::<OBJECT_ATTRIBUTES>() as u32,
            ObjectName: &native_path.0,
            Attributes: OBJ_CASE_INSENSITIVE,
            SecurityDescriptor: descriptor.0.cast(),
            ..Default::default()
        };
        let mut handle = HANDLE::default();
        let mut status = IO_STATUS_BLOCK::default();
        // Acquire protection in the creation operation, before the parent can rename this entry.
        unsafe {
            NtCreateFile(
                &mut handle,
                FILE_GENERIC_READ | DELETE,
                &attributes,
                &mut status,
                None,
                FILE_ATTRIBUTE_DIRECTORY,
                FILE_SHARE_READ,
                FILE_CREATE,
                FILE_DIRECTORY_FILE | FILE_SYNCHRONOUS_IO_NONALERT | FILE_OPEN_REPARSE_POINT,
                None,
                0,
            )
            .ok()
        }
        .map_err(|error| {
            anyhow!(
                "Failed to create installer staging directory {}: {error}",
                path.display()
            )
        })?;
        Ok(Self {
            path: path.to_owned(),
            guard: unsafe { Owned::new(handle) },
        })
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for StagingDirectory {
    fn drop(&mut self) {
        // Delete only the empty directory we created, without reopening a mutable path.
        if let Err(error) = delete_staged_file(*self.guard) {
            log::warn!(
                "Protected installer staging directory may remain at {}: {error}",
                self.path.display()
            );
        }
    }
}

struct NativePath(UNICODE_STRING);

impl NativePath {
    fn new(path: &Path) -> ResultType<Self> {
        let path_string = wide_string(path_for_cmd_environment(path)?);
        let mut native_path = Self(UNICODE_STRING::default());
        unsafe {
            RtlDosPathNameToNtPathName_U_WithStatus(
                PCWSTR(path_string.as_ptr()),
                &mut native_path.0,
                None,
                None,
            )
            .ok()?;
        }
        Ok(native_path)
    }
}

impl Drop for NativePath {
    fn drop(&mut self) {
        unsafe { RtlFreeUnicodeString(&mut self.0) };
    }
}

fn security_descriptor() -> ResultType<Owned<HLOCAL>> {
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            STAGING_DIRECTORY_SDDL,
            SDDL_REVISION_1,
            &mut descriptor,
            None,
        )?;
    }
    Ok(unsafe { Owned::new(HLOCAL(descriptor.0)) })
}
