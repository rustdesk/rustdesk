use std::{
    fs::{self},
    io::{Cursor, Read},
    path::PathBuf,
};

#[cfg(windows)]
const BIN_DATA: &[u8] = include_bytes!("../data.bin");
#[cfg(not(windows))]
const BIN_DATA: &[u8] = &[];
// 4bytes
const LENGTH: usize = 4;
const MD5_LENGTH: usize = 32;
const BUF_SIZE: usize = 4096;
const META_BEGIN: &str = "rustdesk";
const META_END: &str = "rustdesk";

pub(crate) struct BinaryData {
    pub md5_code: &'static [u8],
    // compressed gzip data
    pub raw: &'static [u8],
    pub path: String,
}

pub(crate) struct BinaryReader {
    pub files: Vec<BinaryData>,
}

impl Default for BinaryReader {
    fn default() -> Self {
        let files = BinaryReader::read();
        Self { files }
    }
}

impl BinaryData {
    fn decompress(&self) -> Vec<u8> {
        let cursor = Cursor::new(self.raw);
        let mut decoder = brotli::Decompressor::new(cursor, BUF_SIZE);
        let mut buf = Vec::new();
        decoder.read_to_end(&mut buf).ok();
        buf
    }

    pub fn write_to_file(&self, prefix: &PathBuf) {
        let p = prefix.join(&self.path);
        if let Some(parent) = p.parent() {
            if !parent.exists() {
                let _ = fs::create_dir_all(parent);
            }
        }
        if p.exists() {
            // check md5
            let f = fs::read(p.clone()).unwrap_or_default();
            let digest = format!("{:x}", md5::compute(&f));
            let md5_record = String::from_utf8_lossy(self.md5_code);
            if digest == md5_record {
                // same, skip this file
                println!("skip {}", &self.path);
                return;
            } else {
                println!("writing {}", p.display());
                println!("{} -> {}", md5_record, digest)
            }
        }
        let _ = fs::write(p, self.decompress());
    }
}

impl BinaryReader {
    #[inline]
    pub fn get_exe_md5() -> (String, String) {
        let mut base: usize = META_BEGIN.len();
        let len = Self::get_len(base);
        base += LENGTH;
        let exe = String::from_utf8_lossy(&BIN_DATA[base..base + len]).to_string();
        base += len;
        let md5 = String::from_utf8_lossy(&BIN_DATA[base..base + MD5_LENGTH]).to_string();
        (exe, md5)
    }

    #[inline]
    fn get_len(base: usize) -> usize {
        u32::from_be_bytes([
            BIN_DATA[base],
            BIN_DATA[base + 1],
            BIN_DATA[base + 2],
            BIN_DATA[base + 3],
        ]) as usize
    }

    fn read() -> Vec<BinaryData> {
        let mut base: usize = 0;
        let mut parsed = vec![];
        assert!(BIN_DATA.len() > META_BEGIN.len(), "bin data invalid!");
        let mut iden = String::from_utf8_lossy(&BIN_DATA[base..base + META_BEGIN.len()]);
        if iden != META_BEGIN {
            panic!("bin file is not valid!");
        }
        base += META_BEGIN.len();
        base += LENGTH + Self::get_len(base) + MD5_LENGTH;
        loop {
            iden = String::from_utf8_lossy(&BIN_DATA[base..base + META_END.len()]);
            if iden == META_END {
                break;
            }
            // start reading
            let mut offset = 0;
            let path_length = Self::get_len(base + offset);
            offset += LENGTH;
            let path =
                String::from_utf8_lossy(&BIN_DATA[base + offset..base + offset + path_length])
                    .to_string();
            offset += path_length;
            // file sz
            let file_length = Self::get_len(base + offset);
            offset += LENGTH;
            let raw = &BIN_DATA[base + offset..base + offset + file_length];
            offset += file_length;
            // md5
            let md5 = &BIN_DATA[base + offset..base + offset + MD5_LENGTH];
            offset += MD5_LENGTH;
            parsed.push(BinaryData {
                md5_code: md5,
                raw,
                path,
            });
            base += offset;
        }
        parsed
    }

    #[cfg(linux)]
    pub fn configure_permission(&self, prefix: &PathBuf) {
        use std::os::unix::prelude::PermissionsExt;

        let exe_path = prefix.join(&self.exe);
        if exe_path.exists() {
            if let Ok(f) = File::open(exe_path) {
                if let Ok(meta) = f.metadata() {
                    let mut permissions = meta.permissions();
                    permissions.set_mode(0o755);
                    f.set_permissions(permissions).ok();
                }
            }
        }
    }
}
