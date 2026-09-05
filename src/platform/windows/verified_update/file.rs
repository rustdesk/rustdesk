use super::{ResultType, VERIFIED_UPDATE_FILE_PREFIX};
use hbb_common::{anyhow::anyhow, bail, log};
use std::{
    io::Write,
    os::windows::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};
use winapi::um::{
    winbase::FILE_FLAG_OPEN_REPARSE_POINT,
    winnt::{FILE_ATTRIBUTE_REPARSE_POINT, FILE_SHARE_READ},
};

const UPDATE_FILE_COPY_ATTEMPTS: usize = 16;
const UNSAFE_VERIFIED_MSI_PATH_CHARS: &[char] = &['"', '|', '<', '>', '%'];

pub struct VerifiedUpdateFile {
    _file: std::fs::File,
    path: PathBuf,
}

impl VerifiedUpdateFile {
    pub fn path_str(&self) -> ResultType<&str> {
        let Some(path) = self.path.to_str() else {
            bail!("Invalid update file path: {}", self.path.display());
        };
        Ok(path)
    }

    pub fn cleanup(self) {
        let Self { _file, path } = self;
        drop(_file);
        remove_verified_update_file(&path);
    }

    pub(super) fn verify_size(&self, expected_size: u64) -> ResultType<()> {
        let actual_size = self._file.metadata()?.len();
        if actual_size != expected_size {
            bail!(
                "Update artifact size mismatch for {}: expected {}, got {}",
                self.path.display(),
                expected_size,
                actual_size
            );
        }
        Ok(())
    }
}

pub(super) fn update_file_extension(file: &str) -> Option<String> {
    Path::new(file)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
}

fn verified_update_file_path(file: &str) -> ResultType<PathBuf> {
    let extension = update_file_extension(file).unwrap_or_default();
    if extension != "exe" && extension != "msi" {
        bail!("Unsupported update file format: {}", file);
    }
    let path = std::env::temp_dir().join(format!(
        "{}{}-{}.{}",
        VERIFIED_UPDATE_FILE_PREFIX,
        std::process::id(),
        hbb_common::rand::random::<u64>(),
        extension
    ));
    if extension == "msi" {
        validate_verified_msi_path(&path)?;
    }
    Ok(path)
}

fn validate_verified_msi_path(path: &Path) -> ResultType<()> {
    let Some(path_str) = path.to_str() else {
        bail!("Invalid verified update file path: {}", path.display());
    };
    if path_str.contains(UNSAFE_VERIFIED_MSI_PATH_CHARS) {
        bail!(
            "Refusing to use unsafe verified update file path: {}",
            path_str
        );
    }
    Ok(())
}

// Reject symlinks, junctions, and other reparse points so verification and
// execution operate on a regular installer file, not a redirected target.
fn is_update_file_attributes_trusted(attributes: u32) -> bool {
    attributes & FILE_ATTRIBUTE_REPARSE_POINT == 0
}

// Open reparse points themselves instead of following them, so the
// attribute check can reject redirected update paths.
fn update_file_open_flags() -> u32 {
    FILE_FLAG_OPEN_REPARSE_POINT
}

fn open_update_file_for_verification(file: &str) -> ResultType<std::fs::File> {
    let update_file = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(update_file_open_flags())
        .open(file)
        .map_err(|err| anyhow!("Failed to lock update file {}: {}", file, err))?;
    let metadata = update_file
        .metadata()
        .map_err(|err| anyhow!("Failed to read update file metadata {}: {}", file, err))?;
    if !metadata.is_file() || !is_update_file_attributes_trusted(metadata.file_attributes()) {
        bail!("Refusing to verify untrusted update file: {}", file);
    }
    Ok(update_file)
}

fn copy_update_file_for_verification(file: &str) -> ResultType<PathBuf> {
    let mut source_file = open_update_file_for_verification(file)?;
    for _ in 0..UPDATE_FILE_COPY_ATTEMPTS {
        let path = verified_update_file_path(file)?;
        let mut copy_file = match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .share_mode(FILE_SHARE_READ)
            .custom_flags(update_file_open_flags())
            .open(&path)
        {
            Ok(file) => file,
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => {
                return Err(anyhow!(
                    "Failed to create verified update file {}: {}",
                    path.display(),
                    err
                )
                .into())
            }
        };
        if let Err(err) = std::io::copy(&mut source_file, &mut copy_file) {
            drop(copy_file);
            remove_verified_update_file(&path);
            return Err(err.into());
        }
        if let Err(err) = copy_file.flush() {
            drop(copy_file);
            remove_verified_update_file(&path);
            return Err(err.into());
        }
        drop(copy_file);
        return Ok(path);
    }
    bail!("Failed to create verified update file for {}", file);
}

// Keep the verified copy open without write/delete sharing until the installer starts.
pub fn copy_and_verify_update_file_sha256(
    file: &str,
    expected_sha256: &str,
) -> ResultType<VerifiedUpdateFile> {
    let path = copy_update_file_for_verification(file)?;
    let Some(update_path) = path.to_str().map(str::to_owned) else {
        remove_verified_update_file(&path);
        bail!("Invalid update file path: {}", path.display());
    };

    let mut read_file = match open_update_file_for_verification(&update_path) {
        Ok(file) => file,
        Err(err) => {
            remove_verified_update_file(&path);
            return Err(err);
        }
    };
    if let Err(err) = verify_update_file_sha256(&mut read_file, expected_sha256, &update_path) {
        drop(read_file);
        remove_verified_update_file(&path);
        return Err(err);
    }
    Ok(VerifiedUpdateFile {
        _file: read_file,
        path,
    })
}

fn verify_update_file_sha256(
    update_file: &mut std::fs::File,
    expected_sha256: &str,
    file: &str,
) -> ResultType<()> {
    use crate::update_hash::{verify_sha256_reader, Sha256VerificationError};

    match verify_sha256_reader(update_file, expected_sha256) {
        Ok(()) => Ok(()),
        Err(Sha256VerificationError::InvalidExpected) => {
            bail!("Expected update file SHA256 is malformed for {}", file)
        }
        Err(Sha256VerificationError::Mismatch {
            expected_sha256,
            actual_sha256,
        }) => bail!(
            "SHA256 mismatch for {}: expected {}, got {}",
            file,
            expected_sha256,
            actual_sha256
        ),
        Err(Sha256VerificationError::Io(err)) => Err(err.into()),
    }
}

fn remove_verified_update_file(path: &Path) {
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => log::warn!(
            "Failed to remove verified update file {}: {}",
            path.display(),
            err
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mismatched_sha256_removes_verified_copy() {
        let source_path = std::env::temp_dir().join(format!(
            "rustdesk-update-source-{}-{}.exe",
            std::process::id(),
            hbb_common::rand::random::<u64>()
        ));
        std::fs::write(&source_path, b"update").unwrap();
        let unrelated_path = std::env::temp_dir().join(format!(
            "{}{}-{}.exe",
            VERIFIED_UPDATE_FILE_PREFIX,
            std::process::id(),
            hbb_common::rand::random::<u64>()
        ));
        std::fs::write(&unrelated_path, b"unrelated update").unwrap();

        let result =
            copy_and_verify_update_file_sha256(source_path.to_str().unwrap(), &"0".repeat(64));
        let error = match result {
            Ok(update_file) => {
                update_file.cleanup();
                panic!("mismatched SHA256 should fail");
            }
            Err(error) => error.to_string(),
        };
        let verified_path = error
            .strip_prefix("SHA256 mismatch for ")
            .and_then(|message| message.split_once(": expected "))
            .map(|(path, _)| PathBuf::from(path))
            .expect("mismatch error did not contain verified path");

        assert!(!verified_path.exists());
        assert!(unrelated_path.exists());
        std::fs::remove_file(source_path).unwrap();
        std::fs::remove_file(unrelated_path).unwrap();
    }

    #[test]
    fn verified_update_file_size_must_match_expected_size() {
        let source_path = std::env::temp_dir().join(format!(
            "rustdesk-update-size-test-{}-{}.exe",
            std::process::id(),
            hbb_common::rand::random::<u64>()
        ));
        std::fs::write(&source_path, b"rustdesk").unwrap();
        let verified_file = copy_and_verify_update_file_sha256(
            source_path.to_str().unwrap(),
            "304ca1638c5effa6832e0e15b958a8f74847efe4df9c3f3187216e921c168fed",
        )
        .unwrap();

        assert!(verified_file.verify_size(8).is_ok());
        assert!(verified_file.verify_size(7).is_err());

        verified_file.cleanup();
        std::fs::remove_file(source_path).unwrap();
    }

    #[test]
    fn rejects_reparse_point_update_file_attributes() {
        use winapi::um::winnt::FILE_ATTRIBUTE_NORMAL;

        assert!(is_update_file_attributes_trusted(FILE_ATTRIBUTE_NORMAL));
        assert!(!is_update_file_attributes_trusted(
            FILE_ATTRIBUTE_REPARSE_POINT
        ));
        assert!(!is_update_file_attributes_trusted(
            FILE_ATTRIBUTE_NORMAL | FILE_ATTRIBUTE_REPARSE_POINT
        ));
    }

    #[test]
    fn verified_msi_path_rejects_unsafe_cmd_characters() {
        for character in ['"', '|', '<', '>', '%'] {
            let path = PathBuf::from(format!(r"C:\Temp\unsafe{character}\update.msi"));
            assert!(
                validate_verified_msi_path(&path).is_err(),
                "accepted unsafe character {character:?}"
            );
        }

        for character in ['&', '^'] {
            let path = PathBuf::from(format!(r"C:\Temp\safe{character}path\update.msi"));
            assert!(
                validate_verified_msi_path(&path).is_ok(),
                "rejected quoted-safe character {character:?}"
            );
        }

        assert!(validate_verified_msi_path(Path::new(r"C:\Temp\safe path\update.msi")).is_ok());
    }
}
