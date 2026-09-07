use crate::hbbs_http::create_http_client_with_url_strict;
use hbb_common::{bail, log, ResultType};
use std::{
    io::{Read, Seek, Write},
    path::{Path, PathBuf},
};

const UPDATE_FILE_CREATE_ATTEMPTS: usize = 16;

pub(super) fn ensure_verified_update_artifact(
    download_url: &str,
    file_path: &Path,
    expected_size: u64,
    expected_sha256: &str,
) -> ResultType<()> {
    if let Some(file_size) = cached_update_artifact_size(file_path)? {
        if file_size == expected_size {
            match verify_file_sha256(file_path, expected_sha256) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    log::warn!("Removing cached update file with invalid SHA256: {}", e);
                    remove_cached_update_artifact(file_path)?;
                }
            }
        } else {
            log::warn!(
                "Removing cached update file with size mismatch for {}: expected {}, got {}",
                file_path.display(),
                expected_size,
                file_size
            );
            remove_cached_update_artifact(file_path)?;
        }
    }
    let client = create_http_client_with_url_strict(download_url)?;
    let response = client.get(download_url).send()?;
    if !response.status().is_success() {
        bail!(
            "Failed to download the new version file: {}",
            response.status()
        );
    }
    let mut limited_response = response.take(expected_size.saturating_add(1));
    write_verified_update_artifact(
        file_path,
        &mut limited_response,
        expected_size,
        expected_sha256,
    )
}

fn cached_update_artifact_size(file_path: &Path) -> ResultType<Option<u64>> {
    let metadata = match std::fs::symlink_metadata(file_path) {
        Ok(metadata) => metadata,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    if metadata.file_type().is_file() {
        return Ok(Some(metadata.len()));
    }
    if metadata.file_type().is_symlink() {
        log::warn!("Removing cached update symlink: {}", file_path.display());
        remove_cached_update_artifact(file_path)?;
        return Ok(None);
    }
    bail!(
        "Refusing to use update cache path that is not a regular file: {}",
        file_path.display()
    )
}

fn remove_cached_update_artifact(file_path: &Path) -> ResultType<()> {
    let metadata = match std::fs::symlink_metadata(file_path) {
        Ok(metadata) => metadata,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.into()),
    };
    let file_type = metadata.file_type();
    if file_type.is_file() {
        std::fs::remove_file(file_path)?;
    } else if file_type.is_symlink() {
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::fs::FileTypeExt;
            if file_type.is_symlink_dir() {
                std::fs::remove_dir(file_path)?;
            } else {
                std::fs::remove_file(file_path)?;
            }
        }
        #[cfg(not(target_os = "windows"))]
        std::fs::remove_file(file_path)?;
    } else {
        bail!(
            "Refusing to remove update cache path that is not a file: {}",
            file_path.display()
        );
    }
    Ok(())
}

pub(crate) fn remove_update_file(file_path: &Path) {
    match std::fs::remove_file(file_path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => log::warn!(
            "Failed to remove update file {}: {}",
            file_path.display(),
            e
        ),
    }
}

fn create_download_temp_file(final_path: &Path) -> ResultType<(std::fs::File, PathBuf)> {
    let Some(download_dir) = final_path.parent() else {
        bail!(
            "Update file has no parent directory: {}",
            final_path.display()
        );
    };
    let Some(file_name) = final_path.file_name() else {
        bail!("Update file has no file name: {}", final_path.display());
    };
    let file_name = file_name.to_string_lossy();
    for _ in 0..UPDATE_FILE_CREATE_ATTEMPTS {
        let temp_path = download_dir.join(format!(
            ".{}.{}.{}.download",
            file_name,
            std::process::id(),
            hbb_common::rand::random::<u64>()
        ));
        match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(file) => return Ok((file, temp_path)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.into()),
        }
    }
    bail!("Failed to create temporary update file");
}

fn write_verified_update_artifact<R: Read>(
    final_path: &Path,
    reader: &mut R,
    expected_size: u64,
    expected_sha256: &str,
) -> ResultType<()> {
    let (mut file, temp_path) = create_download_temp_file(final_path)?;
    if let Err(e) = copy_and_verify_update_artifact(
        &mut file,
        &temp_path,
        reader,
        expected_size,
        expected_sha256,
    ) {
        remove_update_file(&temp_path);
        return Err(e);
    }
    drop(file);
    if let Err(e) = remove_cached_update_artifact(final_path) {
        remove_update_file(&temp_path);
        return Err(e);
    }
    if let Err(e) = std::fs::rename(&temp_path, final_path) {
        remove_update_file(&temp_path);
        return Err(e.into());
    }
    Ok(())
}

fn copy_and_verify_update_artifact<R: Read>(
    file: &mut std::fs::File,
    temp_path: &Path,
    reader: &mut R,
    expected_size: u64,
    expected_sha256: &str,
) -> ResultType<()> {
    let bytes_written = std::io::copy(reader, file)?;
    file.flush()?;
    if bytes_written != expected_size {
        bail!(
            "Update artifact size mismatch for {}: expected {}, got {}",
            temp_path.display(),
            expected_size,
            bytes_written
        );
    }
    verify_update_file_sha256(file, temp_path, expected_sha256)
}

fn verify_file_sha256(path: &Path, expected_sha256: &str) -> ResultType<()> {
    let mut file = std::fs::File::open(path)?;
    verify_update_file_sha256(&mut file, path, expected_sha256)
}

fn verify_update_file_sha256<R: Read + Seek>(
    reader: &mut R,
    path: &Path,
    expected_sha256: &str,
) -> ResultType<()> {
    use crate::update_hash::{verify_sha256_reader, Sha256VerificationError};

    match verify_sha256_reader(reader, expected_sha256) {
        Ok(()) => Ok(()),
        Err(Sha256VerificationError::InvalidExpected) => bail!(
            "Expected update file SHA256 is malformed for {}",
            path.display()
        ),
        Err(Sha256VerificationError::Mismatch {
            expected_sha256,
            actual_sha256,
        }) => bail!(
            "SHA256 mismatch for {}: expected {}, got {}",
            path.display(),
            expected_sha256,
            actual_sha256
        ),
        Err(Sha256VerificationError::Io(err)) => Err(err.into()),
    }
}

#[cfg(test)]
mod tests;
