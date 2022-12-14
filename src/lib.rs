#[cfg(not(any(target_os = "ios")))]
/// cbindgen:ignore
pub mod platform;
mod keyboard;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub use platform::{get_cursor, get_cursor_data, get_cursor_pos, start_os_service};
#[cfg(not(any(target_os = "ios")))]
/// cbindgen:ignore
mod server;
#[cfg(not(any(target_os = "ios")))]
pub use self::server::*;
mod client;
#[cfg(not(any(target_os = "ios")))]
mod lan;
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
#[cfg(any(target_os = "android", target_os = "ios", feature = "flutter"))]
mod bridge_generated;
#[cfg(any(target_os = "android", target_os = "ios", feature = "flutter"))]
pub mod flutter;
#[cfg(any(target_os = "android", target_os = "ios", feature = "flutter"))]
pub mod flutter_ffi;
use common::*;
#[cfg(feature = "cli")]
pub mod cli;
#[cfg(not(any(target_os = "android", target_os = "ios", feature = "cli")))]
pub mod core_main;
#[cfg(all(windows, feature = "hbbs"))]
mod hbbs;
mod lang;
#[cfg(windows)]
mod license;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
mod port_forward;

mod tray;

mod ui_cm_interface;
mod ui_interface;
mod ui_session_interface;

mod hbbs_http;

#[cfg(windows)]
pub mod clipboard_file;

#[cfg(all(windows, feature = "with_rc"))]
pub mod rc;
