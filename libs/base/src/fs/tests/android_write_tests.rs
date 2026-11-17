use super::*;

pub(super) fn open_source(output: &Path, request: &Value) -> ResultType<Value> {
    use std::{fs::File, io::Write, os::fd::IntoRawFd};

    let fd = if request["path"].as_str().unwrap().ends_with("stream.txt") {
        let (reader, mut writer) = std::os::unix::net::UnixStream::pair()?;
        writer.write_all(&std::fs::read(output)?)?;
        reader.into_raw_fd()
    } else {
        let file = File::open(output)?;
        let replacement = output.with_extension("replacement");
        if replacement.exists() {
            std::fs::rename(replacement, output)?;
        }
        file.into_raw_fd()
    };
    Ok(json!({"fd": fd, "descriptor": request["path"]}))
}

pub(super) async fn replacement_preserves_opened_digest(path: &str, output: &Path) {
    use filetime::{set_file_mtime, FileTime};

    std::fs::write(output, b"old contents").unwrap();
    set_file_mtime(output, FileTime::from_unix_time(1, 0)).unwrap();
    let replacement = output.with_extension("replacement");
    std::fs::write(&replacement, b"new contents").unwrap();
    set_file_mtime(&replacement, FileTime::from_unix_time(2, 0)).unwrap();
    let path = format!("{path}/notes.txt");
    let mut sender = sending(&path, true);
    assert_eq!(
        sender.init_data_stream_for_cm().await.unwrap(),
        Some((1, 12))
    );
    confirm_overwrite(&mut sender, 0).await;
    assert_eq!(
        sender.read().await.unwrap().unwrap().data.as_ref(),
        b"old contents"
    );
    assert!(sender.read().await.unwrap().unwrap().data.is_empty());
    let mut next = sending(&path, true);
    assert_eq!(next.init_data_stream_for_cm().await.unwrap(), Some((2, 12)));
}

pub(super) async fn failed_block_cleans_pending_receive(path: &str, output: &Path) {
    for name in ["write-error.txt", "write-error-broken.txt"] {
        let mut job = receiving(&format!("{path}/{name}"));
        job.is_remote = false;
        let block = block(0, b"more data");
        // Tokio queues the first write and reports its I/O failure on the next block.
        job.write(block.clone()).await.unwrap();
        assert_eq!(std::fs::read(output).unwrap(), b"partial");
        let error = job.write(block).await.unwrap_err();
        assert!(error.downcast_ref::<std::io::Error>().is_some());
        assert_eq!(job.job_error(), Some(error.to_string()));
        assert!(
            !output.exists(),
            "A failed receive must discard partial data"
        );
    }
}

pub(super) async fn failed_finalization_discards_partial_data(path: &str, output: &Path) {
    for name in ["write-error.txt", "broken.txt", "discard-error-broken.txt"] {
        let mut job = receiving(&format!("{path}/{name}"));
        confirm_overwrite(&mut job, 0).await;
        job.write(block(0, b"partial data")).await.unwrap();
        let error = job.finish_document().await.unwrap_err();
        assert_eq!(job.job_error(), Some(error.to_string()));
        if name.ends_with("broken.txt") {
            assert!(error.to_string().contains("Provider read failed"));
        } else {
            assert!(error.downcast_ref::<std::io::Error>().is_some());
        }
        if name.starts_with("discard-error") {
            assert!(error.to_string().contains("Provider cleanup failed"));
            assert!(output.exists());
            std::fs::remove_file(output).unwrap();
        } else {
            assert!(
                !output.exists(),
                "A failed finalization must discard partial data"
            );
        }
    }
}

pub(super) async fn changed_source_without_timestamp_rejects_resume(path: &str, output: &Path) {
    std::fs::write(output, b"old contents").unwrap();
    filetime::set_file_mtime(output, filetime::FileTime::from_unix_time(0, 0)).unwrap();
    stale_metadata_uses_current_digest(&format!("{path}/notes.txt")).await;
    let path = format!("{path}/stream.txt");
    let mut sender = sending(&path, true);
    let original_digest = sender.init_data_stream_for_cm().await.unwrap();
    assert_eq!(original_digest, Some((0, b"old contents".len() as u64)));
    confirm_overwrite(&mut sender, 0).await;
    assert_eq!(
        sender.read().await.unwrap().unwrap().data.as_ref(),
        b"old contents"
    );
    drop(sender);

    std::fs::write(output, b"new contents").unwrap();
    let mut resumed = sending(&path, true);
    assert_eq!(
        resumed.init_data_stream_for_cm().await.unwrap(),
        original_digest
    );
    let mut confirmation = FileTransferSendConfirmRequest {
        id: resumed.id(),
        ..Default::default()
    };
    confirmation.set_offset_blk(b"old ".len() as u32);
    resumed.confirm(&confirmation).await;
    assert_eq!(
        resumed.job_error().as_deref(),
        Some("Cannot resume a document with an unknown modification time; restart the transfer")
    );
    assert!(resumed.read().await.unwrap().is_none());
    assert!(resumed.job_completed());

    let mut fresh = sending(&path, true);
    fresh.init_data_stream_for_cm().await.unwrap();
    confirm_overwrite(&mut fresh, 0).await;
    assert_eq!(
        fresh.read().await.unwrap().unwrap().data.as_ref(),
        b"new contents"
    );
    assert!(fresh.read().await.unwrap().unwrap().data.is_empty());
    std::fs::remove_file(output).unwrap();
}

async fn stale_metadata_uses_current_digest(path: &str) {
    for (size, modified_time) in [(12, 1), (13, 0)] {
        let mut sender = sending(path, true)
            .with_files(vec![FileEntry {
                size,
                modified_time,
                ..Default::default()
            }])
            .unwrap();
        assert_eq!(
            sender.init_data_stream_for_cm().await.unwrap(),
            Some((0, b"old contents".len() as u64))
        );
        confirm_overwrite(&mut sender, 0).await;
        assert_eq!(
            sender.read().await.unwrap().unwrap().data.as_ref(),
            b"old contents"
        );
        assert!(sender.read().await.unwrap().unwrap().data.is_empty());
    }
}
