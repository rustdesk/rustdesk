use std::{
    fs::File,
    io::{self, Write},
};

pub(crate) fn write(path: &str, config: &str) -> io::Result<()> {
    let mut file = create_private_file(path).map_err(|e| {
        if e.kind() == io::ErrorKind::AlreadyExists {
            io::Error::new(e.kind(), "Choose a new file name for the MCP configuration")
        } else {
            e
        }
    })?;
    if let Err(e) = file
        .write_all(config.as_bytes())
        .and_then(|_| file.sync_all())
    {
        file.set_len(0)?;
        return Err(e);
    }
    Ok(())
}

#[cfg(unix)]
fn create_private_file(path: &str) -> io::Result<File> {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let file = File::options()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    if file.metadata()?.permissions().mode() & 0o077 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Choose a location that supports private files",
        ));
    }
    #[cfg(target_os = "macos")]
    reject_inherited_acl(&file)?;
    Ok(file)
}

#[cfg(target_os = "macos")]
fn reject_inherited_acl(file: &File) -> io::Result<()> {
    use std::{ffi::c_void, os::fd::AsRawFd};
    extern "C" {
        fn acl_get_fd(fd: i32) -> *mut c_void;
        fn acl_free(acl: *mut c_void) -> i32;
    }
    // macOS extended ACLs can grant read access independently of the mode bits.
    let acl = unsafe { acl_get_fd(file.as_raw_fd()) };
    if acl.is_null() {
        let e = io::Error::last_os_error();
        return if e.kind() == io::ErrorKind::NotFound {
            Ok(())
        } else {
            Err(e)
        };
    }
    unsafe { acl_free(acl) };
    Err(io::Error::new(
        io::ErrorKind::PermissionDenied,
        "Choose a folder without inherited access rules",
    ))
}

#[cfg(windows)]
fn create_private_file(path: &str) -> io::Result<File> {
    use crate::platform::windows::{current_process_user_sid_string, wide_string};
    use std::os::windows::io::{AsRawHandle, FromRawHandle};
    use windows::{
        core::{Owned, PCWSTR},
        Win32::{
            Foundation::{GENERIC_READ, GENERIC_WRITE, HANDLE, HLOCAL, WIN32_ERROR},
            Security::{
                Authorization::{
                    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
                },
                PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES,
            },
            Storage::FileSystem::{
                CreateFileW, GetVolumeInformationByHandleW, CREATE_NEW, FILE_ATTRIBUTE_NORMAL,
                FILE_SHARE_NONE,
            },
        },
    };
    if path.contains('\0') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid file name",
        ));
    }
    let sid =
        current_process_user_sid_string().map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    let sddl = wide_string(&format!("D:P(A;;FA;;;{sid})"));
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PCWSTR(sddl.as_ptr()),
            SDDL_REVISION_1,
            &mut descriptor,
            None,
        )?
    };
    let _descriptor = unsafe { Owned::new(HLOCAL(descriptor.0)) };
    let attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: false.into(),
    };
    let path = wide_string(path);
    let handle = unsafe {
        CreateFileW(
            PCWSTR(path.as_ptr()),
            GENERIC_READ.0 | GENERIC_WRITE.0,
            FILE_SHARE_NONE,
            Some(&attributes),
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
    }
    .map_err(|e| match WIN32_ERROR::from_error(&e) {
        Some(code) => io::Error::from_raw_os_error(code.0 as i32),
        None => io::Error::from(e),
    })?;
    let file = unsafe { File::from_raw_handle(handle.0) };
    // FAT volumes can accept a security descriptor without enforcing it.
    let mut flags = 0;
    unsafe {
        GetVolumeInformationByHandleW(
            HANDLE(file.as_raw_handle()),
            None,
            None,
            None,
            Some(&mut flags),
            None,
        )?
    };
    if flags & winapi::um::winnt::FILE_PERSISTENT_ACLS == 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Choose a location that supports private files",
        ));
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_creates_a_private_file_and_preserves_existing_files() {
        let dir = std::env::temp_dir().join(format!("rustdesk-mcp-export-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let file = dir.join("client.json");
        let path = file.to_str().unwrap();
        write(path, "first config").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(path).unwrap().permissions().mode() & 0o077,
                0
            );
        }
        assert_eq!(
            write(path, "different config").unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(std::fs::read_to_string(path).unwrap(), "first config");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
