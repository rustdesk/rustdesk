use super::service::{EmptyExtraFieldService, GenericService, Service};
use hbb_common::{bail, dlopen::symbor::Library, log, ResultType};
use std::{
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

pub const NAME: &'static str = "remote-printer";

const LIB_NAME_PRINTER_DRIVER_ADAPTER: &str = "printer_driver_adapter";

// Return 0 if success, otherwise return error code.
pub type Init = fn(tag_name: *const i8) -> i32;
pub type Uninit = fn();
// dur_mills: Get the file generated in the last `dur_mills` milliseconds.
// file: The file path. The caller should free the memory.
pub type GetPrnFile = fn(dur_mills: u32, file: *mut *mut i8);

macro_rules! make_lib_wrapper {
    ($($field:ident : $tp:ty),+) => {
        struct LibWrapper {
            _lib: Option<Library>,
            $($field: Option<$tp>),+
        }

        impl LibWrapper {
            fn new() -> Self {
                let lib_name = match get_lib_name() {
                    Ok(name) => name,
                    Err(e) => {
                        log::warn!("Failed to get lib name, {}", e);
                        return Self {
                            _lib: None,
                            $( $field: None ),+
                        };
                    }
                };
                let lib = match Library::open(&lib_name) {
                    Ok(lib) => Some(lib),
                    Err(e) => {
                        log::warn!("Failed to load library {}, {}", &lib_name, e);
                        None
                    }
                };

                $(let $field = if let Some(lib) = &lib {
                    match unsafe { lib.symbol::<$tp>(stringify!($field)) } {
                        Ok(m) => {
                            log::info!("method found {}", stringify!($field));
                            Some(*m)
                        },
                        Err(e) => {
                            log::warn!("Failed to load func {}, {}", stringify!($field), e);
                            None
                        }
                    }
                } else {
                    None
                };)+

                Self {
                    _lib: lib,
                    $( $field ),+
                }
            }
        }

        impl Default for LibWrapper {
            fn default() -> Self {
                Self::new()
            }
        }
    }
}

make_lib_wrapper!(
    init: Init,
    uninit: Uninit,
    get_prn_file: GetPrnFile
);

lazy_static::lazy_static! {
    static ref LIB_WRAPPER: Arc<Mutex<LibWrapper>> = Default::default();
}

fn get_lib_name() -> ResultType<String> {
    let exe_file = std::env::current_exe()?;
    if let Some(cur_dir) = exe_file.parent() {
        let dll_name = format!("{}.dll", LIB_NAME_PRINTER_DRIVER_ADAPTER);
        let full_path = cur_dir.join(dll_name);
        if !full_path.exists() {
            bail!("{} not found", full_path.to_string_lossy().as_ref());
        } else {
            Ok(full_path.to_string_lossy().into_owned())
        }
    } else {
        bail!(
            "Invalid exe parent for {}",
            exe_file.to_string_lossy().as_ref()
        );
    }
}

pub fn init(app_name: &str) -> ResultType<()> {
    let lib_wrapper = LIB_WRAPPER.lock().unwrap();
    let Some(fn_init) = lib_wrapper.init.as_ref() else {
        bail!("Failed to load func init");
    };

    let tag_name = std::ffi::CString::new(app_name)?;
    let ret = fn_init(tag_name.as_ptr());
    if ret != 0 {
        bail!("Failed to init printer driver");
    }
    Ok(())
}

pub fn uninit() {
    let lib_wrapper = LIB_WRAPPER.lock().unwrap();
    if let Some(fn_uninit) = lib_wrapper.uninit.as_ref() {
        fn_uninit();
    }
}

fn get_prn_file(dur_mills: u32) -> ResultType<String> {
    let lib_wrapper = LIB_WRAPPER.lock().unwrap();
    if let Some(fn_get_prn_file) = lib_wrapper.get_prn_file.as_ref() {
        let mut file = std::ptr::null_mut();
        fn_get_prn_file(dur_mills, &mut file);
        if file.is_null() {
            return Ok("".to_string());
        }
        let file_path = unsafe {
            std::ffi::CStr::from_ptr(file)
                .to_string_lossy()
                .into_owned()
        };
        unsafe {
            hbb_common::libc::free(file as *mut std::ffi::c_void);
        }
        Ok(file_path)
    } else {
        bail!("Failed to load func get_prn_file");
    }
}

pub fn new(name: String) -> GenericService {
    let svc = EmptyExtraFieldService::new(name, false);
    GenericService::run(&svc.clone(), run);
    svc.sp
}

fn run(sp: EmptyExtraFieldService) -> ResultType<()> {
    while sp.ok() {
        let file = get_prn_file(1000)?;
        if !file.is_empty() {
            log::info!("Get prn file: {}", file);
            crate::server::on_printer_file(file);
        }
        thread::sleep(Duration::from_millis(300));
    }
    Ok(())
}
