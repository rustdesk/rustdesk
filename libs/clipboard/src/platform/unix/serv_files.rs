use crate::{
    platform::unix::{
        filetype::file_list_id,
        local_file::{construct_file_list, LocalFile},
    },
    ClipboardFile, CliprdrError,
};
use hbb_common::{
    bytes::{BufMut, BytesMut},
    log,
};
use parking_lot::Mutex;
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{atomic::Ordering, Arc},
    time::SystemTime,
    usize,
};

// Our clients request at most this much per range: macOS 4 MiB, FUSE less, and Windows
// splits larger reads (WF_CLIPRDR_MAX_RANGE_READ). A larger read is refused, not shortened:
// a short response reads as end of file to IStream callers on Windows.
const MAX_RANGE_READ: u64 = 16 * 1024 * 1024;

lazy_static::lazy_static! {
    // local files are cached, this value should not be changed when copying files
    // Because `CliprdrFileContentsRequest` only contains the index of the file in the list.
    // We need to keep the file list in the same order as the remote side.
    // We may add a `FileId` field to `CliprdrFileContentsRequest` in the future.
    static ref CLIP_FILES: Arc<Mutex<ClipFiles>> = Default::default();
    // Lists replaced by a newer copy after a peer was sent them, oldest first. The peer's
    // streams keep reading the list they came from. Lock after `CLIP_FILES`.
    static ref RETIRED_CLIP_FILES: Mutex<VecDeque<ClipFiles>> = Default::default();
}

const MAX_RETIRED_CLIP_FILES: usize = 4;

// The first descriptor's `clsid`, inside the 32 reserved bytes every peer skips. A random
// nonce there makes each copy's list id distinct, even when two copies have identical
// names, sizes and times (the same file in two folders).
const LIST_NONCE: std::ops::Range<usize> = 8..24;

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
    // `file_list_id(files_pdu)`, which peers send back as `clip_data_id`.
    id: i32,
    // Connections that were sent `files_pdu`.
    served_to: Vec<i32>,
}

impl ClipFiles {
    fn clear(&mut self) {
        self.files.clear();
        self.sigs.clear();
        self.file_list.clear();
        self.first_file_index = usize::MAX;
        self.files_pdu.clear();
        self.id = 0;
        self.served_to.clear();
    }

    fn sync_files(
        &mut self,
        clipboard_files: &[String],
        sigs: Vec<FileSig>,
    ) -> Result<(), CliprdrError> {
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
        self.sigs = sigs;
        Ok(())
    }

    fn build_file_list_pdu(&mut self) -> Result<(), CliprdrError> {
        let mut data = BytesMut::with_capacity(4 + 592 * self.file_list.len());
        data.put_u32_le(self.file_list.len() as u32);
        for file in self.file_list.iter() {
            data.put(file.as_bin()?.as_slice());
        }
        self.files_pdu = data.to_vec();
        if let Some(nonce) = self.files_pdu.get_mut(LIST_NONCE) {
            nonce.copy_from_slice(&rand::random::<[u8; 16]>());
        }
        self.id = file_list_id(&self.files_pdu);
        Ok(())
    }

    // Close files that are not being read, such as the preloaded next file; `read_exact_at`
    // reopens them on the next read. A file part-way through keeps its handle and offset, so
    // its remaining bytes come from the same file even if its path is replaced meanwhile.
    fn release_handles(&mut self) {
        for file in self.file_list.iter_mut() {
            if file.offset.load(Ordering::Relaxed) == 0 {
                file.handle = None;
            }
        }
    }

    // A retired list's file is reopened by path once its handle was closed, and the path may now
    // name a replacement. Check size and modified time on the handle the read will use, and refuse
    // a file that no longer matches the list.
    fn check_retired_file(&mut self, request: &FileContentsRequest) -> Result<(), CliprdrError> {
        let FileContentsRequest::Range { file_idx, .. } = request else {
            return Ok(());
        };
        let Some(file) = self.file_list.get_mut(*file_idx) else {
            return Ok(());
        };
        if file.is_dir {
            return Ok(());
        }
        file.load_handle()?;
        let unchanged = file
            .handle
            .as_ref()
            .and_then(|handle| handle.get_ref().metadata().ok())
            .map_or(false, |md| {
                md.len() == file.size && md.modified().ok() == Some(file.last_write_time)
            });
        if unchanged {
            Ok(())
        } else {
            Err(CliprdrError::InvalidRequest {
                description: format!("file {} changed since its list was sent", file.name),
            })
        }
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
                let read_size = if length > file.size - offset {
                    file.size - offset
                } else {
                    length
                };

                if read_size > MAX_RANGE_READ {
                    return Err(CliprdrError::InvalidRequest {
                        description: format!(
                            "file contents request of {} bytes exceeds the {} byte limit, conn: {}",
                            read_size, MAX_RANGE_READ, conn_id
                        ),
                    });
                }

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
    RETIRED_CLIP_FILES.lock().clear();
}

// The list a request reads from: the one its `clip_data_id` names, else the one last sent to
// the connection. A named list that is gone yields `None`. A peer that sends no id and was
// never sent a list reads the current one, as before.
fn select_clip_files<'a>(
    current: &'a mut ClipFiles,
    retired: &'a mut VecDeque<ClipFiles>,
    conn_id: i32,
    clip_data_id: Option<i32>,
) -> Option<&'a mut ClipFiles> {
    match clip_data_id {
        Some(id) if current.id == id => Some(current),
        Some(id) => retired
            .iter_mut()
            .rev()
            .find(|files| files.id == id && files.served_to.contains(&conn_id)),
        None if current.served_to.contains(&conn_id) => Some(current),
        None => retired
            .iter_mut()
            .rev()
            .find(|files| files.served_to.contains(&conn_id))
            .or(Some(current)),
    }
}

// Keep a list a peer was sent, so its streams do not read the new copy at the same indexes.
fn retire_if_served(mut replaced: ClipFiles) {
    if replaced.served_to.is_empty() {
        return;
    }
    replaced.release_handles();
    let mut retired = RETIRED_CLIP_FILES.lock();
    if retired.len() == MAX_RETIRED_CLIP_FILES {
        retired.pop_front();
    }
    retired.push_back(replaced);
}

pub fn read_file_contents(
    conn_id: i32,
    stream_id: i32,
    list_index: i32,
    dw_flags: i32,
    n_position_low: i32,
    n_position_high: i32,
    cb_requested: i32,
    clip_data_id: Option<i32>,
) -> Vec<Result<ClipboardFile, CliprdrError>> {
    let fcr = if dw_flags == 0x1 {
        FileContentsRequest::Size {
            stream_id,
            file_idx: list_index as usize,
        }
    } else if dw_flags == 0x2 {
        // nPositionLow and cbRequested are UINT32s carried in i32 fields. Sign-extending
        // them rejects offsets whose low word is >= 2 GiB and turns a negative length
        // into a near-u64::MAX read.
        let offset = (n_position_high as u64) << 32 | n_position_low as u32 as u64;
        let length = cb_requested as u32 as u64;

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

    let mut current = CLIP_FILES.lock();
    let mut retired = RETIRED_CLIP_FILES.lock();
    let current_ptr: *const ClipFiles = &*current;
    let Some(clip_files) = select_clip_files(&mut current, &mut retired, conn_id, clip_data_id)
    else {
        return vec![Err(CliprdrError::InvalidRequest {
            description: format!(
                "file list {:?} is not available to conn: {}",
                clip_data_id, conn_id
            ),
        })];
    };
    if !std::ptr::eq(&*clip_files, current_ptr) {
        if let Err(e) = clip_files.check_retired_file(&fcr) {
            return vec![Err(e)];
        }
    }
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
    // Build aside, so a failure leaves the current list in place.
    let mut next = ClipFiles::default();
    next.sync_files(files, current)?;
    next.build_file_list_pdu()?;
    let replaced = std::mem::replace(&mut *files_lock, next);
    retire_if_served(replaced);
    Ok(())
}

pub fn get_file_list_pdu(conn_id: i32) -> Vec<u8> {
    let mut clip_files = CLIP_FILES.lock();
    if !clip_files.files_pdu.is_empty() && !clip_files.served_to.contains(&conn_id) {
        clip_files.served_to.push(conn_id);
    }
    clip_files.files_pdu.clone()
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

    // Tests that drive the global CLIP_FILES must not interleave.
    static CLIP_FILES_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn lock_clip_files() -> std::sync::MutexGuard<'static, ()> {
        CLIP_FILES_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    // Sparse, so a multi-GiB file costs no disk space.
    fn write_at(path: &PathBuf, offset: u64, data: &[u8]) {
        use std::io::{Seek, SeekFrom, Write};
        let mut f = fs::File::create(path).unwrap();
        f.seek(SeekFrom::Start(offset)).unwrap();
        f.write_all(data).unwrap();
    }

    fn range_data(res: Vec<Result<ClipboardFile, CliprdrError>>) -> Vec<u8> {
        match res.into_iter().last() {
            Some(Ok(ClipboardFile::FileContentsResponse { requested_data, .. })) => requested_data,
            other => panic!("unexpected response: {:?}", other),
        }
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
        // reset first.
        let _guard = lock_clip_files();
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

    #[test]
    fn range_request_huge_length_is_clamped() {
        let tmp = TmpDir::new("range");
        let file = tmp.join("data.bin");
        fs::write(&file, b"0123456789").unwrap();
        let files = vec![path_str(&file)];

        let mut clip = ClipFiles::default();
        clip.sync_files(&files, fingerprint(&files)).unwrap();
        let file_idx = clip.first_file_index;

        // offset + length used to wrap to 0, skip the clamp and ask for a u64::MAX buffer.
        let resp = clip
            .serve_file_contents(
                0,
                FileContentsRequest::Range {
                    stream_id: 0,
                    file_idx,
                    offset: 1,
                    length: u64::MAX,
                },
            )
            .unwrap();
        match resp {
            ClipboardFile::FileContentsResponse { requested_data, .. } => {
                assert_eq!(requested_data, b"123456789");
            }
            _ => panic!("unexpected response"),
        }
    }

    #[test]
    fn range_request_decodes_wire_fields_as_u32() {
        let tmp = TmpDir::new("decode");
        let small = tmp.join("small.bin");
        fs::write(&small, b"0123456789").unwrap();
        let large = tmp.join("large.bin");
        write_at(&large, 0x8000_0000, b"tail");

        let _guard = lock_clip_files();
        clear_files();
        sync_files(&[path_str(&small), path_str(&large)]).unwrap();
        let small_idx = CLIP_FILES.lock().first_file_index as i32;
        let large_idx = small_idx + 1;

        // A negative cbRequested is a UINT32 length, clamped to the rest of the file.
        let res = read_file_contents(0, 0, small_idx, 0x2, 1, 0, -1, None);
        assert_eq!(range_data(res), b"123456789");

        // An nPositionLow with bit 31 set is an offset past 2 GiB, not a huge one.
        let res = read_file_contents(0, 0, large_idx, 0x2, 0x8000_0000u32 as i32, 0, 4, None);
        assert_eq!(range_data(res), b"tail");

        clear_files();
    }

    #[test]
    fn range_request_over_limit_is_rejected() {
        let tmp = TmpDir::new("limit");
        let file = tmp.join("big.bin");
        let size = MAX_RANGE_READ + 1;
        write_at(&file, size - 1, b"x");
        let files = vec![path_str(&file)];

        let mut clip = ClipFiles::default();
        clip.sync_files(&files, fingerprint(&files)).unwrap();
        let file_idx = clip.first_file_index;

        let res = clip.serve_file_contents(
            0,
            FileContentsRequest::Range {
                stream_id: 0,
                file_idx,
                offset: 0,
                length: u32::MAX as u64,
            },
        );
        assert!(matches!(res, Err(CliprdrError::InvalidRequest { .. })));
    }

    #[test]
    fn host_copy_during_transfer_keeps_serving_the_sent_list() {
        let tmp = TmpDir::new("recopy_in_flight");
        let first = tmp.join("first.bin");
        fs::write(&first, b"AAAAAAAA").unwrap();
        let second = tmp.join("second.bin");
        fs::write(&second, b"BBBBBBBB").unwrap();

        let _guard = lock_clip_files();
        clear_files();
        sync_files(&[path_str(&first)]).unwrap();
        get_file_list_pdu(1);
        let idx = CLIP_FILES.lock().first_file_index as i32;
        let read = |offset| range_data(read_file_contents(1, 7, idx, 0x2, offset, 0, 4, None));
        assert_eq!(read(0), b"AAAA");

        // Copying another file on this side must not redirect the peer's stream.
        sync_files(&[path_str(&second)]).unwrap();
        assert_eq!(read(4), b"AAAA");

        clear_files();
    }

    #[test]
    fn stream_keeps_its_list_after_a_second_paste() {
        let tmp = TmpDir::new("repaste_in_flight");
        let first = tmp.join("first.bin");
        fs::write(&first, b"AAAAAAAA").unwrap();
        let second = tmp.join("second.bin");
        fs::write(&second, b"BBBBBBBB").unwrap();

        let _guard = lock_clip_files();
        clear_files();
        sync_files(&[path_str(&first)]).unwrap();
        let first_id = file_list_id(&get_file_list_pdu(1));
        let idx = CLIP_FILES.lock().first_file_index as i32;

        // The peer copies the second file on this side and pastes it while the first
        // transfer is still reading.
        sync_files(&[path_str(&second)]).unwrap();
        let second_id = file_list_id(&get_file_list_pdu(1));
        assert_ne!(first_id, second_id);

        let read = |id| range_data(read_file_contents(1, 7, idx, 0x2, 4, 0, 4, Some(id)));
        assert_eq!(read(first_id), b"AAAA");
        assert_eq!(read(second_id), b"BBBB");

        clear_files();
    }

    #[test]
    fn named_list_must_have_been_sent_to_the_conn() {
        let tmp = TmpDir::new("foreign_list");
        let first = tmp.join("first.bin");
        fs::write(&first, b"AAAAAAAA").unwrap();
        let second = tmp.join("second.bin");
        fs::write(&second, b"BBBBBBBB").unwrap();

        let _guard = lock_clip_files();
        clear_files();
        sync_files(&[path_str(&first)]).unwrap();
        let first_id = file_list_id(&get_file_list_pdu(1));
        let idx = CLIP_FILES.lock().first_file_index as i32;
        sync_files(&[path_str(&second)]).unwrap();

        let refused = |conn_id, id| {
            matches!(
                read_file_contents(conn_id, 7, idx, 0x2, 0, 0, 4, Some(id)).last(),
                Some(Err(CliprdrError::InvalidRequest { .. }))
            )
        };
        // Retired list, asked for by a connection it was never sent to.
        assert!(refused(2, first_id));
        // A list id this side never sent.
        assert!(refused(1, first_id ^ 1));

        clear_files();
    }

    #[test]
    fn failed_recopy_keeps_the_current_list() {
        let tmp = TmpDir::new("failed_recopy");
        let first = tmp.join("first.bin");
        fs::write(&first, b"AAAAAAAA").unwrap();
        let missing = tmp.join("missing.bin");

        let _guard = lock_clip_files();
        clear_files();
        sync_files(&[path_str(&first)]).unwrap();
        let pdu = get_file_list_pdu(1);

        assert!(sync_files(&[path_str(&missing)]).is_err());
        assert_eq!(get_file_list_pdu(1), pdu);

        clear_files();
    }

    fn file_index(path: &PathBuf) -> i32 {
        let files = CLIP_FILES.lock();
        files
            .file_list
            .iter()
            .position(|f| &f.path == path)
            .unwrap() as i32
    }

    // Replaces `path` the way editors save: write aside, then rename over it.
    fn replace_atomically(path: &PathBuf, data: &[u8]) {
        let aside = path.with_extension("new");
        fs::write(&aside, data).unwrap();
        fs::rename(&aside, path).unwrap();
    }

    #[test]
    fn retired_list_closes_only_files_not_being_read() {
        let tmp = TmpDir::new("retired_handles");
        let first = tmp.join("first.bin");
        fs::write(&first, b"AAAABBBB").unwrap();
        let next = tmp.join("next.bin");
        fs::write(&next, b"CCCCDDDD").unwrap();
        let other = tmp.join("other.bin");
        fs::write(&other, b"EEEEEEEE").unwrap();

        let _guard = lock_clip_files();
        clear_files();
        sync_files(&[path_str(&first), path_str(&next)]).unwrap();
        let id = file_list_id(&get_file_list_pdu(1));
        let (first_idx, next_idx) = (file_index(&first), file_index(&next));
        let read =
            |idx, offset| range_data(read_file_contents(1, 7, idx, 0x2, offset, 0, 4, Some(id)));
        // Leaves `first` open at offset 4, and preloads `next`.
        assert_eq!(read(first_idx, 0), b"AAAA");

        sync_files(&[path_str(&other)]).unwrap();
        {
            let retired = RETIRED_CLIP_FILES.lock();
            let open = |idx: i32| retired[0].file_list[idx as usize].handle.is_some();
            assert!(open(first_idx));
            assert!(!open(next_idx));
        }
        assert_eq!(read(first_idx, 4), b"BBBB");
        assert_eq!(read(next_idx, 0), b"CCCC");

        clear_files();
    }

    #[test]
    fn files_being_read_keep_their_content_when_their_paths_are_replaced() {
        let tmp = TmpDir::new("replaced_paths");
        let first = tmp.join("first.bin");
        fs::write(&first, b"1111aaaa").unwrap();
        let second = tmp.join("second.bin");
        fs::write(&second, b"2222bbbb").unwrap();
        let other = tmp.join("other.bin");
        fs::write(&other, b"EEEEEEEE").unwrap();

        let _guard = lock_clip_files();
        clear_files();
        sync_files(&[path_str(&first), path_str(&second)]).unwrap();
        let id = file_list_id(&get_file_list_pdu(1));
        let (first_idx, second_idx) = (file_index(&first), file_index(&second));
        let read =
            |idx, offset| range_data(read_file_contents(1, 7, idx, 0x2, offset, 0, 4, Some(id)));
        // Two files part-way through at once.
        assert_eq!(read(first_idx, 0), b"1111");
        assert_eq!(read(second_idx, 0), b"2222");

        sync_files(&[path_str(&other)]).unwrap();
        replace_atomically(&first, b"3333cccc");
        replace_atomically(&second, b"4444dddd");

        assert_eq!(read(first_idx, 4), b"aaaa");
        assert_eq!(read(second_idx, 4), b"bbbb");

        clear_files();
    }

    #[test]
    fn same_file_metadata_in_another_folder_is_another_list() {
        let tmp = TmpDir::new("same_metadata");
        let (dir_a, dir_b) = (tmp.join("a"), tmp.join("b"));
        fs::create_dir_all(&dir_a).unwrap();
        fs::create_dir_all(&dir_b).unwrap();
        let (first, second) = (dir_a.join("report.bin"), dir_b.join("report.bin"));
        fs::write(&first, b"AAAAAAAA").unwrap();
        fs::write(&second, b"BBBBBBBB").unwrap();
        // Same name, size and mtime: the descriptors differ only by the nonce.
        let mtime = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        for path in [&first, &second] {
            fs::File::options()
                .write(true)
                .open(path)
                .unwrap()
                .set_modified(mtime)
                .unwrap();
        }

        let _guard = lock_clip_files();
        clear_files();
        sync_files(&[path_str(&first)]).unwrap();
        let first_id = file_list_id(&get_file_list_pdu(1));
        let idx = CLIP_FILES.lock().first_file_index as i32;
        sync_files(&[path_str(&second)]).unwrap();
        let second_id = file_list_id(&get_file_list_pdu(1));
        assert_ne!(first_id, second_id);

        let read = |id| range_data(read_file_contents(1, 7, idx, 0x2, 4, 0, 4, Some(id)));
        assert_eq!(read(first_id), b"AAAA");
        assert_eq!(read(second_id), b"BBBB");

        clear_files();
    }

    #[test]
    fn a_retired_list_refuses_a_file_replaced_before_its_read() {
        let tmp = TmpDir::new("replaced_unread");
        let first = tmp.join("first.bin");
        fs::write(&first, b"AAAAAAAA").unwrap();
        let second = tmp.join("second.bin");
        fs::write(&second, b"BBBBBBBB").unwrap();
        let other = tmp.join("other.bin");
        fs::write(&other, b"EEEEEEEE").unwrap();

        let _guard = lock_clip_files();
        clear_files();
        sync_files(&[path_str(&first), path_str(&second)]).unwrap();
        let id = file_list_id(&get_file_list_pdu(1));
        let (first_idx, second_idx) = (file_index(&first), file_index(&second));
        let read = |idx, offset| read_file_contents(1, 7, idx, 0x2, offset, 0, 4, Some(id));
        assert_eq!(range_data(read(first_idx, 0)), b"AAAA");

        // Retiring closes `second`, which has not been read yet; it is then replaced.
        sync_files(&[path_str(&other)]).unwrap();
        replace_atomically(&second, b"CCCCCCCCCC");

        assert!(matches!(
            read(second_idx, 0).last(),
            Some(Err(CliprdrError::InvalidRequest { .. }))
        ));
        // The file part-way through is unaffected.
        assert_eq!(range_data(read(first_idx, 4)), b"AAAA");

        clear_files();
    }
}
