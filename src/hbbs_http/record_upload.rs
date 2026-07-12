use crate::hbbs_http::create_http_client;
use bytes::Bytes;
use hbb_common::{bail, config::Config, lazy_static, log, ResultType};
use reqwest::blocking::{Body, Client};
use scrap::record::RecordState;
use serde::Serialize;
use serde_json::Map;
use std::{
    fs::File,
    io::{prelude::*, SeekFrom},
    sync::{mpsc::Receiver, Arc, Mutex},
    time::{Duration, Instant},
};

const MAX_HEADER_LEN: usize = 1024;
const SHOULD_SEND_TIME: Duration = Duration::from_secs(1);
const SHOULD_SEND_SIZE: u64 = 1024 * 1024;

lazy_static::lazy_static! {
    static ref ENABLE: Arc<Mutex<bool>> = Default::default();
}

pub fn is_enable() -> bool {
    ENABLE.lock().unwrap().clone()
}

pub fn run(rx: Receiver<RecordState>) {
    let mut uploader = RecordUploader {
        client: create_http_client(),
        api_server: crate::get_api_server(
            Config::get_option("api-server"),
            Config::get_option("custom-rendezvous-server"),
        ),
        filepath: Default::default(),
        filename: Default::default(),
        upload_size: Default::default(),
        running: Default::default(),
        last_send: Instant::now(),
    };
    std::thread::spawn(move || loop {
        if let Err(e) = match rx.recv() {
            Ok(state) => match state {
                RecordState::NewFile(filepath) => uploader.handle_new_file(filepath),
                RecordState::NewFrame => {
                    if uploader.running {
                        uploader.handle_frame(false)
                    } else {
                        Ok(())
                    }
                }
                RecordState::WriteTail => {
                    if uploader.running {
                        uploader.handle_tail()
                    } else {
                        Ok(())
                    }
                }
                RecordState::RemoveFile => {
                    if uploader.running {
                        uploader.handle_remove()
                    } else {
                        Ok(())
                    }
                }
            },
            Err(e) => {
                log::trace!("upload thread stop: {}", e);
                break;
            }
        } {
            uploader.running = false;
            log::error!("upload stop: {}", e);
        }
    });
}

struct RecordUploader {
    client: Client,
    api_server: String,
    filepath: String,
    filename: String,
    upload_size: u64,
    running: bool,
    last_send: Instant,
}
impl RecordUploader {
    fn send<Q, B>(&self, query: &Q, body: B) -> ResultType<()>
    where
        Q: Serialize + ?Sized,
        B: Into<Body>,
    {
        match self
            .client
            .post(format!("{}/api/record", self.api_server))
            .query(query)
            .body(body)
            .send()
        {
            Ok(resp) => {
                if let Ok(m) = resp.json::<Map<String, serde_json::Value>>() {
                    if let Some(e) = m.get("error") {
                        bail!(e.to_string());
                    }
                }
                Ok(())
            }
            Err(e) => bail!(e.to_string()),
        }
    }

    fn handle_new_file(&mut self, filepath: String) -> ResultType<()> {
        match std::path::PathBuf::from(&filepath).file_name() {
            Some(filename) => match filename.to_owned().into_string() {
                Ok(filename) => {
                    self.filename = filename.clone();
                    self.filepath = filepath.clone();
                    self.upload_size = 0;
                    self.running = true;
                    self.last_send = Instant::now();
                    self.send(&[("type", "new"), ("file", &filename)], Bytes::new())?;
                    Ok(())
                }
                Err(_) => bail!("can't parse filename:{:?}", filename),
            },
            None => bail!("can't parse filepath:{}", filepath),
        }
    }

    fn handle_frame(&mut self, flush: bool) -> ResultType<()> {
        if !flush && self.last_send.elapsed() < SHOULD_SEND_TIME {
            return Ok(());
        }
        match File::open(&self.filepath) {
            Ok(mut file) => match file.metadata() {
                Ok(m) => {
                    let len = m.len();
                    if len <= self.upload_size {
                        return Ok(());
                    }
                    if !flush && len - self.upload_size < SHOULD_SEND_SIZE {
                        return Ok(());
                    }
                    let mut buf = Vec::new();
                    match file.seek(SeekFrom::Start(self.upload_size)) {
                        Ok(_) => match file.read_to_end(&mut buf) {
                            Ok(length) => {
                                self.send(
                                    &[
                                        ("type", "part"),
                                        ("file", &self.filename),
                                        ("offset", &self.upload_size.to_string()),
                                        ("length", &length.to_string()),
                                    ],
                                    buf,
                                )?;
                                self.upload_size = len;
                                self.last_send = Instant::now();
                                Ok(())
                            }
                            Err(e) => bail!(e.to_string()),
                        },
                        Err(e) => bail!(e.to_string()),
                    }
                }
                Err(e) => bail!(e.to_string()),
            },
            Err(e) => bail!(e.to_string()),
        }
    }

    fn handle_tail(&mut self) -> ResultType<()> {
        self.handle_frame(true)?;
        match File::open(&self.filepath) {
            Ok(mut file) => {
                let mut buf = vec![0u8; MAX_HEADER_LEN];
                match file.read(&mut buf) {
                    Ok(length) => {
                        buf.truncate(length);
                        self.send(
                            &[
                                ("type", "tail"),
                                ("file", &self.filename),
                                ("offset", "0"),
                                ("length", &length.to_string()),
                            ],
                            buf,
                        )?;
                        log::info!("upload success, file: {}", self.filename);
                        Ok(())
                    }
                    Err(e) => bail!(e.to_string()),
                }
            }
            Err(e) => bail!(e.to_string()),
        }
    }

    fn handle_remove(&mut self) -> ResultType<()> {
        self.send(
            &[("type", "remove"), ("file", &self.filename)],
            Bytes::new(),
        )?;
        Ok(())
    }
}
