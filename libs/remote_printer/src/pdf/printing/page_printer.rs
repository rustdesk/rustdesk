use super::{
    super::handles::{Buffer, Printer},
    Rasterizer,
};
use hbb_common::{bail, ResultType};
use std::{mem::size_of, time::Duration};
use windows::{
    core::{HSTRING, PCWSTR},
    Foundation::Size,
    Win32::Graphics::{
        Gdi::{
            ResetDCW, DEVMODEW, DMORIENT_LANDSCAPE, DMORIENT_PORTRAIT, DM_IN_BUFFER,
            DM_ORIENTATION, DM_OUT_BUFFER, HDC,
        },
        Printing::{DocumentPropertiesW, PRINTER_ACCESS_USE},
    },
};

const DOCUMENT_PROPERTIES_OK: i32 = 1;
const ORIENTATION_LOG_INTERVAL: Duration = Duration::from_secs(60);

pub(super) struct PagePrinter {
    pub(super) renderer: Rasterizer,
    name: HSTRING,
    printer: Printer,
    mode: Buffer,
}

impl PagePrinter {
    pub(super) fn new(name: HSTRING) -> ResultType<Self> {
        let printer = Printer::open(&name.to_string_lossy(), PRINTER_ACCESS_USE)?;
        let length =
            unsafe { DocumentPropertiesW(None, printer.0, PCWSTR(name.as_ptr()), None, None, 0) };
        if length < size_of::<DEVMODEW>() as i32 {
            bail!("Invalid printer DEVMODE buffer size: {length}");
        }
        // Keep driver-private settings after DEVMODEW when changing its public fields.
        let mut mode = Buffer::new(u32::try_from(length)?)?;
        check_properties(unsafe {
            DocumentPropertiesW(
                None,
                printer.0,
                PCWSTR(name.as_ptr()),
                Some(mode.bytes().as_mut_ptr().cast()),
                None,
                DM_OUT_BUFFER.0,
            )
        })?;
        Ok(Self {
            renderer: Rasterizer::new()?,
            name,
            printer,
            mode,
        })
    }

    pub(super) fn prepare(&mut self, dc: HDC, size: Size) -> ResultType<()> {
        let orientation = if size.Width > size.Height {
            DMORIENT_LANDSCAPE
        } else {
            DMORIENT_PORTRAIT
        } as i16;
        let mode = self.mode.bytes().as_mut_ptr().cast::<DEVMODEW>();
        unsafe {
            if !(*mode).dmFields.contains(DM_ORIENTATION) {
                hbb_common::throttled_log!(
                    ORIENTATION_LOG_INTERVAL,
                    info,
                    "Printer has no orientation setting; fitting PDF pages to its current printable area"
                );
                return Ok(());
            }
            (*mode).Anonymous1.Anonymous1.dmOrientation = orientation;
            check_properties(DocumentPropertiesW(
                None,
                self.printer.0,
                PCWSTR(self.name.as_ptr()),
                Some(mode),
                Some(mode),
                DM_IN_BUFFER.0 | DM_OUT_BUFFER.0,
            ))?;
            if (*mode).Anonymous1.Anonymous1.dmOrientation != orientation {
                bail!("The printer driver rejected the PDF page orientation");
            }
            // ResetDC must run between pages, before reading the new printable area.
            if ResetDCW(dc, mode).is_invalid() {
                bail!("ResetDCW failed: {}", std::io::Error::last_os_error());
            }
        }
        Ok(())
    }
}

fn check_properties(result: i32) -> ResultType<()> {
    if result != DOCUMENT_PROPERTIES_OK {
        bail!("DocumentPropertiesW failed: result={result}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::{
        core::w,
        Win32::Graphics::Gdi::{CreateDCW, DeleteDC},
    };

    #[test]
    #[ignore = "Requires Microsoft Print to PDF; run explicitly on Windows"]
    fn fixed_orientation_accepts_portrait_pdf_pages() {
        let _apartment = super::super::Apartment::initialize().unwrap();
        let mut printer = PagePrinter::new(HSTRING::from("Microsoft Print to PDF")).unwrap();
        let target = PCWSTR(printer.name.as_ptr());
        let dc = unsafe { CreateDCW(w!("WINSPOOL"), target, PCWSTR::null(), None) };
        assert!(!dc.is_invalid());
        unsafe {
            (*printer.mode.bytes().as_mut_ptr().cast::<DEVMODEW>()).dmFields &= !DM_ORIENTATION;
        }
        let result = printer.prepare(
            dc,
            Size {
                Width: 96.0,
                Height: 192.0,
            },
        );
        assert!(unsafe { DeleteDC(dc) }.as_bool());
        result.unwrap();
    }
}
