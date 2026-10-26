use super::{FileDirectory, FileEntry, FileType};
use hbb_common::{anyhow::anyhow, bail, ResultType};
use serde_derive::Deserialize;
use serde_json::{json, Value};
use std::{path::Path, sync::OnceLock};

pub const ROOT: &str = "/@rustdesk/";
type Request = Box<dyn Fn(&str) -> ResultType<String> + Send + Sync>;
static BACKEND: OnceLock<Request> = OnceLock::new();

pub fn initialize(
    request: impl Fn(&str) -> ResultType<String> + Send + Sync + 'static,
) -> ResultType<()> {
    BACKEND
        .set(Box::new(request))
        .map_err(|_| anyhow!("Document storage already initialized"))
}

pub fn is_path(path: &Path) -> bool {
    path.to_str().map_or(false, |p| {
        p == ROOT.trim_end_matches('/') || p.starts_with(ROOT)
    })
}

pub(super) fn validate(path: &str) -> ResultType<()> {
    let relative = path
        .strip_prefix(ROOT)
        .and_then(|path| path.strip_prefix("tree/"))
        .ok_or(anyhow!("Invalid document path"))?;
    if relative
        .split('/')
        .any(|p| p.is_empty() || p == "." || p == ".." || p.contains(['\\', '\0']))
    {
        bail!("Invalid document path");
    }
    Ok(())
}

pub fn request(value: Value) -> ResultType<Value> {
    if let Some(path) = value.get("path").and_then(Value::as_str) {
        validate(path)?;
    }
    let backend = BACKEND
        .get()
        .ok_or(anyhow!("Android document storage is unavailable"))?;
    let response: Value = serde_json::from_str(&backend(&value.to_string())?)?;
    if let Some(error) = response.get("error").and_then(Value::as_str) {
        bail!("{}", error);
    }
    response
        .get("value")
        .cloned()
        .ok_or(anyhow!("Invalid document provider response"))
}

pub fn home() -> ResultType<Option<String>> {
    let value = request(json!({"op": "home"}))?;
    if value.is_null() {
        return Ok(None);
    }
    let path = value.as_str().ok_or(anyhow!("Invalid document home"))?;
    validate(path)?;
    Ok(Some(path.to_owned()))
}

pub fn allowed(path: &str) -> ResultType<bool> {
    if !home()?.map_or(false, |home| Path::new(path).starts_with(home)) {
        return Ok(false);
    }
    request(json!({"op": "allowed", "path": path}))?
        .as_bool()
        .ok_or(anyhow!("Invalid document permission response"))
}

#[derive(Deserialize)]
struct Metadata {
    name: String,
    directory: bool,
    size: i64,
    modified: i64,
}

fn entry(value: Value) -> ResultType<FileEntry> {
    let meta: Metadata = serde_json::from_value(value)?;
    super::validate_file_name_no_traversal(&meta.name)?;
    if meta.name.contains('/') || (!meta.directory && meta.size < 0) {
        bail!("The provider did not supply a transferable file name or size");
    }
    Ok(FileEntry {
        entry_type: if meta.directory {
            FileType::Dir
        } else {
            FileType::File
        }
        .into(),
        is_hidden: meta.name.starts_with('.'),
        name: meta.name,
        size: if meta.directory { 0 } else { meta.size as u64 },
        modified_time: meta.modified.max(0) as u64,
        ..Default::default()
    })
}

pub fn stat(path: &str) -> ResultType<Option<FileEntry>> {
    let value = request(json!({"op": "stat", "path": path}))?;
    if value.is_null() {
        Ok(None)
    } else {
        entry(value).map(Some)
    }
}

pub fn read_dir(path: &str, hidden: bool) -> ResultType<FileDirectory> {
    let values: Vec<Value> = serde_json::from_value(request(json!({"op": "list", "path": path}))?)?;
    let mut entries = Vec::new();
    for value in values {
        let entry = entry(value)?;
        if hidden || !entry.is_hidden {
            entries.push(entry);
        }
    }
    Ok(FileDirectory {
        path: path.to_owned(),
        entries,
        ..Default::default()
    })
}

pub fn recursive_files(path: &str, hidden: bool) -> ResultType<Vec<FileEntry>> {
    let meta = stat(path)?.ok_or(anyhow!("Document does not exist"))?;
    if meta.entry_type.enum_value() == Ok(FileType::File) {
        return Ok(vec![FileEntry {
            name: String::new(),
            ..meta
        }]);
    }
    let mut files = Vec::new();
    let mut pending = vec![(path.to_owned(), String::new())];
    while let Some((directory, prefix)) = pending.pop() {
        for mut entry in read_dir(&directory, hidden)?.entries {
            let path = format!("{}/{}", directory, entry.name);
            entry.name = if prefix.is_empty() {
                entry.name
            } else {
                format!("{}/{}", prefix, entry.name)
            };
            if entry.entry_type.enum_value() == Ok(FileType::Dir) {
                pending.push((path, entry.name));
            } else {
                files.push(entry);
            }
        }
    }
    Ok(files)
}

pub fn empty_dirs(path: &str, hidden: bool) -> ResultType<Vec<FileDirectory>> {
    let mut result = Vec::new();
    let mut pending = vec![path.to_owned()];
    while let Some(path) = pending.pop() {
        let directory = read_dir(&path, hidden)?;
        if directory.entries.is_empty() {
            result.push(directory);
            continue;
        }
        for entry in directory.entries {
            if entry.entry_type.enum_value() == Ok(FileType::Dir) {
                pending.push(format!("{}/{}", path, entry.name));
            }
        }
    }
    Ok(result)
}

pub fn operation(op: &str, path: &str) -> ResultType<()> {
    request(json!({"op": op, "path": path}))?;
    Ok(())
}

pub fn remove_empty_dir(path: &str) -> ResultType<()> {
    if !read_dir(path, true)?.entries.is_empty() {
        bail!("Directory is not empty");
    }
    operation("delete", path)
}

pub fn remove_file(path: &str) -> ResultType<()> {
    let entry = stat(path)?.ok_or(anyhow!("Document does not exist"))?;
    if entry.entry_type.enum_value() != Ok(FileType::File) {
        bail!("Cannot remove a directory as a file");
    }
    operation("delete", path)
}

pub fn remove_empty_tree(path: &str) -> ResultType<()> {
    validate(path)?;
    let tree_prefix = Path::new(ROOT).join("tree");
    if Path::new(path).parent().and_then(Path::parent) == Some(tree_prefix.as_path()) {
        bail!("Cannot delete a shared root");
    }
    let mut directories = vec![path.to_owned()];
    let mut index = 0;
    while index < directories.len() {
        for entry in read_dir(&directories[index], true)?.entries {
            if entry.entry_type.enum_value() != Ok(FileType::Dir) {
                bail!("Directory is not empty");
            }
            directories.push(format!("{}/{}", directories[index], entry.name));
        }
        index += 1;
    }
    for directory in directories.into_iter().rev() {
        remove_empty_dir(&directory)?;
    }
    Ok(())
}

pub fn rename(path: &str, name: &str) -> ResultType<()> {
    super::validate_file_name_no_traversal(name)?;
    request(json!({"op": "rename", "path": path, "name": name}))?;
    Ok(())
}

#[cfg(any(target_os = "android", all(test, unix)))]
pub fn take_file(value: &Value) -> ResultType<std::fs::File> {
    use std::convert::TryFrom;
    use std::os::fd::FromRawFd;
    let fd = value
        .as_i64()
        .and_then(|fd| i32::try_from(fd).ok())
        .filter(|fd| *fd >= 0)
        .ok_or(anyhow!("Invalid document descriptor"))?;
    // Android hands off a duplicate; Java retains the provider's close/error channel.
    Ok(unsafe { std::fs::File::from_raw_fd(fd) })
}

#[cfg(any(target_os = "android", all(test, unix)))]
pub(super) async fn open(
    path: String,
) -> ResultType<(
    tokio::fs::File,
    super::android_descriptor::DocumentDescriptor,
    Option<(u64, u64)>,
)> {
    use std::os::unix::fs::MetadataExt;

    let (file, descriptor, digest) = tokio::task::spawn_blocking(move || {
        let token = request(json!({"op": "open", "path": path}))?;
        let file = take_file(&token["fd"])?;
        let descriptor = super::android_descriptor::DocumentDescriptor::new(&token)?;
        let metadata = file.metadata()?;
        // Stream inode timestamps do not identify the source document's version.
        let digest = metadata
            .is_file()
            .then(|| (metadata.mtime().max(0) as u64, metadata.len()));
        Ok::<_, hbb_common::anyhow::Error>((file, descriptor, digest))
    })
    .await??;
    Ok((tokio::fs::File::from_std(file), descriptor, digest))
}

#[cfg(test)]
#[path = "tests/android_tests.rs"]
mod tests;

#[cfg(test)]
mod metadata_tests {
    use super::*;

    #[test]
    fn negative_modification_time_is_unknown() {
        for modified in [-1, 0, 1] {
            let metadata = entry(json!({
                "name": "notes.txt", "directory": false, "size": 12, "modified": modified
            }))
            .unwrap();
            assert_eq!(metadata.modified_time, modified.max(0) as u64);
        }
    }
}
