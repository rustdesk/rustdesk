//! fuse server implement
//! we use fuse to provide file readers, warping data transfer to file interfaces
//!
//! # Name encoding
//!
//! There are different collection of characters forbidden in file names on different platforms:
//! - windows: `\/:*?"<>|`
//! - macos: `:/`
//! - linux: `/`
//!
//! what makes it troublesome is windows also used '\' as path separator.
//!
//! For now, we transfer all file names with windows separators, UTF-16 encoded.
//! *Need a way to transfer file names with '\' safely*.
//! Maybe we can use URL encoded file names and '/' seperators as a new standard, while keep the support to old schemes.
//!
//! # Note
//! - all files on FS should be read only, and mark the owner to be the current user
//! - any write operations, hard links, and symbolic links on the FS should be denied

use std::{
    collections::{BTreeMap, HashMap},
    ffi::OsString,
    ops::DerefMut,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{Receiver, Sender},
        Arc,
    },
    time::{Duration, SystemTime},
};

use dashmap::DashMap;
use fuser::{ReplyDirectory, Request, FUSE_ROOT_ID};
use hbb_common::{
    bytes::{Buf, Bytes},
    log,
};
use parking_lot::{Condvar, Mutex, RwLock};
use rayon::prelude::*;
use utf16string::WStr;

use crate::ClipboardFile;

/// block size for fuse, align to our asynchronic request size over FileContentsRequest.
///
/// Question: will this hint users to read data in this size?
const BLOCK_SIZE: u32 = 128 * 1024;
/// format ID for file descriptor
///
/// # Note
/// this is a custom format ID, not a standard one
/// still should be pinned to this value in our custom implementation
const FILEDESCRIPTOR_FORMAT_ID: i32 = 49334;

/// read only permission
const PERM_READ: u16 = 0o444;
/// max length of file name
const MAX_NAME_LEN: usize = 255;

// fuse server state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    // active and ready for all incoming requests
    Active,
    // marking and waiting for all FDs to be closed
    // only serve read requests
    Gc,
    // gc completes
    // serve no requests
    GcComplete,
    // fetching new files from remote
    // serve no requests
    // this state is to make sure only one fetching is running
    Fetching,
    // fetched, building new FS
    Building,
}

#[derive(Debug, Default)]
struct PendingRequest {
    content: Mutex<Option<ClipboardFile>>,
    cvar: Condvar,
}

impl PendingRequest {
    pub fn new() -> Self {
        Self {
            content: Mutex::new(None),
            cvar: Condvar::new(),
        }
    }

    pub fn recv_timeout(&self, timeout: Duration) -> Result<ClipboardFile, std::io::Error> {
        let mut guard = self.content.lock();
        let res = self.cvar.wait_for(&mut guard, timeout);
        if res.timed_out() {
            Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "timeout"))
        } else {
            let content = guard.take();
            match content {
                Some(content) => Ok(content),
                None => Err(std::io::Error::new(std::io::ErrorKind::Other, "no content")),
            }
        }
    }

    pub fn set(&self, content: ClipboardFile) {
        let mut guard = self.content.lock();
        guard.insert(content);
        self.cvar.notify_all();
    }
}

/// clipboard message dispatcher
#[derive(Debug, Default)]
struct CliprdrTxnDispatcher {
    txn_handler: DashMap<(i32, Option<i32>), Arc<PendingRequest>>,
}

impl CliprdrTxnDispatcher {
    pub fn send(&self, conn_id: i32, request: ClipboardFile) -> Arc<PendingRequest> {
        let stream_id = match &request {
            ClipboardFile::FormatDataRequest { .. } => None,
            ClipboardFile::FileContentsRequest { stream_id, .. } => Some(stream_id),
            _ => unreachable!(),
        };

        let req = Arc::new(PendingRequest::new());
        self.txn_handler
            .insert((conn_id, stream_id.copied()), req.clone());

        crate::send_data(conn_id, request);
        req
    }

    pub fn recv(&self, conn_id: i32, response: ClipboardFile) {
        let stream_id = match &response {
            ClipboardFile::FormatDataResponse { .. } => None,
            ClipboardFile::FileContentsResponse { stream_id, .. } => Some(stream_id),
            _ => unreachable!(),
        };
        let key = (conn_id, stream_id.cloned());
        match self.txn_handler.remove(&key) {
            Some((_, tx)) => tx.set(response),
            None => log::warn!("no request found for {:?}", key),
        }
    }
}

/// fuse server
/// provides a read-only file system
#[derive(Debug)]
pub(crate) struct FuseServer {
    status: RwLock<Status>,
    dispatcher: CliprdrTxnDispatcher,
    // timeout
    // current files
    // inode mapping:
    // 1 -> root (parent of all files)
    // 2~n+1 -> nth file in the list (n is the length of the list)
    // 0 | n+2.. -> not found
    // Note that the file tree is pre-ordered
    files: RwLock<Vec<FuseNode>>,
    // file handle counter
    file_handle_counter: AtomicU64,
    // file system generations
    generation: AtomicU64,
    // timeout
    timeout: Duration,
}

impl FuseServer {
    /// create a new fuse server
    pub fn new(timeout_secs: u64) -> Self {
        let timeout = Duration::from_secs(timeout_secs as u64);

        Self {
            status: RwLock::new(Status::Active),
            dispatcher: CliprdrTxnDispatcher::default(),
            files: RwLock::new(Vec::new()),
            file_handle_counter: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            timeout,
        }
    }

    /// gc filesystem
    fn gc_files(&self) {
        {
            let mut status = self.status.write();
            // really update only when:
            // running: Active
            if *status != Status::Active {
                return;
            }
            // received update after fetching complete
            // should fetch again
            if *status == Status::Building {
                *status == Status::GcComplete;
                return;
            }
            *status = Status::Gc;
        }

        let mut old = self.files.write();
        old.par_iter_mut().fold(|| (), |_, f| f.gc());

        let mut status = self.status.write();
        *status = Status::GcComplete;
    }

    /// fetch file list from remote
    fn sync_file_system(&self, conn_id: i32) -> Result<bool, std::io::Error> {
        {
            let mut status = self.status.write();
            if *status != Status::GcComplete {
                return Ok(false);
            }
            *status = Status::Fetching;
        }

        // request file list
        let request = ClipboardFile::FormatDataRequest {
            requested_format_id: FILEDESCRIPTOR_FORMAT_ID,
        };
        let rx = self.dispatcher.send(conn_id, request);
        let resp = rx.recv_timeout(self.timeout);
        let descs = match resp {
            Ok(ClipboardFile::FormatDataResponse {
                msg_flags,
                format_data,
            }) => {
                if msg_flags != 0 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "clipboard FUSE server: failed to fetch file list",
                    ));
                }
                let descs = FileDescription::parse_file_descriptors(format_data, conn_id)?;

                descs
            }
            Ok(_) => {
                // rollback status
                let mut status = self.status.write();
                *status = Status::GcComplete;

                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "clipboard FUSE server: invalid response to format data request",
                ));
            }
            Err(e) => {
                // rollback status
                let mut status = self.status.write();
                *status = Status::GcComplete;

                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    format!(
                        "clipboard FUSE server: timeout when waiting for format data response, {}",
                        e
                    ),
                ));
            }
        };

        {
            // fetch successful, start building
            let mut status = self.status.write();
            *status = Status::Building;
        }

        let mut new_tree = FuseNode::build_tree(descs)?;
        let res = new_tree
            .par_iter_mut()
            .filter(|f_node| f_node.is_file() && f_node.attributes.size == 0)
            .fold(|| Ok(()), |_, f_node| self.sync_node_size(f_node))
            .find_last(|p| p.is_err());

        if res.is_some() {
            // rollback status on failure
            let mut status = self.status.write();
            if *status == Status::Building {
                *status = Status::GcComplete;
            }

            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "clipboard FUSE server: failed to fetch file size".to_string(),
            ));
        }

        // replace current file system
        let mut old = self.files.write();
        {
            let mut status = self.status.write();
            if *status != Status::Building {
                // build interrupted, meaning fetched data is outdated
                // do not replace
                return Ok(false);
            }
            *status = Status::Active;
        }
        *old = new_tree;
        self.generation.fetch_add(1, Ordering::Relaxed);

        Ok(true)
    }

    /// replace current files with new files, cucurrently
    ///
    /// # Note
    ///
    /// This function should allow concurrent calls. In short, the server can handle multiple update_file calles
    /// at a short period of time and make sure it call RPCs as few and late as possible.
    ///
    /// ## Function Phases
    ///
    /// ### clear phase
    ///
    /// - just mark all files to be deleted, all new `open` operations will be denied
    /// - current FDs will not be affected, listing (in this level of directory) and reading operations can still be performed.
    /// - this will return only when all FDs are closed, or some unexpected error occurs
    /// - after all FDs are closed and no more FDs can be opened, dropping the current file list will be safe
    ///
    /// ### request phase
    ///
    /// - after all FDs are closed, send a format data request to the clipboard server
    ///
    /// ### replace phase
    ///
    /// - after all FDs are closed, the file list will be replaced with the new file list
    ///
    /// ## Concurrent calls
    ///
    /// ### server is Active
    ///
    /// threads calling this function may win getting the write lock on server.status:
    ///   - the winner will start [clear phase], changing the server to Gc.
    ///   - the loser or later comming threads calling `server.gc_files` will return directly.
    ///
    /// movement: Active -> Gc
    ///
    /// ### server is Gc
    ///
    /// this indicates there must be exactly one thread running in [clear phase].
    ///   - the thread will run `server.sync_file_system` after this phase
    ///   - other threads try to call `server.gc_files` will return directly
    ///   - other threads try to call `server.sync_file_system` will return directly
    ///   - no other threads could be running `server.sync_file_system`
    ///
    /// after all, only one thread will successfully complete the [clear phase], and that thread will try to complete the whole updating.
    ///
    /// movement: Gc -> GcComplete
    ///
    /// ### server is GcComplete
    ///
    /// This indicates there must be at least one thread trying to call `server.sync_file_system`.
    /// threads will trying to get the write lock of status.
    ///   - the winner will set status to Fetching.
    ///   - the latter threads get the write lock, only to find the status is not `GcComplete`, return directly.
    ///   - there might be threads trying to call `server.gc_files`, but will return directly and call `server.sync_file_system`.
    ///
    /// movement: GcComplete -> Fetching
    ///
    /// ### server is Fetching
    ///
    /// This indicates there must be exactly one thread running in `server.sync_file_system`, in its fetching phase.
    ///   - any other threads calling this function will return directly.
    ///   - after fetching finishes, it will set status to Building
    ///   - timeout may reach, then we rollback
    ///
    /// movement: Fetching -> Building
    /// failure: Fetching -> GcComplete
    ///
    /// ### server is Building
    ///
    /// The reason why we have this status is to prevent requesting outdated data.
    /// There should be exactly one thread start running [replace phase] and might be other threads trying to call `gc_files`
    ///   - if the building phase is finished, the thread will set status to Active, and other threads may run [clear phase]
    ///   - if the building phase is interrupted, the thread will quit, and other threads will skip the clear phase, try to fetch directly.
    ///
    /// movements: Building -> Active, Building -> GcComplete
    ///
    pub fn update_files(&self, conn_id: i32) -> Result<bool, std::io::Error> {
        self.gc_files();
        self.sync_file_system(conn_id)
    }

    pub fn recv(&self, conn_id: i32, clip_file: ClipboardFile) {
        self.dispatcher.recv(conn_id, clip_file)
    }

    /// allocate a new file descriptor
    fn alloc_fd(&self) -> u64 {
        self.file_handle_counter.fetch_add(1, Ordering::Relaxed)
    }

    /// find a file by name
    fn find_inode_by_name(&self, name: &str) -> Option<Inode> {
        if name == "/" {
            return Some(1);
        }
        let read = self.files.read();
        return read
            .iter()
            .position(|f| f.name == name)
            .map(|i| i as Inode + 1);
    }

    // synchronize metadata with remote
    fn sync_node_size(&self, node: &mut FuseNode) -> Result<(), std::io::Error> {
        log::debug!(
            "syncing metadata for {:?} on stream: {}",
            node.name,
            node.stream_id
        );

        let request = ClipboardFile::FileContentsRequest {
            stream_id: node.stream_id,
            list_index: node.inode as i32 - 2, // list index at least 2
            dw_flags: 1,

            n_position_low: 0,
            n_position_high: 0,
            cb_requested: 8,
            have_clip_data_id: false,
            clip_data_id: 0,
        };

        let rx = self.dispatcher.send(node.conn_id, request);

        log::debug!(
            "waiting for metadata sync reply for {:?} on channel {}",
            node.name,
            node.conn_id
        );

        let reply = rx.recv_timeout(self.timeout)?;

        log::debug!(
            "got metadata sync reply for {:?} on channel {}",
            node.name,
            node.conn_id
        );

        let size = match reply {
            ClipboardFile::FileContentsResponse {
                msg_flags,
                stream_id,
                requested_data,
            } => {
                if stream_id != node.stream_id {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "stream id mismatch",
                    ));
                }
                if msg_flags & 1 == 0 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "failure request",
                    ));
                }
                if requested_data.len() != 8 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "invalid data length",
                    ));
                }
                let little_endian_value = u64::from_le_bytes(requested_data.try_into().unwrap());
                little_endian_value
            }
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "invalid reply",
                ));
            }
        };
        log::debug!("got metadata sync reply for {:?}: size {}", node.name, size);

        node.attributes.size = size;
        Ok(())
    }

    pub fn read_node(
        &self,
        node: &FuseNode,
        offset: i64,
        size: u32,
    ) -> Result<Vec<u8>, std::io::Error> {
        // todo: async and concurrent read, generate stream_id per request
        log::debug!(
            "reading {:?} offset {} size {} on stream: {}",
            node.name,
            offset,
            size,
            node.stream_id
        );

        let cb_requested = unsafe {
            // convert `size` from u32 to i32
            // yet with same bit representation
            std::mem::transmute::<u32, i32>(size)
        };

        let (n_position_high, n_position_low) =
            ((offset >> 32) as i32, (offset & (u32::MAX as i64)) as i32);
        let request = ClipboardFile::FileContentsRequest {
            stream_id: node.stream_id,
            list_index: node.inode as i32 - 2,
            dw_flags: 2,
            n_position_low,
            n_position_high,
            cb_requested,
            have_clip_data_id: false,
            clip_data_id: 0,
        };

        let rx = self.dispatcher.send(node.conn_id, request);

        log::debug!(
            "waiting for read reply for {:?} on stream: {}",
            node.name,
            node.stream_id
        );

        let reply = rx.recv_timeout(self.timeout)?;

        match reply {
            ClipboardFile::FileContentsResponse {
                msg_flags,
                stream_id,
                requested_data,
            } => {
                if stream_id != node.stream_id {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "stream id mismatch",
                    ));
                }
                if msg_flags & 1 == 0 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "failure request",
                    ));
                }
                Ok(requested_data)
            }
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "invalid reply",
                ));
            }
        }
    }
}

impl fuser::Filesystem for FuseServer {
    fn init(
        &mut self,
        _req: &fuser::Request<'_>,
        _config: &mut fuser::KernelConfig,
    ) -> Result<(), libc::c_int> {
        log::debug!("init fuse server");

        let mut w_guard = self.files.write();
        if w_guard.is_empty() {
            // create a root file
            let root = FuseNode::new_root();
            w_guard.push(root);
        }
        Ok(())
    }

    fn lookup(
        &mut self,
        req: &Request,
        parent: u64,
        name: &std::ffi::OsStr,
        reply: fuser::ReplyEntry,
    ) {
        log::debug!("lookup: parent={}, name={:?}", parent, name);
        if name.len() > MAX_NAME_LEN {
            log::debug!("fuse: name too long");
            reply.error(libc::ENAMETOOLONG);
            return;
        }

        let entries = self.files.read();

        let generation = self.generation.load(Ordering::Relaxed);

        let parent_entry = match entries.get(parent as usize - 1) {
            Some(f) => f,
            None => {
                log::error!("fuse: parent not found");
                reply.error(libc::ENOENT);
                return;
            }
        };

        if parent_entry.attributes.kind != FileType::Directory {
            log::error!("fuse: parent is not a directory");

            reply.error(libc::ENOTDIR);
            return;
        }

        let children_inodes = &parent_entry.children;

        for inode in children_inodes.iter().copied() {
            let child = &entries[inode as usize - 1];
            if &child.name == &name.to_string_lossy() {
                let ttl = std::time::Duration::new(0, 0);
                reply.entry(&ttl, &(&child.attributes).into(), generation);
                log::debug!("fuse: found child");
                return;
            }
        }
        // error
        reply.error(libc::ENOENT);
        log::debug!("fuse: child not found");
        return;
    }

    fn opendir(&mut self, _req: &Request<'_>, ino: u64, flags: i32, reply: fuser::ReplyOpen) {
        log::debug!("opendir: ino={}, flags={}", ino, flags);

        let files = self.files.read();
        let Some(entry) = files.get(ino as usize - 1) else {
            reply.error(libc::ENOENT);
            log::error!("fuse: opendir: entry not found");
            return;
        };
        if entry.attributes.kind != FileType::Directory {
            reply.error(libc::ENOTDIR);
            log::error!("fuse: opendir: entry is not a directory");
            return;
        }
        // in gc, deny open
        if entry.marked() {
            log::error!("fuse: opendir: entry is in gc");
            reply.error(libc::EBUSY);
            return;
        }
        if flags & libc::O_RDONLY == 0 {
            log::error!("fuse: entry is read only");
            reply.error(libc::EACCES);
            return;
        }

        let fh = self.alloc_fd();
        entry.add_handler(fh);
        reply.opened(fh, 0);
        return;
    }

    fn readdir(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        log::debug!("readdir: ino={}, fh={}, offset={}", ino, fh, offset);

        let files = self.files.read();
        let Some(entry) = files.get(ino as usize - 1) else {
            reply.error(libc::ENOENT);
            log::error!("fuse: readdir: entry not found");
            return;
        };
        if !entry.have_handler(fh) {
            reply.error(libc::EBADF);
            log::error!("fuse: readdir: entry has no such handler");
            return;
        }
        if entry.attributes.kind != FileType::Directory {
            reply.error(libc::ENOTDIR);
            log::error!("fuse: readdir: entry is not a directory");
            return;
        }

        let mut offset = offset as usize;
        let mut entries = Vec::new();

        let self_entry = (ino, FileType::Directory, OsString::from("."));
        entries.push(self_entry);

        if let Some(parent_inode) = entry.parent {
            entries.push((parent_inode, FileType::Directory, OsString::from("..")));
        }

        for inode in entry.children.iter().copied() {
            let child = &files[inode as usize - 1];
            let kind = child.attributes.kind;
            let name = OsString::from(&child.name);
            let child_entry = (inode, kind, name.to_owned());
            entries.push(child_entry);
        }

        for (i, entry) in entries.into_iter().enumerate().skip(offset) {
            if reply.add(entry.0, i as i64 + 1, entry.1.into(), entry.2) {
                break;
            }
        }

        reply.ok();
        return;
    }

    fn releasedir(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        fh: u64,
        flags: i32,
        reply: fuser::ReplyEmpty,
    ) {
        let files = self.files.read();
        let Some(entry) = files.get(ino as usize - 1) else {
            reply.error(libc::ENOENT);
            log::error!("fuse: releasedir: entry not found");
            return;
        };
        if entry.attributes.kind != FileType::Directory {
            reply.error(libc::ENOTDIR);
            log::error!("fuse: releasedir: entry is not a directory");
            return;
        }
        if !entry.have_handler(fh) {
            reply.error(libc::EBADF);
            log::error!("fuse: releasedir: entry has no such handler");
            return;
        }

        entry.unregister_handler(fh);
        reply.ok();
        return;
    }

    fn open(&mut self, _req: &Request<'_>, ino: u64, flags: i32, reply: fuser::ReplyOpen) {
        let files = self.files.read();
        let Some(entry) = files.get(ino as usize - 1) else {
            reply.error(libc::ENOENT);
            log::error!("fuse: open: entry not found");
            return;
        };

        // todo: support link file
        if entry.attributes.kind != FileType::File {
            reply.error(libc::ENFILE);
            log::error!("fuse: open: entry is not a file");
            return;
        }
        // check flags
        if flags & libc::O_RDONLY == 0 {
            reply.error(libc::EACCES);
            log::error!("fuse: open: entry is read only");
            return;
        }
        // check gc
        if entry.marked() {
            reply.error(libc::EBUSY);
            log::error!("fuse: open: entry is in gc");
            return;
        }

        let fh = self.alloc_fd();
        entry.add_handler(fh);
        reply.opened(fh, 0);
        return;
    }

    fn read(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        fh: u64,
        offset: i64,
        size: u32,
        flags: i32,
        lock_owner: Option<u64>,
        reply: fuser::ReplyData,
    ) {
        let files = self.files.read();
        let Some(entry) = files.get(ino as usize - 1) else {
            reply.error(libc::ENOENT);
            log::error!("fuse: read: entry not found");
            return;
        };
        if !entry.have_handler(fh) {
            reply.error(libc::EBADF);
            log::error!("fuse: read: entry has no such handler");
            return;
        }
        if entry.attributes.kind != FileType::File {
            reply.error(libc::ENFILE);
            log::error!("fuse: read: entry is not a file");
            return;
        }
        // check flags
        if flags & libc::O_RDONLY == 0 {
            reply.error(libc::EACCES);
            log::error!("fuse: read: entry is read only");
            return;
        }

        if entry.marked() {
            reply.error(libc::EBUSY);
            log::error!("fuse: read: entry is in gc");
            return;
        }

        let bytes = match self.read_node(entry, offset, size) {
            Ok(b) => b,
            Err(e) => {
                log::error!("failed to read entry: {:?}", e);
                reply.error(libc::EIO);
                return;
            }
        };

        reply.data(bytes.as_slice());
    }

    fn release(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        fh: u64,
        _flags: i32,
        _lock_owner: Option<u64>,
        _flush: bool,
        reply: fuser::ReplyEmpty,
    ) {
        let files = self.files.read();
        let Some(entry) = files.get(ino as usize - 1) else {
            reply.error(libc::ENOENT);
            log::error!("fuse: release: entry not found");
            return;
        };

        if let Err(_) = entry.unregister_handler(fh) {
            reply.error(libc::EBADF);
            log::error!("fuse: release: entry has no such handler");
            return;
        }
        reply.ok();
        return;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDescription {
    pub conn_id: i32,
    pub name: PathBuf,
    pub kind: FileType,
    pub atime: SystemTime,
    pub last_modified: SystemTime,
    pub last_metadata_changed: SystemTime,
    pub creation_time: SystemTime,

    pub size: u64,

    pub perm: u16,
}

impl FileDescription {
    pub fn new(name: &str, kind: FileType, size: u64, conn_id: i32) -> Self {
        Self {
            conn_id,
            size,
            name: PathBuf::from(name),
            kind,
            atime: SystemTime::now(),
            last_modified: SystemTime::now(),
            last_metadata_changed: SystemTime::now(),
            creation_time: SystemTime::now(),
            perm: PERM_READ,
        }
    }
    fn parse_file_descriptor(
        bytes: &mut Bytes,
        conn_id: i32,
    ) -> Result<FileDescription, std::io::Error> {
        // begin of epoch used by microsoft
        // 1601-01-01 00:00:00 + LDAP_EPOCH_DELTA*(100 ns) = 1970-01-01 00:00:00
        const LDAP_EPOCH_DELTA: u64 = 116444772610000000;
        let flags = bytes.get_u32_le();
        // skip reserved 32 bytes
        bytes.advance(32);
        let attributes = bytes.get_u32_le();
        // skip reserverd 16 bytes
        bytes.advance(16);
        // last write time from 1601-01-01 00:00:00, in 100ns
        let last_write_time = bytes.get_u64_le();
        // file size
        let file_size_high = bytes.get_u32_le();
        let file_size_low = bytes.get_u32_le();
        // utf16 file name, double \0 terminated, in 520 bytes block
        // read with another pointer, and advance the main pointer
        let block = bytes.clone();
        bytes.advance(520);

        let block = &block[..520];
        let wstr = WStr::from_utf16le(block)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let valid_attributes = flags & 0x01 != 0;
        if !valid_attributes {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "only valid attributes are supported",
            ));
        }

        // todo: check normal, hidden, system, readonly, archive...
        let directory = attributes & 0x10 != 0;

        let kind = if directory {
            FileType::Directory
        } else {
            FileType::File
        };

        let valid_size = flags & 0x80 != 0;
        let size = if valid_size {
            ((file_size_high as u64) << 32) + file_size_low as u64
        } else {
            0
        };

        let valid_write_time = flags & 0x100 != 0;
        let last_modified = if valid_write_time && last_write_time >= LDAP_EPOCH_DELTA {
            let last_write_time = (last_write_time - LDAP_EPOCH_DELTA) * 100;
            let last_write_time = Duration::from_nanos(last_write_time);
            SystemTime::UNIX_EPOCH + last_write_time
        } else {
            SystemTime::UNIX_EPOCH
        };

        let name = wstr.to_utf8().replace('\\', "/");
        let name = PathBuf::from(name);

        let desc = FileDescription {
            conn_id,
            name,
            kind,
            atime: last_modified,
            last_modified,
            last_metadata_changed: last_modified,

            creation_time: last_modified,
            size,
            perm: PERM_READ,
        };

        Ok(desc)
    }

    /// parse file descriptions from a format data response PDU
    /// which containing a CSPTR_FILEDESCRIPTORW indicated format data
    pub fn parse_file_descriptors(
        file_descriptor_pdu: Vec<u8>,
        conn_id: i32,
    ) -> Result<Vec<Self>, std::io::Error> {
        let mut data = Bytes::from(file_descriptor_pdu);
        if data.remaining() < 4 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid file descriptor pdu",
            ));
        }

        let count = data.get_u32_le() as usize;
        if data.remaining() == 0 && count == 0 {
            return Ok(Vec::new());
        }

        if data.remaining() != 592 * count {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid file descriptor pdu",
            ));
        }

        let mut files = Vec::with_capacity(count);
        for _ in 0..count {
            let desc = Self::parse_file_descriptor(&mut data, conn_id)?;
            files.push(desc);
        }

        Ok(files)
    }
}

/// a node in the FUSE file tree
#[derive(Debug)]
struct FuseNode {
    /// connection id
    pub conn_id: i32,

    // todo: use stream_id to identify a FileContents request-reply
    // instead of a whole file
    /// stream id
    pub stream_id: i32,

    pub inode: u64,

    /// parent inode
    pub parent: Option<u64>,

    /// file name
    pub name: String,
    /// file attributes
    pub attributes: InodeAttributes,
    /// children inodes
    pub children: Vec<Inode>,

    /// marked gc
    pub file_handlers: FileHandles,
}

impl FuseNode {
    pub fn new(name: &str, inode: Inode, attributes: InodeAttributes, conn_id: i32) -> Self {
        Self {
            conn_id,
            stream_id: rand::random(),
            inode,
            name: name.to_owned(),
            parent: None,
            attributes,
            children: Vec::new(),
            file_handlers: FileHandles::new(),
        }
    }

    pub fn from_description(inode: Inode, desc: FileDescription) -> Self {
        Self {
            conn_id: desc.conn_id,
            stream_id: rand::random(),
            inode,
            name: desc.name.to_str().unwrap().to_owned(),
            parent: None,
            attributes: InodeAttributes::from_description(inode, desc),
            children: Vec::new(),
            file_handlers: FileHandles::new(),
        }
    }

    pub fn new_root() -> Self {
        Self {
            conn_id: 0,
            stream_id: rand::random(),
            inode: 1,
            name: String::from("/"),
            parent: None,
            attributes: InodeAttributes::new_root(),
            children: Vec::new(),
            file_handlers: FileHandles::new(),
        }
    }

    pub fn is_file(&self) -> bool {
        self.attributes.kind == FileType::File
    }

    pub fn is_dir(&self) -> bool {
        self.attributes.kind == FileType::Directory
    }

    pub fn marked(&self) -> bool {
        self.file_handlers.marked()
    }

    /// mark all files to be deleted
    pub fn gc(&mut self) {
        self.file_handlers.mark_and_wait()
    }

    pub fn add_handler(&self, fh: u64) {
        self.file_handlers.add_handler(fh)
    }

    pub fn unregister_handler(&self, fh: u64) -> Result<(), std::io::Error> {
        self.file_handlers.unregister(fh)
    }

    pub fn have_handler(&self, fh: u64) -> bool {
        self.file_handlers.have_handler(fh)
    }

    /// add a child inode
    fn add_child(&mut self, inode: Inode) {
        self.children.push(inode);
    }

    /// calculate the file tree from a pre-ordered file list
    /// ## implement detail:
    /// - a new root entry will be prepended to the list
    /// - all file names will be trimed to the last component
    pub fn build_tree(files: Vec<FileDescription>) -> Result<Vec<Self>, std::io::Error> {
        let mut tree_list = Vec::with_capacity(files.len() + 1);
        let root = Self::new_root();
        tree_list.push(root);
        // build the tree first
        // root map, name -> inode
        let mut sub_root_map = HashMap::new();
        sub_root_map.insert(Path::new("/").to_path_buf(), FUSE_ROOT_ID);
        sub_root_map.insert(Path::new("").to_path_buf(), FUSE_ROOT_ID);

        for (inode, mut file) in files
            .into_iter()
            .enumerate()
            .map(|(i, f)| (i as u64 + 2, f))
        {
            let FileDescription { name, .. } = file.clone();

            let parent_inode = match name.parent() {
                Some(parent) => sub_root_map.get(parent).cloned().ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("parent path {} not found", parent.display()),
                    )
                })?,
                None => {
                    // parent should be root
                    FUSE_ROOT_ID
                }
            };

            tree_list[parent_inode as usize - 1].add_child(inode);

            if file.kind == FileType::Directory {
                sub_root_map.insert(name.clone(), inode);
            }

            let base_name = name.file_name().ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("invalid file name {}", name.display()),
                )
            })?;
            file.name = Path::new(base_name).to_path_buf();

            let mut fuse_node = FuseNode::from_description(inode, file);
            fuse_node.parent = Some(parent_inode);

            tree_list.push(fuse_node)
        }
        Ok(tree_list)
    }
}

pub type Inode = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    File,
    Directory,
    // todo: support symlink
    Symlink,
}

impl From<FileType> for fuser::FileType {
    fn from(value: FileType) -> Self {
        match value {
            FileType::File => Self::RegularFile,
            FileType::Directory => Self::Directory,
            FileType::Symlink => Self::Symlink,
        }
    }
}

#[derive(Debug, Clone)]
pub struct InodeAttributes {
    inode: Inode,
    size: u64,
    // file reference meta
    // should be the only mutable field in this struct
    last_accessed: std::time::SystemTime,
    last_modified: std::time::SystemTime,
    last_metadata_changed: std::time::SystemTime,
    creation_time: std::time::SystemTime,
    kind: FileType,

    // not implemented
    xattrs: BTreeMap<Vec<u8>, Vec<u8>>,
}

impl InodeAttributes {
    pub fn new(inode: u64, size: u64, kind: FileType) -> Self {
        Self {
            inode,
            size,
            last_accessed: std::time::SystemTime::now(),
            last_modified: std::time::SystemTime::now(),
            last_metadata_changed: std::time::SystemTime::now(),
            creation_time: std::time::SystemTime::now(),
            kind,
            xattrs: BTreeMap::new(),
        }
    }

    pub fn from_description(inode: u64, desc: FileDescription) -> Self {
        Self {
            inode,
            size: desc.size,
            last_modified: desc.last_modified,
            last_metadata_changed: desc.last_metadata_changed,
            creation_time: desc.creation_time,
            last_accessed: SystemTime::now(),
            kind: desc.kind,

            xattrs: BTreeMap::new(),
        }
    }

    pub fn new_root() -> Self {
        Self::new(FUSE_ROOT_ID, 0, FileType::Directory)
    }

    pub fn access(&mut self) {
        self.last_accessed = std::time::SystemTime::now();
    }
}

impl From<&InodeAttributes> for fuser::FileAttr {
    fn from(value: &InodeAttributes) -> Self {
        Self {
            ino: value.inode,
            size: value.size,
            blocks: value.size.div_ceil(BLOCK_SIZE as u64),
            atime: value.last_accessed,
            mtime: value.last_modified,
            ctime: value.last_metadata_changed,
            crtime: value.creation_time,
            kind: value.kind.into(),

            // read only
            perm: PERM_READ,

            nlink: 1,
            // set to current user
            uid: unsafe { libc::getuid() },
            // set to current user
            gid: unsafe { libc::getgid() },
            rdev: 0,
            blksize: BLOCK_SIZE,
            // todo: support macos flags
            flags: 0,
        }
    }
}

#[derive(Debug)]
struct FileHandles {
    waiter: Condvar,
    handlers: Mutex<Vec<u64>>,
    gc: AtomicBool,
}

impl FileHandles {
    pub fn new() -> Self {
        Self {
            waiter: Condvar::new(),
            // the vector in handlers is sorted, from small to big
            // prove:
            // - later allocated handler will be bigger than previous ones
            // - new handlers will append to the end of the vector
            // - dropping old handlers won't affect the ordering
            handlers: Mutex::new(Vec::new()),
            gc: AtomicBool::new(false),
        }
    }

    pub fn add_handler(&self, fh: u64) {
        if self.marked() {
            panic!("adding new handler to a marked ref counter");
        }
        self.handlers.lock().push(fh);
    }

    // wait till gc completes
    pub fn mark_and_wait(&self) {
        let mut handlers = self.handlers.lock();
        self.gc.store(true, Ordering::Relaxed);
        loop {
            if handlers.is_empty() {
                return;
            }
            self.waiter.wait(&mut handlers);
        }
    }

    pub fn marked(&self) -> bool {
        self.gc.load(Ordering::Relaxed)
    }

    pub fn have_handler(&self, handler: u64) -> bool {
        let handlers = self.handlers.lock();
        handlers.binary_search(&handler).is_ok()
    }

    pub fn unregister(&self, handler: u64) -> Result<(), std::io::Error> {
        let mut handlers = self.handlers.lock();

        let Ok(idx) = handlers.binary_search(&handler) else {
            let e = std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid handler");
            return Err(e);
        };

        handlers.remove(idx);
        self.waiter.notify_all();
        Ok(())
    }
}

#[cfg(test)]
mod fuse_test {
    use super::*;

    // todo: more tests needed!

    fn generate_descriptions() -> Vec<FileDescription> {
        let folder0 = FileDescription::new("folder0", FileType::Directory, 0, 0);
        let file0 = FileDescription::new("folder0/file0", FileType::File, 1, 0);
        let file1 = FileDescription::new("folder0/file1", FileType::File, 1, 0);
        let folder1 = FileDescription::new("folder1", FileType::Directory, 0, 0);
        let file2 = FileDescription::new("folder1/file2", FileType::File, 4, 0);

        vec![folder0, file0, file1, folder1, file2]
    }

    #[test]
    fn build_tree() {
        // Tree:
        //  - folder0
        //      - file0
        //      - file1
        //  - folder1
        //      - file2
        let source_list = generate_descriptions();

        let build_res = FuseNode::build_tree(source_list);
        assert!(build_res.is_ok());
        // expected tree:
        // - /
        //  - folder0
        //      - file0
        //      - file1
        //  - folder1
        //      - file2
        let tree_list = build_res.unwrap();
        assert_eq!(tree_list.len(), 6);

        assert_eq!(tree_list[0].name, "/"); // inode 1
        assert_eq!(tree_list[0].children, vec![2, 5]);

        assert_eq!(tree_list[1].name, "folder0"); // inode 2
        assert_eq!(tree_list[1].children, vec![3, 4]);

        assert_eq!(tree_list[2].name, "file0"); // inode 3
        assert_eq!(tree_list[2].children, vec![]);

        assert_eq!(tree_list[3].name, "file1"); // inode 4
        assert_eq!(tree_list[3].children, vec![]);

        assert_eq!(tree_list[4].name, "folder1"); // inode 5
        assert_eq!(tree_list[4].children, vec![6]);

        assert_eq!(tree_list[5].name, "file2"); // inode 6
        assert_eq!(tree_list[5].children, vec![]);
    }
}
