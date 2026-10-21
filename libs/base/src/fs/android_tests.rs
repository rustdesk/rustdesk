use super::*;

#[cfg(unix)]
use super::super::{
    DataSource, FileTransferBlock, FileTransferSendConfirmRequest, JobType, TransferJob,
};

#[cfg(unix)]
fn provider(
    output: &Path,
    request: &str,
    closed: &tokio::sync::mpsc::UnboundedSender<std::thread::ThreadId>,
) -> ResultType<String> {
    use std::{fs::File, os::fd::IntoRawFd};

    let request: Value = serde_json::from_str(request)?;
    let metadata = json!({"name": "notes.txt", "directory": false, "size": 12, "modified": 0});
    let value = match request["op"].as_str() {
        Some("home") => json!("/@rustdesk/tree/selected/Documents"),
        Some("allowed") => json!(true),
        Some("list") => json!([metadata]),
        Some("stat") if request["path"].as_str().unwrap().ends_with(".txt") => metadata,
        Some("stat") => json!({"name": "Documents", "directory": true, "size": 0, "modified": 0}),
        Some("begin") => {
            json!({"fd": File::create(output)?.into_raw_fd(), "descriptor": request["path"]})
        }
        Some("open") if request["path"].as_str().unwrap().ends_with("stream.txt") => {
            let (source, _other) = std::os::unix::net::UnixStream::pair()?;
            json!({"fd": source.into_raw_fd(), "descriptor": request["path"]})
        }
        Some("open") => {
            json!({"fd": File::open(output)?.into_raw_fd(), "descriptor": request["path"]})
        }
        Some("close")
            if request["descriptor"]
                .as_str()
                .unwrap()
                .ends_with("broken.txt") =>
        {
            return Ok(json!({"error": "Provider read failed"}).to_string())
        }
        Some("close") => {
            if request["error"] == "Document transfer interrupted" {
                closed.send(std::thread::current().id())?;
            }
            json!(true)
        }
        Some("finish") if request["path"].as_str().unwrap().ends_with("failed.txt") => {
            return Ok(json!({"error": "Publishing failed"}).to_string())
        }
        Some("finish") => json!(true),
        Some("discard") => {
            std::fs::remove_file(output)?;
            json!(true)
        }
        _ => bail!("Unexpected operation"),
    };
    Ok(json!({"value": value}).to_string())
}

#[cfg(unix)]
fn receiving(path: &str) -> TransferJob {
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
    .with_files(vec![FileEntry::default()])
    .unwrap()
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
    job.write(FileTransferBlock {
        id: 1,
        data: b"hello, world".to_vec().into(),
        ..Default::default()
    })
    .await
    .unwrap();
    let eof = job
        .write(FileTransferBlock {
            id: 1,
            ..Default::default()
        })
        .await;
    assert_eq!(eof.is_ok(), succeeds);
    assert_eq!(job.finish_document().await.is_ok(), succeeds);
    assert_eq!(job.job_error().is_some(), !succeeds);
    assert_eq!(std::fs::read(output).unwrap(), b"hello, world");
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
    let (closed_tx, mut closed_rx) = tokio::sync::mpsc::unbounded_channel();
    initialize(move |request| provider(&provider_output, request, &closed_tx)).unwrap();
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
    assert!(remove_empty_tree(path).is_err());
    assert!(super::super::remove_file(path).is_err());
    for (name, succeeds) in [
        ("notes.txt", true),
        ("failed.txt", false),
        ("broken.txt", false),
    ] {
        receive(&format!("{path}/{name}"), succeeds, &output).await;
    }
    provider_error_at_eof(path).await;
    provider_error_on_resume(path).await;
    assert_ne!(
        closed_rx.recv().await.unwrap(),
        std::thread::current().id(),
        "Interrupted document cleanup must not block the async worker"
    );
    send_and_cancel(path, &output).await;
    let mut empty = receiving(&format!("{path}/empty.txt"));
    empty
        .write(FileTransferBlock {
            id: 1,
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(std::fs::metadata(&output).unwrap().len(), 0);
    std::fs::remove_file(output).unwrap();
}

#[cfg(unix)]
async fn send_and_cancel(path: &str, output: &Path) {
    let mut resumed = receiving(&format!("{path}/resumed.txt"));
    resumed.is_resume = true;
    assert!(resumed
        .write(FileTransferBlock {
            id: 1,
            ..Default::default()
        })
        .await
        .is_err());
    assert!(resumed.job_error().is_some());
    let mut sender = sending(&format!("{path}/notes.txt"), false);
    sender.init_data_stream_for_cm().await.unwrap();
    assert_eq!(
        sender.read().await.unwrap().unwrap().data.as_ref(),
        b"hello, world"
    );
    assert!(sender.read().await.unwrap().unwrap().data.is_empty());
    let mut canceled = receiving(&format!("{path}/canceled.txt"));
    canceled
        .write(FileTransferBlock {
            id: 1,
            data: b"partial".to_vec().into(),
            ..Default::default()
        })
        .await
        .unwrap();
    canceled.cancel_document().await.unwrap();
    assert!(!output.exists());
}

#[cfg(unix)]
async fn provider_error_at_eof(path: &str) {
    let mut sender = sending(&format!("{path}/broken.txt"), true);
    assert!(sender.init_data_stream_for_cm().await.unwrap().is_some());
    let mut confirmation = FileTransferSendConfirmRequest {
        id: 2,
        ..Default::default()
    };
    confirmation.set_offset_blk(0);
    sender.confirm(&confirmation).await;
    assert!(sender.read().await.unwrap().is_some());
    assert!(sender.read().await.is_err());
    assert!(sender.init_data_stream_for_cm().await.is_err());
    assert!(sender.read().await.unwrap().is_none());
    assert!(sender.job_completed());
    assert_eq!(sender.job_error().as_deref(), Some("Provider read failed"));
}

#[cfg(unix)]
async fn provider_error_on_resume(path: &str) {
    let mut sender = sending(&format!("{path}/stream.txt"), true);
    assert!(sender.init_data_stream_for_cm().await.unwrap().is_some());
    let mut confirmation = FileTransferSendConfirmRequest {
        id: 2,
        ..Default::default()
    };
    confirmation.set_offset_blk(1);
    sender.confirm(&confirmation).await;
    let error = sender.job_error().unwrap();
    assert_eq!(
        sender
            .init_data_stream_for_cm()
            .await
            .unwrap_err()
            .to_string(),
        error
    );
    assert!(sender.read().await.unwrap().is_none());
    assert!(sender.job_completed());
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
