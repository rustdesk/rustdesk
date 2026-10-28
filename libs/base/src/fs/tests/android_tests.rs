use super::*;

#[cfg(unix)]
#[path = "android_write_tests.rs"]
mod write_tests;

#[cfg(unix)]
use super::super::{
    DataSource, FileTransferBlock, FileTransferSendConfirmRequest, JobType, TransferJob,
};

#[cfg(unix)]
fn provider(output: &Path, request: &str) -> ResultType<String> {
    use std::{fs::File, os::fd::IntoRawFd};

    let request: Value = serde_json::from_str(request)?;
    let modified = output.metadata().map_or(0, |m| std::os::unix::fs::MetadataExt::mtime(&m));
    let metadata = json!({"name": "notes.txt", "directory": false, "size": 12, "modified": modified});
    let value = match request["op"].as_str() {
        Some("home") => json!("/@rustdesk/tree/selected/Documents"),
        Some("allowed") => json!(true),
        Some("list") => json!([metadata]),
        Some("stat") if request["path"].as_str().unwrap().ends_with(".txt") => metadata,
        Some("stat") => json!({"name": "Documents", "directory": true, "size": 0, "modified": 0}),
        Some("begin") if request["path"].as_str().unwrap().contains("write-error") => {
            std::fs::write(output, b"partial")?;
            json!({"fd": File::open(output)?.into_raw_fd(), "descriptor": request["path"]})
        }
        Some("begin") => {
            if output.exists() && request["overwrite"].as_bool() == Some(false) {
                return Ok(json!({"error": "Document overwrite requires confirmation"}).to_string());
            }
            json!({"fd": File::create(output)?.into_raw_fd(), "descriptor": request["path"]})
        }
        Some("open") => write_tests::open_source(output, &request)?,
        Some("close")
            if request["descriptor"]
                .as_str()
                .unwrap()
                .ends_with("broken.txt") =>
        {
            return Ok(json!({"error": "Provider read failed"}).to_string())
        }
        Some("close") => json!(true),
        Some("finish") if request["path"].as_str().unwrap().ends_with("failed.txt") => {
            return Ok(json!({"error": "Publishing failed"}).to_string())
        }
        Some("finish") => json!(true),
        Some("discard") => {
            if request["path"].as_str().unwrap().contains("discard-error") {
                bail!("Provider cleanup failed");
            }
            std::fs::remove_file(output)?;
            json!(true)
        }
        _ => bail!("Unexpected operation"),
    };
    Ok(json!({"value": value}).to_string())
}

#[cfg(unix)]
fn receiving(path: &str) -> TransferJob {
    receiving_files(path, vec![FileEntry::default()])
}

#[cfg(unix)]
fn receiving_files(path: &str, files: Vec<FileEntry>) -> TransferJob {
    TransferJob::new_write(
        1,
        JobType::Generic,
        String::new(),
        DataSource::FilePath(path.into()),
        0,
        false,
        true,
        false,
    )
    .with_files(files)
    .unwrap()
}

#[cfg(unix)]
fn block(file_num: i32, data: &[u8]) -> FileTransferBlock {
    FileTransferBlock {
        id: 1,
        file_num,
        data: data.to_vec().into(),
        ..Default::default()
    }
}

#[cfg(unix)]
fn sending(path: &str, overwrite_detection: bool) -> TransferJob {
    TransferJob::new_read(
        2,
        JobType::Generic,
        String::new(),
        DataSource::FilePath(path.into()),
        0,
        false,
        false,
        overwrite_detection,
    )
    .unwrap()
}

#[cfg(unix)]
async fn receive(path: &str, succeeds: bool, output: &Path) {
    let mut job = receiving(path);
    confirm_overwrite(&mut job, 0).await;
    job.write(block(0, b"hello, world")).await.unwrap();
    let eof = job.write(block(0, b"")).await;
    assert_eq!(eof.is_ok(), succeeds);
    assert_eq!(job.finish_document().await.is_ok(), succeeds);
    assert_eq!(job.job_error().is_some(), !succeeds);
    job.cancel_document().await.unwrap();
    assert_eq!(std::fs::read(output).unwrap(), b"hello, world");
}

#[cfg(unix)]
async fn confirm_overwrite(job: &mut TransferJob, file_num: i32) {
    let mut confirmation = FileTransferSendConfirmRequest {
        id: job.id(),
        file_num,
        ..Default::default()
    };
    confirmation.set_offset_blk(0);
    job.confirm(&confirmation).await;
}

#[cfg(unix)]
async fn overwrite_requires_matching_confirmation(path: &str, output: &Path) {
    std::fs::write(output, b"original").unwrap();
    let mut job = receiving(&format!("{path}/notes.txt"));
    assert!(job.write(block(0, b"")).await.is_err());
    assert_eq!(std::fs::read(output).unwrap(), b"original");

    let files = ["first.txt", "second.txt"]
        .map(|name| FileEntry {
            name: name.into(),
            ..Default::default()
        })
        .to_vec();
    let mut job = receiving_files(path, files.clone());
    confirm_overwrite(&mut job, 0).await;
    assert!(job.set_files(vec![files[1].clone()]).is_err());
    assert_eq!(job.files(), &files);
    job.write(block(0, b"")).await.unwrap();
    std::fs::write(output, b"second original").unwrap();
    assert!(job.write(block(1, b"")).await.is_err());
    assert_eq!(std::fs::read(output).unwrap(), b"second original");

    let mut job = receiving_files(path, files);
    confirm_overwrite(&mut job, 1).await;
    job.write(block(1, b"")).await.unwrap();
    assert!(std::fs::read(output).unwrap().is_empty());
}

#[cfg(unix)]
async fn host_upload_respects_overwrite_support(path: &str, output: &Path) {
    for (overwrite_detection, exists) in [(false, false), (false, true), (true, true)] {
        if exists {
            std::fs::write(output, b"original").unwrap();
        }
        let mut job = TransferJob::new_write(
            1,
            JobType::Generic,
            String::new(),
            DataSource::FilePath(format!("{path}/notes.txt").into()),
            0,
            false,
            false,
            overwrite_detection,
        )
        .with_files(vec![FileEntry::default()])
        .unwrap();
        let result = job.write(block(0, b"replacement")).await;
        if exists && !overwrite_detection {
            assert_eq!(
                result.unwrap_err().to_string(),
                "Document overwrite requires confirmation"
            );
            assert_eq!(std::fs::read(output).unwrap(), b"original");
        } else {
            result.unwrap();
            job.finish_document().await.unwrap();
            assert_eq!(std::fs::read(output).unwrap(), b"replacement");
        }
    }
}

#[cfg(unix)]
#[tokio::test]
async fn selected_folder_transfers_directly_and_preserves_data_on_publish_failure() {
    use std::time::{SystemTime, UNIX_EPOCH};

    let output = std::env::temp_dir().join(format!(
        "rustdesk-documents-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let provider_output = output.clone();
    initialize(move |request| provider(&provider_output, request)).unwrap();
    let path = "/@rustdesk/tree/selected/Documents";
    assert!(allowed(path).unwrap() && allowed(&format!("{path}/notes.txt")).unwrap());
    assert!(!allowed(&format!("{path}-private/notes.txt")).unwrap());
    assert!(!allowed("/@rustdesk/tree/previous/Documents/notes.txt").unwrap());
    let directory = super::super::read_dir(Path::new(path), false).unwrap();
    assert_eq!(directory.path, path);
    assert_eq!(directory.entries[0].name, "notes.txt");
    let files = super::super::get_recursive_files(path, false).unwrap();
    assert_eq!(files[0].name, "notes.txt");
    assert_eq!(files[0].size, 12);
    assert!(remove_empty_tree(path).unwrap_err().to_string() == "Cannot delete a shared root");
    assert!(super::super::remove_file(path).is_err());
    host_upload_respects_overwrite_support(path, &output).await;
    overwrite_requires_matching_confirmation(path, &output).await;
    for (name, succeeds) in [("notes.txt", true), ("failed.txt", false)] {
        receive(&format!("{path}/{name}"), succeeds, &output).await;
    }
    let destination = format!("{path}/notes.txt");
    let entry = stat(&destination).unwrap().unwrap();
    let digest = super::super::FileTransferDigest {
        file_size: entry.size,
        last_modified: entry.modified_time,
        ..Default::default()
    };
    assert!(matches!(
        super::super::is_write_need_confirmation(false, &destination, &digest).unwrap(),
        super::super::DigestCheckResult::NeedConfirm(digest) if digest.is_identical
    ));
    provider_error_at_eof(path).await;
    send_and_cancel(path, &output).await;
    write_tests::failed_block_cleans_pending_receive(path, &output).await;
    write_tests::failed_finalization_discards_partial_data(path, &output).await;
    write_tests::replacement_preserves_opened_digest(path, &output).await;
    write_tests::changed_source_without_timestamp_rejects_resume(path, &output).await;
    let mut empty = receiving(&format!("{path}/empty.txt"));
    empty.write(block(0, b"")).await.unwrap();
    assert_eq!(std::fs::metadata(&output).unwrap().len(), 0);
    std::fs::remove_file(output).unwrap();
}

#[cfg(unix)]
async fn send_and_cancel(path: &str, output: &Path) {
    let mut sender = sending(&format!("{path}/notes.txt"), false);
    sender.init_data_stream_for_cm().await.unwrap();
    assert_eq!(
        sender.read().await.unwrap().unwrap().data.as_ref(),
        b"hello, world"
    );
    assert!(sender.read().await.unwrap().unwrap().data.is_empty());
    let mut canceled = receiving(&format!("{path}/canceled.txt"));
    confirm_overwrite(&mut canceled, 0).await;
    canceled.write(block(0, b"partial")).await.unwrap();
    canceled.cancel_document().await.unwrap();
    assert!(!output.exists());
}

#[cfg(unix)]
async fn provider_error_at_eof(path: &str) {
    let mut sender = sending(&format!("{path}/broken.txt"), true);
    assert!(sender.init_data_stream_for_cm().await.unwrap().is_some());
    confirm_overwrite(&mut sender, 0).await;
    assert!(sender.read().await.unwrap().is_some());
    assert!(sender.read().await.is_err());
    assert!(sender.init_data_stream_for_cm().await.is_err());
    assert!(sender.read().await.unwrap().is_none());
    assert!(sender.job_completed());
    assert_eq!(sender.job_error().as_deref(), Some("Provider read failed"));
}

#[test]
fn document_paths_cannot_escape_the_selected_location() {
    for path in [
        "/@rustdesk/tree/id/folder/../secret",
        "/@rustdesk//Download",
        "/@rustdesk/tree/id/a\\b",
        "/@rustdesk/file/id/report.txt",
        "/@rustdesk/save/id/report.txt",
        "/@rustdesk/Download/report.txt",
    ] {
        assert!(validate(path).is_err(), "{}", path);
    }
    assert!(validate("/@rustdesk/tree/id/folder/report.txt").is_ok());
}
