use super::{
    item_data_provider::create_pasteboard_file_url_provider,
    paste_observer::PasteObserver,
    paste_task::{FileContentsResponse, PasteTask},
};
use crate::{
    platform::unix::{
        filetype::FileDescription, FILECONTENTS_FORMAT_NAME, FILEDESCRIPTORW_FORMAT_NAME,
    },
    send_data, ClipboardFile, CliprdrError, CliprdrServiceContext, ProgressPercent,
};
use hbb_common::{allow_err, bail, log, ResultType};
use objc2::{msg_send_id, rc::autoreleasepool, rc::Id, runtime::ProtocolObject, ClassType};
use objc2_app_kit::{NSPasteboard, NSPasteboardTypeFileURL};
use objc2_foundation::{NSArray, NSString};
use std::{
    io,
    path::Path,
    sync::{
        mpsc::{channel, Receiver, RecvTimeoutError, Sender},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

lazy_static::lazy_static! {
    static ref PASTE_OBSERVER_INFO: Arc<Mutex<Option<PasteObserverInfo>>> = Default::default();
}

pub const TEMP_FILE_PREFIX: &str = ".rustdesk_";

#[derive(Default, Debug, Clone, PartialEq)]
pub(super) struct PasteObserverInfo {
    pub file_descriptor_id: i32,
    pub conn_id: i32,
    pub source_path: String,
    pub target_path: String,
}

impl PasteObserverInfo {
    fn exit_msg() -> Self {
        Self::default()
    }
}

struct ContextInfo {
    tx: Sender<io::Result<PasteObserverInfo>>,
    handle: thread::JoinHandle<()>,
}

pub struct PasteboardContext {
    pasteboard: Id<NSPasteboard>,
    observer: Arc<Mutex<PasteObserver>>,
    tx_handle: Option<ContextInfo>,
    tx_remove_file: Option<Sender<String>>,
    remove_file_handle: Option<thread::JoinHandle<()>>,
    tx_paste_task: Sender<FileContentsResponse>,
    paste_task: Arc<Mutex<PasteTask>>,
}

unsafe impl Send for PasteboardContext {}
unsafe impl Sync for PasteboardContext {}

impl Drop for PasteboardContext {
    fn drop(&mut self) {
        self.observer.lock().unwrap().stop();
        if let Some(tx_handle) = self.tx_handle.take() {
            if tx_handle.tx.send(Ok(PasteObserverInfo::exit_msg())).is_ok() {
                tx_handle.handle.join().ok();
            }
        }
    }
}

impl CliprdrServiceContext for PasteboardContext {
    fn set_is_stopped(&mut self) -> Result<(), CliprdrError> {
        Ok(())
    }

    fn empty_clipboard(&mut self, conn_id: i32) -> Result<bool, CliprdrError> {
        Ok(self.empty_clipboard_(conn_id))
    }

    fn server_clip_file(&mut self, conn_id: i32, msg: ClipboardFile) -> Result<(), CliprdrError> {
        self.server_clip_file_(conn_id, msg)
    }

    fn get_progress_percent(&self) -> Option<ProgressPercent> {
        self.paste_task.lock().unwrap().progress_percent()
    }

    fn cancel(&mut self) {
        self.paste_task.lock().unwrap().cancel();
    }
}

impl PasteboardContext {
    fn init(&mut self) {
        let (tx_remove_file, rx_remove_file) = channel();
        let handle_remove_file = Self::init_thread_remove_file(rx_remove_file);
        self.tx_remove_file = Some(tx_remove_file.clone());
        self.remove_file_handle = Some(handle_remove_file);

        let (tx, rx) = channel();
        let observer: Arc<Mutex<PasteObserver>> = self.observer.clone();
        let handle = Self::init_thread_observer(tx_remove_file, rx, observer);
        self.tx_handle = Some(ContextInfo { tx, handle });
    }

    fn init_thread_observer(
        tx_remove_file: Sender<String>,
        rx: Receiver<io::Result<PasteObserverInfo>>,
        observer: Arc<Mutex<PasteObserver>>,
    ) -> thread::JoinHandle<()> {
        let exit_msg = PasteObserverInfo::exit_msg();
        thread::spawn(move || loop {
            match rx.recv() {
                Ok(Ok(task_info)) => {
                    if task_info == exit_msg {
                        log::debug!("pasteboard item data provider: exit");
                        break;
                    }
                    tx_remove_file.send(task_info.source_path.clone()).ok();
                    observer.lock().unwrap().start(task_info);
                }
                Ok(Err(e)) => {
                    log::error!("pasteboard item data provider, inner error: {e}");
                }
                Err(e) => {
                    log::error!("pasteboard item data provider, error: {e}");
                    break;
                }
            }
        })
    }

    fn init_thread_remove_file(rx: Receiver<String>) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            let mut cur_file: Option<String> = None;
            loop {
                match rx.recv_timeout(Duration::from_secs(30)) {
                    Ok(path) => {
                        if let Some(file) = cur_file.take() {
                            if !file.is_empty() {
                                std::fs::remove_file(&file).ok();
                            }
                        }
                        if !path.is_empty() {
                            cur_file = Some(path);
                        }
                    }
                    Err(e) => {
                        if let Some(file) = cur_file.take() {
                            if !file.is_empty() {
                                std::fs::remove_file(&file).ok();
                            }
                        }
                        if e == RecvTimeoutError::Disconnected {
                            break;
                        }
                    }
                }
            }
        })
    }

    // Just removing the file can also make paste option in the context menu disappear.
    fn empty_clipboard_(&mut self, _conn_id: i32) -> bool {
        self.tx_remove_file
            .as_ref()
            .map(|tx| tx.send("".to_string()).ok());
        true
    }

    fn temp_files_count() -> usize {
        let mut count = 0;
        if let Ok(entries) = std::fs::read_dir("/tmp") {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.is_file() {
                        if let Some(file_name) = path.file_name() {
                            if let Some(file_name_str) = file_name.to_str() {
                                if file_name_str.starts_with(TEMP_FILE_PREFIX) {
                                    count += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        count
    }

    fn server_clip_file_(&mut self, conn_id: i32, msg: ClipboardFile) -> Result<(), CliprdrError> {
        match msg {
            ClipboardFile::FormatList { format_list } => {
                let temp_files = Self::temp_files_count();
                if temp_files >= 3 {
                    // The temp files should be 0 or 1 in normal case.
                    // We should not continue to paste files if there are more than 3 temp files.
                    return Err(CliprdrError::CommonError {
                        description: format!(
                            "too many temp files, current: {}, limit: {}",
                            temp_files, 3
                        ),
                    });
                }

                let task_lock = self.paste_task.lock().unwrap();
                if !task_lock.is_finished() {
                    return Err(CliprdrError::CommonError {
                        description: "previous file paste task is not finished".to_string(),
                    });
                }
                self.handle_format_list(conn_id, format_list)?;
            }
            ClipboardFile::FormatDataResponse {
                msg_flags,
                format_data,
            } => {
                self.handle_format_data_response(conn_id, msg_flags, format_data)?;
            }
            ClipboardFile::FileContentsResponse {
                msg_flags,
                stream_id,
                requested_data,
            } => {
                self.handle_file_contents_response(conn_id, msg_flags, stream_id, requested_data)?;
            }
            ClipboardFile::TryEmpty => self.handle_try_empty(conn_id),
            _ => {}
        }
        Ok(())
    }

    fn handle_format_list(
        &self,
        conn_id: i32,
        format_list: Vec<(i32, String)>,
    ) -> Result<(), CliprdrError> {
        if let Some(tx_handle) = self.tx_handle.as_ref() {
            if !format_list
                .iter()
                .find(|(_, name)| name == FILECONTENTS_FORMAT_NAME)
                .map(|(id, _)| *id)
                .is_some()
            {
                return Err(CliprdrError::CommonError {
                    description: "no file contents format found".to_string(),
                });
            };
            let Some(file_descriptor_id) = format_list
                .iter()
                .find(|(_, name)| name == FILEDESCRIPTORW_FORMAT_NAME)
                .map(|(id, _)| *id)
            else {
                return Err(CliprdrError::CommonError {
                    description: "no file descriptor format found".to_string(),
                });
            };

            autoreleasepool(|_| self.set_clipboard_item(tx_handle, conn_id, file_descriptor_id))?;
        } else {
            return Err(CliprdrError::CommonError {
                description: "pasteboard context is not inited".to_string(),
            });
        }
        Ok(())
    }

    fn set_clipboard_item(
        &self,
        tx_handle: &ContextInfo,
        conn_id: i32,
        file_descriptor_id: i32,
    ) -> Result<(), CliprdrError> {
        let tx = tx_handle.tx.clone();
        let provider = create_pasteboard_file_url_provider(
            PasteObserverInfo {
                file_descriptor_id,
                conn_id,
                source_path: "".to_string(),
                target_path: "".to_string(),
            },
            tx,
        );
        unsafe {
            let types = NSArray::from_vec(vec![NSString::from_str(
                &NSPasteboardTypeFileURL.to_string(),
            )]);
            let item = objc2_app_kit::NSPasteboardItem::new();
            item.setDataProvider_forTypes(&ProtocolObject::from_id(provider), &types);
            self.pasteboard.clearContents();
            if !self
                .pasteboard
                .writeObjects(&Id::cast(NSArray::from_vec(vec![item])))
            {
                return Err(CliprdrError::CommonError {
                    description: "failed to write objects".to_string(),
                });
            }
        }
        Ok(())
    }

    fn handle_format_data_response(
        &self,
        conn_id: i32,
        msg_flags: i32,
        format_data: Vec<u8>,
    ) -> Result<(), CliprdrError> {
        log::debug!("handle format data response, msg_flags: {msg_flags}");
        if msg_flags != 0x1 {
            // return failure message?
        }

        let mut task_lock = self.paste_task.lock().unwrap();
        let target_dir = PASTE_OBSERVER_INFO
            .lock()
            .unwrap()
            .as_ref()
            .map(|task| task.target_path.clone());
        // unreachable in normal case
        let Some(target_dir) = target_dir.as_ref().map(|d| Path::new(d).parent()).flatten() else {
            return Err(CliprdrError::CommonError {
                description: "failed to get parent path".to_string(),
            });
        };
        // unreachable in normal case
        if !target_dir.exists() {
            return Err(CliprdrError::CommonError {
                description: "target path does not exist".to_string(),
            });
        }
        let target_dir = target_dir.to_owned();
        match FileDescription::parse_file_descriptors(format_data, conn_id) {
            Ok(files) => {
                task_lock.start(target_dir, files);
                Ok(())
            }
            Err(e) => {
                PASTE_OBSERVER_INFO
                    .lock()
                    .unwrap()
                    .replace(PasteObserverInfo::default());
                Err(e)
            }
        }
    }

    fn handle_file_contents_response(
        &self,
        conn_id: i32,
        msg_flags: i32,
        stream_id: i32,
        requested_data: Vec<u8>,
    ) -> Result<(), CliprdrError> {
        log::debug!("handle file contents response");
        self.tx_paste_task
            .send(FileContentsResponse {
                conn_id,
                msg_flags,
                stream_id,
                requested_data,
            })
            .ok();
        Ok(())
    }

    fn handle_try_empty(&mut self, conn_id: i32) {
        log::debug!("empty_clipboard called");
        let ret = self.empty_clipboard_(conn_id);
        log::debug!(
            "empty_clipboard called, conn_id {}, return {}",
            conn_id,
            ret
        );
    }
}

fn handle_paste_result(task_info: &PasteObserverInfo) {
    log::info!(
        "file {} is pasted to {}",
        &task_info.source_path,
        &task_info.target_path
    );
    if Path::new(&task_info.target_path).parent().is_none() {
        log::error!(
            "failed to get parent path of {}, no need to perform pasting",
            &task_info.target_path
        );
        return;
    }

    PASTE_OBSERVER_INFO
        .lock()
        .unwrap()
        .replace(task_info.clone());
    // to-do: add a timeout to clear data in `PASTE_OBSERVER_INFO`.
    std::fs::remove_file(&task_info.source_path).ok();
    std::fs::remove_file(&task_info.target_path).ok();
    let data = ClipboardFile::FormatDataRequest {
        requested_format_id: task_info.file_descriptor_id,
    };
    allow_err!(send_data(task_info.conn_id as _, data));
}

#[inline]
pub fn create_pasteboard_context() -> ResultType<Box<PasteboardContext>> {
    let pasteboard: Option<Id<NSPasteboard>> =
        unsafe { msg_send_id![NSPasteboard::class(), generalPasteboard] };
    let Some(pasteboard) = pasteboard else {
        bail!("failed to get general pasteboard");
    };
    let mut observer = PasteObserver::new();
    observer.init(handle_paste_result)?;
    let (tx, rx) = channel();
    let mut context = Box::new(PasteboardContext {
        pasteboard,
        observer: Arc::new(Mutex::new(observer)),
        tx_handle: None,
        tx_remove_file: None,
        remove_file_handle: None,
        tx_paste_task: tx,
        paste_task: Arc::new(Mutex::new(PasteTask::new(rx))),
    });
    context.init();
    Ok(context)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_temp_files_count() {
        let mut c = super::PasteboardContext::temp_files_count();

        let mut created_files = vec![];
        for _ in 0..10 {
            let path = format!(
                "/tmp/{}{}",
                super::TEMP_FILE_PREFIX,
                uuid::Uuid::new_v4().to_string()
            );
            if std::fs::File::create(&path).is_ok() {
                created_files.push(path);
                c += 1;
            }
        }

        assert_eq!(c, super::PasteboardContext::temp_files_count());

        // Clean up the created files.
        for file in created_files {
            std::fs::remove_file(&file).ok();
        }
    }
}
