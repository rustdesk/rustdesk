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
    io::{self, Write},
    os::windows::fs::MetadataExt,
    path::{Component, Path, PathBuf},
};

use crate::bin_reader::{normalize_path, BinaryData, BinaryReader};

mod protected_dir;

use protected_dir::ProtectedDirectory;
use windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;

const INVALID_FILENAME_CHARS: &str = "<>:\"/\\|?*";

pub(super) struct SecureUpdateDir {
    root: ProtectedDirectory,
    directories: HashSet<String>,
}

impl SecureUpdateDir {
    pub(super) fn create() -> io::Result<Self> {
        Ok(Self {
            root: ProtectedDirectory::create()?,
            directories: HashSet::new(),
        })
    }

    pub(super) fn extract(&mut self, reader: &BinaryReader) -> io::Result<PathBuf> {
        for data in &reader.files {
            self.write_file(data)?;
        }
        let executable = self.root.path().join(embedded_path(&reader.exe)?);
        ensure_regular_file(&executable)?;
        Ok(executable)
    }

    pub(super) fn cleanup(self) -> io::Result<()> {
        self.root.cleanup()
    }

    fn write_file(&mut self, data: &BinaryData) -> io::Result<()> {
        let relative = embedded_path(&data.path)?;
        if let Some(parent) = relative.parent() {
            self.create_parent_directories(parent)?;
        }
        let contents = data.verified_contents()?;
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
        update_dir.cleanup()
    }
}
