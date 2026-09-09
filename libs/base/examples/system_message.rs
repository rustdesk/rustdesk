extern crate base;
#[cfg(target_os = "linux")]
use base::platform::linux;
#[cfg(target_os = "macos")]
use base::platform::macos;

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
