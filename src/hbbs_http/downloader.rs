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
use std::{collections::HashMap, path::PathBuf, sync::Mutex};

lazy_static! {
    static ref DOWNLOADERS: Mutex<HashMap<String, Downloader>> = Default::default();
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
    pub finished: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

struct Downloader {
    data: Vec<u8>,
    path: Option<PathBuf>,
    total_size: u64,
    artifact_sha256: String,
    downloaded_size: u64,
    error: Option<String>,
    finished: bool,
    tx_cancel: UnboundedSender<()>,
}

impl Downloader {
    fn matches_request(&self, path: &Option<PathBuf>, options: &DownloadOptions) -> bool {
        self.path.as_ref() == path.as_ref()
            && self.total_size == options.expected_size
            && self.artifact_sha256 == options.artifact_sha256
    }
}

pub struct DownloadOptions {
    pub expected_size: u64,
    pub artifact_sha256: String,
}

struct DownloadRequest {
    id: String,
    url: String,
    path: Option<PathBuf>,
    expected_size: u64,
}

// The caller should check if the file is downloaded successfully and remove the job from the map.
pub fn download_file(
    url: String,
    path: Option<PathBuf>,
    options: DownloadOptions,
) -> ResultType<String> {
    let id = url.clone();
    let (tx, rx) = unbounded_channel();
    {
        let mut downloaders = DOWNLOADERS.lock().unwrap();
        if let Some(existing) = downloaders.get(&id) {
            if existing.error.is_none() {
                if existing.matches_request(&path, &options) {
                    return Ok(id);
                }
                bail!("Existing download job does not match requested artifact");
            }
        }

        let stale_path = downloaders
            .remove(&id)
            .and_then(|downloader| downloader.path);
        if stale_path.as_ref() != path.as_ref() {
            if let Some(p) = stale_path {
                if p.exists() {
                    if let Err(e) = std::fs::remove_file(&p) {
                        log::warn!(
                            "Failed to remove stale download file {}: {}",
                            p.display(),
                            e
                        );
                    }
                }
            }
        }

        if let Some(path) = path.as_ref() {
            prepare_download_path(path)?;
        }

        downloaders.insert(
            id.clone(),
            Downloader {
                data: Vec::new(),
                path: path.clone(),
                total_size: options.expected_size,
                artifact_sha256: options.artifact_sha256,
                downloaded_size: 0,
                error: None,
                tx_cancel: tx,
                finished: false,
            },
        );
    }

    let request = DownloadRequest {
        id: id.clone(),
        url,
        path,
        expected_size: options.expected_size,
    };
    let worker_id = id.clone();
    std::thread::spawn(move || match do_download(request, rx) {
        Ok(is_all_downloaded) => {
            let mut downloaded_size = 0;
            let mut total_size = 0;
            DOWNLOADERS
                .lock()
                .unwrap()
                .get_mut(&worker_id)
                .map(|downloader| {
                    downloaded_size = downloader.downloaded_size;
                    total_size = downloader.total_size;
                });
            log::info!(
                "Download {} end, {}/{}, {:.2} %",
                &worker_id,
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
                if let Some(downloader) = DOWNLOADERS.lock().unwrap().remove(&worker_id) {
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
            log::error!("Download {}, failed: {}", &worker_id, &err);
            DOWNLOADERS
                .lock()
                .unwrap()
                .get_mut(&worker_id)
                .map(|downloader| {
                    downloader.error = Some(err);
                });
        }
    });

    Ok(id)
}

fn prepare_download_path(path: &PathBuf) -> ResultType<()> {
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn do_download(
    request: DownloadRequest,
    mut rx_cancel: UnboundedReceiver<()>,
) -> ResultType<bool> {
    let client = create_http_client_async_with_url_strict(&request.url).await?;

    let mut is_all_downloaded = false;
    let mut downloaded_size = 0_u64;
    let mut response;
    tokio::select! {
        _ = rx_cancel.recv() => {
            return Ok(is_all_downloaded);
        }
        resp = client.get(request.url).send() => {
            response = resp?.error_for_status()?;
        }
    }

    let mut dest: Option<File> = None;
    if let Some(p) = request.path {
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
                        let next_size = downloaded_size
                            .checked_add(chunk.len() as u64)
                            .ok_or_else(|| hbb_common::anyhow::anyhow!("Download size overflow"))?;
                        if next_size > request.expected_size {
                            bail!(
                                "Download size exceeds expected size: expected {}, got {}",
                                request.expected_size,
                                next_size
                            );
                        }
                        match dest {
                            Some(ref mut f) => {
                                f.write_all(&chunk).await?;
                                f.flush().await?;
                            }
                            None => {
                                let mut downloaders = DOWNLOADERS.lock().unwrap();
                                let Some(downloader) = downloaders.get_mut(&request.id) else {
                                    bail!("Downloader not found");
                                };
                                downloader.data.extend_from_slice(&chunk);
                            }
                        }
                        downloaded_size = next_size;
                        let mut downloaders = DOWNLOADERS.lock().unwrap();
                        let Some(downloader) = downloaders.get_mut(&request.id) else {
                            bail!("Downloader not found");
                        };
                        downloader.downloaded_size = downloaded_size;
                    }
                    Ok(None) => {
                        is_all_downloaded = true;
                        break;
                    },
                    Err(e) => {
                        log::error!("Download {} failed: {}", request.id, e);
                        return Err(e.into());
                    }
                }
            }
        }
    }

    if let Some(mut f) = dest.take() {
        f.flush().await?;
    }

    if is_all_downloaded && downloaded_size != request.expected_size {
        bail!(
            "Download size mismatch: expected {}, got {}",
            request.expected_size,
            downloaded_size
        );
    }
    if is_all_downloaded {
        let mut downloaders = DOWNLOADERS.lock().unwrap();
        let Some(downloader) = downloaders.get_mut(&request.id) else {
            bail!("Downloader not found");
        };
        downloader.finished = true;
    }
    Ok(is_all_downloaded)
}

pub fn get_download_data(id: &str) -> ResultType<DownloadData> {
    let downloaders = DOWNLOADERS.lock().unwrap();
    if let Some(downloader) = downloaders.get(id) {
        let data = if downloader.finished && downloader.path.is_none() {
            downloader.data.clone()
        } else {
            Vec::new()
        };
        let download_data = DownloadData {
            data,
            path: downloader.path.clone(),
            total_size: Some(downloader.total_size),
            downloaded_size: downloader.downloaded_size,
            finished: downloader.finished,
            error: downloader.error.clone(),
        };
        Ok(download_data)
    } else {
        bail!("Downloader not found")
    }
}

pub fn cancel(id: &str) {
    if let Some(downloader) = DOWNLOADERS.lock().unwrap().get(id) {
        // The receiver may not be able to receive the cancel signal, so keep the job until it exits.
        let _ = downloader.tx_cancel.send(());
    }
}

pub fn remove(id: &str) {
    let _ = DOWNLOADERS.lock().unwrap().remove(id);
}
