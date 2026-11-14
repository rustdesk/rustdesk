// OpenHarmony platform module: hardware H264/H265 decoding via the NDK
// AVCodec C API (ohcodec), and the controlled-side capture source
// (AVScreenCapture raw stream) feeding the same channel contract as the
// Android JNI layer.

// Pure Rust (FFI declarations only, nothing links unless called): also
// compiled under cfg(test) so their unit tests run on the CI host.
#[cfg(any(target_env = "ohos", test))]
pub mod decoder;
#[cfg(any(target_env = "ohos", test))]
pub mod direct_render;
#[cfg(any(target_env = "ohos", test))]
pub mod ohcodec;
#[cfg(any(target_env = "ohos", test))]
pub mod ffi;

// Talks to the live capture service; ohos builds only.
#[cfg(target_env = "ohos")]
pub mod screencap;
