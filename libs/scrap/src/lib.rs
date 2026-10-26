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

/// Tell the decoder whether the renderer can take decoded GPU frames.
///
/// When it cannot, the decoder keeps producing software frames exactly like the
/// RGBA path always did.  This has to be a decoder-side decision: the decoder
/// skips its software transfer as soon as it hands out a GPU frame, so a caller
/// that rejects one afterwards has nothing left to present.
#[cfg(target_os = "macos")]
pub fn set_gpu_frames_wanted(wanted: bool) {
    ::hwcodec::ffmpeg_ram::set_pixelbuffer_wanted(wanted);
}
