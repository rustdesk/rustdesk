extern crate hbb_common;
#[cfg(target_os = "linux")]
use hbb_common::platform::linux;
#[cfg(target_os = "macos")]
use hbb_common::platform::macos;

fn main() {
    #[cfg(target_os = "linux")]
    let res = linux::system_message("test title", "test message", true);
    #[cfg(target_os = "macos")]
    let res = macos::alert(
        "System Preferences".to_owned(),
        "warning".to_owned(),
        "test title".to_owned(),
        "test message".to_owned(),
        ["Ok".to_owned()].to_vec(),
    );
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    println!("result {:?}", &res);
}
