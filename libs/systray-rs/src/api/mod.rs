#[cfg(target_os = "windows")]
#[path = "win32/mod.rs"]
pub mod api;

#[cfg(target_os = "linux")]
#[path = "linux/mod.rs"]
pub mod api;

#[cfg(target_os = "macos")]
#[path = "cocoa/mod.rs"]
pub mod api;
