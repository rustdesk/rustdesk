use super::local_file::LocalFile;
use crate::{platform::unix::local_file::construct_file_list, ClipboardFile, CliprdrError};
use hbb_common::{
    bytes::{BufMut, BytesMut},
    log,
};
use parking_lot::Mutex;
use std::{path::PathBuf, sync::Arc, time::SystemTime, usize};

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

// Cheap fingerprint of one top-level selected entry. A change in size/mtime --
// or a directory in the selection -- forces sync_files() to rebuild (see below).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct FileSig {
    size: u64,
    mtime: Option<SystemTime>,
    is_dir: bool,
}

// Stat the top-level selected paths only (no recursion), same order as `files`.
fn fingerprint(files: &[String]) -> Vec<FileSig> {
    files
        .iter()
        .map(|s| match std::fs::metadata(s) {
            Ok(mt) => FileSig {
                size: mt.len(),
                mtime: mt.modified().ok(),
                is_dir: mt.is_dir(),
            },
            Err(_) => FileSig::default(),
        })
        .collect()
}

#[derive(Default)]
struct ClipFiles {
    files: Vec<String>,
    // Fingerprint of `files` (same len/order); detects in-place edits on re-copy.
    sigs: Vec<FileSig>,
    file_list: Vec<LocalFile>,
    first_file_index: usize,
    files_pdu: Vec<u8>,
}

impl ClipFiles {
    fn clear(&mut self) {
        self.files.clear();
        self.sigs.clear();
        self.file_list.clear();
        self.first_file_index = usize::MAX;
        self.files_pdu.clear();
    }

    fn sync_files(&mut self, clipboard_files: &[String]) -> Result<(), CliprdrError> {
        let clipboard_paths = clipboard_files
            .iter()
            .map(|s| PathBuf::from(s))
            .collect::<Vec<_>>();
        self.file_list = construct_file_list(&clipboard_paths)?;
        self.first_file_index = self
            .file_list
            .iter()
            .position(|f| !f.path.is_dir())
            .unwrap_or(usize::MAX);
        self.files = clipboard_files.to_vec();
        self.sigs = fingerprint(clipboard_files);
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

    fn get_files_for_audit(&self, request: &FileContentsRequest) -> Option<ClipboardFile> {
        if let FileContentsRequest::Range {
            file_idx, offset, ..
        } = request
        {
            if *file_idx == self.first_file_index && *offset == 0 {
                let files: Vec<(String, u64)> = self
                    .file_list
                    .iter()
                    .filter_map(|f| {
                        if f.path.is_file() {
                            Some((f.path.to_string_lossy().to_string(), f.size))
                        } else {
                            None
                        }
                    })
                    .collect::<_>();
                if files.is_empty() {
                    return None;
                } else {
                    return Some(ClipboardFile::Files { files });
                }
            }
        }
        None
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
) -> Vec<Result<ClipboardFile, CliprdrError>> {
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
        return vec![Err(CliprdrError::InvalidRequest {
            description: format!("got invalid FileContentsRequest, dw_flats: {dw_flags}"),
        })];
    };

    let mut clip_files = CLIP_FILES.lock();
    let mut res = vec![];
    if let Some(files_res) = clip_files.get_files_for_audit(&fcr) {
        res.push(Ok(files_res));
    }
    res.push(clip_files.serve_file_contents(conn_id, fcr));
    res
}

pub fn sync_files(files: &[String]) -> Result<(), CliprdrError> {
    // Dedup: skip the rebuild only when paths + sizes + mtimes match and no dir is
    // selected (a dir's own mtime doesn't change when a file inside it is edited).
    let current = fingerprint(files);
    let mut files_lock = CLIP_FILES.lock();
    if files_lock.files == files
        && files_lock.sigs == current
        && !current.iter().any(|sig| sig.is_dir)
    {
        return Ok(());
    }
    files_lock.sync_files(files)?;
    Ok(files_lock.build_file_list_pdu())
}

pub fn get_file_list_pdu() -> Vec<u8> {
    CLIP_FILES.lock().files_pdu.clone()
}

#[cfg(test)]
mod sig_test {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    // Unique temp dir under the system temp dir; removed on drop (no dev-dep).
    struct TmpDir(PathBuf);
    impl TmpDir {
        fn new(tag: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let mut dir = std::env::temp_dir();
            dir.push(format!("rustdesk_sig_test_{}_{}", tag, nanos));
            fs::create_dir_all(&dir).unwrap();
            TmpDir(dir)
        }
        fn join(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }
    impl Drop for TmpDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn path_str(p: &PathBuf) -> String {
        p.to_string_lossy().to_string()
    }

    #[test]
    fn fingerprint_missing_path_is_default() {
        let tmp = TmpDir::new("missing");
        let missing = path_str(&tmp.join("does_not_exist.bin"));
        let sigs = fingerprint(&[missing]);
        assert_eq!(sigs.len(), 1);
        // A path that can't be stat'd -> default sig, which forces a rebuild.
        assert_eq!(sigs[0], FileSig::default());
        assert_eq!(sigs[0].mtime, None);
    }

    #[test]
    fn fingerprint_detects_inplace_edit() {
        let tmp = TmpDir::new("edit");
        let file = tmp.join("a.bin");
        fs::write(&file, b"small").unwrap();
        let p = path_str(&file);

        let before = fingerprint(&[p.clone()]);
        // Same content, same path: fingerprint must be stable.
        let again = fingerprint(&[p.clone()]);
        assert_eq!(before, again);
        assert_eq!(before[0].size, 5);
        assert!(!before[0].is_dir);

        // Edit in place so the file grows.
        fs::write(&file, b"much larger contents than before").unwrap();
        let after = fingerprint(&[p]);
        assert_ne!(before, after);
        assert!(after[0].size > before[0].size);
    }

    #[test]
    fn fingerprint_flags_directory() {
        let tmp = TmpDir::new("dir");
        let sub = tmp.join("subdir");
        fs::create_dir_all(&sub).unwrap();
        let sigs = fingerprint(&[path_str(&sub)]);
        assert_eq!(sigs.len(), 1);
        assert!(sigs[0].is_dir);
    }

    #[test]
    fn recopy_after_edit_refreshes_cached_size() {
        let tmp = TmpDir::new("recopy");
        let file = tmp.join("doc.bin");
        fs::write(&file, b"v1").unwrap(); // 2 bytes
        let files = vec![path_str(&file)];

        // Drive the public, guarded `sync_files` over the global CLIP_FILES;
        // reset first (this is the only test that touches the global).
        clear_files();

        sync_files(&files).unwrap();
        {
            let cache = CLIP_FILES.lock();
            let idx = cache.first_file_index;
            assert_eq!(cache.file_list[idx].size, 2);
        }

        // In-place edit grows the file; the re-copy must rebuild. Pre-fix the
        // path-only guard early-returned and left the cached size stale at 2.
        fs::write(&file, b"v2 is bigger").unwrap(); // 12 bytes
        sync_files(&files).unwrap();
        {
            let cache = CLIP_FILES.lock();
            let idx = cache.first_file_index;
            assert_eq!(cache.file_list[idx].size, 12);
        }

        clear_files(); // leave the global clean for other tests
    }
}
