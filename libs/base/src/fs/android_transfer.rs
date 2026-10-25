use super::{
    android, android_descriptor::DocumentDescriptor, file_transfer_send_confirm_request,
    DataSource, DataStream, FileTransferBlock, FileTransferSendConfirmRequest, TransferJob,
};
use hbb_common::{anyhow::anyhow, bail, compress::decompress, ResultType};
use serde_json::{json, Value};
use tokio::{
    fs::File,
    io::{AsyncSeekExt, AsyncWriteExt},
    task::spawn_blocking,
};

#[derive(Debug, Default)]
pub(super) struct DocumentTransfer {
    reader: Option<DocumentDescriptor>,
    pending: Option<PendingWrite>,
    confirmed_file: Option<i32>,
    pub error: Option<String>,
}

#[derive(Debug)]
struct PendingWrite {
    file: File,
    descriptor: DocumentDescriptor,
    token: Value,
    path: String,
}

impl TransferJob {
    pub fn is_document(&self) -> bool {
        matches!(&self.data_source, DataSource::FilePath(path) if android::is_path(path))
    }

    pub(super) fn confirm_document(&mut self, request: &FileTransferSendConfirmRequest) {
        if request.id != self.id {
            return;
        }
        let overwrite = matches!(
            request.union,
            Some(file_transfer_send_confirm_request::Union::OffsetBlk(0))
                | Some(file_transfer_send_confirm_request::Union::Skip(false))
        );
        self.document_transfer.confirmed_file = (overwrite
            && self.files.get(request.file_num as usize).is_some())
        .then_some(request.file_num);
    }

    fn document_path(&self, file_num: usize) -> ResultType<String> {
        let DataSource::FilePath(base) = &self.data_source else {
            bail!("Not a document transfer");
        };
        let entry = self
            .files
            .get(file_num)
            .ok_or(anyhow!("Wrong file number"))?;
        super::validate_file_name_no_traversal(&entry.name)?;
        Ok(Self::join(base, &entry.name).to_string_lossy().into_owned())
    }

    pub(super) async fn open_document_stream(&mut self) -> ResultType<bool> {
        if let Some(error) = &self.document_transfer.error {
            self.file_num = self.files.len() as i32;
            self.data_stream.take();
            bail!("{}", error);
        }
        if self.file_num as usize >= self.files.len() {
            self.data_stream.take();
            return Ok(true);
        }
        if self.data_stream.is_some() {
            return Ok(false);
        }
        let result = android::open(self.document_path(self.file_num as usize)?).await;
        self.file_confirmed = false;
        self.file_is_waiting = false;
        match result {
            Ok((file, descriptor, digest)) => {
                let entry = self
                    .files
                    .get_mut(self.file_num as usize)
                    .ok_or(anyhow!("Wrong file number"))?;
                entry.modified_time = digest.map_or(0, |(modified, _)| modified);
                entry.size = digest.map_or(entry.size, |(_, size)| size);
                self.data_stream = Some(DataStream::FileStream(file));
                self.document_transfer.reader = Some(descriptor);
            }
            Err(error) => {
                self.fail_document_read(error.to_string());
                return Err(error);
            }
        }
        Ok(false)
    }

    pub(super) async fn finish_document_read(&mut self) -> ResultType<()> {
        self.data_stream.take();
        let descriptor = self
            .document_transfer
            .reader
            .take()
            .ok_or(anyhow!("No open document reader"))?;
        let result = descriptor.close("").await;
        if let Err(error) = &result {
            self.fail_document_read(error.to_string());
        }
        result
    }

    pub(super) fn fail_document_read(&mut self, error: String) {
        self.document_transfer.error = Some(error);
        self.file_num = self.files.len() as i32;
        self.file_confirmed = false;
        self.file_is_waiting = false;
        self.document_transfer.reader.take();
    }

    pub(super) async fn write_document(&mut self, block: FileTransferBlock) -> ResultType<()> {
        if let Some(error) = &self.document_transfer.error {
            bail!("{}", error);
        }
        let result = self.write_document_block(&block).await;
        if let Err(error) = &result {
            self.document_transfer.error = Some(error.to_string());
            if let Err(cleanup_error) = self.cancel_document().await {
                const CLEANUP_ERROR_LOG_INTERVAL: std::time::Duration =
                    std::time::Duration::from_secs(60);
                hbb_common::throttled_log!(
                    CLEANUP_ERROR_LOG_INTERVAL,
                    warn,
                    "Failed to clean up document transfer {}: {}",
                    self.id,
                    cleanup_error
                );
            }
        }
        result
    }

    async fn write_document_block(&mut self, block: &FileTransferBlock) -> ResultType<()> {
        if self.is_resume {
            bail!("Document transfers must be restarted after interruption");
        }
        let path = self.document_path(block.file_num as usize)?;
        if self.document_transfer.pending.as_ref().map(|p| &p.path) != Some(&path) {
            self.finish_document().await?;
            // Upload senders handle confirmation when overwrite detection is supported.
            let overwrite = (!self.is_remote && self.enable_overwrite_detection)
                || self.document_transfer.confirmed_file.take() == Some(block.file_num);
            self.document_transfer.pending = Some(begin_write(path, overwrite).await?);
            self.file_num = block.file_num;
        }
        let pending = self
            .document_transfer
            .pending
            .as_mut()
            .ok_or(anyhow!("No open document"))?;
        let data = if block.compressed {
            std::borrow::Cow::Owned(decompress(&block.data))
        } else {
            std::borrow::Cow::Borrowed(block.data.as_ref())
        };
        pending.file.write_all(&data).await?;
        self.finished_size += data.len() as u64;
        self.transferred += block.data.len() as u64;
        if block.data.is_empty() {
            self.finish_document().await?;
        }
        Ok(())
    }

    pub async fn finish_document(&mut self) -> ResultType<()> {
        if let Some(error) = &self.document_transfer.error {
            bail!("{}", error);
        }
        let Some(pending) = self.document_transfer.pending.take() else {
            return Ok(());
        };
        let result = finish_write(pending).await;
        if let Err(error) = &result {
            self.document_transfer.error = Some(error.to_string());
        }
        result
    }

    pub async fn cancel_document(&mut self) -> ResultType<()> {
        let Some(pending) = self.document_transfer.pending.take() else {
            return Ok(());
        };
        let PendingWrite {
            file,
            descriptor,
            path,
            token,
        } = pending;
        drop(file);
        let result = descriptor.close("Document transfer canceled").await;
        discard_write(path, token, result).await
    }

    pub(super) async fn seek_document(&mut self, offset: u64) -> ResultType<()> {
        let Some(DataStream::FileStream(file)) = self.data_stream.as_mut() else {
            bail!("Document receives must be restarted after interruption");
        };
        let entry = self
            .files
            .get(self.file_num as usize)
            .ok_or(anyhow!("Wrong file number"))?;
        if offset > 0 && entry.modified_time == 0 {
            bail!(
                "Cannot resume a document with an unknown modification time; restart the transfer"
            );
        }
        file.seek(std::io::SeekFrom::Start(offset)).await?;
        self.finished_size += offset;
        self.transferred += offset;
        Ok(())
    }
}

async fn begin_write(path: String, overwrite: bool) -> ResultType<PendingWrite> {
    spawn_blocking(move || {
        let token = android::request(json!({
            "op": "begin", "path": path, "overwrite": overwrite
        }))?;
        let file = android::take_file(&token["fd"])?;
        let descriptor = DocumentDescriptor::new(&token)?;
        Ok(PendingWrite {
            file: File::from_std(file),
            descriptor,
            token,
            path,
        })
    })
    .await?
}

async fn finish_write(pending: PendingWrite) -> ResultType<()> {
    let PendingWrite {
        mut file,
        descriptor,
        path,
        token,
    } = pending;
    let written: ResultType<()> = async {
        file.flush().await?;
        file.sync_all().await?;
        Ok(())
    }
    .await;
    drop(file);
    let message = written
        .as_ref()
        .err()
        .map(ToString::to_string)
        .unwrap_or_default();
    let closed = descriptor.close(&message).await;
    let result = match (written, closed) {
        (Err(error), Err(close_error)) => Err(anyhow!(
            "{}; closing document failed: {}",
            error,
            close_error
        )),
        (Err(error), _) | (_, Err(error)) => Err(error),
        _ => Ok(()),
    };
    if result.is_err() {
        return discard_write(path, token, result).await;
    }
    spawn_blocking(move || {
        android::request(json!({
            "op": "finish", "path": path, "token": token
        }))
    })
    .await??;
    Ok(())
}

async fn discard_write(path: String, token: Value, result: ResultType<()>) -> ResultType<()> {
    let cleanup: ResultType<()> = async {
        spawn_blocking(move || {
            android::request(json!({
                "op": "discard", "path": path, "token": token
            }))
        })
        .await??;
        Ok(())
    }
    .await;
    match (result, cleanup) {
        (Err(error), Err(cleanup)) => {
            bail!("{}; temporary document cleanup failed: {}", error, cleanup)
        }
        (Err(error), _) | (_, Err(error)) => Err(error),
        _ => Ok(()),
    }
}
