use super::*;

pub(super) async fn failed_block_cleans_pending_receive(path: &str, output: &Path) {
    for name in ["write-error.txt", "write-error-broken.txt"] {
        let mut job = receiving(&format!("{path}/{name}"));
        job.is_remote = false;
        let block = FileTransferBlock {
            id: job.id(),
            data: b"more data".to_vec().into(),
            ..Default::default()
        };
        // Tokio queues the first write and reports its I/O failure on the next block.
        job.write(block.clone()).await.unwrap();
        assert_eq!(std::fs::read(output).unwrap(), b"partial");
        let error = job.write(block).await.unwrap_err();
        assert!(error.downcast_ref::<std::io::Error>().is_some());
        assert_eq!(job.job_error(), Some(error.to_string()));
        if name.ends_with("broken.txt") {
            assert_eq!(std::fs::read(output).unwrap(), b"partial");
            std::fs::remove_file(output).unwrap();
        } else {
            assert!(
                !output.exists(),
                "A failed receive must discard partial data"
            );
        }
    }
}
