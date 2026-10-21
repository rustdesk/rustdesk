use super::*;

struct DownloadDir(PathBuf);

impl DownloadDir {
    fn new() -> Self {
        static NEXT_DIRECTORY: AtomicI32 = AtomicI32::new(1);
        let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "rustdesk_download_{}_{nonce}_{sequence}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn job(&self) -> TransferJob {
        self.job_with_files(vec![FileEntry::default()])
    }

    fn job_with_files(&self, files: Vec<FileEntry>) -> TransferJob {
        TransferJob::new_write(
            1,
            JobType::Generic,
            "/remote/file.txt".into(),
            DataSource::FilePath(self.0.join("file.txt")),
            0,
            false,
            true,
            true,
        )
        .with_files(files)
        .unwrap()
    }
}

impl Drop for DownloadDir {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn block() -> FileTransferBlock {
    FileTransferBlock {
        id: 1,
        file_num: 0,
        data: b"received".to_vec().into(),
        ..Default::default()
    }
}

fn approval(file_num: i32, offset: u32) -> FileTransferSendConfirmRequest {
    FileTransferSendConfirmRequest {
        id: 1,
        file_num,
        union: Some(file_transfer_send_confirm_request::Union::OffsetBlk(offset)),
        ..Default::default()
    }
}

#[test]
fn download_listing_rejects_unsafe_names_and_keeps_drive_rows() {
    for name in ["../outside", "/outside", ".", "..", "", "dir/file"] {
        let entry = FileEntry {
            name: name.into(),
            ..Default::default()
        };
        assert!(
            download::validate_listing(&[entry], false).is_err(),
            "{}",
            name
        );
    }
    let entries = [
        FileEntry {
            name: "report.txt".into(),
            ..Default::default()
        },
        FileEntry {
            name: "report:notes".into(),
            ..Default::default()
        },
        FileEntry {
            name: "C:".into(),
            entry_type: FileType::DirDrive.into(),
            ..Default::default()
        },
    ];
    download::validate_listing(&entries, true).unwrap();
}

#[tokio::test]
async fn download_requires_confirmation_to_overwrite() {
    let dir = DownloadDir::new();
    let target = dir.0.join("file.txt");
    std::fs::write(&target, b"original").unwrap();
    let mut job = dir.job();
    assert!(job.write(block()).await.is_err());
    assert_eq!(std::fs::read(&target).unwrap(), b"original");
    assert!(!dir.0.join("file.txt.download").exists());

    job.confirm(&approval(0, 0)).await;
    job.write(block()).await.unwrap();
    assert!(job.set_files(vec![FileEntry::default()]).is_err());
    job.finish_download().unwrap();
    assert_eq!(std::fs::read(&target).unwrap(), b"received");
}

#[tokio::test]
async fn download_requires_separate_confirmation_for_each_existing_file() {
    let dir = DownloadDir::new();
    let mut job = dir.job_with_files(vec![
        FileEntry {
            name: "first.txt".into(),
            ..Default::default()
        },
        FileEntry {
            name: "second.txt".into(),
            ..Default::default()
        },
    ]);
    job.write(block()).await.unwrap();
    let second = dir.0.join("file.txt/second.txt");
    std::fs::write(&second, b"original").unwrap();
    assert!(job
        .write(FileTransferBlock {
            file_num: 1,
            ..block()
        })
        .await
        .is_err());
    assert_eq!(std::fs::read(&second).unwrap(), b"original");
    assert_eq!(
        std::fs::read(dir.0.join("file.txt/first.txt")).unwrap(),
        b"received"
    );
    job.confirm(&approval(1, 0)).await;
    job.write(FileTransferBlock {
        file_num: 1,
        ..block()
    })
    .await
    .unwrap();
    job.finish_download().unwrap();
    assert_eq!(std::fs::read(&second).unwrap(), b"received");
}

#[tokio::test]
async fn download_preserves_received_files_when_publication_fails() {
    for next_is_digest in [false, true] {
        let dir = DownloadDir::new();
        let files = ["first.txt", "second.txt"]
            .iter()
            .map(|&name| FileEntry {
                name: name.into(),
                ..Default::default()
            })
            .collect();
        let mut job = dir.job_with_files(files);
        job.write(block()).await.unwrap();
        let target = dir.0.join("file.txt/first.txt");
        std::fs::write(&target, b"existing").unwrap();
        if next_is_digest {
            assert!(job
                .download_digest(
                    true,
                    &FileTransferDigest {
                        id: 1,
                        file_num: 1,
                        ..Default::default()
                    },
                )
                .is_err());
        } else {
            assert!(job
                .write(FileTransferBlock {
                    file_num: 1,
                    ..block()
                })
                .await
                .is_err());
        }
        job.remove_download_file();
        assert_eq!(std::fs::read(target).unwrap(), b"existing");
        assert_eq!(
            std::fs::read(dir.0.join("file.txt/first.txt.download")).unwrap(),
            b"received"
        );
        assert!(dir.0.join("file.txt/first.txt.digest").exists());
    }
}

#[tokio::test]
async fn download_recovers_interrupted_transfer() {
    for (resuming, changed) in [(false, false), (true, true), (true, false)] {
        let dir = DownloadDir::new();
        let mut digest = FileTransferDigest {
            id: 1,
            file_size: block().data.len() as _,
            last_modified: 1,
            ..Default::default()
        };
        let mut interrupted = dir.job();
        interrupted.set_digest(digest.file_size, digest.last_modified);
        let partial = FileTransferBlock {
            data: b"rec".to_vec().into(),
            ..block()
        };
        interrupted.write(partial).await.unwrap();
        drop(interrupted);

        let mut job = dir.job();
        job.is_resume = resuming;
        digest.last_modified += u64::from(changed);
        job.set_digest(digest.file_size, digest.last_modified);
        let confirmation = match job.download_digest(true, &digest).unwrap() {
            DigestCheckResult::NeedConfirm(confirmation) => confirmation,
            _ => panic!("interrupted download must require confirmation"),
        };
        let offset = if resuming && !changed {
            b"rec".len()
        } else {
            0
        };
        assert_eq!(confirmation.is_identical, offset > 0);
        assert_eq!(confirmation.transferred_size, offset as u64);
        assert!(job.write(block()).await.is_err());
        assert_eq!(
            std::fs::read(dir.0.join("file.txt.download")).unwrap(),
            b"rec"
        );
        job.confirm(&approval(0, offset as _)).await;
        job.write(FileTransferBlock {
            data: block().data.slice(offset..),
            ..block()
        })
        .await
        .unwrap();
        job.finish_download().unwrap();
        assert_eq!(std::fs::read(dir.0.join("file.txt")).unwrap(), b"received");
    }
}

#[tokio::test]
async fn download_does_not_publish_or_remove_unreceived_staging_files() {
    let dir = DownloadDir::new();
    let target = dir.0.join("file.txt");
    let staging = dir.0.join("file.txt.download");
    std::fs::write(&target, b"original").unwrap();
    std::fs::write(&staging, b"another transfer").unwrap();
    let job = dir.job();
    job.modify_time();
    job.remove_download_file();
    assert_eq!(std::fs::read(&target).unwrap(), b"original");
    assert_eq!(std::fs::read(&staging).unwrap(), b"another transfer");

    let metadata_only = DownloadDir::new();
    let metadata = metadata_only.0.join("file.txt.digest");
    std::fs::write(&metadata, b"unrelated file").unwrap();
    assert!(metadata_only.job().write(block()).await.is_err());
    assert_eq!(std::fs::read(metadata).unwrap(), b"unrelated file");
}

#[test]
#[cfg(windows)]
fn download_rejects_windows_streams_and_ambiguous_components() {
    let dir = DownloadDir::new();
    for name in ["nested/file:stream", "nested/.. /outside", "nested/file."] {
        assert!(
            download::validated_target(&dir.0.join("docs"), name).is_err(),
            "{}",
            name
        );
    }
}

#[tokio::test]
#[cfg_attr(windows, ignore = "requires symlink privilege")]
async fn download_refuses_symlink_destination_directory() {
    let dir = DownloadDir::new();
    let outside = dir.0.join("outside");
    let link = dir.0.join("file.txt");
    std::fs::create_dir_all(&outside).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(&outside, &link).unwrap();
    let mut job = dir.job_with_files(vec![FileEntry {
        name: "child.txt".into(),
        ..Default::default()
    }]);
    assert!(job.write(block()).await.is_err());
    job.modify_time();
    assert!(!outside.join("child.txt").exists());
    assert!(!outside.join("child.txt.download").exists());
}

#[tokio::test]
#[cfg_attr(windows, ignore = "requires symlink privilege")]
async fn download_refuses_linked_target_and_staging_files() {
    for suffix in ["", ".download", ".digest"] {
        let dir = DownloadDir::new();
        let outside = dir.0.join("outside.txt");
        let link = dir.0.join(format!("file.txt{suffix}"));
        std::fs::write(&outside, b"original").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(&outside, &link).unwrap();
        let mut job = dir.job();
        assert!(job.write(block()).await.is_err());
        job.modify_time();
        job.remove_download_file();
        assert_eq!(std::fs::read(&outside).unwrap(), b"original");
        assert!(std::fs::symlink_metadata(link)
            .unwrap()
            .file_type()
            .is_symlink());
    }
}
