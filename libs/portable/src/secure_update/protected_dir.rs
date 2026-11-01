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
    ptr,
};

use winapi::{
    shared::{
        bcrypt::{BCryptGenRandom, BCRYPT_SUCCESS, BCRYPT_USE_SYSTEM_PREFERRED_RNG},
        minwindef::{FALSE, MAX_PATH},
        sddl::{ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1},
    },
    um::{
        fileapi::{CreateDirectoryW, CreateFileW, CREATE_NEW},
        handleapi::INVALID_HANDLE_VALUE,
        minwinbase::SECURITY_ATTRIBUTES,
        shlobj::{SHGetSpecialFolderPathW, CSIDL_COMMON_APPDATA},
        winbase::{LocalFree, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT},
        winnt::{
            FILE_ATTRIBUTE_NORMAL, FILE_ATTRIBUTE_REPARSE_POINT, FILE_SHARE_READ, FILE_SHARE_WRITE,
            GENERIC_READ, GENERIC_WRITE, PSECURITY_DESCRIPTOR,
        },
    },
};

const CREATE_ATTEMPTS: usize = 16;
const RANDOM_BYTE_COUNT: usize = 16;
const HEX_CHARS_PER_BYTE: usize = 2;
const UPDATE_DIR_PREFIX: &str = "RustDeskUpdate-";
const UPDATE_DIR_SDDL: &str = "O:BAG:BAD:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)";
const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";
const HALF_BYTE_BITS: u8 = 4;
const HALF_BYTE_MASK: u8 = 0x0f;

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
        create_file(path, &self.descriptor)
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

struct SecurityDescriptor(PSECURITY_DESCRIPTOR);

impl SecurityDescriptor {
    fn new() -> io::Result<Self> {
        let sddl = to_wide(OsStr::new(UPDATE_DIR_SDDL));
        let mut descriptor = ptr::null_mut();
        let converted = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1 as u32,
                &mut descriptor,
                ptr::null_mut(),
            )
        };
        if converted == FALSE {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(descriptor))
    }
}

impl Drop for SecurityDescriptor {
    fn drop(&mut self) {
        unsafe {
            let _ = LocalFree(self.0.cast());
        }
    }
}

fn create_directory(path: &Path, descriptor: &SecurityDescriptor) -> io::Result<()> {
    let path_wide = to_wide(path.as_os_str());
    let mut attributes = security_attributes(descriptor);
    if unsafe { CreateDirectoryW(path_wide.as_ptr(), &mut attributes) } == FALSE {
        return Err(path_error(
            "Failed to create protected directory",
            path,
            io::Error::last_os_error(),
        ));
    }
    Ok(())
}

fn create_file(path: &Path, descriptor: &SecurityDescriptor) -> io::Result<File> {
    let path_wide = to_wide(path.as_os_str());
    let mut attributes = security_attributes(descriptor);
    let handle = unsafe {
        CreateFileW(
            path_wide.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ,
            &mut attributes,
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
            ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(path_error(
            "Failed to create protected file",
            path,
            io::Error::last_os_error(),
        ));
    }
    Ok(unsafe { File::from_raw_handle(handle.cast()) })
}

fn open_directory(path: &Path) -> io::Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(|error| path_error("Failed to lock protected directory", path, error))?;
    let metadata = file.metadata()?;
    if !metadata.is_dir() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
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
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: FALSE,
    }
}

fn program_data_dir() -> io::Result<PathBuf> {
    let mut path = [0_u16; MAX_PATH];
    let found = unsafe {
        SHGetSpecialFolderPathW(
            ptr::null_mut(),
            path.as_mut_ptr(),
            CSIDL_COMMON_APPDATA,
            FALSE,
        )
    };
    if found == FALSE {
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
    let status = unsafe {
        BCryptGenRandom(
            ptr::null_mut(),
            bytes.as_mut_ptr(),
            bytes.len() as u32,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    if !BCRYPT_SUCCESS(status) {
        return Err(io::Error::other(format!(
            "BCryptGenRandom failed with NTSTATUS 0x{status:08x}"
        )));
    }
    let mut suffix = String::with_capacity(RANDOM_BYTE_COUNT * HEX_CHARS_PER_BYTE);
    for byte in bytes {
        suffix.push(HEX_DIGITS[(byte >> HALF_BYTE_BITS) as usize] as char);
        suffix.push(HEX_DIGITS[(byte & HALF_BYTE_MASK) as usize] as char);
    }
    Ok(suffix)
}

fn path_error(action: &str, path: &Path, error: io::Error) -> io::Error {
    io::Error::new(
        error.kind(),
        format!("{action} {}: {error}", path.display()),
    )
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
        drop(directory.create_file(&path.join("payload.bin"))?);
        directory.cleanup()?;
        assert!(!path.try_exists()?);
        Ok(())
    }
}
