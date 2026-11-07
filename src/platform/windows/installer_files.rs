mod staging;

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
    _directory: staging::StagingDirectory,
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
        let filename = path
            .file_name()
            .ok_or_else(|| anyhow!("Protected installer path has no filename"))?;
        let directory = staging::StagingDirectory::new(&path.with_extension("stage"))?;
        let path = directory.path().join(filename);
        let mut file = create_protected_file(&path)?;
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
            path,
            cleanup_handle: Some(cleanup_handle),
            _directory: directory,
        })
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn directory(&self) -> &Path {
        self._directory.path()
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
pub(super) mod tests;
