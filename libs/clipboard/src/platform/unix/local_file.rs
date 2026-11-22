use super::{BLOCK_SIZE, LDAP_EPOCH_DELTA};
use crate::{
    platform::unix::{
        FLAGS_FD_ATTRIBUTES, FLAGS_FD_LAST_WRITE, FLAGS_FD_PROGRESSUI, FLAGS_FD_SIZE,
        FLAGS_FD_UNIX_MODE,
    },
    CliprdrError,
};
use hbb_common::{
    bytes::{BufMut, BytesMut},
    log,
};
use std::{
    collections::HashSet,
    fs::File,
    io::{BufRead, BufReader, Read, Seek},
    os::unix::prelude::PermissionsExt,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::SystemTime,
};
use utf16string::WString;

#[derive(Debug)]
pub(super) struct LocalFile {
    pub relative_root: PathBuf,
    pub path: PathBuf,

    pub handle: Option<BufReader<File>>,
    pub offset: AtomicU64,

    pub name: String,
    pub size: u64,
    pub last_write_time: SystemTime,
    pub is_dir: bool,
    pub perm: u32,
    pub read_only: bool,
    pub hidden: bool,
    pub system: bool,
    pub archive: bool,
    pub normal: bool,
}

impl LocalFile {
    pub fn try_open(relative_root: &Path, path: &Path) -> Result<Self, CliprdrError> {
        let mt = std::fs::metadata(path).map_err(|e| CliprdrError::FileError {
            path: path.to_string_lossy().to_string(),
            err: e,
        })?;
        let size = mt.len() as u64;
        let is_dir = mt.is_dir();
        let read_only = mt.permissions().readonly();
        let system = false;
        let hidden = path.to_string_lossy().starts_with('.');
        let archive = false;
        let normal = !(is_dir || read_only || system || hidden || archive);
        let last_write_time = mt.modified().unwrap_or(SystemTime::UNIX_EPOCH);

        let perm = mt.permissions().mode();

        let name = path
            .display()
            .to_string()
            .trim_start_matches('/')
            .replace('/', "\\");

        // NOTE: open files lazily
        let handle = None;
        let offset = AtomicU64::new(0);

        Ok(Self {
            name,
            relative_root: relative_root.to_path_buf(),
            path: path.to_path_buf(),
            handle,
            offset,
            size,
            last_write_time,
            is_dir,
            read_only,
            system,
            hidden,
            perm,
            archive,
            normal,
        })
    }
    pub fn as_bin(&self) -> Vec<u8> {
        let mut buf = BytesMut::with_capacity(592);

        let read_only_flag = if self.read_only { 0x1 } else { 0 };
        let hidden_flag = if self.hidden { 0x2 } else { 0 };
        let system_flag = if self.system { 0x4 } else { 0 };
        let directory_flag = if self.is_dir { 0x10 } else { 0 };
        let archive_flag = if self.archive { 0x20 } else { 0 };
        let normal_flag = if self.normal { 0x80 } else { 0 };

        let file_attributes: u32 = read_only_flag
            | hidden_flag
            | system_flag
            | directory_flag
            | archive_flag
            | normal_flag;

        let win32_time = self
            .last_write_time
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
            / 100
            + LDAP_EPOCH_DELTA;

        let size_high = (self.size >> 32) as u32;
        let size_low = (self.size & (u32::MAX as u64)) as u32;

        let path = self
            .path
            .strip_prefix(&self.relative_root)
            .unwrap_or(&self.path)
            .to_string_lossy()
            .into_owned();

        let wstr: WString<utf16string::LE> = WString::from(&path);
        let name = wstr.as_bytes();

        log::trace!(
            "put file to list: name_len {}, name {}",
            name.len(),
            &self.name
        );

        let flags = FLAGS_FD_SIZE
            | FLAGS_FD_LAST_WRITE
            | FLAGS_FD_ATTRIBUTES
            | FLAGS_FD_PROGRESSUI
            | FLAGS_FD_UNIX_MODE;

        // flags, 4 bytes
        buf.put_u32_le(flags);
        // 32 bytes reserved
        buf.put(&[0u8; 32][..]);
        // file attributes, 4 bytes
        buf.put_u32_le(file_attributes);

        // NOTE: this is not used in windows
        // in the specification, this is 16 bytes reserved
        // lets use the last 4 bytes to store the file mode
        //
        // 12 bytes reserved
        buf.put(&[0u8; 12][..]);
        // file permissions, 4 bytes
        buf.put_u32_le(self.perm);

        // last write time, 8 bytes
        buf.put_u64_le(win32_time);
        // file size (high)
        buf.put_u32_le(size_high);
        // file size (low)
        buf.put_u32_le(size_low);
        // put name and padding to 520 bytes
        let name_len = name.len();
        buf.put(name);
        buf.put(&vec![0u8; 520 - name_len][..]);

        buf.to_vec()
    }

    #[inline]
    pub fn load_handle(&mut self) -> Result<(), CliprdrError> {
        if !self.is_dir && self.handle.is_none() {
            let handle = std::fs::File::open(&self.path).map_err(|e| CliprdrError::FileError {
                path: self.path.to_string_lossy().to_string(),
                err: e,
            })?;
            let mut reader = BufReader::with_capacity(BLOCK_SIZE as usize * 2, handle);
            reader.fill_buf().map_err(|e| CliprdrError::FileError {
                path: self.path.to_string_lossy().to_string(),
                err: e,
            })?;
            self.handle = Some(reader);
        };
        Ok(())
    }

    pub fn read_exact_at(&mut self, buf: &mut [u8], offset: u64) -> Result<(), CliprdrError> {
        self.load_handle()?;

        let Some(handle) = self.handle.as_mut() else {
            return Err(CliprdrError::FileError {
                path: self.path.to_string_lossy().to_string(),
                err: std::io::Error::new(std::io::ErrorKind::NotFound, "file handle not found"),
            });
        };

        if offset != self.offset.load(Ordering::Relaxed) {
            handle
                .seek(std::io::SeekFrom::Start(offset))
                .map_err(|e| CliprdrError::FileError {
                    path: self.path.to_string_lossy().to_string(),
                    err: e,
                })?;
        }
        handle
            .read_exact(buf)
            .map_err(|e| CliprdrError::FileError {
                path: self.path.to_string_lossy().to_string(),
                err: e,
            })?;
        let new_offset = offset + (buf.len() as u64);
        self.offset.store(new_offset, Ordering::Relaxed);

        // gc file handle
        if new_offset >= self.size {
            self.offset.store(0, Ordering::Relaxed);
            self.handle = None;
        }

        Ok(())
    }
}

pub(super) fn construct_file_list(paths: &[PathBuf]) -> Result<Vec<LocalFile>, CliprdrError> {
    fn constr_file_lst(
        relative_root: &Path,
        path: &Path,
        file_list: &mut Vec<LocalFile>,
        visited: &mut HashSet<PathBuf>,
    ) -> Result<(), CliprdrError> {
        // prevent fs loop
        if visited.contains(path) {
            return Ok(());
        }
        visited.insert(path.to_path_buf());

        let local_file = LocalFile::try_open(relative_root, path)?;
        file_list.push(local_file);

        let mt = std::fs::metadata(path).map_err(|e| CliprdrError::FileError {
            path: path.to_string_lossy().to_string(),
            err: e,
        })?;

        if mt.is_dir() {
            let dir = std::fs::read_dir(path).map_err(|e| CliprdrError::FileError {
                path: path.to_string_lossy().to_string(),
                err: e,
            })?;
            for entry in dir {
                let entry = entry.map_err(|e| CliprdrError::FileError {
                    path: path.to_string_lossy().to_string(),
                    err: e,
                })?;
                let path = entry.path();
                constr_file_lst(relative_root, &path, file_list, visited)?;
            }
        }
        Ok(())
    }

    let mut file_list = Vec::new();
    let mut visited = HashSet::new();

    let relative_root = paths
        .first()
        .ok_or(CliprdrError::InvalidRequest {
            description: "empty file list".to_string(),
        })?
        .parent()
        .ok_or(CliprdrError::InvalidRequest {
            description: "empty parent".to_string(),
        })?
        .to_path_buf();
    for path in paths {
        constr_file_lst(&relative_root, path, &mut file_list, &mut visited)?;
    }
    Ok(file_list)
}

#[cfg(test)]
mod file_list_test {
    use std::{path::PathBuf, sync::atomic::AtomicU64};

    use hbb_common::bytes::{BufMut, BytesMut};

    use crate::{platform::unix::filetype::FileDescription, CliprdrError};

    use super::LocalFile;

    #[inline]
    fn generate_tree(prefix: &str) -> Vec<LocalFile> {
        // generate a tree of local files, no handles
        // - /
        // |- a.txt
        // |- b
        //    |- c.txt
        #[inline]
        fn generate_file(path: &str, name: &str, is_dir: bool) -> LocalFile {
            LocalFile {
                relative_root: PathBuf::from("."),
                path: PathBuf::from(path),
                handle: None,
                name: name.to_string(),
                size: 0,
                offset: AtomicU64::new(0),
                last_write_time: std::time::SystemTime::UNIX_EPOCH,
                read_only: false,
                is_dir,
                perm: 0o754,
                hidden: false,
                system: false,
                archive: false,
                normal: false,
            }
        }

        let p = prefix;

        let (r_path, a_path, b_path, c_path) = if !prefix.is_empty() {
            (
                p.to_string(),
                format!("{}/a.txt", p),
                format!("{}/b", p),
                format!("{}/b/c.txt", p),
            )
        } else {
            (
                ".".to_string(),
                "a.txt".to_string(),
                "b".to_string(),
                "b/c.txt".to_string(),
            )
        };

        let root = generate_file(&r_path, ".", true);
        let a = generate_file(&a_path, "a.txt", false);
        let b = generate_file(&b_path, "b", true);
        let c = generate_file(&c_path, "c.txt", false);

        vec![root, a, b, c]
    }

    fn as_bin_parse_test(prefix: &str) -> Result<(), CliprdrError> {
        let tree = generate_tree(prefix);
        let mut pdu = BytesMut::with_capacity(4 + 592 * tree.len());
        pdu.put_u32_le(tree.len() as u32);
        for file in tree {
            pdu.put(file.as_bin().as_slice());
        }

        let parsed = FileDescription::parse_file_descriptors(pdu.to_vec(), 0)?;
        assert_eq!(parsed.len(), 4);

        if !prefix.is_empty() {
            assert_eq!(parsed[0].name.to_str().unwrap(), format!("{}", prefix));
            assert_eq!(
                parsed[1].name.to_str().unwrap(),
                format!("{}/a.txt", prefix)
            );
            assert_eq!(parsed[2].name.to_str().unwrap(), format!("{}/b", prefix));
            assert_eq!(
                parsed[3].name.to_str().unwrap(),
                format!("{}/b/c.txt", prefix)
            );
        } else {
            assert_eq!(parsed[0].name.to_str().unwrap(), ".");
            assert_eq!(parsed[1].name.to_str().unwrap(), "a.txt");
            assert_eq!(parsed[2].name.to_str().unwrap(), "b");
            assert_eq!(parsed[3].name.to_str().unwrap(), "b/c.txt");
        }

        assert!(parsed[0].perm & 0o777 == 0o754);
        assert!(parsed[1].perm & 0o777 == 0o754);
        assert!(parsed[2].perm & 0o777 == 0o754);
        assert!(parsed[3].perm & 0o777 == 0o754);

        Ok(())
    }

    #[test]
    fn test_parse_file_descriptors() -> Result<(), CliprdrError> {
        as_bin_parse_test("")?;
        as_bin_parse_test("/")?;
        as_bin_parse_test("test")?;
        as_bin_parse_test("/test")?;
        Ok(())
    }
}
