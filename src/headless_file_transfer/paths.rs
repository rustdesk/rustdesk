use std::{
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use hbb_common::message_proto::{FileEntry, FileType};

use super::HeadlessFileTransferError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FileSnapshot {
    pub(crate) path: PathBuf,
    pub(crate) size: u64,
    pub(crate) modified: SystemTime,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RemoteFilePath {
    pub(crate) parent: String,
    pub(crate) name: String,
}

pub(crate) fn inspect_push_source(path: &Path) -> Result<FileSnapshot, HeadlessFileTransferError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        HeadlessFileTransferError::LocalPrecondition(format!(
            "cannot inspect source {}: {error}",
            path.display()
        ))
    })?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(HeadlessFileTransferError::LocalPrecondition(format!(
            "source must be a regular non-symlink file: {}",
            path.display()
        )));
    }
    let modified = metadata.modified().map_err(|error| {
        HeadlessFileTransferError::LocalPrecondition(format!(
            "cannot read source modification time {}: {error}",
            path.display()
        ))
    })?;

    Ok(FileSnapshot {
        path: path.to_path_buf(),
        size: metadata.len(),
        modified,
    })
}

pub(crate) fn verify_source_unchanged(
    snapshot: &FileSnapshot,
) -> Result<(), HeadlessFileTransferError> {
    let current = inspect_push_source(&snapshot.path)?;
    if current.size != snapshot.size || current.modified != snapshot.modified {
        return Err(HeadlessFileTransferError::LocalPrecondition(format!(
            "source changed during transfer preparation: {}",
            snapshot.path.display()
        )));
    }
    Ok(())
}

pub(crate) fn inspect_pull_destination(
    path: &Path,
    overwrite: bool,
) -> Result<(), HeadlessFileTransferError> {
    if path.as_os_str().is_empty() {
        return Err(HeadlessFileTransferError::LocalPrecondition(
            "destination path must not be empty".into(),
        ));
    }

    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent_metadata = fs::metadata(parent).map_err(|error| {
        HeadlessFileTransferError::LocalPrecondition(format!(
            "destination parent is unavailable {}: {error}",
            parent.display()
        ))
    })?;
    if !parent_metadata.is_dir() {
        return Err(HeadlessFileTransferError::LocalPrecondition(format!(
            "destination parent is not a directory: {}",
            parent.display()
        )));
    }

    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
                return Err(HeadlessFileTransferError::LocalPrecondition(format!(
                    "destination must be a regular non-symlink file: {}",
                    path.display()
                )));
            }
            if !overwrite {
                return Err(HeadlessFileTransferError::DestinationExists(format!(
                    "destination already exists: {}",
                    path.display()
                )));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(HeadlessFileTransferError::LocalPrecondition(format!(
                "cannot inspect destination {}: {error}",
                path.display()
            )));
        }
    }

    Ok(())
}

pub(crate) fn split_remote_file_path(
    path: &str,
    peer_platform: &str,
) -> Result<RemoteFilePath, HeadlessFileTransferError> {
    let separator =
        |character: char| character == '/' || (peer_platform == "Windows" && character == '\\');
    let Some(separator_index) = path.rfind(separator) else {
        if path.is_empty() {
            return Err(HeadlessFileTransferError::Protocol(
                "remote file path must include a filename".into(),
            ));
        }
        return Ok(RemoteFilePath {
            parent: ".".into(),
            name: path.into(),
        });
    };
    let name = &path[separator_index + 1..];
    if name.is_empty() {
        return Err(HeadlessFileTransferError::Protocol(
            "remote file path must not end with a separator".into(),
        ));
    }
    let parent = &path[..separator_index];
    let is_windows_drive_root =
        peer_platform == "Windows" && parent.len() == 2 && parent.as_bytes()[1] == b':';

    Ok(RemoteFilePath {
        parent: if parent.is_empty() || is_windows_drive_root {
            path[..=separator_index].into()
        } else {
            parent.into()
        },
        name: name.into(),
    })
}

pub(crate) fn single_regular_file_size(
    entries: &[FileEntry],
) -> Result<u64, HeadlessFileTransferError> {
    let [entry] = entries else {
        return Err(HeadlessFileTransferError::Protocol(
            "remote transfer did not return exactly one entry".into(),
        ));
    };
    if entry.entry_type.enum_value() != Ok(FileType::File) {
        return Err(HeadlessFileTransferError::Protocol(
            "remote transfer entry is not a regular file".into(),
        ));
    }
    Ok(entry.size)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use hbb_common::message_proto::{FileEntry, FileType};

    use super::{
        inspect_pull_destination, inspect_push_source, single_regular_file_size,
        split_remote_file_path, verify_source_unchanged, RemoteFilePath,
    };

    static NEXT_TEST_DIRECTORY_ID: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let sequence = NEXT_TEST_DIRECTORY_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "rdh-hft-{}-{unique}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn push_accepts_only_a_regular_non_symlink_file() {
        let temp = TestDirectory::new();
        let file = temp.path().join("probe.bin");
        fs::write(&file, b"abc").unwrap();
        assert_eq!(inspect_push_source(&file).unwrap().size, 3);
        assert!(inspect_push_source(temp.path()).is_err());

        let link = temp.path().join("probe-link");
        std::os::unix::fs::symlink(&file, &link).unwrap();
        assert!(inspect_push_source(&link).is_err());

        let socket = temp.path().join("probe.sock");
        let _listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
        assert!(inspect_push_source(&socket).is_err());
    }

    #[test]
    fn push_snapshot_detects_source_changes() {
        let temp = TestDirectory::new();
        let file = temp.path().join("probe.bin");
        fs::write(&file, b"abc").unwrap();
        let snapshot = inspect_push_source(&file).unwrap();

        fs::write(&file, b"changed").unwrap();
        assert!(verify_source_unchanged(&snapshot).is_err());
    }

    #[test]
    fn pull_destination_requires_existing_parent_and_explicit_overwrite() {
        let temp = TestDirectory::new();
        let target = temp.path().join("target.bin");
        inspect_pull_destination(&target, false).unwrap();
        fs::write(&target, b"old").unwrap();
        assert_eq!(
            inspect_pull_destination(&target, false)
                .unwrap_err()
                .status(),
            7
        );
        inspect_pull_destination(&target, true).unwrap();
        assert!(inspect_pull_destination(&temp.path().join("missing/target.bin"), true).is_err());
    }

    #[test]
    fn pull_destination_rejects_non_regular_existing_targets() {
        let temp = TestDirectory::new();
        let directory = temp.path().join("directory");
        fs::create_dir(&directory).unwrap();
        assert!(inspect_pull_destination(&directory, true).is_err());

        let file = temp.path().join("probe.bin");
        fs::write(&file, b"abc").unwrap();
        let link = temp.path().join("probe-link");
        std::os::unix::fs::symlink(&file, &link).unwrap();
        assert!(inspect_pull_destination(&link, true).is_err());
    }

    #[test]
    fn splits_windows_and_unix_remote_paths_for_postflight() {
        assert_eq!(
            split_remote_file_path(r"C:\Users\82520\probe.bin", "Windows").unwrap(),
            RemoteFilePath {
                parent: r"C:\Users\82520".into(),
                name: "probe.bin".into(),
            }
        );
        assert_eq!(
            split_remote_file_path(r"C:\probe.bin", "Windows").unwrap(),
            RemoteFilePath {
                parent: r"C:\".into(),
                name: "probe.bin".into(),
            }
        );
        assert_eq!(
            split_remote_file_path("C:/probe.bin", "Windows").unwrap(),
            RemoteFilePath {
                parent: "C:/".into(),
                name: "probe.bin".into(),
            }
        );
        assert_eq!(
            split_remote_file_path("/tmp/probe.bin", "Linux").unwrap(),
            RemoteFilePath {
                parent: "/tmp".into(),
                name: "probe.bin".into(),
            }
        );
        assert_eq!(
            split_remote_file_path("probe.bin", "Windows").unwrap(),
            RemoteFilePath {
                parent: ".".into(),
                name: "probe.bin".into(),
            }
        );
    }

    #[test]
    fn remote_file_path_rejects_empty_names_and_trailing_separators() {
        assert!(split_remote_file_path("", "Linux").is_err());
        assert!(split_remote_file_path("/tmp/", "Linux").is_err());
        assert!(split_remote_file_path(r"C:\Users\82520\", "Windows").is_err());
    }

    #[test]
    fn accepts_exactly_one_regular_file_entry() {
        let file = FileEntry {
            entry_type: FileType::File.into(),
            size: 42,
            ..Default::default()
        };
        assert_eq!(single_regular_file_size(&[file]).unwrap(), 42);
        assert!(single_regular_file_size(&[]).is_err());
        assert!(single_regular_file_size(&[FileEntry {
            entry_type: FileType::Dir.into(),
            ..Default::default()
        }])
        .is_err());
    }
}
