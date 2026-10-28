use super::{InvokeUiSession, Remote, MILLI1};
use base::fs;
use hbb_common::{
    allow_err, log,
    tokio::{self, task::JoinHandle, time},
    ResultType, Stream,
};
use std::{
    collections::HashMap,
    future::{poll_fn, Future},
    pin::Pin,
    task::Poll,
};

#[derive(Default)]
pub(super) struct PendingReads(HashMap<i32, PendingRead>);

struct PendingRead {
    task: JoinHandle<ResultType<fs::TransferJob>>,
    mode: ReadMode,
    meta: fs::TransferJobMeta,
}

#[derive(Clone, Copy)]
enum ReadMode {
    Send,
    Restore,
    Resume,
}

impl PendingReads {
    pub(super) fn start(
        &mut self,
        meta: fs::TransferJobMeta,
        restore: bool,
        read: impl FnOnce() -> ResultType<fs::TransferJob> + Send + 'static,
    ) {
        self.cancel(meta.id);
        self.0.insert(
            meta.id,
            PendingRead {
                task: tokio::task::spawn_blocking(read),
                mode: if restore {
                    ReadMode::Restore
                } else {
                    ReadMode::Send
                },
                meta,
            },
        );
    }

    pub(super) fn resume(&mut self, id: i32) -> bool {
        let Some(pending) = self.0.get_mut(&id) else {
            return false;
        };
        pending.mode = ReadMode::Resume;
        true
    }

    pub(super) fn cancel(&mut self, id: i32) {
        if let Some(pending) = self.0.remove(&id) {
            pending.task.abort();
        }
    }

    pub(super) fn stop_scans(&self) {
        // The session saves unfinished transfers after the connection loop exits.
        for pending in self.0.values() {
            pending.task.abort();
        }
    }

    pub(super) fn metadata(&self) -> ResultType<Vec<String>> {
        self.0
            .values()
            .map(|pending| serde_json::to_string(&pending.meta).map_err(Into::into))
            .collect()
    }

    pub(super) async fn next(&mut self) -> (i32, ResultType<fs::TransferJob>) {
        let (id, result, mode) = poll_fn(|cx| {
            for (&id, pending) in &mut self.0 {
                if let Poll::Ready(result) = Pin::new(&mut pending.task).poll(cx) {
                    return Poll::Ready((id, result, pending.mode));
                }
            }
            Poll::Pending
        })
        .await;
        self.0.remove(&id);
        let result = result
            .map_err(hbb_common::anyhow::Error::from)
            .and_then(|result| result)
            .map(|mut job| {
                job.is_last_job = matches!(mode, ReadMode::Restore);
                job.is_resume = matches!(mode, ReadMode::Resume);
                job.set_finished_size_on_resume();
                job
            });
        (id, result)
    }
}

impl Drop for PendingReads {
    fn drop(&mut self) {
        // Running provider calls cannot be interrupted, but their results must not start a transfer.
        self.stop_scans();
    }
}

impl<T: InvokeUiSession> Remote<T> {
    pub(super) async fn finish_document_read(
        &mut self,
        (id, result): (i32, ResultType<fs::TransferJob>),
        peer: &mut Stream,
    ) {
        let job = match result {
            Ok(job) => job,
            Err(error) => {
                self.handle_job_status(id, -1, Some(error.to_string()));
                return;
            }
        };
        if let fs::DataSource::FilePath(path) = &job.data_source {
            self.handler
                .update_folder_files(id, job.files(), fs::get_string(path), true, true);
        }
        let message = (!job.is_last_job).then(|| {
            fs::new_receive(
                id,
                job.remote.clone(),
                job.file_num,
                job.files().clone(),
                job.total_size(),
            )
        });
        self.read_jobs.push(job);
        self.timer = crate::rustdesk_interval(time::interval(MILLI1));
        if let Some(message) = message {
            allow_err!(peer.send(&message).await);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbb_common::tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        sync::oneshot,
        time::{timeout, Duration},
    };

    const TEST_TIMEOUT: Duration = Duration::from_secs(1);

    fn job(id: i32, destination: &str) -> ResultType<fs::TransferJob> {
        let mut job = fs::TransferJob::default();
        job.id = id;
        job.remote = destination.to_owned();
        Ok(job)
    }

    fn meta(id: i32, destination: &str) -> fs::TransferJobMeta {
        job(id, destination).unwrap().gen_meta()
    }

    #[tokio::test]
    async fn connection_responds_during_scan_and_reports_scan_error() {
        let mut reads = PendingReads::default();
        let (started, started_rx) = oneshot::channel();
        let (release, scan_rx) = std::sync::mpsc::channel();
        reads.start(meta(1, "failed"), false, move || {
            started.send(()).unwrap();
            scan_rx.recv().unwrap();
            Err(hbb_common::anyhow::anyhow!("Provider unavailable"))
        });
        started_rx.await.unwrap();
        let (mut client, mut peer) = tokio::io::duplex(16);
        peer.write_u8(42).await.unwrap();
        let received = tokio::select! {
            _ = reads.next() => panic!("Scan finished before it was released"),
            value = client.read_u8() => value.unwrap(),
        };
        assert_eq!(received, 42);
        release.send(()).unwrap();
        let (id, result) = timeout(TEST_TIMEOUT, reads.next()).await.unwrap();
        assert_eq!(id, 1);
        assert_eq!(result.unwrap_err().to_string(), "Provider unavailable");
    }

    #[tokio::test]
    async fn disconnect_preserves_metadata_without_reviving_canceled_scans() {
        let mut reads = PendingReads::default();
        let (started, started_rx) = oneshot::channel();
        let (release, scan_rx) = std::sync::mpsc::channel();
        reads.start(meta(1, "canceled"), false, move || {
            started.send(()).unwrap();
            scan_rx.recv().unwrap();
            job(1, "canceled")
        });
        started_rx.await.unwrap();
        reads.cancel(1);
        assert!(reads.metadata().unwrap().is_empty());
        reads.start(meta(1, "replacement"), false, || job(1, "replacement"));
        release.send(()).unwrap();
        let (_, result) = timeout(TEST_TIMEOUT, reads.next()).await.unwrap();
        assert_eq!(result.unwrap().remote, "replacement");
        assert!(reads.metadata().unwrap().is_empty());
        let expected = fs::TransferJobMeta {
            id: 2,
            remote: "/downloads/source".to_owned(),
            to: "/@rustdesk/tree/grant/Documents/source".to_owned(),
            show_hidden: true,
            file_num: 7,
            is_remote: false,
        };
        reads.start(expected.clone(), true, || job(2, "disconnected"));
        assert!(reads.resume(2));
        reads.stop_scans();
        assert_eq!(
            reads.metadata().unwrap(),
            vec![serde_json::to_string(&expected).unwrap()]
        );
    }

    #[tokio::test]
    async fn restored_transfer_waits_for_resume_even_during_scan() {
        let mut reads = PendingReads::default();
        reads.start(meta(1, "paused"), true, || job(1, "paused"));
        let (_, result) = timeout(TEST_TIMEOUT, reads.next()).await.unwrap();
        let paused = result.unwrap();
        assert!(paused.is_last_job);
        assert!(!paused.is_resume);
        reads.start(meta(2, "resumed"), true, || job(2, "resumed"));
        assert!(reads.resume(2));
        let (_, result) = timeout(TEST_TIMEOUT, reads.next()).await.unwrap();
        let resumed = result.unwrap();
        assert!(!resumed.is_last_job);
        assert!(resumed.is_resume);
    }
}
