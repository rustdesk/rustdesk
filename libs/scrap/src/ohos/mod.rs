// OpenHarmony platform module: hardware H264/H265 decoding via the NDK
// AVCodec C API (ohcodec), and the controlled-side capture source
// (AVScreenCapture raw stream) feeding the same channel contract as the
// Android JNI layer.

pub mod ohcodec;

pub mod ffi;
pub mod screencap;
