pub mod idd;
use std::ffi::CStr;

pub const DRIVER_INSTALL_PATH: &str = "GetRytRemoteIddDriver/GetRytRemoteIddDriver.inf";
pub const DRIVER_DOWNLOAD_URL: &str = "";

pub unsafe fn get_last_msg() -> Result<&'static str, std::str::Utf8Error> {
    CStr::from_ptr(idd::GetLastMsg()).to_str()
}
