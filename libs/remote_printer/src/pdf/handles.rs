use hbb_common::{bail, ResultType};
use std::{mem::size_of, time::Duration};
use windows::{
    core::{HSTRING, PCWSTR},
    Win32::{
        Foundation::{CloseHandle, ERROR_INSUFFICIENT_BUFFER, HANDLE},
        Graphics::Printing::{
            ClosePrinter, OpenPrinterW, PRINTER_ACCESS_RIGHTS, PRINTER_DEFAULTSW, PRINTER_HANDLE,
        },
    },
};

const CLEANUP_LOG_INTERVAL: Duration = Duration::from_secs(60);

pub(crate) struct Handle(pub HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        if let Err(error) = unsafe { CloseHandle(self.0) } {
            hbb_common::throttled_log!(
                CLEANUP_LOG_INTERVAL,
                warn,
                "PDF handle cleanup failed: {error}"
            );
        }
    }
}

pub(crate) struct Printer(pub PRINTER_HANDLE);

impl Printer {
    pub(crate) fn open(name: &str, access: PRINTER_ACCESS_RIGHTS) -> ResultType<Self> {
        if name.is_empty() || name.contains('\0') {
            bail!("Invalid printer name");
        }
        let name = HSTRING::from(name);
        let defaults = PRINTER_DEFAULTSW {
            DesiredAccess: access,
            ..Default::default()
        };
        let mut handle = PRINTER_HANDLE::default();
        unsafe { OpenPrinterW(PCWSTR(name.as_ptr()), &mut handle, Some(&defaults))? };
        Ok(Self(handle))
    }
}

impl Drop for Printer {
    fn drop(&mut self) {
        if let Err(error) = unsafe { ClosePrinter(self.0) } {
            hbb_common::throttled_log!(CLEANUP_LOG_INTERVAL, warn, "ClosePrinter failed: {error}");
        }
    }
}

pub(crate) struct Buffer {
    storage: Vec<usize>,
    length: usize,
}

impl Buffer {
    pub(crate) fn query(
        mut read: impl FnMut(Option<&mut [u8]>, &mut u32) -> windows::core::Result<()>,
    ) -> ResultType<Option<Self>> {
        let mut needed = 0;
        check_size(read(None, &mut needed))?;
        if needed == 0 {
            return Ok(None);
        }
        let mut buffer = Self::new(needed)?;
        read(Some(buffer.bytes()), &mut needed)?;
        Ok(Some(buffer))
    }

    pub(crate) fn new(length: u32) -> ResultType<Self> {
        let length = length as usize;
        if length == 0 {
            bail!("The print API returned an empty buffer size");
        }
        let words = length.div_ceil(size_of::<usize>());
        let mut storage = Vec::new();
        storage.try_reserve_exact(words)?;
        storage.resize(words, 0);
        Ok(Self { storage, length })
    }

    pub(crate) fn bytes(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.storage.as_mut_ptr().cast(), self.length) }
    }

    // T must match the initialized Win32 structure returned for the requested API level.
    pub(crate) unsafe fn item<T: Copy>(&self, index: usize) -> ResultType<T> {
        let offset = index
            .checked_mul(size_of::<T>())
            .ok_or_else(|| hbb_common::anyhow::anyhow!("Print API offset overflow"))?;
        if offset
            .checked_add(size_of::<T>())
            .map_or(true, |end| end > self.length)
        {
            bail!("The print API returned an invalid structure count");
        }
        Ok(std::ptr::read_unaligned(
            self.storage.as_ptr().cast::<u8>().add(offset).cast(),
        ))
    }
}

pub(crate) fn check_size(result: windows::core::Result<()>) -> ResultType<()> {
    match result {
        Ok(()) => Ok(()),
        Err(error) if error.code() == ERROR_INSUFFICIENT_BUFFER.to_hresult() => Ok(()),
        Err(error) => Err(error.into()),
    }
}
