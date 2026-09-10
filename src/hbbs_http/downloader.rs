use super::create_http_client_async_with_url_strict;
use hbb_common::{
    bail,
    lazy_static::lazy_static,
    log,
    tokio::{
        self,
        fs::File,
        io::AsyncWriteExt,
        sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    },
    ResultType,
};
use serde_derive::Serialize;
use std::{
    cmp::Reverse,
    collections::{BinaryHeap, HashMap},
    path::PathBuf,
    sync::{mpsc, Mutex},
    time::{Duration, Instant},
};
use uuid::Uuid;

lazy_static! {
    static ref DOWNLOADERS: Mutex<HashMap<String, Downloader>> = Default::default();
    static ref AUTO_DELETE_TX: std::io::Result<mpsc::Sender<AutoDeleteTask>> = {
        let (tx, rx) = mpsc::channel();
        // Share one worker that outlives each download's temporary Tokio runtime.
        std::thread::Builder::new()
            .name("download-cleanup".to_owned())
            .spawn(move || auto_delete_worker(rx))
            .map(|_| tx)
    };
}

#[derive(Eq, Ord, PartialEq, PartialOrd)]
struct AutoDeleteTask {
    deadline: Instant,
    id: String,
    generation: Uuid,
}

impl AutoDeleteTask {
    fn remove(self) {
        let mut downloaders = DOWNLOADERS.lock().unwrap();
        if downloaders.get(&self.id).map(|d| d.generation) == Some(self.generation) {
            downloaders.remove(&self.id);
        }
    }
}

fn auto_delete_worker(rx: mpsc::Receiver<AutoDeleteTask>) {
    let mut pending = BinaryHeap::<Reverse<AutoDeleteTask>>::new();
    loop {
        let task = if let Some(Reverse(task)) = pending.peek() {
            let now = Instant::now();
            if task.deadline <= now {
                if let Some(Reverse(task)) = pending.pop() {
                    task.remove();
                }
                continue;
            }
            match rx.recv_timeout(task.deadline - now) {
                Ok(task) => task,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        } else {
            match rx.recv() {
                Ok(task) => task,
                Err(_) => break,
            }
        };
        pending.push(Reverse(task));
    }
    // Do not abandon queued entries if the worker's sender is dropped.
    for Reverse(task) in pending {
        task.remove();
    }
}

fn enqueue_auto_delete(
    sender: Result<&mpsc::Sender<AutoDeleteTask>, &std::io::Error>,
    task: AutoDeleteTask,
) {
    match sender {
        Ok(sender) => {
            if let Err(err) = sender.send(task) {
                log::warn!("Failed to schedule download cleanup: {}", err);
                err.0.remove();
            }
        }
        Err(err) => {
            log::warn!("Failed to start download cleanup worker: {}", err);
            task.remove();
        }
    }
}

fn schedule_auto_delete(id: String, generation: Uuid, dur: Duration) {
    enqueue_auto_delete(
        AUTO_DELETE_TX.as_ref(),
        AutoDeleteTask {
            deadline: Instant::now() + dur,
            id,
            generation,
        },
    );
}

/// This struct is used to return the download data to the caller.
/// The caller should check if the file is downloaded successfully and remove the job from the map.
/// If the file is not downloaded successfully, the `data` field will be empty.
/// If the file is downloaded successfully, the `data` field will contain the downloaded data if `path` is None.
#[derive(Serialize, Debug)]
pub struct DownloadData {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub data: Vec<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_size: Option<u64>,
    pub downloaded_size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

struct Downloader {
    generation: Uuid,
    data: Vec<u8>,
    path: Option<PathBuf>,
    // Some file may be empty, so we use Option<u64> to indicate if the size is known
    total_size: Option<u64>,
    downloaded_size: u64,
    error: Option<String>,
    finished: bool,
    tx_cancel: UnboundedSender<()>,
}

// The caller should check if the file is downloaded successfully and remove the job from the map.
pub fn download_file(
    url: String,
    path: Option<PathBuf>,
    auto_del_dur: Option<Duration>,
) -> ResultType<String> {
    let id = url.clone();
    // First pass: if a non-error downloader exists for this URL, reuse it.
    // If an errored downloader exists, remove it so this call can retry.
    let mut stale_path = None;
    {
        let mut downloaders = DOWNLOADERS.lock().unwrap();
        if let Some(downloader) = downloaders.get(&id) {
            if downloader.error.is_none() {
                return Ok(id);
            }
            stale_path = downloader.path.clone();
            downloaders.remove(&id);
        }
    }
    if let Some(p) = stale_path {
        if p.exists() {
            if let Err(e) = std::fs::remove_file(&p) {
                log::warn!("Failed to remove stale download file {}: {}", p.display(), e);
            }
        }
    }

    if let Some(path) = path.as_ref() {
        if path.exists() {
            bail!("File {} already exists", path.display());
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let (tx, rx) = unbounded_channel();
    let generation = Uuid::new_v4();
    let downloader = Downloader {
        generation,
        data: Vec::new(),
        path: path.clone(),
        total_size: None,
        downloaded_size: 0,
        error: None,
        tx_cancel: tx,
        finished: false,
    };
    // Second pass (atomic with insert) to avoid race with another concurrent caller.
    let mut stale_path_after_check = None;
    {
        let mut downloaders = DOWNLOADERS.lock().unwrap();
        if let Some(existing) = downloaders.get(&id) {
            if existing.error.is_none() {
                return Ok(id);
            }
            stale_path_after_check = existing.path.clone();
            downloaders.remove(&id);
        }
        downloaders.insert(id.clone(), downloader);
    }
    if let Some(p) = stale_path_after_check {
        if p.exists() {
            if let Err(e) = std::fs::remove_file(&p) {
                log::warn!("Failed to remove stale download file {}: {}", p.display(), e);
            }
        }
    }

    let id2 = id.clone();
    std::thread::spawn(
        move || match do_download(&id2, generation, url, path, auto_del_dur, rx) {
            Ok(is_all_downloaded) => {
                let mut downloaded_size = 0;
                let mut total_size = 0;
                DOWNLOADERS.lock().unwrap().get_mut(&id2).map(|downloader| {
                    downloaded_size = downloader.downloaded_size;
                    total_size = downloader.total_size.unwrap_or(0);
                });
                log::info!(
                    "Download {} end, {}/{}, {:.2} %",
                    &id2,
                    downloaded_size,
                    total_size,
                    if total_size == 0 {
                        0.0
                    } else {
                        downloaded_size as f64 / total_size as f64 * 100.0
                    }
                );

                let is_canceled = !is_all_downloaded;
                if is_canceled {
                    if let Some(downloader) = DOWNLOADERS.lock().unwrap().remove(&id2) {
                        if let Some(p) = downloader.path {
                            if p.exists() {
                                std::fs::remove_file(p).ok();
                            }
                        }
                    }
                }
            }
            Err(e) => {
                let err = e.to_string();
                log::error!("Download {}, failed: {}", &id2, &err);
                DOWNLOADERS.lock().unwrap().get_mut(&id2).map(|downloader| {
                    downloader.error = Some(err);
                });
            }
        },
    );

    Ok(id)
}

#[tokio::main(flavor = "current_thread")]
async fn do_download(
    id: &str,
    generation: Uuid,
    url: String,
    path: Option<PathBuf>,
    auto_del_dur: Option<Duration>,
    mut rx_cancel: UnboundedReceiver<()>,
) -> ResultType<bool> {
    let client = create_http_client_async_with_url_strict(&url).await?;

    let mut is_all_downloaded = false;
    tokio::select! {
        _ = rx_cancel.recv() => {
            return Ok(is_all_downloaded);
        }
        head_resp = client.head(&url).send() => {
            match head_resp {
                Ok(resp) => {
                    if resp.status().is_success() {
                        let total_size = resp
                            .headers()
                            .get(reqwest::header::CONTENT_LENGTH)
                            .and_then(|ct_len| ct_len.to_str().ok())
                            .and_then(|ct_len| ct_len.parse::<u64>().ok());
                        let Some(total_size) = total_size else {
                            bail!("Failed to get content length");
                        };
                        DOWNLOADERS.lock().unwrap().get_mut(id).map(|downloader| {
                            downloader.total_size = Some(total_size);
                        });
                    } else {
                        bail!("Failed to get content length: {}", resp.status());
                    }
                }
                Err(e) => {
                    return Err(e.into());
                }
            }
        }
    }

    let mut response;
    tokio::select! {
        _ = rx_cancel.recv() => {
            return Ok(is_all_downloaded);
        }
        resp = client.get(url).send() => {
            response = resp?;
        }
    }

    let mut dest: Option<File> = None;
    if let Some(p) = path {
        dest = Some(File::create(p).await?);
    }

    loop {
        tokio::select! {
            _ = rx_cancel.recv() => {
                break;
            }
            chunk = response.chunk() => {
                match chunk {
                    Ok(Some(chunk)) => {
                        match dest {
                            Some(ref mut f) => {
                                f.write_all(&chunk).await?;
                                f.flush().await?;
                                DOWNLOADERS.lock().unwrap().get_mut(id).map(|downloader| {
                                    downloader.downloaded_size += chunk.len() as u64;
                                });
                            }
                            None => {
                                DOWNLOADERS.lock().unwrap().get_mut(id).map(|downloader| {
                                    downloader.data.extend_from_slice(&chunk);
                                    downloader.downloaded_size += chunk.len() as u64;
                                });
                            }
                        }
                    }
                    Ok(None) => {
                        is_all_downloaded = true;
                        break;
                    },
                    Err(e) => {
                        log::error!("Download {} failed: {}", id, e);
                        return Err(e.into());
                    }
                }
            }
        }
    }

    if let Some(mut f) = dest.take() {
        f.flush().await?;
    }

    if let Some(ref mut downloader) = DOWNLOADERS.lock().unwrap().get_mut(id) {
        downloader.finished = true;
    }
    if is_all_downloaded {
        if let Some(dur) = auto_del_dur {
            schedule_auto_delete(id.to_string(), generation, dur);
        }
    }
    Ok(is_all_downloaded)
}

pub fn get_download_data(id: &str) -> ResultType<DownloadData> {
    let downloaders = DOWNLOADERS.lock().unwrap();
    if let Some(downloader) = downloaders.get(id) {
        let downloaded_size = downloader.downloaded_size;
        let total_size = downloader.total_size.clone();
        let error = downloader.error.clone();
        let data = if total_size.unwrap_or(0) == downloaded_size && downloader.path.is_none() {
            downloader.data.clone()
        } else {
            Vec::new()
        };
        let path = downloader.path.clone();
        let download_data = DownloadData {
            data,
            path,
            total_size,
            downloaded_size,
            error,
        };
        Ok(download_data)
    } else {
        bail!("Downloader not found")
    }
}

pub fn cancel(id: &str) {
    if let Some(downloader) = DOWNLOADERS.lock().unwrap().get(id) {
        // downloader.is_canceled.store(true, Ordering::SeqCst);
        // The receiver may not be able to receive the cancel signal, so we also set the atomic bool to true
        let _ = downloader.tx_cancel.send(());
    }
}

pub fn remove(id: &str) {
    let _ = DOWNLOADERS.lock().unwrap().remove(id);
}

#[cfg(test)]
mod tests {
    use super::{
        auto_delete_worker, enqueue_auto_delete, schedule_auto_delete, AutoDeleteTask, Downloader,
        DOWNLOADERS,
    };
    use hbb_common::tokio::{runtime::Builder, sync::mpsc::unbounded_channel};
    use std::{
        sync::mpsc,
        time::{Duration, Instant},
    };
    use uuid::Uuid;

    fn insert_downloader(id: &str) -> Uuid {
        let generation = Uuid::new_v4();
        let (tx_cancel, _) = unbounded_channel();
        DOWNLOADERS.lock().unwrap().insert(
            id.to_owned(),
            Downloader {
                generation,
                data: Vec::new(),
                path: None,
                total_size: Some(0),
                downloaded_size: 0,
                error: None,
                finished: true,
                tx_cancel,
            },
        );
        generation
    }

    fn wait_until_removed(id: &str) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while DOWNLOADERS.lock().unwrap().contains_key(id) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!DOWNLOADERS.lock().unwrap().contains_key(id));
    }

    #[test]
    fn auto_delete_outlives_download_runtime() {
        let id = Uuid::new_v4().to_string();
        let generation = insert_downloader(&id);

        Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(async {
                schedule_auto_delete(id.clone(), generation, Duration::from_millis(20));
            });

        wait_until_removed(&id);
    }

    #[test]
    fn auto_delete_preserves_replacement() {
        let id = Uuid::new_v4().to_string();
        let generation = insert_downloader(&id);
        let (tx, rx) = mpsc::channel();
        enqueue_auto_delete(
            Ok(&tx),
            AutoDeleteTask {
                deadline: Instant::now(),
                id: id.clone(),
                generation,
            },
        );
        DOWNLOADERS.lock().unwrap().remove(&id);
        let replacement = insert_downloader(&id);

        // A later deadline acts as a barrier: the old cleanup must run first.
        let barrier_id = Uuid::new_v4().to_string();
        enqueue_auto_delete(
            Ok(&tx),
            AutoDeleteTask {
                deadline: Instant::now() + Duration::from_millis(20),
                generation: insert_downloader(&barrier_id),
                id: barrier_id.clone(),
            },
        );
        let worker = std::thread::spawn(move || auto_delete_worker(rx));
        wait_until_removed(&barrier_id);
        drop(tx);
        worker.join().unwrap();
        let remaining = DOWNLOADERS.lock().unwrap().remove(&id).unwrap();
        assert_eq!(remaining.generation, replacement);
    }

    #[test]
    fn auto_delete_honors_earlier_deadlines() {
        let late_id = Uuid::new_v4().to_string();
        schedule_auto_delete(
            late_id.clone(),
            insert_downloader(&late_id),
            Duration::from_secs(60),
        );
        let early_id = Uuid::new_v4().to_string();
        schedule_auto_delete(
            early_id.clone(),
            insert_downloader(&early_id),
            Duration::from_millis(20),
        );
        wait_until_removed(&early_id);
        assert!(DOWNLOADERS.lock().unwrap().remove(&late_id).is_some());
    }

    #[test]
    fn failed_scheduling_removes_only_the_original_instance() {
        let startup_error =
            std::io::Error::new(std::io::ErrorKind::Other, "thread creation failed");
        let (tx, rx) = mpsc::channel();
        drop(rx);
        for sender in [Err(&startup_error), Ok(&tx)] {
            for replace in [false, true] {
                let id = Uuid::new_v4().to_string();
                let generation = insert_downloader(&id);
                let replacement = replace.then(|| insert_downloader(&id));
                enqueue_auto_delete(
                    sender,
                    AutoDeleteTask {
                        deadline: Instant::now() + Duration::from_secs(60),
                        id: id.clone(),
                        generation,
                    },
                );
                let remaining = DOWNLOADERS.lock().unwrap().remove(&id);
                assert_eq!(remaining.map(|d| d.generation), replacement);
            }
        }
    }
}
