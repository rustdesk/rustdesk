mod keyboard;
#[cfg(all(target_env = "ohos", not(feature = "flutter")))]
compile_error!(
    "OpenHarmony controller builds require the `flutter` feature as the existing RustDesk session compatibility layer; this does not build or embed the Flutter UI."
);
#[cfg(target_env = "ohos")]
extern crate rdev_ohos as rdev;
/// cbindgen:ignore
pub mod platform;
#[cfg(not(any(target_os = "android", target_os = "ios", target_env = "ohos")))]
pub use platform::{
    clip_cursor, get_cursor, get_cursor_data, get_cursor_pos, get_focused_display, set_cursor_pos,
    start_os_service,
};
#[cfg(not(target_os = "ios"))]
/// cbindgen:ignore
mod server;
#[cfg(not(target_os = "ios"))]
pub use self::server::*;
mod client;
pub use client::peer_online::query_online_states_result;
mod lan;
#[cfg(not(target_os = "ios"))]
mod rendezvous_mediator;
#[cfg(not(target_os = "ios"))]
pub use self::rendezvous_mediator::*;
/// cbindgen:ignore
pub mod common;
#[cfg(not(any(target_os = "ios")))]
pub mod ipc;
#[cfg(not(any(
    target_os = "android",
    target_os = "ios",
    target_env = "ohos",
    feature = "flutter"
)))]
pub mod ui;
mod version;
pub use version::*;
#[cfg(all(
    any(target_os = "android", target_os = "ios", feature = "flutter"),
    not(target_env = "ohos")
))]
mod bridge_generated;
#[cfg(any(target_os = "android", target_os = "ios", feature = "flutter"))]
pub mod flutter;
#[cfg(any(target_os = "android", target_os = "ios", feature = "flutter"))]
pub mod flutter_ffi;
use common::*;
mod auth_2fa;
#[cfg(not(target_os = "ios"))]
mod clipboard;
#[cfg(not(any(target_os = "android", target_os = "ios", target_env = "ohos")))]
pub mod core_main;
mod custom_server;
mod lang;
#[cfg(not(any(target_os = "android", target_os = "ios", target_env = "ohos")))]
mod port_forward;

#[cfg(not(any(target_os = "android", target_os = "ios", target_env = "ohos")))]
mod tray;

#[cfg(not(any(target_os = "android", target_os = "ios", target_env = "ohos")))]
mod whiteboard;

#[cfg(not(any(target_os = "android", target_os = "ios", target_env = "ohos")))]
mod updater;

mod ui_cm_interface;
mod ui_interface;
mod ui_session_interface;

mod hbbs_http;
pub use hbbs_http::validate_rustdesk_api_server;

#[cfg(any(
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos")),
    target_os = "macos",
    all(target_env = "ohos", feature = "cliprdr-file-service")
))]
pub mod clipboard_file;

pub mod privacy_mode;

#[cfg(windows)]
pub mod virtual_display_manager;

mod kcp_stream;
