use super::create_http_client_async;
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
use std::{collections::HashMap, path::PathBuf, sync::Mutex, time::Duration};

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

struct Downloader {
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
    if DOWNLOADERS.lock().unwrap().contains_key(&id) {
        return Ok(id);
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
    let downloader = Downloader {
        data: Vec::new(),
        path: path.clone(),
        total_size: None,
        downloaded_size: 0,
        error: None,
        tx_cancel: tx,
        finished: false,
    };
    let mut downloaders = DOWNLOADERS.lock().unwrap();
    downloaders.insert(id.clone(), downloader);

    let id2 = id.clone();
    std::thread::spawn(
        move || match do_download(&id2, url, path, auto_del_dur, rx) {
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
    url: String,
    path: Option<PathBuf>,
    auto_del_dur: Option<Duration>,
    mut rx_cancel: UnboundedReceiver<()>,
) -> ResultType<bool> {
    let client = create_http_client_async();

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
        let id_del = id.to_string();
        if let Some(dur) = auto_del_dur {
            tokio::spawn(async move {
                tokio::time::sleep(dur).await;
                DOWNLOADERS.lock().unwrap().remove(&id_del);
            });
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
