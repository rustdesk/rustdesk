#[cfg(quartz)]
extern crate block;
#[macro_use]
extern crate cfg_if;
pub use hbb_common::libc;
#[cfg(dxgi)]
extern crate winapi;

pub use common::*;

#[cfg(quartz)]
pub mod quartz;

#[cfg(x11)]
pub mod x11;

#[cfg(all(x11, feature = "wayland"))]
pub mod wayland;

#[cfg(dxgi)]
pub mod dxgi;

#[cfg(target_os = "android")]
pub mod android;

mod common;

/// Output of a decode call's GPU path.
///
/// `Some(frame)` means the decoded frame was delivered as a GPU frame and the
/// caller must present that instead of the software pixels. On platforms
/// without a GPU-frame path the option is always `None`, so callers need no
/// cfg of their own.
#[cfg(target_os = "macos")]
pub use ::hwcodec::ffmpeg_ram::pixelbuffer::PixelBufferFrame;
#[cfg(target_os = "macos")]
pub type PixelBufferOutput = Option<::hwcodec::ffmpeg_ram::pixelbuffer::PixelBufferFrame>;
#[cfg(not(target_os = "macos"))]
pub type PixelBufferOutput = Option<std::convert::Infallible>;
