use hbb_common::{anyhow::anyhow, bail, libloading::Library, ResultType};
use std::{
    ffi::{c_char, c_int, c_void, CStr, CString},
    os::unix::ffi::OsStrExt,
    path::Path,
    ptr,
    sync::OnceLock,
};

#[cfg(target_os = "linux")]
const LIBRARY: &str = "libcups.so.2";
#[cfg(target_os = "macos")]
const LIBRARY: &str = "/usr/lib/libcups.2.dylib";
const IPP_SUCCESS_MAX: c_int = 0x00ff;
const DOCUMENT_FORMAT: &[u8] = b"document-format\0";
const PDF_MIME: &[u8] = b"application/pdf\0";
const DOCUMENT_TITLE: &[u8] = b"Remote PDF print job\0";

#[repr(C)]
#[derive(Clone, Copy)]
struct OptionValue {
    name: *mut c_char,
    value: *mut c_char,
}

#[repr(C)]
struct Destination {
    name: *mut c_char,
    instance: *mut c_char,
    is_default: c_int,
    num_options: c_int,
    options: *mut OptionValue,
}

struct Api {
    get_destinations: unsafe extern "C" fn(*mut c_void, *mut *mut Destination) -> c_int,
    get_named: unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> *mut Destination,
    free: unsafe extern "C" fn(c_int, *mut Destination),
    print: unsafe extern "C" fn(
        *const c_char,
        *const c_char,
        *const c_char,
        c_int,
        *mut OptionValue,
    ) -> c_int,
    last_error: unsafe extern "C" fn() -> c_int,
    error_string: unsafe extern "C" fn() -> *const c_char,
    _library: &'static Library,
}

impl Api {
    fn load() -> ResultType<Self> {
        // CUPS registers TLS destructors that must remain loaded until process exit.
        static LIBRARY_HANDLE: OnceLock<Library> = OnceLock::new();
        let library = match LIBRARY_HANDLE.get() {
            Some(library) => library,
            None => {
                let loaded = unsafe { Library::new(LIBRARY)? };
                LIBRARY_HANDLE.get_or_init(|| loaded)
            }
        };
        unsafe {
            Ok(Self {
                get_destinations: *library.get(b"cupsGetDests2\0")?,
                get_named: *library.get(b"cupsGetNamedDest\0")?,
                free: *library.get(b"cupsFreeDests\0")?,
                print: *library.get(b"cupsPrintFile\0")?,
                last_error: *library.get(b"cupsLastError\0")?,
                error_string: *library.get(b"cupsLastErrorString\0")?,
                _library: library,
            })
        }
    }

    fn error(&self) -> hbb_common::anyhow::Error {
        let message = unsafe { (self.error_string)() };
        match text(message) {
            Ok(message) => anyhow!("CUPS: {message}"),
            Err(error) => error.context("CUPS did not return a valid error message"),
        }
    }
}

struct Destinations<'a> {
    api: &'a Api,
    count: c_int,
    data: *mut Destination,
}

impl Destinations<'_> {
    fn entries(&self) -> ResultType<&[Destination]> {
        if self.count < 0 {
            bail!("CUPS returned a negative destination count");
        }
        if self.count == 0 {
            return Ok(&[]);
        }
        if self.data.is_null() {
            bail!("CUPS returned no destination data");
        }
        Ok(unsafe { std::slice::from_raw_parts(self.data, self.count as usize) })
    }
}

impl Drop for Destinations<'_> {
    fn drop(&mut self) {
        unsafe { (self.api.free)(self.count, self.data) };
    }
}

pub(super) fn available() -> ResultType<bool> {
    Api::load()?;
    Ok(true)
}

pub(super) fn printer_names() -> ResultType<Vec<String>> {
    let api = Api::load()?;
    let mut data = ptr::null_mut();
    let count = unsafe { (api.get_destinations)(ptr::null_mut(), &mut data) };
    let destinations = Destinations {
        api: &api,
        count,
        data,
    };
    if count <= 0 && unsafe { (api.last_error)() } > IPP_SUCCESS_MAX {
        return Err(api.error());
    }
    let mut names = Vec::new();
    for destination in destinations.entries()? {
        if remote_queue(destination)? {
            continue;
        }
        names.push(destination_name(destination)?);
    }
    Ok(names)
}

pub(super) fn print_file(path: &Path, printer: &str) -> ResultType<u32> {
    let api = Api::load()?;
    let (name, instance) = printer.split_once('/').map_or((printer, ""), |parts| parts);
    let name = CString::new(name)?;
    let instance = CString::new(instance)?;
    let data = unsafe {
        (api.get_named)(
            ptr::null_mut(),
            optional_name(&name),
            optional_name(&instance),
        )
    };
    if data.is_null() {
        return Err(api
            .error()
            .context("The selected/default CUPS printer is unavailable"));
    }
    let destinations = Destinations {
        api: &api,
        count: 1,
        data,
    };
    let destination = destinations
        .entries()?
        .first()
        .ok_or_else(|| anyhow!("CUPS returned no selected printer"))?;
    if remote_queue(destination)? {
        bail!("A received PDF cannot be sent to a remote printing queue");
    }
    let mut options = pdf_options(destination)?;
    let path = CString::new(path.as_os_str().as_bytes())?;
    let job = unsafe {
        (api.print)(
            destination.name,
            path.as_ptr(),
            DOCUMENT_TITLE.as_ptr().cast(),
            c_int::try_from(options.len())?,
            options.as_mut_ptr(),
        )
    };
    if job <= 0 {
        return Err(api.error());
    }
    Ok(u32::try_from(job)?)
}

fn pdf_options(destination: &Destination) -> ResultType<Vec<OptionValue>> {
    let mut options = options(destination)?
        .iter()
        .copied()
        .filter(|option| {
            !option.name.is_null()
                && unsafe { CStr::from_ptr(option.name).to_bytes() } != b"document-format"
        })
        .collect::<Vec<_>>();
    options.push(OptionValue {
        name: DOCUMENT_FORMAT.as_ptr().cast_mut().cast(),
        value: PDF_MIME.as_ptr().cast_mut().cast(),
    });
    Ok(options)
}

fn optional_name(value: &CStr) -> *const c_char {
    if value.is_empty() {
        ptr::null()
    } else {
        value.as_ptr()
    }
}

fn destination_name(destination: &Destination) -> ResultType<String> {
    let name = text(destination.name)?;
    if destination.instance.is_null() {
        return Ok(name.to_owned());
    }
    Ok(format!("{name}/{}", text(destination.instance)?))
}

fn remote_queue(destination: &Destination) -> ResultType<bool> {
    let name = text(destination.name)?;
    let app = crate::get_app_name();
    if name == format!("{app} Printer") || name == format!("{app} PDF Printer") {
        return Ok(true);
    }
    for option in options(destination)? {
        if text(option.name)? == "printer-info"
            && text(option.value)?.starts_with("RustDesk PDF printer v1:")
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn options(destination: &Destination) -> ResultType<&[OptionValue]> {
    if destination.num_options < 0 {
        bail!("CUPS returned a negative option count");
    }
    if destination.num_options == 0 {
        return Ok(&[]);
    }
    if destination.options.is_null() {
        bail!("CUPS returned no printer options");
    }
    Ok(
        unsafe {
            std::slice::from_raw_parts(destination.options, destination.num_options as usize)
        },
    )
}

fn text(value: *const c_char) -> ResultType<String> {
    if value.is_null() {
        bail!("CUPS returned an empty string pointer");
    }
    Ok(unsafe { CStr::from_ptr(value).to_str()? }.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cups_worker_exits_after_releasing_its_api() {
        std::thread::spawn(|| {
            let api = Api::load().unwrap();
            // Initialize CUPS thread-local state without contacting a print server.
            unsafe { (api.last_error)() };
        })
        .join()
        .unwrap();
    }
}
