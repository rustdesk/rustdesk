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
#[cfg(all(target_os = "macos", feature = "hwcodec"))]
pub use ::hwcodec::ffmpeg_ram::pixelbuffer::PixelBufferFrame;
#[cfg(all(target_os = "macos", feature = "hwcodec"))]
pub type PixelBufferOutput = Option<::hwcodec::ffmpeg_ram::pixelbuffer::PixelBufferFrame>;
#[cfg(not(all(target_os = "macos", feature = "hwcodec")))]
pub type PixelBufferOutput = Option<std::convert::Infallible>;

/// The flag a decoder consults before it hands out decoded GPU frames.
///
/// Shared by reference with every decoder (see
/// `hwcodec::ffmpeg_ram::decode::Decoder::set_pixelbuffer_wanted`), so the answer
/// can change between frames.  Flipping it needs no decoder restart.
///
/// It starts `false`: the direct path is opt-in, because a decoder that hands out
/// a GPU frame skips the software transfer for it. Anything that has not asked
/// for these surfaces -- a non-Flutter UI, a build without the plugin entry
/// point, another process linking this crate -- keeps receiving software frames.
#[cfg(all(target_os = "macos", feature = "hwcodec"))]
pub fn gpu_frames_wanted_flag() -> std::sync::Arc<std::sync::atomic::AtomicBool> {
    use std::sync::{atomic::AtomicBool, Arc, OnceLock};
    static FLAG: OnceLock<Arc<AtomicBool>> = OnceLock::new();
    FLAG.get_or_init(|| Arc::new(AtomicBool::new(false))).clone()
}

/// Tell the decoder whether the renderer can take decoded GPU frames.
///
/// When it cannot, the decoder keeps producing software frames exactly like the
/// RGBA path always did.  This has to be a decoder-side decision: the decoder
/// skips its software transfer as soon as it hands out a GPU frame, so a caller
/// that rejects one afterwards has nothing left to present.
#[cfg(all(target_os = "macos", feature = "hwcodec"))]
pub fn set_gpu_frames_wanted(wanted: bool) {
    gpu_frames_wanted_flag().store(wanted, std::sync::atomic::Ordering::Relaxed);
}
