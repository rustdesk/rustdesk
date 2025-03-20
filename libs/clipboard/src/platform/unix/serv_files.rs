use super::local_file::LocalFile;
use crate::{platform::unix::local_file::construct_file_list, ClipboardFile, CliprdrError};
use hbb_common::{
    bytes::{BufMut, BytesMut},
    log,
};
use parking_lot::Mutex;
use std::{path::PathBuf, sync::Arc};

lazy_static::lazy_static! {
    // local files are cached, this value should not be changed when copying files
    // Because `CliprdrFileContentsRequest` only contains the index of the file in the list.
    // We need to keep the file list in the same order as the remote side.
    // We may add a `FileId` field to `CliprdrFileContentsRequest` in the future.
    static ref CLIP_FILES: Arc<Mutex<ClipFiles>> = Default::default();
}

#[derive(Debug)]
enum FileContentsRequest {
    Size {
        stream_id: i32,
        file_idx: usize,
    },

    Range {
        stream_id: i32,
        file_idx: usize,
        offset: u64,
        length: u64,
    },
}

#[derive(Default)]
struct ClipFiles {
    files: Vec<String>,
    file_list: Vec<LocalFile>,
    files_pdu: Vec<u8>,
}

impl ClipFiles {
    fn clear(&mut self) {
        self.files.clear();
        self.file_list.clear();
        self.files_pdu.clear();
    }

    fn sync_files(&mut self, clipboard_files: &[String]) -> Result<(), CliprdrError> {
        let clipboard_paths = clipboard_files
            .iter()
            .map(|s| PathBuf::from(s))
            .collect::<Vec<_>>();
        self.file_list = construct_file_list(&clipboard_paths)?;
        self.files = clipboard_files.to_vec();
        Ok(())
    }

    fn build_file_list_pdu(&mut self) {
        let mut data = BytesMut::with_capacity(4 + 592 * self.file_list.len());
        data.put_u32_le(self.file_list.len() as u32);
        for file in self.file_list.iter() {
            data.put(file.as_bin().as_slice());
        }
        self.files_pdu = data.to_vec()
    }

    fn serve_file_contents(
        &mut self,
        conn_id: i32,
        request: FileContentsRequest,
    ) -> Result<ClipboardFile, CliprdrError> {
        let (file_idx, file_contents_resp) = match request {
            FileContentsRequest::Size {
                stream_id,
                file_idx,
            } => {
                log::debug!("file contents (size) requested from conn: {}", conn_id);
                let Some(file) = self.file_list.get(file_idx) else {
                    log::error!(
                        "invalid file index {} requested from conn: {}",
                        file_idx,
                        conn_id
                    );
                    return Err(CliprdrError::InvalidRequest {
                        description: format!(
                            "invalid file index {} requested from conn: {}",
                            file_idx, conn_id
                        ),
                    });
                };

                log::debug!(
                    "conn {} requested file-{}: {}",
                    conn_id,
                    file_idx,
                    file.name
                );

                let size = file.size;
                (
                    file_idx,
                    ClipboardFile::FileContentsResponse {
                        msg_flags: 0x1,
                        stream_id,
                        requested_data: size.to_le_bytes().to_vec(),
                    },
                )
            }
            FileContentsRequest::Range {
                stream_id,
                file_idx,
                offset,
                length,
            } => {
                log::debug!(
                    "file contents (range from {} length {}) request from conn: {}",
                    offset,
                    length,
                    conn_id
                );
                let Some(file) = self.file_list.get_mut(file_idx) else {
                    log::error!(
                        "invalid file index {} requested from conn: {}",
                        file_idx,
                        conn_id
                    );
                    return Err(CliprdrError::InvalidRequest {
                        description: format!(
                            "invalid file index {} requested from conn: {}",
                            file_idx, conn_id
                        ),
                    });
                };
                log::debug!(
                    "conn {} requested file-{}: {}",
                    conn_id,
                    file_idx,
                    file.name
                );

                if offset > file.size {
                    log::error!("invalid reading offset requested from conn: {}", conn_id);
                    return Err(CliprdrError::InvalidRequest {
                        description: format!(
                            "invalid reading offset requested from conn: {}",
                            conn_id
                        ),
                    });
                }
                let read_size = if offset + length > file.size {
                    file.size - offset
                } else {
                    length
                };

                let mut buf = vec![0u8; read_size as usize];

                file.read_exact_at(&mut buf, offset)?;

                (
                    file_idx,
                    ClipboardFile::FileContentsResponse {
                        msg_flags: 0x1,
                        stream_id,
                        requested_data: buf,
                    },
                )
            }
        };

        log::debug!("file contents sent to conn: {}", conn_id);
        // hot reload next file
        for next_file in self.file_list.iter_mut().skip(file_idx + 1) {
            if !next_file.is_dir {
                next_file.load_handle()?;
                break;
            }
        }
        Ok(file_contents_resp)
    }
}

#[inline]
pub fn clear_files() {
    CLIP_FILES.lock().clear();
}

pub fn read_file_contents(
    conn_id: i32,
    stream_id: i32,
    list_index: i32,
    dw_flags: i32,
    n_position_low: i32,
    n_position_high: i32,
    cb_requested: i32,
) -> Result<ClipboardFile, CliprdrError> {
    let fcr = if dw_flags == 0x1 {
        FileContentsRequest::Size {
            stream_id,
            file_idx: list_index as usize,
        }
    } else if dw_flags == 0x2 {
        let offset = (n_position_high as u64) << 32 | n_position_low as u64;
        let length = cb_requested as u64;

        FileContentsRequest::Range {
            stream_id,
            file_idx: list_index as usize,
            offset,
            length,
        }
    } else {
        return Err(CliprdrError::InvalidRequest {
            description: format!("got invalid FileContentsRequest, dw_flats: {dw_flags}"),
        });
    };

    CLIP_FILES.lock().serve_file_contents(conn_id, fcr)
}

pub fn sync_files(files: &[String]) -> Result<(), CliprdrError> {
    let mut files_lock = CLIP_FILES.lock();
    if files_lock.files == files {
        return Ok(());
    }
    files_lock.sync_files(files)?;
    Ok(files_lock.build_file_list_pdu())
}

pub fn get_file_list_pdu() -> Vec<u8> {
    CLIP_FILES.lock().files_pdu.clone()
}
