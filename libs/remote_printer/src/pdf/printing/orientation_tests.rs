use super::*;
use crate::pdf::handles::{Buffer, Printer};
use std::{
    io::Write,
    time::{Duration, Instant},
};
use windows::Win32::{
    Foundation::ERROR_INVALID_PARAMETER,
    Graphics::Printing::{
        GetJobW, JOB_INFO_1W, JOB_STATUS_ERROR, JOB_STATUS_PRINTED, PRINTER_ACCESS_USE,
    },
};

const PRINTER: &str = "Microsoft Print to PDF";
const OUTPUT_TIMEOUT: Duration = Duration::from_secs(10);
const OUTPUT_POLL: Duration = Duration::from_millis(25);
const JOB_INFO_LEVEL: u32 = 1;

#[test]
#[ignore = "Requires Microsoft Print to PDF; run explicitly on Windows"]
fn printed_pdf_preserves_mixed_page_orientations() {
    let directory = std::env::temp_dir().join(format!(
        "rustdesk-pdf-orientation-{}",
        hbb_common::uuid::Uuid::new_v4()
    ));
    crate::pdf::identity::create_private_directory(&directory).unwrap();
    let input = directory.join("input.pdf");
    let output = directory.join("output.pdf");
    std::fs::write(&input, mixed_pdf()).unwrap();
    std::thread::scope(|scope| {
        scope.spawn(|| {
            let apartment = Apartment::initialize().unwrap();
            let document = apartment.load_pdf(&input).unwrap();
            assert_eq!(orientations(&document), [true, false, true, false]);
            print_to_file(&document, &output);
        });
    });
    let apartment = Apartment::initialize().unwrap();
    let printed = apartment.load_pdf(&output).unwrap();
    let actual = orientations(&printed);
    drop(printed);
    std::fs::remove_file(input).unwrap();
    std::fs::remove_file(output).unwrap();
    std::fs::remove_dir(directory).unwrap();
    assert_eq!(actual, [true, false, true, false]);
}

fn print_to_file(document: &PdfDocument, output: &Path) {
    let name = HSTRING::from(PRINTER);
    let destination = HSTRING::from(output.as_os_str());
    let dc = unsafe { CreateDCW(w!("WINSPOOL"), PCWSTR(name.as_ptr()), PCWSTR::null(), None) };
    assert!(!dc.is_invalid());
    let mut printer = PagePrinter::new(name).unwrap();
    let info = DOCINFOW {
        cbSize: size_of::<DOCINFOW>() as i32,
        lpszDocName: w!("RustDesk PDF orientation regression"),
        lpszOutput: PCWSTR(destination.as_ptr()),
        ..Default::default()
    };
    let job_id = unsafe { StartDocW(dc, &info) };
    check_gdi(job_id, "StartDocW").unwrap();
    let result = draw_pages(dc, document, &mut printer)
        .and_then(|()| check_gdi(unsafe { EndDoc(dc) }, "EndDoc"));
    if result.is_err() {
        check_gdi(unsafe { AbortDoc(dc) }, "AbortDoc").unwrap();
    }
    assert!(unsafe { DeleteDC(dc) }.as_bool());
    result.unwrap();
    wait_for_job(job_id as u32);
}

fn wait_for_job(id: u32) {
    let printer = Printer::open(PRINTER, PRINTER_ACCESS_USE).unwrap();
    let deadline = Instant::now() + OUTPUT_TIMEOUT;
    loop {
        let result = Buffer::query(|buffer, needed| unsafe {
            GetJobW(printer.0, id, JOB_INFO_LEVEL, buffer, needed).ok()
        });
        match result {
            Ok(Some(buffer)) => {
                let info = unsafe { buffer.item::<JOB_INFO_1W>(0) }.unwrap();
                assert_eq!(info.Status & JOB_STATUS_ERROR, 0, "Print job failed");
                if info.Status & JOB_STATUS_PRINTED != 0 {
                    return;
                }
            }
            Err(error)
                if error
                    .downcast_ref::<windows::core::Error>()
                    .is_some_and(|error| error.code() == ERROR_INVALID_PARAMETER.to_hresult()) =>
            {
                return
            }
            result => panic!("Cannot query print job: {:?}", result.err()),
        }
        assert!(Instant::now() < deadline, "Print job did not complete");
        std::thread::sleep(OUTPUT_POLL);
    }
}

fn orientations(document: &PdfDocument) -> Vec<bool> {
    (0..document.PageCount().unwrap())
        .map(|index| {
            let page = document.GetPage(index).unwrap();
            let size = page.Size().unwrap();
            page.Close().unwrap();
            size.Width > size.Height
        })
        .collect()
}

fn mixed_pdf() -> Vec<u8> {
    let mut objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R 4 0 R 5 0 R 6 0 R] /Count 4 >>".to_owned(),
    ];
    for (media_box, rotation) in [
        ("792 612", 0),
        ("612 792", 0),
        ("612 792", 90),
        ("612 792", 0),
    ] {
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {media_box}] /Rotate {rotation} /Resources << >> /Contents 7 0 R >>"
        ));
    }
    let content = "0 0 1 rg 36 36 180 72 re f\n";
    objects.push(format!(
        "<< /Length {} >>\nstream\n{content}endstream",
        content.len()
    ));
    let mut pdf = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        writeln!(pdf, "{} 0 obj\n{object}\nendobj", index + 1).unwrap();
    }
    let start_xref = pdf.len();
    writeln!(pdf, "xref\n0 {}\n0000000000 65535 f ", objects.len() + 1).unwrap();
    for offset in offsets {
        writeln!(pdf, "{offset:010} 00000 n ").unwrap();
    }
    writeln!(
        pdf,
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{start_xref}\n%%EOF",
        objects.len() + 1
    )
    .unwrap();
    pdf
}
