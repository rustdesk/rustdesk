use std::{
    collections::HashSet,
    fs::{self},
    io::{Cursor, Read},
    path::Path,
};

// The generic payload, shared by every customer and compiled in once per release.
#[cfg(windows)]
const BIN_DATA: &[u8] = include_bytes!("../data.bin");

// The per-customer payload, injected into the RCDATA resource after the template
// has been built, so that customizing a client needs no recompilation.
#[cfg(windows)]
const PACKAGE_RESOURCE_NAME: &str = "RDPKG";

// 4bytes
const LENGTH: usize = 4;
const IDENTIFIER: &[u8] = b"rustdesk";
const IDENTIFIER_LENGTH: usize = 8;
const MD5_LENGTH: usize = 32;
const BUF_SIZE: usize = 4096;

pub(crate) struct BinaryData {
    pub md5_code: &'static [u8],
    // compressed gzip data
    pub raw: &'static [u8],
    pub path: String,
}

pub(crate) struct BinaryReader {
    pub files: Vec<BinaryData>,
    pub exe: String,
    // Paths supplied by the per-customer package. Recorded so that a file dropped
    // from a later package -- a logo the customer removed, say -- can be deleted
    // from an existing extraction, which the timestamp wipe no longer covers now
    // that the packer is built once per release rather than once per customer.
    pub package_paths: Vec<String>,
}

impl BinaryReader {
    pub fn new() -> Result<Self, String> {
        let package = read_package()?;
        let package_paths = package.0.iter().map(|f| f.path.clone()).collect();
        let (files, exe) = merge(read_embedded()?, package);
        Ok(Self {
            files,
            exe,
            package_paths,
        })
    }
}

// Folds the per-customer package into the generic payload.
fn merge(
    embedded: (Vec<BinaryData>, String),
    package: (Vec<BinaryData>, String),
) -> (Vec<BinaryData>, String) {
    let (mut files, generic_exe) = embedded;
    let (package_files, package_exe) = package;

    let exe = if package_exe.is_empty() {
        generic_exe.clone()
    } else {
        package_exe
    };

    // The generic payload ships the executable under its stock name, the package
    // decides the final one. Rename on extraction so the process is always
    // `<appname>.exe`, which the app itself relies on to find its own sessions.
    if !generic_exe.is_empty() && normalize_path(&exe) != normalize_path(&generic_exe) {
        let generic_key = normalize_path(&generic_exe);
        for file in files.iter_mut() {
            if normalize_path(&file.path) == generic_key {
                file.path = exe.clone();
            }
        }
    }

    // Per-customer entries replace the generic ones they shadow.
    if !package_files.is_empty() {
        let overridden: HashSet<String> = package_files
            .iter()
            .map(|file| normalize_path(&file.path))
            .collect();
        files.retain(|file| !overridden.contains(&normalize_path(&file.path)));
        files.extend(package_files);
    }

    (files, exe)
}

pub(crate) fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
        .trim_start_matches("./")
        .to_lowercase()
}

fn read_u32(blob: &[u8], at: usize) -> Option<u32> {
    let bytes = blob.get(at..at + LENGTH)?;
    Some(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

// Returns the files and the executable to launch, or None if the blob is absent or malformed.
fn parse(blob: &'static [u8]) -> Option<(Vec<BinaryData>, String)> {
    let mut base = 0usize;
    let mut parsed = Vec::new();
    if blob.get(base..base + IDENTIFIER_LENGTH)? != IDENTIFIER {
        return None;
    }
    base += IDENTIFIER_LENGTH;
    loop {
        if blob.get(base..base + IDENTIFIER_LENGTH)? == IDENTIFIER {
            base += IDENTIFIER_LENGTH;
            break;
        }
        let path_length = read_u32(blob, base)? as usize;
        base += LENGTH;
        let path = std::str::from_utf8(blob.get(base..base + path_length)?)
            .ok()?
            .to_owned();
        base += path_length;
        let file_length = read_u32(blob, base)? as usize;
        base += LENGTH;
        let raw = blob.get(base..base + file_length)?;
        base += file_length;
        let md5_code = blob.get(base..base + MD5_LENGTH)?;
        base += MD5_LENGTH;
        parsed.push(BinaryData {
            md5_code,
            raw,
            path,
        });
    }
    let executable = std::str::from_utf8(blob.get(base..)?).ok()?.to_owned();
    Some((parsed, executable))
}

#[cfg(windows)]
fn read_embedded() -> Result<(Vec<BinaryData>, String), String> {
    parse(BIN_DATA).ok_or_else(|| "bin file is not valid!".to_owned())
}

#[cfg(not(windows))]
fn read_embedded() -> Result<(Vec<BinaryData>, String), String> {
    Ok(Default::default())
}

fn parse_package_blob(blob: Option<&'static [u8]>) -> Result<(Vec<BinaryData>, String), String> {
    let Some(blob) = blob else {
        return Ok(Default::default());
    };
    let package = parse(blob).ok_or_else(|| "RDPKG resource is invalid".to_owned())?;
    if package.1.trim().is_empty() {
        return Err("RDPKG resource has no executable".to_owned());
    }
    Ok(package)
}

#[cfg(windows)]
fn read_package() -> Result<(Vec<BinaryData>, String), String> {
    parse_package_blob(read_resource(PACKAGE_RESOURCE_NAME))
}

#[cfg(not(windows))]
fn read_package() -> Result<(Vec<BinaryData>, String), String> {
    Ok(Default::default())
}

// Reads an RCDATA resource out of the running image. Resources live in the mapped
// image for the lifetime of the process, so the slice is genuinely 'static and no
// copy is needed.
#[cfg(windows)]
fn read_resource(name: &str) -> Option<&'static [u8]> {
    use std::ptr::null_mut;
    use winapi::um::libloaderapi::{FindResourceW, LoadResource, LockResource, SizeofResource};

    // MAKEINTRESOURCEW(10), avoids depending on the winuser feature for RT_RCDATA.
    const RT_RCDATA: *const u16 = 10 as _;

    let name: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let info = FindResourceW(null_mut(), name.as_ptr(), RT_RCDATA);
        if info.is_null() {
            return None;
        }
        let size = SizeofResource(null_mut(), info) as usize;
        if size == 0 {
            return None;
        }
        let handle = LoadResource(null_mut(), info);
        if handle.is_null() {
            return None;
        }
        let data = LockResource(handle) as *const u8;
        if data.is_null() {
            return None;
        }
        Some(std::slice::from_raw_parts(data, size))
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

    pub fn write_to_file(&self, prefix: &Path) {
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
    #[cfg(linux)]
    pub fn configure_permission(&self, prefix: &Path) {
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

#[cfg(test)]
mod tests {
    use super::*;

    // Builds a blob in the same layout generate.py writes, so these tests pin the
    // cross-language format contract as well as the merge rules.
    fn blob(files: &[(&str, &[u8])], exe: &str) -> &'static [u8] {
        let mut out = Vec::new();
        out.extend_from_slice(IDENTIFIER);
        for (path, data) in files {
            out.extend_from_slice(&(path.len() as u32).to_be_bytes());
            out.extend_from_slice(path.as_bytes());
            out.extend_from_slice(&(data.len() as u32).to_be_bytes());
            out.extend_from_slice(data);
            out.extend_from_slice(&[b'a'; MD5_LENGTH]);
        }
        out.extend_from_slice(IDENTIFIER);
        out.extend_from_slice(exe.as_bytes());
        Box::leak(out.into_boxed_slice())
    }

    fn entry<'a>(files: &'a [BinaryData], path: &str) -> Option<&'a BinaryData> {
        files
            .iter()
            .find(|file| normalize_path(&file.path) == normalize_path(path))
    }

    #[test]
    fn parses_the_generate_py_layout() {
        let (files, exe) = parse(blob(
            &[("./rustdesk.exe", b"app"), ("./custom.txt", b"cfg")],
            "./rustdesk.exe",
        ))
        .unwrap();
        assert_eq!(exe, "./rustdesk.exe");
        assert_eq!(files.len(), 2);
        assert_eq!(entry(&files, "./custom.txt").unwrap().raw, b"cfg");
    }

    #[test]
    fn rejects_malformed_blobs() {
        assert!(parse(b"".as_slice()).is_none());
        assert!(parse(b"notrustd".as_slice()).is_none());
        // Truncated mid-record rather than panicking on a slice out of range.
        assert!(parse(b"rustdesk\x00\x00\x00\x40partial".as_slice()).is_none());
    }

    #[test]
    fn distinguishes_an_absent_package_from_a_malformed_one() {
        assert!(parse_package_blob(None).unwrap().0.is_empty());
        assert!(parse_package_blob(Some(b"damaged")).is_err());
        assert!(parse_package_blob(Some(blob(&[("./custom.txt", b"cfg")], ""))).is_err());
    }

    #[test]
    fn without_a_package_the_stock_payload_is_untouched() {
        let embedded = parse(blob(&[("./rustdesk.exe", b"app")], "./rustdesk.exe")).unwrap();
        let (files, exe) = merge(embedded, Default::default());
        assert_eq!(exe, "./rustdesk.exe");
        assert!(entry(&files, "./rustdesk.exe").is_some());
    }

    #[test]
    fn renames_the_stock_executable_to_the_package_name() {
        // x86: the big executable stays in the generic payload and only gets renamed.
        let embedded = parse(blob(
            &[("./rustdesk.exe", b"app"), ("./sciter.dll", b"dll")],
            "./rustdesk.exe",
        ))
        .unwrap();
        let package = parse(blob(&[("./custom.txt", b"cfg")], "./acme.exe")).unwrap();

        let (files, exe) = merge(embedded, package);

        assert_eq!(exe, "./acme.exe");
        assert!(entry(&files, "./acme.exe").is_some());
        assert!(entry(&files, "./rustdesk.exe").is_none());
        // Untouched neighbours survive.
        assert_eq!(entry(&files, "./sciter.dll").unwrap().raw, b"dll");
        assert_eq!(entry(&files, "./custom.txt").unwrap().raw, b"cfg");
    }

    #[test]
    fn package_entries_win_over_the_generic_payload() {
        // x64: the customized executable and icons ship in the package instead.
        let embedded = parse(blob(
            &[
                ("./data/flutter_assets/assets/icon.ico", b"stock-icon"),
                ("./librustdesk.dll", b"core"),
            ],
            "./rustdesk.exe",
        ))
        .unwrap();
        let package = parse(blob(
            &[
                ("./acme.exe", b"branded"),
                ("./data/flutter_assets/assets/icon.ico", b"acme-icon"),
            ],
            "./acme.exe",
        ))
        .unwrap();

        let (files, exe) = merge(embedded, package);

        assert_eq!(exe, "./acme.exe");
        assert_eq!(
            entry(&files, "./data/flutter_assets/assets/icon.ico")
                .unwrap()
                .raw,
            b"acme-icon"
        );
        assert_eq!(
            files
                .iter()
                .filter(|f| normalize_path(&f.path) == "data/flutter_assets/assets/icon.ico")
                .count(),
            1
        );
        assert_eq!(entry(&files, "./librustdesk.dll").unwrap().raw, b"core");
    }

    #[test]
    fn package_paths_are_recorded_for_the_dropped_file_sweep() {
        let package = parse(blob(
            &[("./custom.txt", b"cfg"), ("./data/logo.png", b"img")],
            "./acme.exe",
        ))
        .unwrap();
        let mut paths: Vec<String> = package.0.iter().map(|f| f.path.clone()).collect();
        paths.sort();
        assert_eq!(paths, vec!["./custom.txt", "./data/logo.png"]);

        // Merging must not disturb them: the generic payload contributes none.
        let embedded = parse(blob(&[("./librustdesk.dll", b"core")], "./rustdesk.exe")).unwrap();
        let (files, _) = merge(embedded, package);
        assert!(entry(&files, "./data/logo.png").is_some());
    }

    #[test]
    fn matches_paths_across_separator_styles() {
        // generate.py emits backslashes when it runs on Windows.
        let embedded = parse(blob(&[(".\\rustdesk.exe", b"app")], ".\\rustdesk.exe")).unwrap();
        let package = parse(blob(&[("./custom.txt", b"cfg")], "./acme.exe")).unwrap();

        let (files, exe) = merge(embedded, package);

        assert_eq!(exe, "./acme.exe");
        assert!(entry(&files, "./acme.exe").is_some());
        assert!(entry(&files, ".\\rustdesk.exe").is_none());
    }
}
