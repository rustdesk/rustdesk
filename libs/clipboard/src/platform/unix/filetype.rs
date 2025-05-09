use super::{FLAGS_FD_ATTRIBUTES, FLAGS_FD_LAST_WRITE, FLAGS_FD_UNIX_MODE, LDAP_EPOCH_DELTA};
use crate::CliprdrError;
use hbb_common::{
    bytes::{Buf, Bytes},
    log,
};
use serde_derive::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    time::{Duration, SystemTime},
};
use utf16string::WStr;

#[cfg(target_os = "linux")]
pub type Inode = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileType {
    File,
    Directory,
    // todo: support symlink
    Symlink,
}

/// read only permission
pub const PERM_READ: u16 = 0o444;
/// read and write permission
pub const PERM_RW: u16 = 0o644;
/// only self can read and readonly
pub const PERM_SELF_RO: u16 = 0o400;
/// rwx
pub const PERM_RWX: u16 = 0o755;
#[allow(dead_code)]
/// max length of file name
pub const MAX_NAME_LEN: usize = 255;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    fn parse_file_descriptor(
        bytes: &mut Bytes,
        conn_id: i32,
    ) -> Result<FileDescription, CliprdrError> {
        let flags = bytes.get_u32_le();
        // skip reserved 32 bytes
        bytes.advance(32);
        let attributes = bytes.get_u32_le();

        // in original specification, this is 16 bytes reserved
        // we use the last 4 bytes to store the file mode
        // skip reserved 12 bytes
        bytes.advance(12);
        let perm = bytes.get_u32_le() as u16;

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
        let wstr = WStr::from_utf16le(block).map_err(|e| {
            log::error!("cannot convert file descriptor path: {:?}", e);
            CliprdrError::ConversionFailure
        })?;

        let from_unix = flags & FLAGS_FD_UNIX_MODE != 0;

        let valid_attributes = flags & FLAGS_FD_ATTRIBUTES != 0;
        if !valid_attributes {
            return Err(CliprdrError::InvalidRequest {
                description: "file description must have valid attributes".to_string(),
            });
        }

        // todo: check normal, hidden, system, readonly, archive...
        let directory = attributes & 0x10 != 0;
        let normal = attributes == 0x80;
        let hidden = attributes & 0x02 != 0;
        let readonly = attributes & 0x01 != 0;

        let perm = if from_unix {
            // as is
            perm
            // cannot set as is...
        } else if normal {
            PERM_RWX
        } else if readonly {
            PERM_READ
        } else if hidden {
            PERM_SELF_RO
        } else if directory {
            PERM_RWX
        } else {
            PERM_RW
        };

        let kind = if directory {
            FileType::Directory
        } else {
            FileType::File
        };

        // to-do: use `let valid_size = flags & FLAGS_FD_SIZE != 0;`
        // We use `true` to for compatibility with Windows.
        // let valid_size = flags & FLAGS_FD_SIZE != 0;
        let valid_size = true;
        let size = if valid_size {
            ((file_size_high as u64) << 32) + file_size_low as u64
        } else {
            0
        };

        let valid_write_time = flags & FLAGS_FD_LAST_WRITE != 0;
        let last_modified = if valid_write_time && last_write_time >= LDAP_EPOCH_DELTA {
            let last_write_time = (last_write_time - LDAP_EPOCH_DELTA) * 100;
            let last_write_time = Duration::from_nanos(last_write_time);
            SystemTime::UNIX_EPOCH + last_write_time
        } else {
            SystemTime::UNIX_EPOCH
        };

        let name = wstr.to_utf8().replace('\\', "/");
        let name = PathBuf::from(name.trim_end_matches('\0'));

        let desc = FileDescription {
            conn_id,
            name,
            kind,
            atime: last_modified,
            last_modified,
            last_metadata_changed: last_modified,
            creation_time: last_modified,
            size,
            perm,
        };

        Ok(desc)
    }

    /// parse file descriptions from a format data response PDU
    /// which containing a CSPTR_FILEDESCRIPTORW indicated format data
    pub fn parse_file_descriptors(
        file_descriptor_pdu: Vec<u8>,
        conn_id: i32,
    ) -> Result<Vec<Self>, CliprdrError> {
        let mut data = Bytes::from(file_descriptor_pdu);
        if data.remaining() < 4 {
            return Err(CliprdrError::InvalidRequest {
                description: "file descriptor request with infficient length".to_string(),
            });
        }

        let count = data.get_u32_le() as usize;
        if data.remaining() == 0 && count == 0 {
            return Ok(Vec::new());
        }

        if data.remaining() != 592 * count {
            return Err(CliprdrError::InvalidRequest {
                description: "file descriptor request with invalid length".to_string(),
            });
        }

        let mut files = Vec::with_capacity(count);
        for _ in 0..count {
            let desc = Self::parse_file_descriptor(&mut data, conn_id)?;
            files.push(desc);
        }

        Ok(files)
    }
}
