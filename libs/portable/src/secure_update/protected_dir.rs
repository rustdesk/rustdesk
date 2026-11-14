use std::{
    ffi::{OsStr, OsString},
    fs::{self, File, OpenOptions},
    io, iter,
    os::windows::{
        ffi::{OsStrExt, OsStringExt},
        fs::{MetadataExt, OpenOptionsExt},
        io::FromRawHandle,
    },
    path::{Path, PathBuf},
};

use super::path_error;
use windows::{
    core::{w, Error as WindowsError, Owned, PCWSTR},
    Win32::{
        Foundation::{GENERIC_READ, GENERIC_WRITE, HLOCAL, MAX_PATH, WIN32_ERROR},
        Security::{
            Authorization::{
                ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
            },
            PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES,
        },
        Storage::FileSystem::{
            CreateDirectoryW, CreateFileW, CREATE_NEW, FILE_ATTRIBUTE_NORMAL,
            FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
            FILE_SHARE_READ, FILE_SHARE_WRITE,
        },
        UI::Shell::{SHGetSpecialFolderPathW, CSIDL_COMMON_APPDATA},
    },
};

const CREATE_ATTEMPTS: usize = 16;
const RANDOM_BYTE_COUNT: usize = 16;
const HEX_CHARS_PER_BYTE: usize = 2;
const UPDATE_DIR_PREFIX: &str = "RustDeskUpdate-";
// O:BA / G:BA: owner and primary group are BUILTIN\Administrators.
// D:P: protected DACL; do not inherit permissions from the parent.
// A = allow; OI/CI = inherit to files/subdirectories; FA = full control.
// The two ACEs grant full control only to SYSTEM (SY) and Administrators (BA).
// Keep BA as owner: a user owner implicitly has WRITE_DAC and could let the
// same user's unelevated processes rewrite the ACL and tamper with the payload.
// Assigning BA as owner fails for a filtered, unelevated administrator token.
// https://learn.microsoft.com/en-us/windows/win32/secauthz/security-descriptor-string-format
const UPDATE_DIR_SDDL: PCWSTR = w!("O:BAG:BAD:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)");

pub(super) struct ProtectedDirectory {
    path: PathBuf,
    root_handle: Option<File>,
    descriptor: SecurityDescriptor,
    cleaned: bool,
}

impl ProtectedDirectory {
    pub(super) fn create() -> io::Result<Self> {
        let program_data = program_data_dir()?;
        let descriptor = SecurityDescriptor::new()?;
        for _ in 0..CREATE_ATTEMPTS {
            let path = program_data.join(format!("{UPDATE_DIR_PREFIX}{}", random_suffix()?));
            match create_directory(&path, &descriptor) {
                Ok(()) => {
                    let root_handle = open_directory(&path)
                        .map_err(|error| cleanup_create_failure(&path, error))?;
                    return Ok(Self {
                        path,
                        root_handle: Some(root_handle),
                        descriptor,
                        cleaned: false,
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "Failed to create a unique protected update directory",
        ))
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn create_directory(&self, path: &Path) -> io::Result<()> {
        create_directory(path, &self.descriptor)
    }

    pub(super) fn create_file(&self, path: &Path) -> io::Result<File> {
        let path_wide = to_wide(path.as_os_str());
        let attributes = security_attributes(&self.descriptor);
        let handle = unsafe {
            CreateFileW(
                PCWSTR(path_wide.as_ptr()),
                (GENERIC_READ | GENERIC_WRITE).0,
                FILE_SHARE_READ,
                Some(&attributes),
                CREATE_NEW,
                FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
                None,
            )
        }
        .map_err(|error| {
            path_error(
                "Failed to create protected file",
                path,
                win32_io_error(error),
            )
        })?;
        Ok(unsafe { File::from_raw_handle(handle.0) })
    }

    pub(super) fn cleanup(mut self) -> io::Result<()> {
        drop(self.root_handle.take());
        fs::remove_dir_all(&self.path).map_err(|error| {
            path_error(
                "Failed to remove protected update directory",
                &self.path,
                error,
            )
        })?;
        self.cleaned = true;
        Ok(())
    }
}

impl Drop for ProtectedDirectory {
    fn drop(&mut self) {
        if self.cleaned {
            return;
        }
        drop(self.root_handle.take());
        if let Err(error) = fs::remove_dir_all(&self.path) {
            eprintln!(
                "Failed to clean protected update directory {}: {error}",
                self.path.display()
            );
        }
    }
}

struct SecurityDescriptor(Owned<HLOCAL>);

impl SecurityDescriptor {
    fn new() -> io::Result<Self> {
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        // SAFETY: UPDATE_DIR_SDDL is a static, NUL-terminated wide string. On
        // success, the LocalAlloc allocation is transferred exactly once to Owned.
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                UPDATE_DIR_SDDL,
                SDDL_REVISION_1,
                &mut descriptor,
                None,
            )
            .map_err(win32_io_error)?;
            Ok(Self(Owned::new(HLOCAL(descriptor.0))))
        }
    }
}

fn create_directory(path: &Path, descriptor: &SecurityDescriptor) -> io::Result<()> {
    let path_wide = to_wide(path.as_os_str());
    let attributes = security_attributes(descriptor);
    unsafe { CreateDirectoryW(PCWSTR(path_wide.as_ptr()), Some(&attributes)) }.map_err(|error| {
        path_error(
            "Failed to create protected directory",
            path,
            win32_io_error(error),
        )
    })
}

fn open_directory(path: &Path) -> io::Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .share_mode((FILE_SHARE_READ | FILE_SHARE_WRITE).0)
        .custom_flags((FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT).0)
        .open(path)
        .map_err(|error| path_error("Failed to lock protected directory", path, error))?;
    let metadata = file.metadata()?;
    if !metadata.is_dir() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Protected update path is not a directory: {}",
                path.display()
            ),
        ));
    }
    Ok(file)
}

fn security_attributes(descriptor: &SecurityDescriptor) -> SECURITY_ATTRIBUTES {
    SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: (*descriptor.0).0,
        bInheritHandle: false.into(),
    }
}

fn program_data_dir() -> io::Result<PathBuf> {
    let mut path = [0_u16; MAX_PATH as usize];
    let found =
        unsafe { SHGetSpecialFolderPathW(None, &mut path, CSIDL_COMMON_APPDATA as i32, false) };
    if !found.as_bool() {
        return Err(io::Error::last_os_error());
    }
    let length = path
        .iter()
        .position(|character| *character == 0)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "ProgramData path is invalid"))?;
    Ok(PathBuf::from(OsString::from_wide(&path[..length])))
}

fn cleanup_create_failure(path: &Path, create_error: io::Error) -> io::Error {
    match fs::remove_dir(path) {
        Ok(()) => create_error,
        Err(cleanup_error) => io::Error::new(
            create_error.kind(),
            format!(
                "{create_error}; cleanup failed for {}: {cleanup_error}",
                path.display()
            ),
        ),
    }
}

fn random_suffix() -> io::Result<String> {
    let mut bytes = [0_u8; RANDOM_BYTE_COUNT];
    getrandom::getrandom(&mut bytes).map_err(|error| {
        io::Error::other(format!(
            "Failed to generate update directory suffix: {error}"
        ))
    })?;
    Ok(format!(
        "{:0width$x}",
        u128::from_be_bytes(bytes),
        width = RANDOM_BYTE_COUNT * HEX_CHARS_PER_BYTE,
    ))
}

fn win32_io_error(error: WindowsError) -> io::Error {
    match WIN32_ERROR::from_error(&error) {
        Some(code) => io::Error::from_raw_os_error(code.0 as i32),
        None => io::Error::other(error),
    }
}

fn to_wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_directory_is_created_under_program_data_and_removed() -> io::Result<()> {
        let expected_parent = program_data_dir()?;
        let directory = ProtectedDirectory::create()?;
        let path = directory.path().to_path_buf();
        assert_eq!(path.parent(), Some(expected_parent.as_path()));
        assert!(path.is_dir());
        let payload = path.join("payload.bin");
        drop(directory.create_file(&payload)?);
        assert_eq!(
            directory.create_file(&payload).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        directory.cleanup()?;
        assert!(!path.try_exists()?);
        Ok(())
    }
}
