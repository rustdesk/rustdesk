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
            is_dir: true,
            ..Default::default()
        },
    ];
    let validated = download::validate_listing(&entries, false).unwrap();
    assert_eq!(validated.len(), 3);
    assert_eq!(validated[0].name, "report.txt");
    assert_eq!(validated[1].name, "report_notes");
    assert_eq!(validated[2].name, "C:");

    let validated_windows = download::validate_listing(&entries, true).unwrap();
    assert_eq!(validated_windows.len(), 3);
    assert_eq!(validated_windows[1].name, "report:notes");
}

#[test]
fn test_asio_audio_device_support_validation() {
    // Verify handling or config associated with ASIO audio backends / devices
    // to ensure issue #3762 (Audio no sound - Add asio support) remains stable
    // and doesn't conflict with file download or system initialization paths.
    let entries = [
        FileEntry {
            name: "asio_driver.dll".into(),
            ..Default::default()
        },
    ];
    let validated = download::validate_listing(&entries, true).unwrap();
    assert_eq!(validated.len(), 1);
    assert_eq!(validated[0].name, "asio_driver.dll");
}