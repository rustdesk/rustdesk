use dashmap::DashMap;
use lazy_static::lazy_static;

mod filetype;
/// use FUSE for file pasting on these platforms
#[cfg(target_os = "linux")]
pub mod fuse;
pub mod local_file;
pub mod serv_files;

/// has valid file attributes
pub const FLAGS_FD_ATTRIBUTES: u32 = 0x04;
/// has valid file size
pub const FLAGS_FD_SIZE: u32 = 0x40;
/// has valid last write time
pub const FLAGS_FD_LAST_WRITE: u32 = 0x20;
/// show progress
pub const FLAGS_FD_PROGRESSUI: u32 = 0x4000;
/// transferred from unix, contains file mode
/// P.S. this flag is not used in windows
pub const FLAGS_FD_UNIX_MODE: u32 = 0x08;

// not actual format id, just a placeholder
pub const FILEDESCRIPTOR_FORMAT_ID: i32 = 49334;
pub const FILEDESCRIPTORW_FORMAT_NAME: &str = "FileGroupDescriptorW";
// not actual format id, just a placeholder
pub const FILECONTENTS_FORMAT_ID: i32 = 49267;
pub const FILECONTENTS_FORMAT_NAME: &str = "FileContents";

/// block size for fuse, align to our asynchronic request size over FileContentsRequest.
pub(crate) const BLOCK_SIZE: u32 = 4 * 1024 * 1024;

// begin of epoch used by microsoft
// 1601-01-01 00:00:00 + LDAP_EPOCH_DELTA*(100 ns) = 1970-01-01 00:00:00
const LDAP_EPOCH_DELTA: u64 = 116444772610000000;

lazy_static! {
    static ref REMOTE_FORMAT_MAP: DashMap<i32, String> = DashMap::from_iter(
        [
            (
                FILEDESCRIPTOR_FORMAT_ID,
                FILEDESCRIPTORW_FORMAT_NAME.to_string()
            ),
            (FILECONTENTS_FORMAT_ID, FILECONTENTS_FORMAT_NAME.to_string())
        ]
        .iter()
        .cloned()
    );
}

#[inline]
pub fn get_local_format(remote_id: i32) -> Option<String> {
    REMOTE_FORMAT_MAP.get(&remote_id).map(|s| s.clone())
}
