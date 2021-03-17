use crate::{bail, message_proto::*, ResultType};
use std::path::{Path, PathBuf};
// https://doc.rust-lang.org/std/os/windows/fs/trait.MetadataExt.html
use crate::{
    compress::{compress, decompress},
    config::{Config, COMPRESS_LEVEL},
};
#[cfg(windows)]
use std::os::windows::prelude::*;
use tokio::{fs::File, prelude::*};

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

#[derive(Default)]
pub struct TransferJob {
    id: i32,
    path: PathBuf,
    files: Vec<FileEntry>,
    file_num: i32,
    file: Option<File>,
    total_size: u64,
    finished_size: u64,
    transfered: u64,
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
    pub fn new_write(id: i32, path: String, files: Vec<FileEntry>) -> Self {
        let total_size = files.iter().map(|x| x.size as u64).sum();
        Self {
            id,
            path: get_path(&path),
            files,
            total_size,
            ..Default::default()
        }
    }

    pub fn new_read(id: i32, path: String, include_hidden: bool) -> ResultType<Self> {
        let files = get_recursive_files(&path, include_hidden)?;
        let total_size = files.iter().map(|x| x.size as u64).sum();
        Ok(Self {
            id,
            path: get_path(&path),
            files,
            total_size,
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
    pub fn transfered(&self) -> u64 {
        self.transfered
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
        self.transfered += block.data.len() as u64;
        Ok(())
    }

    #[inline]
    fn join(&self, name: &str) -> PathBuf {
        if name.is_empty() {
            self.path.clone()
        } else {
            self.path.join(name)
        }
    }

    pub async fn read(&mut self) -> ResultType<Option<FileTransferBlock>> {
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
                }
                Err(err) => {
                    self.file_num += 1;
                    return Err(err.into());
                }
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
        } else {
            self.finished_size += offset as u64;
            if !is_compressed_file(name) {
                let tmp = compress(&buf, COMPRESS_LEVEL);
                if tmp.len() < buf.len() {
                    buf = tmp;
                    compressed = true;
                }
            }
            self.transfered += buf.len() as u64;
        }
        Ok(Some(FileTransferBlock {
            id: self.id,
            file_num: file_num as _,
            data: buf.into(),
            compressed,
            ..Default::default()
        }))
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
pub fn new_dir(id: i32, files: Vec<FileEntry>) -> Message {
    let mut resp = FileResponse::new();
    resp.set_dir(FileDirectory {
        id,
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
pub fn new_receive(id: i32, path: String, files: Vec<FileEntry>) -> Message {
    let mut action = FileAction::new();
    action.set_receive(FileTransferReceiveRequest {
        id,
        path,
        files: files.into(),
        ..Default::default()
    });
    let mut msg_out = Message::new();
    msg_out.set_file_action(action);
    msg_out
}

#[inline]
pub fn new_send(id: i32, path: String, include_hidden: bool) -> Message {
    let mut action = FileAction::new();
    action.set_send(FileTransferSendRequest {
        id,
        path,
        include_hidden,
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
        match job.read().await {
            Err(err) => {
                stream
                    .send(&new_error(job.id(), err, job.file_num()))
                    .await?;
            }
            Ok(Some(block)) => {
                stream.send(&new_block(block)).await?;
            }
            Ok(None) => {
                finished.push(job.id());
                stream.send(&new_done(job.id(), job.file_num())).await?;
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
