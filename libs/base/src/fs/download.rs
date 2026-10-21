use super::{
    get_string, is_write_need_confirmation, join_validated_path, validate_file_name_no_traversal,
    validate_no_symlink_components, DataSource, DataStream, DigestCheckResult, TransferJob,
};
use crate::message_proto::{
    file_transfer_send_confirm_request, FileEntry, FileTransferBlock, FileTransferDigest,
    FileTransferSendConfirmRequest, FileType,
};
use hbb_common::{anyhow::anyhow, bail, compress::decompress, ResultType};
use std::{
    borrow::Cow,
    io::ErrorKind,
    path::{Path, PathBuf},
};
use tokio::{
    fs::{self, OpenOptions},
    io::{AsyncSeekExt, AsyncWriteExt},
};

const SINGLE_LINK_COUNT: u64 = 1;

#[derive(Default, Debug)]
pub(super) struct DownloadState {
    pub(super) confirmation: Option<(i32, u32)>,
}

pub fn validate_listing(entries: &[FileEntry], peer_windows: bool) -> ResultType<()> {
    for entry in entries {
        let name = &entry.name;
        if entry.entry_type.enum_value() == Ok(FileType::DirDrive)
            && matches!(name.as_bytes(), [drive, b':'] if drive.is_ascii_alphabetic())
        {
            continue;
        }
        if name.is_empty()
            || name == "."
            || name == ".."
            || name.contains('\0')
            || name.contains('/')
            || (peer_windows
                && (name.contains('\\')
                    || matches!(name.as_bytes(), [drive, b':', ..] if drive.is_ascii_alphabetic())))
        {
            bail!("invalid file name in remote directory listing: {name}");
        }
    }
    Ok(())
}

pub fn validate_download_name(path: &str, peer_windows: bool) -> ResultType<()> {
    let separator = |c| c == '/' || (peer_windows && c == '\\');
    let path = path.trim_end_matches(separator);
    validate_component(path.rsplit(separator).next().unwrap_or(""))
}

fn validate_component(name: &str) -> ResultType<()> {
    validate_file_name_no_traversal(name)?;
    if name.is_empty() || name == "." || name.contains('/') {
        bail!("invalid download file name: {name}");
    }
    #[cfg(windows)]
    if name.contains(['\\', ':', '<', '>', '"', '|', '?', '*'])
        || name.ends_with('.')
        || name.ends_with(' ')
    {
        bail!("invalid Windows download file name: {name}");
    }
    Ok(())
}

pub(super) fn validated_target(base: &PathBuf, name: &str) -> ResultType<PathBuf> {
    let parent = base
        .parent()
        .ok_or_else(|| anyhow!("download destination has no parent"))?;
    let leaf = base
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow!("invalid download destination"))?;
    validate_component(leaf)?;
    #[cfg(windows)]
    for component in name.split(['/', '\\']).filter(|part| !part.is_empty()) {
        validate_component(component)?;
    }
    validate_no_symlink_components(&parent.to_path_buf(), leaf)?;
    let target = join_validated_path(base, name)?;
    let parent = target
        .parent()
        .ok_or_else(|| anyhow!("download file has no parent"))?;
    let leaf = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow!("invalid download file name"))?;
    for suffix in ["download", "digest"] {
        validate_no_symlink_components(&parent.to_path_buf(), &format!("{leaf}.{suffix}"))?;
    }
    Ok(target)
}

fn remove_if_exists(path: &Path) -> ResultType<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

async fn remove_if_exists_async(path: &Path) -> ResultType<()> {
    match fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

async fn validate_resume_file(file: &tokio::fs::File) -> ResultType<()> {
    let metadata = file.metadata().await?;
    if !metadata.is_file() {
        bail!("download resume staging path is not a regular file");
    }
    #[cfg(unix)]
    let links = std::os::unix::fs::MetadataExt::nlink(&metadata);
    #[cfg(windows)]
    let links = {
        use std::{mem::MaybeUninit, os::windows::io::AsRawHandle};
        use winapi::{
            shared::minwindef::FALSE,
            um::fileapi::{GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION},
        };
        let file = file.try_clone().await?;
        tokio::task::spawn_blocking(move || -> ResultType<u64> {
            let mut information = MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
            if unsafe {
                GetFileInformationByHandle(file.as_raw_handle() as _, information.as_mut_ptr())
            } == FALSE
            {
                return Err(std::io::Error::last_os_error().into());
            }
            Ok(u64::from(
                unsafe { information.assume_init() }.nNumberOfLinks,
            ))
        })
        .await??
    };
    if links != SINGLE_LINK_COUNT {
        bail!("hard-linked download resume staging file is not allowed");
    }
    Ok(())
}

impl TransferJob {
    pub(super) fn is_download(&self) -> bool {
        self.download.is_some()
    }

    pub fn download_target(&self, file_num: i32) -> ResultType<PathBuf> {
        let entry = self
            .files
            .get(file_num as usize)
            .ok_or_else(|| anyhow!("Wrong file number"))?;
        let DataSource::FilePath(base) = &self.data_source else {
            bail!("download destination is not a file path");
        };
        validated_target(base, &entry.name)
    }

    pub fn download_digest(
        &mut self,
        can_resume: bool,
        digest: &FileTransferDigest,
    ) -> ResultType<DigestCheckResult> {
        if self.file_num != digest.file_num && self.data_stream.is_some() {
            let result = self.finish_download();
            self.data_stream.take();
            result?;
        }
        let target = self.download_target(digest.file_num)?;
        let resume = can_resume && self.is_resume && !target.try_exists()?;
        let result = is_write_need_confirmation(resume, &get_string(&target), digest)?;
        if matches!(result, DigestCheckResult::NoSuchFile)
            && (Path::new(&format!("{}.download", target.display())).try_exists()?
                || Path::new(&format!("{}.digest", target.display())).try_exists()?)
        {
            return Ok(DigestCheckResult::NeedConfirm(FileTransferDigest {
                id: digest.id,
                file_num: digest.file_num,
                ..Default::default()
            }));
        }
        Ok(result)
    }

    pub(super) fn confirm_download(&mut self, request: &FileTransferSendConfirmRequest) {
        if request.id != self.id {
            return;
        }
        let Some(download) = self.download.as_mut() else {
            return;
        };
        download.confirmation = self.files.get(request.file_num as usize).and_then(|_| {
            let offset = match request.union {
                Some(file_transfer_send_confirm_request::Union::OffsetBlk(offset)) => offset,
                Some(file_transfer_send_confirm_request::Union::Skip(false)) => 0,
                _ => return None,
            };
            Some((request.file_num, offset))
        });
    }

    fn download_confirmation_offset(&self, file_num: i32) -> Option<u32> {
        let (confirmed, offset) = self.download.as_ref()?.confirmation.as_ref()?;
        (*confirmed == file_num).then_some(*offset)
    }

    async fn start_download_file(&mut self, file_num: i32) -> ResultType<()> {
        let result = self.finish_download();
        self.data_stream.take();
        result?;
        let target = self.download_target(file_num)?;
        let staging = PathBuf::from(format!("{}.download", target.display()));
        let digest = PathBuf::from(format!("{}.digest", target.display()));
        let confirmation = self.download_confirmation_offset(file_num);
        if (fs::try_exists(&target).await?
            || fs::try_exists(&staging).await?
            || fs::try_exists(&digest).await?)
            && confirmation.is_none()
        {
            bail!("download overwrite requires confirmation");
        }
        if let Some(offset) = confirmation.filter(|offset| *offset > 0) {
            return self.resume_download(file_num, offset).await;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).await?;
        }
        remove_if_exists_async(&staging).await?;
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)
            .await?;
        self.file_num = file_num;
        self.data_stream = Some(DataStream::FileStream(file));
        remove_if_exists_async(&digest).await?;
        let mut metadata = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&digest)
            .await?;
        metadata
            .write_all(serde_json::to_string(&self.digest)?.as_bytes())
            .await?;
        metadata.flush().await?;
        Ok(())
    }

    async fn resume_download(&mut self, file_num: i32, offset: u32) -> ResultType<()> {
        let target = self.download_target(file_num)?;
        if fs::try_exists(&target).await? {
            bail!("download overwrite requires confirmation");
        }
        let digest = fs::metadata(format!("{}.digest", target.display())).await?;
        if !digest.is_file() {
            bail!("download resume metadata is not a regular file");
        }
        let mut file = OpenOptions::new()
            .write(true)
            .open(format!("{}.download", target.display()))
            .await?;
        validate_resume_file(&file).await?;
        if file.metadata().await?.len() != u64::from(offset) {
            bail!("download resume offset does not match staging file size");
        }
        file.seek(std::io::SeekFrom::Start(u64::from(offset)))
            .await?;
        self.file_num = file_num;
        self.data_stream = Some(DataStream::FileStream(file));
        self.transferred += u64::from(offset);
        self.finished_size += u64::from(offset);
        Ok(())
    }

    pub(super) async fn write_download(&mut self, block: FileTransferBlock) -> ResultType<()> {
        if block.id != self.id {
            bail!("Wrong id");
        }
        if block.file_num as usize >= self.files.len() {
            bail!("Wrong file number");
        }
        if block.file_num != self.file_num || self.data_stream.is_none() {
            self.start_download_file(block.file_num).await?;
        }
        let data = if block.compressed {
            Cow::Owned(decompress(&block.data))
        } else {
            Cow::Borrowed(block.data.as_ref())
        };
        let Some(DataStream::FileStream(stream)) = self.data_stream.as_mut() else {
            bail!("download file stream is missing");
        };
        stream.write_all(&data).await?;
        stream.flush().await?;
        self.finished_size += data.len() as u64;
        self.transferred += block.data.len() as u64;
        Ok(())
    }

    pub fn finish_download(&self) -> ResultType<()> {
        if !self.is_download() {
            self.modify_time();
            return Ok(());
        }
        if self.data_stream.is_none() {
            return Ok(());
        }
        let target = self.download_target(self.file_num)?;
        if target.try_exists()? && self.download_confirmation_offset(self.file_num) != Some(0) {
            bail!("download overwrite requires confirmation");
        }
        std::fs::rename(format!("{}.download", target.display()), &target)?;
        remove_if_exists(&PathBuf::from(format!("{}.digest", target.display())))?;
        let entry = self
            .files
            .get(self.file_num as usize)
            .ok_or_else(|| anyhow!("Wrong file number"))?;
        filetime::set_file_mtime(
            &target,
            filetime::FileTime::from_unix_time(entry.modified_time as _, 0),
        )?;
        Ok(())
    }
}
