extern crate hbb_common;
#[cfg(target_os = "linux")]
use hbb_common::platform::linux;
#[cfg(target_os = "macos")]
use hbb_common::platform::macos;

fn main() {
    #[cfg(target_os = "linux")]
    linux::system_message("test title", "test message", true).ok();
    #[cfg(target_os = "macos")]
    macos::alert(
        "RustDesk".to_owned(),
        "warning".to_owned(),
        "test title".to_owned(),
        "test message".to_owned(),
        ["Ok".to_owned()].to_vec(),
    )
    .ok();
}
