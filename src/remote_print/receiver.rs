use hbb_common::ResultType;

#[cfg(all(windows, feature = "flutter"))]
pub(crate) fn available() -> bool {
    let result = std::thread::spawn(remote_printer::pdf::available).join();
    match result {
        Ok(Ok(available)) => available,
        Ok(Err(error)) => {
            hbb_common::throttled_log!(
                std::time::Duration::from_secs(60),
                error,
                "Windows PDF backend unavailable: {error:#}"
            );
            false
        }
        Err(_) => {
            hbb_common::log::error!("Windows PDF capability thread failed");
            false
        }
    }
}

#[cfg(all(feature = "flutter", any(target_os = "linux", target_os = "macos")))]
pub(crate) fn available() -> bool {
    match super::cups::available() {
        Ok(available) => available,
        Err(error) => {
            hbb_common::throttled_log!(
                std::time::Duration::from_secs(60),
                error,
                "CUPS PDF backend unavailable: {error:#}"
            );
            false
        }
    }
}

#[cfg(not(all(
    feature = "flutter",
    any(windows, target_os = "linux", target_os = "macos")
)))]
pub(crate) fn available() -> bool {
    false
}

pub(crate) fn print(bytes: &[u8], printer: &str) -> ResultType<u32> {
    #[cfg(windows)]
    {
        let document = super::files::PdfFile::from_bytes(bytes)?;
        return remote_printer::pdf::print_file(document.path(), printer);
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let document = super::files::PdfFile::from_bytes(bytes)?;
        return super::cups::print_file(document.path(), printer);
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        let _ = (bytes, printer);
        hbb_common::bail!("PDF printing is unavailable on this platform");
    }
}

pub(crate) fn printer_names() -> ResultType<Vec<String>> {
    #[cfg(windows)]
    return remote_printer::pdf::printer_names();
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    return super::cups::printer_names();
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    hbb_common::bail!("PDF printing is unavailable on this platform");
}
