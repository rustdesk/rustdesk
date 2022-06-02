#[cfg(not(any(target_os = "ios")))]
/// cbindgen:ignore
pub mod platform;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub use platform::{get_cursor, get_cursor_data, get_cursor_pos, start_os_service};
#[cfg(not(any(target_os = "ios")))]
/// cbindgen:ignore
mod server;
#[cfg(not(any(target_os = "ios")))]
pub use self::server::*;
mod client;
#[cfg(not(any(target_os = "ios")))]
mod rendezvous_mediator;
#[cfg(not(any(target_os = "ios")))]
pub use self::rendezvous_mediator::*;
/// cbindgen:ignore
pub mod common;
#[cfg(not(any(target_os = "ios")))]
pub mod ipc;
#[cfg(not(any(target_os = "android", target_os = "ios", feature = "cli")))]
pub mod ui;
mod version;
pub use version::*;
#[cfg(any(target_os = "android", target_os = "ios"))]
mod bridge_generated;
#[cfg(any(target_os = "android", target_os = "ios"))]
pub mod mobile;
#[cfg(any(target_os = "android", target_os = "ios"))]
pub mod mobile_ffi;
use common::*;
#[cfg(feature = "cli")]
pub mod cli;
#[cfg(all(windows, feature = "hbbs"))]
mod hbbs;
mod lang;
#[cfg(windows)]
mod license;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
mod port_forward;
#[cfg(windows)]
mod tray;

#[cfg(windows)]
pub mod clipboard_file;

#[cfg(all(windows, feature = "with_rc"))]
pub mod rc;
