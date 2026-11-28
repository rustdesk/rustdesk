use hbb_common::{bail, ResultType};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    time::Duration,
};
use uuid::Uuid;

const PDF_HEADER: &[u8] = b"%PDF-";
const PDF_END: &[u8] = b"%%EOF";
const PDF_TRAILER_BYTES: u64 = 1024;
#[cfg(unix)]
const PRIVATE_DIRECTORY_MODE: u32 = 0o700;

pub(crate) struct PdfFile {
    directory: PathBuf,
    path: PathBuf,
}

pub(crate) struct PdfWriter {
    // Close the file before its directory is removed on failure.
    file: File,
    document: PdfFile,
}

impl PdfWriter {
    pub(crate) fn create() -> ResultType<Self> {
        let directory = std::env::temp_dir().join(format!("rustdesk-pdf-{}", Uuid::new_v4()));
        private_directory(&directory)?;
        let document = PdfFile {
            path: directory.join("document.pdf"),
            directory,
        };
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&document.path)?;
        Ok(Self { file, document })
    }

    pub(crate) fn write(&mut self, bytes: &[u8]) -> ResultType<()> {
        self.file.write_all(bytes)?;
        Ok(())
    }

    pub(crate) fn finish(mut self) -> ResultType<PdfFile> {
        self.file.flush()?;
        validate_envelope(&mut self.file)?;
        self.file.sync_all()?;
        drop(self.file);
        Ok(self.document)
    }
}

impl PdfFile {
    pub(crate) fn from_bytes(bytes: &[u8]) -> ResultType<Self> {
        let mut writer = PdfWriter::create()?;
        writer.write(bytes)?;
        writer.finish()
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn read(&self) -> ResultType<Vec<u8>> {
        Ok(fs::read(&self.path)?)
    }
}

fn validate_envelope(file: &mut File) -> ResultType<()> {
    file.seek(SeekFrom::Start(0))?;
    let mut header = [0; PDF_HEADER.len()];
    file.read_exact(&mut header)?;
    if header != PDF_HEADER {
        bail!("The captured document is not PDF");
    }
    let length = file.metadata()?.len();
    file.seek(SeekFrom::Start(length.saturating_sub(PDF_TRAILER_BYTES)))?;
    let mut tail = Vec::new();
    file.read_to_end(&mut tail)?;
    if !tail.windows(PDF_END.len()).any(|window| window == PDF_END) {
        bail!("The PDF output is incomplete");
    }
    Ok(())
}

#[cfg(windows)]
fn private_directory(path: &Path) -> ResultType<()> {
    remote_printer::pdf::create_private_directory(path)
}

#[cfg(unix)]
fn private_directory(path: &Path) -> ResultType<()> {
    use std::os::unix::fs::DirBuilderExt;
    fs::DirBuilder::new()
        .mode(PRIVATE_DIRECTORY_MODE)
        .create(path)?;
    Ok(())
}

impl Drop for PdfFile {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_file(&self.path) {
            if error.kind() != std::io::ErrorKind::NotFound {
                hbb_common::throttled_log!(
                    Duration::from_secs(60),
                    error,
                    "PDF temporary file cleanup failed: {error}"
                );
            }
        }
        if let Err(error) = fs::remove_dir(&self.directory) {
            hbb_common::throttled_log!(
                Duration::from_secs(60),
                error,
                "PDF temporary directory cleanup failed: {error}"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incomplete_pdf_is_rejected_and_cleaned() {
        let mut writer = PdfWriter::create().unwrap();
        let directory = writer.document.directory.clone();
        writer.write(b"%PDF-1.7\n").unwrap();
        assert!(writer.finish().is_err());
        assert!(!directory.exists());
    }

    #[test]
    fn concurrent_pdf_files_have_independent_paths_and_lifetimes() {
        let bytes = b"%PDF-1.7\n%%EOF\n";
        let first = PdfFile::from_bytes(bytes).unwrap();
        let second = PdfFile::from_bytes(bytes).unwrap();
        assert_ne!(first.path(), second.path());
        drop(first);
        assert_eq!(second.read().unwrap(), bytes);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&second.directory)
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                PRIVATE_DIRECTORY_MODE
            );
        }
    }
}
