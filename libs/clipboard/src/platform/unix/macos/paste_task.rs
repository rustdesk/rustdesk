use crate::{
    platform::unix::{FileDescription, FileType, BLOCK_SIZE},
    send_data, ClipboardFile, CliprdrError, ProgressPercent,
};
use hbb_common::{allow_err, log, tokio::time::Instant};
use std::{
    cmp::min,
    fs::{File, FileTimes},
    io::{BufWriter, Write},
    os::macos::fs::FileTimesExt,
    path::{Path, PathBuf},
    sync::{
        mpsc::{Receiver, RecvTimeoutError},
        Arc, Mutex,
    },
    thread,
    time::{Duration, SystemTime},
};

const RECV_RETRY_TIMES: usize = 3;

const DOWNLOAD_EXTENSION: &str = "rddownload";
const RECEIVE_WAIT_TIMEOUT: Duration = Duration::from_millis(5_000);

// https://stackoverflow.com/a/15112784/1926020
// "1984-01-24 08:00:00 +0000"
const TIMESTAMP_FOR_FILE_PROGRESS_COMPLETED: u64 = 443779200;
const ATTR_PROGRESS_FRACTION_COMPLETED: &str = "com.apple.progress.fractionCompleted";

pub struct FileContentsResponse {
    pub conn_id: i32,
    pub msg_flags: i32,
    pub stream_id: i32,
    pub requested_data: Vec<u8>,
}

#[derive(Debug)]
struct PasteTaskProgress {
    // Use list index to identify the file
    // `list_index` is also used as the stream id
    list_index: i32,
    offset: u64,
    total_size: u64,
    current_size: u64,
    last_sent_time: Instant,
    download_file_index: i32,
    download_file_size: u64,
    download_file_path: String,
    download_file_current_size: u64,
    file_handle: Option<BufWriter<File>>,
    error: Option<CliprdrError>,
    is_canceled: bool,
}

struct PasteTaskHandle {
    progress: PasteTaskProgress,
    target_dir: PathBuf,
    files: Vec<FileDescription>,
}

pub struct PasteTask {
    exit: Arc<Mutex<bool>>,
    handle: Arc<Mutex<Option<PasteTaskHandle>>>,
    handle_worker: Option<thread::JoinHandle<()>>,
}

impl Drop for PasteTask {
    fn drop(&mut self) {
        *self.exit.lock().unwrap() = true;
        if let Some(handle_worker) = self.handle_worker.take() {
            handle_worker.join().ok();
        }
    }
}

impl PasteTask {
    const INVALID_FILE_INDEX: i32 = -1;

    pub fn new(rx_file_contents: Receiver<FileContentsResponse>) -> Self {
        let exit = Arc::new(Mutex::new(false));
        let handle = Arc::new(Mutex::new(None));
        let handle_worker =
            Self::init_worker_thread(exit.clone(), handle.clone(), rx_file_contents);
        Self {
            handle,
            exit,
            handle_worker: Some(handle_worker),
        }
    }

    pub fn start(&mut self, target_dir: PathBuf, files: Vec<FileDescription>) {
        let mut task_lock = self.handle.lock().unwrap();
        if task_lock
            .as_ref()
            .map(|x| !x.is_finished())
            .unwrap_or(false)
        {
            log::error!("Previous paste task is not finished, ignore new request.");
            return;
        }
        let total_size = files.iter().map(|f| f.size).sum();
        let mut task_handle = PasteTaskHandle {
            progress: PasteTaskProgress {
                list_index: -1,
                offset: 0,
                total_size,
                current_size: 0,
                last_sent_time: Instant::now(),
                download_file_index: Self::INVALID_FILE_INDEX,
                download_file_size: 0,
                download_file_path: "".to_owned(),
                download_file_current_size: 0,
                file_handle: None,
                error: None,
                is_canceled: false,
            },
            target_dir,
            files,
        };
        task_handle.update_next(0).ok();
        if task_handle.is_finished() {
            task_handle.on_finished();
        } else {
            if let Err(e) = task_handle.send_file_contents_request() {
                log::error!("Failed to send file contents request, error: {}", &e);
                task_handle.on_error(e);
            }
        }
        *task_lock = Some(task_handle);
    }

    pub fn cancel(&self) {
        let mut task_handle = self.handle.lock().unwrap();
        if let Some(task_handle) = task_handle.as_mut() {
            task_handle.progress.is_canceled = true;
            task_handle.on_cancelled();
        }
    }

    fn init_worker_thread(
        exit: Arc<Mutex<bool>>,
        handle: Arc<Mutex<Option<PasteTaskHandle>>>,
        rx_file_contents: Receiver<FileContentsResponse>,
    ) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            let mut retry_count = 0;
            loop {
                if *exit.lock().unwrap() {
                    break;
                }

                match rx_file_contents.recv_timeout(Duration::from_millis(300)) {
                    Ok(file_contents) => {
                        let mut task_lock = handle.lock().unwrap();
                        let Some(task_handle) = task_lock.as_mut() else {
                            continue;
                        };
                        if task_handle.is_finished() {
                            continue;
                        }

                        if file_contents.stream_id != task_handle.progress.list_index {
                            // ignore invalid stream id
                            continue;
                        } else if file_contents.msg_flags != 0x01 {
                            retry_count += 1;
                            if retry_count > RECV_RETRY_TIMES {
                                task_handle.progress.error = Some(CliprdrError::InvalidRequest {
                                    description: format!(
                                        "Failed to read file contents, stream id: {}, msg_flags: {}",
                                        file_contents.stream_id,
                                        file_contents.msg_flags
                                    ),
                                });
                            }
                        } else {
                            let resp_list_index = file_contents.stream_id;
                            let Some(file) = &task_handle.files.get(resp_list_index as usize)
                            else {
                                // unreachable
                                // Because `task_handle.progress.list_index >= task_handle.files.len()` should always be false
                                log::warn!(
                                    "Invalid response list index: {}, file length: {}",
                                    resp_list_index,
                                    task_handle.files.len()
                                );
                                continue;
                            };
                            if file.conn_id != file_contents.conn_id {
                                // unreachable
                                // We still add log here to make sure we can see the error message when it happens.
                                log::error!(
                                    "Invalid response conn id: {}, expected: {}",
                                    file_contents.conn_id,
                                    file.conn_id
                                );
                                continue;
                            }

                            if let Err(e) = task_handle.handle_file_contents_response(file_contents)
                            {
                                log::error!("Failed to handle file contents response: {}", &e);
                                task_handle.on_error(e);
                            }
                        }

                        if !task_handle.is_finished() {
                            if let Err(e) = task_handle.send_file_contents_request() {
                                log::error!("Failed to send file contents request: {}", &e);
                                task_handle.on_error(e);
                            }
                        } else {
                            retry_count = 0;
                            task_handle.on_finished();
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => {
                        let mut task_lock = handle.lock().unwrap();
                        if let Some(task_handle) = task_lock.as_mut() {
                            if task_handle.check_receive_timemout() {
                                retry_count = 0;
                                task_handle.on_finished();
                            }
                        }
                    }
                    Err(RecvTimeoutError::Disconnected) => {
                        break;
                    }
                }
            }
        })
    }

    pub fn is_finished(&self) -> bool {
        self.handle
            .lock()
            .unwrap()
            .as_ref()
            .map(|handle| handle.is_finished())
            .unwrap_or(true)
    }

    pub fn progress_percent(&self) -> Option<ProgressPercent> {
        self.handle
            .lock()
            .unwrap()
            .as_ref()
            .map(|handle| handle.progress_percent())
    }
}

impl PasteTaskHandle {
    fn update_next(&mut self, size: u64) -> Result<(), CliprdrError> {
        if self.is_finished() {
            return Ok(());
        }
        self.progress.current_size += size;

        let is_start = self.progress.list_index == -1;
        if is_start || (self.progress.offset + size) >= self.progress.download_file_size {
            if !is_start {
                self.on_done();
            }
            for i in (self.progress.list_index + 1)..self.files.len() as i32 {
                let Some(file_desc) = self.files.get(i as usize) else {
                    return Err(CliprdrError::InvalidRequest {
                        description: format!("Invalid file index: {}", i),
                    });
                };
                match file_desc.kind {
                    FileType::File => {
                        if file_desc.size == 0 {
                            if let Some(new_file_path) =
                                Self::get_new_filename(&self.target_dir, file_desc)
                            {
                                if let Ok(f) = std::fs::File::create(&new_file_path) {
                                    f.set_len(0).ok();
                                    Self::set_file_metadata(&f, file_desc);
                                }
                            };
                        } else {
                            self.progress.list_index = i;
                            self.progress.offset = 0;
                            self.open_new_writer()?;
                            break;
                        }
                    }
                    FileType::Directory => {
                        let path = self.target_dir.join(&file_desc.name);
                        if !path.exists() {
                            std::fs::create_dir_all(path).ok();
                        }
                    }
                    FileType::Symlink => {
                        // to-do: handle symlink
                    }
                }
            }
        } else {
            self.progress.offset += size;
            self.progress.download_file_current_size += size;
            self.update_progress_completed(None);
        }
        if self.progress.file_handle.is_none() {
            self.progress.list_index = self.files.len() as i32;
            self.progress.offset = 0;
            self.progress.download_file_size = 0;
            self.progress.download_file_current_size = 0;
        }
        Ok(())
    }

    fn start_progress_completed(&self) {
        if let Some(file) = self.progress.file_handle.as_ref() {
            let creation_time =
                SystemTime::UNIX_EPOCH + Duration::from_secs(TIMESTAMP_FOR_FILE_PROGRESS_COMPLETED);
            file.get_ref()
                .set_times(FileTimes::new().set_created(creation_time))
                .ok();
            xattr::set(
                &self.progress.download_file_path,
                ATTR_PROGRESS_FRACTION_COMPLETED,
                "0.0".as_bytes(),
            )
            .ok();
        }
    }

    fn update_progress_completed(&mut self, fraction_completed: Option<f64>) {
        let fraction_completed = fraction_completed.unwrap_or_else(|| {
            let current_size = self.progress.download_file_current_size as f64;
            let total_size = self.progress.download_file_size as f64;
            if total_size > 0.0 {
                current_size / total_size
            } else {
                1.0
            }
        });
        xattr::set(
            &self.progress.download_file_path,
            ATTR_PROGRESS_FRACTION_COMPLETED,
            &fraction_completed.to_string().as_bytes(),
        )
        .ok();
    }

    #[inline]
    fn remove_progress_completed(path: &str) {
        if !path.is_empty() {
            xattr::remove(path, ATTR_PROGRESS_FRACTION_COMPLETED).ok();
        }
    }

    fn open_new_writer(&mut self) -> Result<(), CliprdrError> {
        let Some(file) = &self.files.get(self.progress.list_index as usize) else {
            return Err(CliprdrError::InvalidRequest {
                description: format!(
                    "Invalid file index: {}, file count: {}",
                    self.progress.list_index,
                    self.files.len()
                ),
            });
        };

        let original_file_path = self
            .target_dir
            .join(&file.name)
            .to_string_lossy()
            .to_string();
        let Some(download_file_path) = Self::get_first_filename(
            format!("{}.{}", original_file_path, DOWNLOAD_EXTENSION),
            file.kind,
        ) else {
            return Err(CliprdrError::CommonError {
                description: format!("Failed to get download file path: {}", original_file_path),
            });
        };
        let Some(download_path_parent) = Path::new(&download_file_path).parent() else {
            return Err(CliprdrError::CommonError {
                description: format!(
                    "Failed to get parent of the download file path: {}",
                    original_file_path
                ),
            });
        };
        if !download_path_parent.exists() {
            if let Err(e) = std::fs::create_dir_all(download_path_parent) {
                return Err(CliprdrError::FileError {
                    path: download_path_parent.to_string_lossy().to_string(),
                    err: e,
                });
            }
        }
        match std::fs::File::create(&download_file_path) {
            Ok(handle) => {
                let writer = BufWriter::with_capacity(BLOCK_SIZE as usize * 2, handle);
                self.progress.download_file_index = self.progress.list_index;
                self.progress.download_file_size = file.size;
                self.progress.download_file_path = download_file_path;
                self.progress.download_file_current_size = 0;
                self.progress.file_handle = Some(writer);
                self.start_progress_completed();
            }
            Err(e) => {
                self.progress.error = Some(CliprdrError::FileError {
                    path: download_file_path,
                    err: e,
                });
            }
        };
        Ok(())
    }

    fn get_first_filename(path: String, r#type: FileType) -> Option<String> {
        let p = Path::new(&path);
        if !p.exists() {
            return Some(path);
        } else {
            for i in 1..9999999 {
                let new_path = match r#type {
                    FileType::File => {
                        if let Some(ext) = p.extension() {
                            let new_name = format!(
                                "{}-{}.{}",
                                p.file_stem().unwrap_or_default().to_string_lossy(),
                                i,
                                ext.to_string_lossy()
                            );
                            p.with_file_name(new_name).to_string_lossy().to_string()
                        } else {
                            format!("{} ({})", path, i)
                        }
                    }
                    FileType::Directory => format!("{} ({})", path, i),
                    FileType::Symlink => {
                        // to-do: handle symlink
                        return None;
                    }
                };
                if !Path::new(&new_path).exists() {
                    return Some(new_path);
                }
            }
        }
        // unreachable
        None
    }

    fn progress_percent(&self) -> ProgressPercent {
        let percent = self.progress.current_size as f64 / self.progress.total_size as f64;
        ProgressPercent {
            percent,
            is_canceled: self.progress.is_canceled,
            is_failed: self.progress.error.is_some(),
        }
    }

    fn is_finished(&self) -> bool {
        self.progress.is_canceled
            || self.progress.error.is_some()
            || self.progress.list_index >= self.files.len() as i32
    }

    fn check_receive_timemout(&mut self) -> bool {
        if !self.is_finished() {
            if self.progress.last_sent_time.elapsed() > RECEIVE_WAIT_TIMEOUT {
                self.progress.error = Some(CliprdrError::InvalidRequest {
                    description: "Failed to read file contents".to_string(),
                });
                return true;
            }
        }
        false
    }

    fn on_finished(&mut self) {
        if self.progress.error.is_some() {
            self.on_cancelled();
        } else {
            self.on_done();
        }
        if self.progress.current_size != self.progress.total_size {
            self.progress.error = Some(CliprdrError::InvalidRequest {
                description: "Failed to download all files".to_string(),
            });
        }
    }

    fn on_error(&mut self, error: CliprdrError) {
        self.progress.error = Some(error);
        self.on_cancelled();
    }

    fn on_cancelled(&mut self) {
        self.progress.file_handle = None;
        std::fs::remove_file(&self.progress.download_file_path).ok();
    }

    fn on_done(&mut self) {
        self.update_progress_completed(Some(1.0));
        Self::remove_progress_completed(&self.progress.download_file_path);

        let Some(file) = self.progress.file_handle.as_mut() else {
            return;
        };
        if self.progress.download_file_index == PasteTask::INVALID_FILE_INDEX {
            return;
        }

        if let Err(e) = file.flush() {
            log::error!("Failed to flush file: {:?}", e);
        }
        self.progress.file_handle = None;

        let Some(file_desc) = self.files.get(self.progress.download_file_index as usize) else {
            // unreachable
            log::error!(
                "Failed to get file description: {}",
                self.progress.download_file_index
            );
            return;
        };
        let Some(rename_to_path) = Self::get_new_filename(&self.target_dir, file_desc) else {
            return;
        };
        match std::fs::rename(&self.progress.download_file_path, &rename_to_path) {
            Ok(_) => Self::set_file_metadata2(&rename_to_path, file_desc),
            Err(e) => {
                log::error!("Failed to rename file: {:?}", e);
            }
        }
        self.progress.download_file_path = "".to_owned();
        self.progress.download_file_index = PasteTask::INVALID_FILE_INDEX;
    }

    fn get_new_filename(target_dir: &PathBuf, file_desc: &FileDescription) -> Option<String> {
        let mut rename_to_path = target_dir
            .join(&file_desc.name)
            .to_string_lossy()
            .to_string();
        if Path::new(&rename_to_path).exists() {
            let Some(new_path) = Self::get_first_filename(rename_to_path.clone(), file_desc.kind)
            else {
                log::error!("Failed to get new file name: {}", &rename_to_path);
                return None;
            };
            rename_to_path = new_path;
        }
        Some(rename_to_path)
    }

    #[inline]
    fn set_file_metadata(f: &File, file_desc: &FileDescription) {
        let times = FileTimes::new()
            .set_accessed(file_desc.atime)
            .set_modified(file_desc.last_modified)
            .set_created(file_desc.creation_time);
        f.set_times(times).ok();
    }

    #[inline]
    fn set_file_metadata2(path: &str, file_desc: &FileDescription) {
        let times = FileTimes::new()
            .set_accessed(file_desc.atime)
            .set_modified(file_desc.last_modified)
            .set_created(file_desc.creation_time);
        File::options()
            .write(true)
            .open(path)
            .map(|f| f.set_times(times))
            .ok();
    }

    fn send_file_contents_request(&mut self) -> Result<(), CliprdrError> {
        if self.is_finished() {
            return Ok(());
        }

        let stream_id = self.progress.list_index;
        let list_index = self.progress.list_index;
        let Some(file) = &self.files.get(list_index as usize) else {
            // unreachable
            return Err(CliprdrError::InvalidRequest {
                description: format!("Invalid file index: {}", list_index),
            });
        };
        let cb_requested = min(BLOCK_SIZE as u64, file.size - self.progress.offset);
        let conn_id = file.conn_id;

        let (n_position_high, n_position_low) = (
            (self.progress.offset >> 32) as i32,
            (self.progress.offset & (u32::MAX as u64)) as i32,
        );
        let request = ClipboardFile::FileContentsRequest {
            stream_id,
            list_index,
            dw_flags: 2,
            n_position_low,
            n_position_high,
            cb_requested: cb_requested as _,
            have_clip_data_id: false,
            clip_data_id: 0,
        };
        allow_err!(send_data(conn_id, request));
        self.progress.last_sent_time = Instant::now();

        Ok(())
    }

    fn handle_file_contents_response(
        &mut self,
        file_contents: FileContentsResponse,
    ) -> Result<(), CliprdrError> {
        if let Some(file) = self.progress.file_handle.as_mut() {
            let data = file_contents.requested_data.as_slice();
            let mut write_len = 0;
            while write_len < data.len() {
                match file.write(&data[write_len..]) {
                    Ok(len) => {
                        write_len += len;
                    }
                    Err(e) => {
                        return Err(CliprdrError::FileError {
                            path: self.progress.download_file_path.clone(),
                            err: e,
                        });
                    }
                }
            }
            self.update_next(write_len as _)?;
        } else {
            return Err(CliprdrError::FileError {
                path: self.progress.download_file_path.clone(),
                err: std::io::Error::new(std::io::ErrorKind::NotFound, "file handle is not opened"),
            });
        }
        Ok(())
    }
}
