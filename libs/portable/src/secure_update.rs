//! Protected payload workspace for Windows portable `--update`.
//!
//! An elevated updater must not load EXEs or DLLs from the ordinary user-writable
//! LocalAppData extraction cache. ProgramData provides a machine-wide parent for
//! a fresh workspace, separate from normal launches. Protection comes from the
//! Administrators owner and SYSTEM/Administrators-only DACL applied at creation,
//! together with path/reparse-point checks and a root handle without delete sharing.
//! Windows\Temp can also host a protected private workspace; using the same parent
//! as the installer handoff would not by itself protect the extracted payload.
//!
//! ```text
//! elevated packer --update
//!     -> %ProgramData%\RustDeskUpdate-<random> (protected at creation)
//!     -> extract EXE/DLLs -> run updater -> wait -> attempt cleanup
//! ```
//!
//! Other implementations choose directories by purpose (upstream source/docs
//! checked in November 2026):
//! - Chromium's elevated updater uses random subdirectories of SystemRoot\SystemTemp,
//!   falling back to Program Files when unavailable. SystemTemp differs from Windows\Temp.
//!   https://chromium.googlesource.com/chromium/src/+/main/chrome/updater/util/util.cc
//!   https://github.com/chromium/chromium/blob/main/base/base_paths_win.cc
//! - Inno Setup creates a temporary IS-*.tmp directory and deletes it on exit;
//!   its documented example is C:\Windows\Temp\IS-xxxxx.tmp.
//!   https://jrsoftware.org/ishelp/topic_consts.htm
//! - Firefox keeps update data under ProgramData\Mozilla-...\updates\<install hash>,
//!   while staging the updated application under <install dir>\updated.
//!   https://github.com/mozilla-firefox/firefox/blob/main/toolkit/mozapps/update/common/commonupdatedir.cpp
//!   https://github.com/mozilla-firefox/firefox/blob/main/toolkit/xre/nsUpdateDriver.cpp
//! - Visual Studio keeps its installation/repair package cache under
//!   %ProgramData%\Microsoft\VisualStudio\Packages.
//!   https://learn.microsoft.com/en-us/visualstudio/install/configure-policies-for-enterprise-deployments?view=vs-2022
//! Cache locations and temporary execution workspaces serve different purposes;
//! these examples do not establish a universally safer parent directory.

use std::{
    collections::HashSet,
    ffi::OsStr,
    fs,
    io::{self, Read, Write},
    os::windows::{fs::MetadataExt, process::CommandExt},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
};

use crate::{
    bin_reader::{normalize_path, BinaryData, BinaryReader},
    use_null_stdio, APPNAME_RUNTIME_ENV_KEY,
};

mod protected_dir;

use protected_dir::ProtectedDirectory;
use windows::Win32::{
    Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT, System::Threading::CREATE_NO_WINDOW,
};

const INVALID_FILENAME_CHARS: &str = "<>:\"/\\|?*";
const DECOMPRESS_BUFFER_SIZE: usize = 4096;

pub(super) fn execute_update(reader: &BinaryReader, args: &[String]) -> io::Result<()> {
    let mut update_dir = SecureUpdateDir::create()?;
    let executable = update_dir.extract(reader)?;
    let portable_exe = std::env::current_exe()?;
    let portable_name = portable_exe.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Portable executable name is unavailable",
        )
    })?;
    let mut command = Command::new(executable);
    command
        .args(args)
        .env(APPNAME_RUNTIME_ENV_KEY, portable_name)
        .creation_flags(CREATE_NO_WINDOW.0);
    if use_null_stdio() {
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
    }
    let status = command.status()?;
    if !status.success() {
        return Err(io::Error::other(format!(
            "Embedded updater exited with status {status}"
        )));
    }
    // Cleanup failure must not change the updater's successful exit status.
    if let Err(error) = update_dir.root.cleanup() {
        eprintln!("{error}");
    }
    Ok(())
}

struct SecureUpdateDir {
    root: ProtectedDirectory,
    directories: HashSet<String>,
}

impl SecureUpdateDir {
    fn create() -> io::Result<Self> {
        Ok(Self {
            root: ProtectedDirectory::create()?,
            directories: HashSet::new(),
        })
    }

    fn extract(&mut self, reader: &BinaryReader) -> io::Result<PathBuf> {
        for data in &reader.files {
            self.write_file(data)?;
        }
        let executable = self.root.path().join(embedded_path(&reader.exe)?);
        ensure_regular_file(&executable)?;
        Ok(executable)
    }

    fn write_file(&mut self, data: &BinaryData) -> io::Result<()> {
        let relative = embedded_path(&data.path)?;
        if let Some(parent) = relative.parent() {
            self.create_parent_directories(parent)?;
        }
        let contents = verified_contents(data)?;
        let path = self.root.path().join(relative);
        let mut file = self.root.create_file(&path)?;
        file.write_all(&contents)
            .map_err(|error| path_error("Failed to write update file", &path, error))?;
        file.sync_all()
            .map_err(|error| path_error("Failed to sync update file", &path, error))?;
        Ok(())
    }

    fn create_parent_directories(&mut self, relative: &Path) -> io::Result<()> {
        let mut current = self.root.path().to_path_buf();
        for component in relative.components() {
            current.push(component);
            if self
                .directories
                .insert(normalize_path(&current.to_string_lossy()))
            {
                self.root.create_directory(&current)?;
            }
        }
        Ok(())
    }
}

fn verified_contents(data: &BinaryData) -> io::Result<Vec<u8>> {
    let mut decoder = brotli::Decompressor::new(data.raw, DECOMPRESS_BUFFER_SIZE);
    let mut contents = Vec::new();
    decoder.read_to_end(&mut contents).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Failed to decompress {}: {error}", data.path),
        )
    })?;
    let expected = std::str::from_utf8(data.md5_code).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Invalid embedded digest for {}: {error}", data.path),
        )
    })?;
    let actual = format!("{:x}", md5::compute(&contents));
    if actual != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Embedded digest mismatch for {}: expected {expected}, got {actual}",
                data.path
            ),
        ));
    }
    Ok(contents)
}

fn embedded_path(path: &str) -> io::Result<PathBuf> {
    let mut relative = PathBuf::new();
    for component in Path::new(path).components() {
        match component {
            Component::CurDir => {}
            Component::Normal(value) if is_safe_component(value) => relative.push(value),
            _ => return Err(invalid_embedded_path(path)),
        }
    }
    if relative.as_os_str().is_empty() {
        return Err(invalid_embedded_path(path));
    }
    Ok(relative)
}

fn is_safe_component(value: &OsStr) -> bool {
    let value = value.to_string_lossy();
    if value.ends_with(' ')
        || value.ends_with('.')
        || value
            .chars()
            .any(|character| character.is_control() || INVALID_FILENAME_CHARS.contains(character))
    {
        return false;
    }
    let stem = value
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    !is_reserved_device_name(&stem)
}

fn is_reserved_device_name(stem: &str) -> bool {
    if matches!(
        stem,
        "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$" | "CONIN$" | "CONOUT$"
    ) {
        return true;
    }
    ["COM", "LPT"].iter().any(|prefix| {
        stem.strip_prefix(prefix).is_some_and(|number| {
            matches!(
                number,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
    })
}

fn ensure_regular_file(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.is_file() && metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 == 0 {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        format!(
            "Embedded executable is not a regular file: {}",
            path.display()
        ),
    ))
}

fn invalid_embedded_path(path: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("Invalid embedded update path: {path}"),
    )
}

fn path_error(action: &str, path: &Path, error: io::Error) -> io::Error {
    io::Error::new(
        error.kind(),
        format!("{action} {}: {error}", path.display()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_case_package_paths_share_parent_directories() -> io::Result<()> {
        let mut update_dir = SecureUpdateDir::create()?;
        update_dir.create_parent_directories(Path::new("data/flutter_assets"))?;
        update_dir.create_parent_directories(Path::new("Data/Flutter_Assets"))?;
        update_dir.root.cleanup()
    }
}
