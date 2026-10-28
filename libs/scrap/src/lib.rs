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

/// A decoder-side hand-off switch: "can the renderer take decoded GPU frames?".
///
/// One of these belongs to **one** decoder owner -- a session, or a display's
/// video handler -- never to the process. Two sessions can render differently
/// (one through the plugin texture, one through RGBA), and a shared switch makes
/// them fight: turning the direct path on for the texture session would make the
/// software session's decoder skip its software transfer and stall, while turning
/// it off for the software session would disable the optimisation everywhere.
///
/// The handle is shared by reference with the decoder
/// (`hwcodec::ffmpeg_ram::decode::Decoder::set_pixelbuffer_wanted`), so the answer
/// can change between frames without restarting the decoder.
///
/// It starts `false`. Every owner that has not asked for GPU surfaces -- a build
/// without the plugin entry point, texture rendering off, a UI that never opts in
/// -- keeps receiving software frames, which is the behaviour this crate had
/// before the direct path existed.
#[cfg(all(target_os = "macos", feature = "hwcodec"))]
pub type GpuFramesWanted = std::sync::Arc<std::sync::atomic::AtomicBool>;

/// Create a fresh hand-off switch. Each decoder owner gets its own.
#[cfg(all(target_os = "macos", feature = "hwcodec"))]
pub fn gpu_frames_wanted_flag(wanted: bool) -> GpuFramesWanted {
    std::sync::Arc::new(std::sync::atomic::AtomicBool::new(wanted))
}
