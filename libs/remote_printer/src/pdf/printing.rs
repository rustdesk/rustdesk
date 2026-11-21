use self::{apartment::Apartment, page_printer::PagePrinter};
use super::{handles::check_size, queues, raster::Rasterizer};
use hbb_common::{bail, ResultType};
use std::{mem::size_of, path::Path};
use windows::{
    core::{w, HSTRING, PCWSTR, PWSTR},
    Data::Pdf::{PdfDocument, PdfPage},
    Win32::{
        Graphics::Gdi::{
            CreateDCW, DeleteDC, GetDeviceCaps, StretchDIBits, BITMAPINFO, BITMAPINFOHEADER,
            BI_RGB, DIB_RGB_COLORS, HDC, HORZRES, LOGPIXELSX, LOGPIXELSY, SRCCOPY, VERTRES,
        },
        Graphics::Printing::GetDefaultPrinterW,
        Storage::Xps::{AbortDoc, EndDoc, EndPage, StartDocW, StartPage, DOCINFOW},
    },
};

mod apartment;
mod page_printer;

const DIPS_PER_INCH: f64 = 96.0;
const MAX_RASTER_DPI: f64 = 600.0;
const COLOR_BITS: u16 = 32;
const COLOR_PLANES: u16 = 1;

struct Layout {
    extent: [u32; 2],
    raster_extent: [u32; 2],
    offset: [i32; 2],
}

pub fn print_file(input: &Path, printer: &str) -> ResultType<u32> {
    super::identity::require_printing_user()?;
    let apartment = Apartment::initialize()?;
    let target = if printer.is_empty() {
        default_printer()?
    } else {
        printer.to_owned()
    };
    if super::is_remote_queue(&target)? {
        bail!("A received PDF cannot be sent to a remote printing queue");
    }
    let target = HSTRING::from(target);
    let input = if input.is_absolute() {
        input.to_path_buf()
    } else {
        std::env::current_dir()?.join(input)
    };
    let document = apartment.load_pdf(&input)?;
    if document.PageCount()? == 0 {
        bail!("The PDF has no pages");
    }
    let dc = unsafe {
        CreateDCW(
            w!("WINSPOOL"),
            PCWSTR(target.as_ptr()),
            PCWSTR::null(),
            None,
        )
    };
    if dc.is_invalid() {
        return Err(std::io::Error::last_os_error().into());
    }
    let result = submit(dc, &document, target);
    if !unsafe { DeleteDC(dc) }.as_bool() {
        let error = std::io::Error::last_os_error();
        hbb_common::log::error!("PDF printer DeleteDC failed: {error}");
        result?;
        return Err(error.into());
    }
    result
}

fn submit(dc: HDC, document: &PdfDocument, target: HSTRING) -> ResultType<u32> {
    let mut printer = PagePrinter::new(target)?;
    let info = DOCINFOW {
        cbSize: size_of::<DOCINFOW>() as i32,
        lpszDocName: w!("Remote PDF print job"),
        ..Default::default()
    };
    let job_id = unsafe { StartDocW(dc, &info) };
    check_gdi(job_id, "StartDocW")?;
    let result = draw_pages(dc, document, &mut printer)
        .and_then(|()| check_gdi(unsafe { EndDoc(dc) }, "EndDoc"));
    if result.is_err() {
        if let Err(error) = check_gdi(unsafe { AbortDoc(dc) }, "AbortDoc") {
            hbb_common::log::error!("Failed to abort the PDF print job: {error}");
        }
    }
    result?;
    Ok(u32::try_from(job_id)?)
}

fn draw_pages(dc: HDC, document: &PdfDocument, printer: &mut PagePrinter) -> ResultType<()> {
    for index in 0..document.PageCount()? {
        let page = document.GetPage(index)?;
        let result = draw_page(dc, &page, printer);
        let close = page.Close();
        result?;
        close?;
    }
    Ok(())
}

fn draw_page(dc: HDC, page: &PdfPage, printer: &mut PagePrinter) -> ResultType<()> {
    printer.prepare(dc, page.Size()?)?;
    let layout = layout(dc, page)?;
    let pixels = printer.renderer.render(page, layout.raster_extent)?;
    let width = i32::try_from(layout.raster_extent[0])?;
    let height = i32::try_from(layout.raster_extent[1])?;
    let bitmap = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: COLOR_PLANES,
            biBitCount: COLOR_BITS,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    check_gdi(unsafe { StartPage(dc) }, "StartPage")?;
    let copied = unsafe {
        StretchDIBits(
            dc,
            layout.offset[0],
            layout.offset[1],
            i32::try_from(layout.extent[0])?,
            i32::try_from(layout.extent[1])?,
            0,
            0,
            width,
            height,
            Some(pixels.as_ptr().cast()),
            &bitmap,
            DIB_RGB_COLORS,
            SRCCOPY,
        )
    };
    check_gdi(copied, "StretchDIBits")?;
    check_gdi(unsafe { EndPage(dc) }, "EndPage")?;
    Ok(())
}

fn layout(dc: HDC, page: &PdfPage) -> ResultType<Layout> {
    let size = page.Size()?;
    let page_extent = [f64::from(size.Width), f64::from(size.Height)];
    let printable = unsafe {
        [
            GetDeviceCaps(Some(dc), HORZRES),
            GetDeviceCaps(Some(dc), VERTRES),
        ]
    };
    let dpi = unsafe {
        [
            GetDeviceCaps(Some(dc), LOGPIXELSX),
            GetDeviceCaps(Some(dc), LOGPIXELSY),
        ]
    };
    page_layout(page_extent, printable, dpi)
}

fn page_layout(page_extent: [f64; 2], printable: [i32; 2], dpi: [i32; 2]) -> ResultType<Layout> {
    if printable.iter().chain(dpi.iter()).any(|value| *value <= 0)
        || page_extent
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
    {
        bail!("Invalid PDF or printer page dimensions");
    }
    let scale_x = f64::from(printable[0]) * DIPS_PER_INCH / f64::from(dpi[0]) / page_extent[0];
    let scale_y = f64::from(printable[1]) * DIPS_PER_INCH / f64::from(dpi[1]) / page_extent[1];
    let scale = scale_x.min(scale_y);
    let extent = [
        (page_extent[0] * scale * f64::from(dpi[0]) / DIPS_PER_INCH).round() as u32,
        (page_extent[1] * scale * f64::from(dpi[1]) / DIPS_PER_INCH).round() as u32,
    ];
    if extent.contains(&0) {
        bail!("The PDF page has no printable pixels");
    }
    let offset = [
        (printable[0] - i32::try_from(extent[0])?) / 2,
        (printable[1] - i32::try_from(extent[1])?) / 2,
    ];
    Ok(Layout {
        extent,
        raster_extent: std::array::from_fn(|axis| {
            let scale = (MAX_RASTER_DPI / f64::from(dpi[axis])).min(1.0);
            (f64::from(extent[axis]) * scale).round().max(1.0) as u32
        }),
        offset,
    })
}

fn check_gdi(value: i32, operation: &str) -> ResultType<()> {
    if value <= 0 {
        bail!(
            "{operation} failed: result={value}, last_error={}",
            std::io::Error::last_os_error()
        );
    }
    Ok(())
}

pub fn available() -> ResultType<bool> {
    super::identity::require_printing_user()?;
    let apartment = Apartment::initialize()?;
    if !apartment.pdf_type_present()? {
        return Ok(false);
    }
    super::raster::load_pdf_library()?;
    Ok(true)
}

pub fn printer_names() -> ResultType<Vec<String>> {
    Ok(queues::printer_names(queues::all()?))
}

fn default_printer() -> ResultType<String> {
    let mut length = 0;
    check_size(unsafe { GetDefaultPrinterW(None, &mut length).ok() })?;
    let mut name = vec![0u16; length as usize];
    unsafe { GetDefaultPrinterW(Some(PWSTR(name.as_mut_ptr())), &mut length).ok()? };
    let name = String::from_utf16(&name[..name.len().saturating_sub(1)])?;
    if name.is_empty() {
        bail!("No default printer is configured");
    }
    Ok(name)
}

#[cfg(test)]
mod orientation_tests;

#[cfg(test)]
mod tests {
    #[test]
    fn high_dpi_rasters_preserve_a4_and_letter_output_size() {
        for (printable, raster) in [
            ([9921, 14031], [4961, 7016]),
            ([10200, 13200], [5100, 6600]),
        ] {
            let page = printable.map(|pixels| f64::from(pixels) * 96.0 / 1200.0);
            let layout = super::page_layout(page, printable, [1200, 1200]).unwrap();
            assert_eq!(layout.raster_extent, raster);
            assert_eq!(layout.extent, printable.map(|pixels| pixels as u32));
            assert_eq!(layout.offset, [0, 0]);
        }
    }

    #[test]
    fn raster_retains_lower_dpi_and_centering() {
        for dpi in [300, 600] {
            let layout =
                super::page_layout([96.0, 192.0], [8 * dpi, 10 * dpi], [dpi, dpi]).unwrap();
            let extent = [5 * dpi as u32, 10 * dpi as u32];
            assert_eq!(layout.raster_extent, extent);
            assert_eq!(layout.extent, extent);
            assert_eq!(layout.offset, [3 * dpi / 2, 0]);
        }
    }

    #[test]
    fn raster_caps_each_axis_of_an_asymmetric_printer() {
        let layout = super::page_layout([96.0, 192.0], [8 * 1200, 10 * 300], [1200, 300]).unwrap();
        assert_eq!(layout.raster_extent, [5 * 600, 10 * 300]);
        assert_eq!(layout.extent, [5 * 1200, 10 * 300]);
        assert_eq!(layout.offset, [3 * 1200 / 2, 0]);
    }

    #[test]
    fn windows_pdf_renderer_initializes() {
        const PROBE_ATTEMPTS: usize = 2;
        for _ in 0..PROBE_ATTEMPTS {
            assert!(std::thread::spawn(super::available)
                .join()
                .unwrap()
                .unwrap());
        }
        let _apartment = super::Apartment::initialize().unwrap();
        super::Rasterizer::new().unwrap();
    }
}
