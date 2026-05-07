use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use time::format_description::FormatItem;
use time::macros::format_description;
use time::OffsetDateTime;

static START_TIMESTAMP_FORMAT: &[FormatItem<'static>] =
    format_description!("[year][month][day]-[hour][minute][second]");
static END_TIMESTAMP_FORMAT: &[FormatItem<'static>] = format_description!("[hour][minute][second]");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentPaths {
    pub temporary: PathBuf,
    pub complete: PathBuf,
}

pub struct StorageManager {
    dir: PathBuf,
}

impl StorageManager {
    pub fn new(dir: &Path) -> hbb_common::ResultType<Self> {
        if dir.exists() && !dir.is_dir() {
            anyhow::bail!("output directory must be a directory");
        }
        fs::create_dir_all(dir)?;
        restrict_directory(dir)?;
        let storage = Self {
            dir: dir.to_path_buf(),
        };
        storage.clean_orphaned_temp_files()?;
        Ok(storage)
    }

    pub fn next_segment_path(&self) -> hbb_common::ResultType<PathBuf> {
        Ok(self.next_segment_paths()?.complete)
    }

    pub fn next_segment_paths(&self) -> hbb_common::ResultType<SegmentPaths> {
        for _ in 0..1000 {
            let id = uuid::Uuid::new_v4().to_string();
            let temporary = self.dir.join(format!("{id}.webm.tmp"));
            let complete = self.dir.join(format!("{id}.webm"));
            if !temporary.exists() && !complete.exists() {
                return Ok(SegmentPaths {
                    temporary,
                    complete,
                });
            }
        }
        anyhow::bail!("unable to allocate unique segment path");
    }

    pub fn segment_paths_for_period(
        &self,
        start: SystemTime,
        end: SystemTime,
    ) -> hbb_common::ResultType<SegmentPaths> {
        let base_name = format!(
            "{}-{}",
            format_timestamp(start, START_TIMESTAMP_FORMAT)?,
            format_timestamp(end, END_TIMESTAMP_FORMAT)?
        );
        for index in 0..1000 {
            let name = if index == 0 {
                base_name.clone()
            } else {
                format!("{base_name}-{index}")
            };
            let temporary = self.dir.join(format!("{name}.webm.tmp"));
            let complete = self.dir.join(format!("{name}.webm"));
            if !temporary.exists() && !complete.exists() {
                return Ok(SegmentPaths {
                    temporary,
                    complete,
                });
            }
        }
        anyhow::bail!("unable to allocate unique segment path");
    }

    pub fn mark_complete(&self, paths: &SegmentPaths) -> hbb_common::ResultType<()> {
        fs::rename(&paths.temporary, &paths.complete)?;
        restrict_file(&paths.complete)?;
        Ok(())
    }

    pub fn clean_orphaned_temp_files(&self) -> hbb_common::ResultType<()> {
        for entry in fs::read_dir(&self.dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("tmp")
                && path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().ends_with(".webm.tmp"))
                && entry.metadata()?.is_file()
            {
                fs::remove_file(path)?;
            }
        }
        Ok(())
    }

    pub fn enforce_retention_cap(&self, retention_days: u64) -> hbb_common::ResultType<()> {
        let cutoff = SystemTime::now()
            .checked_sub(Duration::from_secs(
                retention_days.saturating_mul(24 * 60 * 60),
            ))
            .unwrap_or(SystemTime::UNIX_EPOCH);
        for entry in fs::read_dir(&self.dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("webm") {
                continue;
            }
            let metadata = entry.metadata()?;
            if metadata.is_file() && metadata.modified()? <= cutoff {
                fs::remove_file(path)?;
            }
        }
        Ok(())
    }

    pub fn enforce_storage_cap(&self, max_bytes: u64) -> hbb_common::ResultType<()> {
        let mut complete_segments = Vec::new();
        for entry in fs::read_dir(&self.dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("webm") {
                continue;
            }
            let metadata = entry.metadata()?;
            if !metadata.is_file() {
                continue;
            }
            let modified = metadata.modified()?;
            complete_segments.push((modified, metadata.len(), path));
        }

        complete_segments
            .sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.2.cmp(&right.2)));
        let mut total: u64 = complete_segments.iter().map(|(_, size, _)| *size).sum();
        for (_, size, path) in complete_segments {
            if total <= max_bytes {
                break;
            }
            fs::remove_file(&path)?;
            total = total.saturating_sub(size);
        }

        Ok(())
    }
}

fn format_timestamp(
    time: SystemTime,
    format: &[FormatItem<'static>],
) -> hbb_common::ResultType<String> {
    let datetime: OffsetDateTime = time.into();
    Ok(datetime.format(format)?)
}

#[cfg(unix)]
fn restrict_directory(dir: &Path) -> hbb_common::ResultType<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(not(unix))]
fn restrict_directory(_dir: &Path) -> hbb_common::ResultType<()> {
    Ok(())
}

#[cfg(unix)]
pub(crate) fn restrict_file(path: &Path) -> hbb_common::ResultType<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
pub(crate) fn restrict_file(_path: &Path) -> hbb_common::ResultType<()> {
    Ok(())
}
