#[cfg(windows)]
use std::os::windows::prelude::*;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_derive::{Deserialize, Serialize};
use tokio::{fs::File, io::*};

use crate::{bail, get_version_number, message_proto::*, ResultType, Stream};
// https://doc.rust-lang.org/std/os/windows/fs/trait.MetadataExt.html
use crate::{
    compress::{compress, decompress},
    config::{Config, COMPRESS_LEVEL},
};

pub fn read_dir(path: &PathBuf, include_hidden: bool) -> ResultType<FileDirectory> {
    let mut dir = FileDirectory {
        path: get_string(&path),
        ..Default::default()
    };
    #[cfg(windows)]
    if "/" == &get_string(&path) {
        let drives = unsafe { winapi::um::fileapi::GetLogicalDrives() };
        for i in 0..32 {
            if drives & (1 << i) != 0 {
                let name = format!(
                    "{}:",
                    std::char::from_u32('A' as u32 + i as u32).unwrap_or('A')
                );
                dir.entries.push(FileEntry {
                    name,
                    entry_type: FileType::DirDrive.into(),
                    ..Default::default()
                });
            }
        }
        return Ok(dir);
    }
    for entry in path.read_dir()? {
        if let Ok(entry) = entry {
            let p = entry.path();
            let name = p
                .file_name()
                .map(|p| p.to_str().unwrap_or(""))
                .unwrap_or("")
                .to_owned();
            if name.is_empty() {
                continue;
            }
            let mut is_hidden = false;
            let meta;
            if let Ok(tmp) = std::fs::symlink_metadata(&p) {
                meta = tmp;
            } else {
                continue;
            }
            // docs.microsoft.com/en-us/windows/win32/fileio/file-attribute-constants
            #[cfg(windows)]
            if meta.file_attributes() & 0x2 != 0 {
                is_hidden = true;
            }
            #[cfg(not(windows))]
            if name.find('.').unwrap_or(usize::MAX) == 0 {
                is_hidden = true;
            }
            if is_hidden && !include_hidden {
                continue;
            }
            let (entry_type, size) = {
                if p.is_dir() {
                    if meta.file_type().is_symlink() {
                        (FileType::DirLink.into(), 0)
                    } else {
                        (FileType::Dir.into(), 0)
                    }
                } else {
                    if meta.file_type().is_symlink() {
                        (FileType::FileLink.into(), 0)
                    } else {
                        (FileType::File.into(), meta.len())
                    }
                }
            };
            let modified_time = meta
                .modified()
                .map(|x| {
                    x.duration_since(std::time::SystemTime::UNIX_EPOCH)
                        .map(|x| x.as_secs())
                        .unwrap_or(0)
                })
                .unwrap_or(0) as u64;
            dir.entries.push(FileEntry {
                name: get_file_name(&p),
                entry_type,
                is_hidden,
                size,
                modified_time,
                ..Default::default()
            });
        }
    }
    Ok(dir)
}

#[inline]
pub fn get_file_name(p: &PathBuf) -> String {
    p.file_name()
        .map(|p| p.to_str().unwrap_or(""))
        .unwrap_or("")
        .to_owned()
}

#[inline]
pub fn get_string(path: &PathBuf) -> String {
    path.to_str().unwrap_or("").to_owned()
}

#[inline]
pub fn get_path(path: &str) -> PathBuf {
    Path::new(path).to_path_buf()
}

#[inline]
pub fn get_home_as_string() -> String {
    get_string(&Config::get_home())
}

fn read_dir_recursive(
    path: &PathBuf,
    prefix: &PathBuf,
    include_hidden: bool,
) -> ResultType<Vec<FileEntry>> {
    let mut files = Vec::new();
    if path.is_dir() {
        // to-do: symbol link handling, cp the link rather than the content
        // to-do: file mode, for unix
        let fd = read_dir(&path, include_hidden)?;
        for entry in fd.entries.iter() {
            match entry.entry_type.enum_value() {
                Ok(FileType::File) => {
                    let mut entry = entry.clone();
                    entry.name = get_string(&prefix.join(entry.name));
                    files.push(entry);
                }
                Ok(FileType::Dir) => {
                    if let Ok(mut tmp) = read_dir_recursive(
                        &path.join(&entry.name),
                        &prefix.join(&entry.name),
                        include_hidden,
                    ) {
                        for entry in tmp.drain(0..) {
                            files.push(entry);
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(files)
    } else if path.is_file() {
        let (size, modified_time) = if let Ok(meta) = std::fs::metadata(&path) {
            (
                meta.len(),
                meta.modified()
                    .map(|x| {
                        x.duration_since(std::time::SystemTime::UNIX_EPOCH)
                            .map(|x| x.as_secs())
                            .unwrap_or(0)
                    })
                    .unwrap_or(0) as u64,
            )
        } else {
            (0, 0)
        };
        files.push(FileEntry {
            entry_type: FileType::File.into(),
            size,
            modified_time,
            ..Default::default()
        });
        Ok(files)
    } else {
        bail!("Not exists");
    }
}

pub fn get_recursive_files(path: &str, include_hidden: bool) -> ResultType<Vec<FileEntry>> {
    read_dir_recursive(&get_path(path), &get_path(""), include_hidden)
}

#[inline]
pub fn is_file_exists(file_path: &str) -> bool {
    return Path::new(file_path).exists();
}

#[inline]
pub fn can_enable_overwrite_detection(version: i64) -> bool {
    version >= get_version_number("1.1.10")
}

#[derive(Default)]
pub struct TransferJob {
    pub id: i32,
    pub remote: String,
    pub path: PathBuf,
    pub show_hidden: bool,
    pub is_remote: bool,
    pub is_last_job: bool,
    pub file_num: i32,
    pub files: Vec<FileEntry>,

    file: Option<File>,
    total_size: u64,
    finished_size: u64,
    transferred: u64,
    enable_overwrite_detection: bool,
    file_confirmed: bool,
    // indicating the last file is skipped
    file_skipped: bool,
    file_is_waiting: bool,
    default_overwrite_strategy: Option<bool>,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct TransferJobMeta {
    #[serde(default)]
    pub id: i32,
    #[serde(default)]
    pub remote: String,
    #[serde(default)]
    pub to: String,
    #[serde(default)]
    pub show_hidden: bool,
    #[serde(default)]
    pub file_num: i32,
    #[serde(default)]
    pub is_remote: bool,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct RemoveJobMeta {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub is_remote: bool,
    #[serde(default)]
    pub no_confirm: bool,
}

#[inline]
fn get_ext(name: &str) -> &str {
    if let Some(i) = name.rfind(".") {
        return &name[i + 1..];
    }
    ""
}

#[inline]
fn is_compressed_file(name: &str) -> bool {
    let ext = get_ext(name);
    ext == "xz"
        || ext == "gz"
        || ext == "zip"
        || ext == "7z"
        || ext == "rar"
        || ext == "bz2"
        || ext == "tgz"
        || ext == "png"
        || ext == "jpg"
}

impl TransferJob {
    pub fn new_write(
        id: i32,
        remote: String,
        path: String,
        file_num: i32,
        show_hidden: bool,
        is_remote: bool,
        files: Vec<FileEntry>,
        enable_overwrite_detection: bool,
    ) -> Self {
        log::info!("new write {}", path);
        let total_size = files.iter().map(|x| x.size as u64).sum();
        Self {
            id,
            remote,
            path: get_path(&path),
            file_num,
            show_hidden,
            is_remote,
            files,
            total_size,
            enable_overwrite_detection,
            ..Default::default()
        }
    }

    pub fn new_read(
        id: i32,
        remote: String,
        path: String,
        file_num: i32,
        show_hidden: bool,
        is_remote: bool,
        enable_overwrite_detection: bool,
    ) -> ResultType<Self> {
        log::info!("new read {}", path);
        let files = get_recursive_files(&path, show_hidden)?;
        let total_size = files.iter().map(|x| x.size as u64).sum();
        Ok(Self {
            id,
            remote,
            path: get_path(&path),
            file_num,
            show_hidden,
            is_remote,
            files,
            total_size,
            enable_overwrite_detection,
            ..Default::default()
        })
    }

    #[inline]
    pub fn files(&self) -> &Vec<FileEntry> {
        &self.files
    }

    #[inline]
    pub fn set_files(&mut self, files: Vec<FileEntry>) {
        self.files = files;
    }

    #[inline]
    pub fn id(&self) -> i32 {
        self.id
    }

    #[inline]
    pub fn total_size(&self) -> u64 {
        self.total_size
    }

    #[inline]
    pub fn finished_size(&self) -> u64 {
        self.finished_size
    }

    #[inline]
    pub fn transferred(&self) -> u64 {
        self.transferred
    }

    #[inline]
    pub fn file_num(&self) -> i32 {
        self.file_num
    }

    pub fn modify_time(&self) {
        let file_num = self.file_num as usize;
        if file_num < self.files.len() {
            let entry = &self.files[file_num];
            let path = self.join(&entry.name);
            let download_path = format!("{}.download", get_string(&path));
            std::fs::rename(&download_path, &path).ok();
            filetime::set_file_mtime(
                &path,
                filetime::FileTime::from_unix_time(entry.modified_time as _, 0),
            )
            .ok();
        }
    }

    pub fn remove_download_file(&self) {
        let file_num = self.file_num as usize;
        if file_num < self.files.len() {
            let entry = &self.files[file_num];
            let path = self.join(&entry.name);
            let download_path = format!("{}.download", get_string(&path));
            std::fs::remove_file(&download_path).ok();
        }
    }

    pub async fn write(&mut self, block: FileTransferBlock) -> ResultType<()> {
        if block.id != self.id {
            bail!("Wrong id");
        }
        let file_num = block.file_num as usize;
        if file_num >= self.files.len() {
            bail!("Wrong file number");
        }
        if file_num != self.file_num as usize || self.file.is_none() {
            self.modify_time();
            if let Some(file) = self.file.as_mut() {
                file.sync_all().await?;
            }
            self.file_num = block.file_num;
            let entry = &self.files[file_num];
            let path = self.join(&entry.name);
            if let Some(p) = path.parent() {
                std::fs::create_dir_all(p).ok();
            }
            let path = format!("{}.download", get_string(&path));
            self.file = Some(File::create(&path).await?);
        }
        if block.compressed {
            let tmp = decompress(&block.data);
            self.file.as_mut().unwrap().write_all(&tmp).await?;
            self.finished_size += tmp.len() as u64;
        } else {
            self.file.as_mut().unwrap().write_all(&block.data).await?;
            self.finished_size += block.data.len() as u64;
        }
        self.transferred += block.data.len() as u64;
        Ok(())
    }

    #[inline]
    pub fn join(&self, name: &str) -> PathBuf {
        if name.is_empty() {
            self.path.clone()
        } else {
            self.path.join(name)
        }
    }

    pub async fn read(&mut self, stream: &mut Stream) -> ResultType<Option<FileTransferBlock>> {
        let file_num = self.file_num as usize;
        if file_num >= self.files.len() {
            self.file.take();
            return Ok(None);
        }
        let name = &self.files[file_num].name;
        if self.file.is_none() {
            match File::open(self.join(&name)).await {
                Ok(file) => {
                    self.file = Some(file);
                    self.file_confirmed = false;
                    self.file_is_waiting = false;
                }
                Err(err) => {
                    self.file_num += 1;
                    self.file_confirmed = false;
                    self.file_is_waiting = false;
                    return Err(err.into());
                }
            }
        }
        if self.enable_overwrite_detection {
            if !self.file_confirmed() {
                if !self.file_is_waiting() {
                    self.send_current_digest(stream).await?;
                    self.set_file_is_waiting(true);
                }
                return Ok(None);
            }
        }
        const BUF_SIZE: usize = 128 * 1024;
        let mut buf: Vec<u8> = Vec::with_capacity(BUF_SIZE);
        unsafe {
            buf.set_len(BUF_SIZE);
        }
        let mut compressed = false;
        let mut offset: usize = 0;
        loop {
            match self.file.as_mut().unwrap().read(&mut buf[offset..]).await {
                Err(err) => {
                    self.file_num += 1;
                    self.file = None;
                    self.file_confirmed = false;
                    self.file_is_waiting = false;
                    return Err(err.into());
                }
                Ok(n) => {
                    offset += n;
                    if n == 0 || offset == BUF_SIZE {
                        break;
                    }
                }
            }
        }
        unsafe { buf.set_len(offset) };
        if offset == 0 {
            self.file_num += 1;
            self.file = None;
            self.file_confirmed = false;
            self.file_is_waiting = false;
        } else {
            self.finished_size += offset as u64;
            if !is_compressed_file(name) {
                let tmp = compress(&buf, COMPRESS_LEVEL);
                if tmp.len() < buf.len() {
                    buf = tmp;
                    compressed = true;
                }
            }
            self.transferred += buf.len() as u64;
        }
        Ok(Some(FileTransferBlock {
            id: self.id,
            file_num: file_num as _,
            data: buf.into(),
            compressed,
            ..Default::default()
        }))
    }

    async fn send_current_digest(&mut self, stream: &mut Stream) -> ResultType<()> {
        let mut msg = Message::new();
        let mut resp = FileResponse::new();
        let meta = self.file.as_ref().unwrap().metadata().await?;
        let last_modified = meta
            .modified()?
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_secs();
        resp.set_digest(FileTransferDigest {
            id: self.id,
            file_num: self.file_num,
            last_modified,
            file_size: meta.len(),
            ..Default::default()
        });
        msg.set_file_response(resp);
        stream.send(&msg).await?;
        log::info!(
            "id: {}, file_num:{}, digest message is sent. waiting for confirm. msg: {:?}",
            self.id,
            self.file_num,
            msg
        );
        Ok(())
    }

    pub fn set_overwrite_strategy(&mut self, overwrite_strategy: Option<bool>) {
        self.default_overwrite_strategy = overwrite_strategy;
    }

    pub fn default_overwrite_strategy(&self) -> Option<bool> {
        self.default_overwrite_strategy
    }

    pub fn set_file_confirmed(&mut self, file_confirmed: bool) {
        log::info!("id: {}, file_confirmed: {}", self.id, file_confirmed);
        self.file_confirmed = file_confirmed;
        self.file_skipped = false;
    }

    pub fn set_file_is_waiting(&mut self, file_is_waiting: bool) {
        self.file_is_waiting = file_is_waiting;
    }

    #[inline]
    pub fn file_is_waiting(&self) -> bool {
        self.file_is_waiting
    }

    #[inline]
    pub fn file_confirmed(&self) -> bool {
        self.file_confirmed
    }

    /// Indicating whether the last file is skipped
    #[inline]
    pub fn file_skipped(&self) -> bool {
        self.file_skipped
    }

    /// Indicating whether the whole task is skipped
    #[inline]
    pub fn job_skipped(&self) -> bool {
        self.file_skipped() && self.files.len() == 1
    }

    /// Get job error message, useful for getting status when job had finished
    pub fn job_error(&self) -> Option<String> {
        if self.job_skipped() {
            return Some("skipped".to_string());
        }
        None
    }

    pub fn set_file_skipped(&mut self) -> bool {
        log::debug!("skip file {} in job {}", self.file_num, self.id);
        self.file.take();
        self.set_file_confirmed(false);
        self.set_file_is_waiting(false);
        self.file_num += 1;
        self.file_skipped = true;
        true
    }

    pub fn confirm(&mut self, r: &FileTransferSendConfirmRequest) -> bool {
        if self.file_num() != r.file_num {
            log::info!("file num truncated, ignoring");
        } else {
            match r.union {
                Some(file_transfer_send_confirm_request::Union::Skip(s)) => {
                    if s {
                        log::debug!("skip file id:{}, file_num:{}", r.id, r.file_num);
                        self.set_file_skipped();
                    } else {
                        self.set_file_confirmed(true);
                    }
                }
                Some(file_transfer_send_confirm_request::Union::OffsetBlk(_offset)) => {
                    self.set_file_confirmed(true);
                }
                _ => {}
            }
        }
        true
    }

    #[inline]
    pub fn gen_meta(&self) -> TransferJobMeta {
        TransferJobMeta {
            id: self.id,
            remote: self.remote.to_string(),
            to: self.path.to_string_lossy().to_string(),
            file_num: self.file_num,
            show_hidden: self.show_hidden,
            is_remote: self.is_remote,
        }
    }
}

#[inline]
pub fn new_error<T: std::string::ToString>(id: i32, err: T, file_num: i32) -> Message {
    let mut resp = FileResponse::new();
    resp.set_error(FileTransferError {
        id,
        error: err.to_string(),
        file_num,
        ..Default::default()
    });
    let mut msg_out = Message::new();
    msg_out.set_file_response(resp);
    msg_out
}

#[inline]
pub fn new_dir(id: i32, path: String, files: Vec<FileEntry>) -> Message {
    let mut resp = FileResponse::new();
    resp.set_dir(FileDirectory {
        id,
        path,
        entries: files.into(),
        ..Default::default()
    });
    let mut msg_out = Message::new();
    msg_out.set_file_response(resp);
    msg_out
}

#[inline]
pub fn new_block(block: FileTransferBlock) -> Message {
    let mut resp = FileResponse::new();
    resp.set_block(block);
    let mut msg_out = Message::new();
    msg_out.set_file_response(resp);
    msg_out
}

#[inline]
pub fn new_send_confirm(r: FileTransferSendConfirmRequest) -> Message {
    let mut msg_out = Message::new();
    let mut action = FileAction::new();
    action.set_send_confirm(r);
    msg_out.set_file_action(action);
    msg_out
}

#[inline]
pub fn new_receive(id: i32, path: String, file_num: i32, files: Vec<FileEntry>) -> Message {
    let mut action = FileAction::new();
    action.set_receive(FileTransferReceiveRequest {
        id,
        path,
        files: files.into(),
        file_num,
        ..Default::default()
    });
    let mut msg_out = Message::new();
    msg_out.set_file_action(action);
    msg_out
}

#[inline]
pub fn new_send(id: i32, path: String, file_num: i32, include_hidden: bool) -> Message {
    log::info!("new send: {},id : {}", path, id);
    let mut action = FileAction::new();
    action.set_send(FileTransferSendRequest {
        id,
        path,
        include_hidden,
        file_num,
        ..Default::default()
    });
    let mut msg_out = Message::new();
    msg_out.set_file_action(action);
    msg_out
}

#[inline]
pub fn new_done(id: i32, file_num: i32) -> Message {
    let mut resp = FileResponse::new();
    resp.set_done(FileTransferDone {
        id,
        file_num,
        ..Default::default()
    });
    let mut msg_out = Message::new();
    msg_out.set_file_response(resp);
    msg_out
}

#[inline]
pub fn remove_job(id: i32, jobs: &mut Vec<TransferJob>) {
    *jobs = jobs.drain(0..).filter(|x| x.id() != id).collect();
}

#[inline]
pub fn get_job(id: i32, jobs: &mut Vec<TransferJob>) -> Option<&mut TransferJob> {
    jobs.iter_mut().filter(|x| x.id() == id).next()
}

pub async fn handle_read_jobs(
    jobs: &mut Vec<TransferJob>,
    stream: &mut crate::Stream,
) -> ResultType<()> {
    let mut finished = Vec::new();
    for job in jobs.iter_mut() {
        if job.is_last_job {
            continue;
        }
        match job.read(stream).await {
            Err(err) => {
                stream
                    .send(&new_error(job.id(), err, job.file_num()))
                    .await?;
            }
            Ok(Some(block)) => {
                stream.send(&new_block(block)).await?;
            }
            Ok(None) => {
                if !job.enable_overwrite_detection || (!job.file_confirmed && !job.file_is_waiting)
                {
                    // for getting error detail, we do not remove this job, we will handle it in io loop
                    if job.job_error().is_none() {
                        finished.push(job.id());
                    }
                    stream.send(&new_done(job.id(), job.file_num())).await?;
                } else {
                    // waiting confirmation.
                }
            }
        }
    }
    for id in finished {
        remove_job(id, jobs);
    }
    Ok(())
}

pub fn remove_all_empty_dir(path: &PathBuf) -> ResultType<()> {
    let fd = read_dir(path, true)?;
    for entry in fd.entries.iter() {
        match entry.entry_type.enum_value() {
            Ok(FileType::Dir) => {
                remove_all_empty_dir(&path.join(&entry.name)).ok();
            }
            Ok(FileType::DirLink) | Ok(FileType::FileLink) => {
                std::fs::remove_file(&path.join(&entry.name)).ok();
            }
            _ => {}
        }
    }
    std::fs::remove_dir(path).ok();
    Ok(())
}

#[inline]
pub fn remove_file(file: &str) -> ResultType<()> {
    std::fs::remove_file(get_path(file))?;
    Ok(())
}

#[inline]
pub fn create_dir(dir: &str) -> ResultType<()> {
    std::fs::create_dir_all(get_path(dir))?;
    Ok(())
}

#[inline]
pub fn transform_windows_path(entries: &mut Vec<FileEntry>) {
    for entry in entries {
        entry.name = entry.name.replace("\\", "/");
    }
}

pub enum DigestCheckResult {
    IsSame,
    NeedConfirm(FileTransferDigest),
    NoSuchFile,
}

#[inline]
pub fn is_write_need_confirmation(
    file_path: &str,
    digest: &FileTransferDigest,
) -> ResultType<DigestCheckResult> {
    let path = Path::new(file_path);
    if path.exists() && path.is_file() {
        let metadata = std::fs::metadata(path)?;
        let modified_time = metadata.modified()?;
        let remote_mt = Duration::from_secs(digest.last_modified);
        let local_mt = modified_time.duration_since(UNIX_EPOCH)?;
        if remote_mt == local_mt && digest.file_size == metadata.len() {
            return Ok(DigestCheckResult::IsSame);
        }
        Ok(DigestCheckResult::NeedConfirm(FileTransferDigest {
            id: digest.id,
            file_num: digest.file_num,
            last_modified: local_mt.as_secs(),
            file_size: metadata.len(),
            ..Default::default()
        }))
    } else {
        Ok(DigestCheckResult::NoSuchFile)
    }
}
