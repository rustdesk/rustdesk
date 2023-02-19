extern crate hbb_common;

fn main() {
    #[cfg(target_os = "linux")]
    linux::system_message("test title", "test message", true).ok();
    #[cfg(target_os = "macos")]
    macos::alert(
        "RustDesk".to_owned(),
        "critical".to_owned(),
        "test title".to_owned(),
        "test message".to_owned(),
        ["Ok".to_owned()].to_vec(),
    )
    .ok();
}
