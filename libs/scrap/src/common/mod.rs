pub use self::vpxcodec::*;
use hbb_common::message_proto::{video_frame, VideoFrame};

cfg_if! {
    if #[cfg(quartz)] {
        mod quartz;
        pub use self::quartz::*;
    } else if #[cfg(x11)] {
        cfg_if! {
            if #[cfg(feature="wayland")] {
        mod linux;
        mod wayland;
        mod x11;
        pub use self::linux::*;
        pub use self::x11::Frame;
        pub use self::wayland::set_map_err;
            } else {
                mod x11;
                pub use self::x11::*;
            }
        }
    } else if #[cfg(dxgi)] {
        mod dxgi;
        pub use self::dxgi::*;
    } else if #[cfg(target_os = "android")] {
        mod android;
        pub use self::android::*;
    }else {
        //TODO: Fallback implementation.
    }
}

pub mod codec;
pub mod convert;
#[cfg(feature = "hwcodec")]
pub mod hwcodec;
#[cfg(feature = "mediacodec")]
pub mod mediacodec;
pub mod vpxcodec;
pub use self::convert::*;
pub const STRIDE_ALIGN: usize = 64; // commonly used in libvpx vpx_img_alloc caller
pub const HW_STRIDE_ALIGN: usize = 0; // recommended by av_frame_get_buffer

pub mod record;
mod vpx;

#[derive(Copy, Clone)]
pub enum ImageFormat {
    Raw,
    ABGR,
    ARGB,
}

pub struct ImageRgb {
    pub raw: Vec<u8>,
    pub w: usize,
    pub h: usize,
    fmt: ImageFormat,
    stride: usize,
}

impl ImageRgb {
    pub fn new(fmt: ImageFormat, stride: usize) -> Self {
        Self {
            raw: Vec::new(),
            w: 0,
            h: 0,
            fmt,
            stride,
        }
    }

    #[inline]
    pub fn fmt(&self) -> ImageFormat {
        self.fmt
    }

    #[inline]
    pub fn stride(&self) -> usize {
        self.stride
    }
}

#[inline]
pub fn would_block_if_equal(old: &mut Vec<u8>, b: &[u8]) -> std::io::Result<()> {
    // does this really help?
    if b == &old[..] {
        return Err(std::io::ErrorKind::WouldBlock.into());
    }
    old.resize(b.len(), 0);
    old.copy_from_slice(b);
    Ok(())
}

pub trait TraitCapturer {
    fn set_use_yuv(&mut self, use_yuv: bool);

    // We doesn't support
    #[cfg(not(any(target_os = "ios")))]
    fn frame<'a>(&'a mut self, timeout: std::time::Duration) -> std::io::Result<Frame<'a>>;

    #[cfg(windows)]
    fn is_gdi(&self) -> bool;
    #[cfg(windows)]
    fn set_gdi(&mut self) -> bool;
}

#[cfg(x11)]
#[inline]
pub fn is_x11() -> bool {
    hbb_common::platform::linux::is_x11_or_headless()
}

#[cfg(x11)]
#[inline]
pub fn is_cursor_embedded() -> bool {
    if is_x11() {
        x11::IS_CURSOR_EMBEDDED
    } else {
        wayland::is_cursor_embedded()
    }
}

#[cfg(not(x11))]
#[inline]
pub fn is_cursor_embedded() -> bool {
    false
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecName {
    VP8,
    VP9,
    H264(String),
    H265(String),
}

#[derive(PartialEq, Debug, Clone)]
pub enum CodecFormat {
    VP8,
    VP9,
    H264,
    H265,
    Unknown,
}

impl From<&VideoFrame> for CodecFormat {
    fn from(it: &VideoFrame) -> Self {
        match it.union {
            Some(video_frame::Union::Vp8s(_)) => CodecFormat::VP8,
            Some(video_frame::Union::Vp9s(_)) => CodecFormat::VP9,
            Some(video_frame::Union::H264s(_)) => CodecFormat::H264,
            Some(video_frame::Union::H265s(_)) => CodecFormat::H265,
            _ => CodecFormat::Unknown,
        }
    }
}

impl From<&CodecName> for CodecFormat {
    fn from(value: &CodecName) -> Self {
        match value {
            CodecName::VP8 => Self::VP8,
            CodecName::VP9 => Self::VP9,
            CodecName::H264(_) => Self::H264,
            CodecName::H265(_) => Self::H265,
        }
    }
}

impl ToString for CodecFormat {
    fn to_string(&self) -> String {
        match self {
            CodecFormat::VP8 => "VP8".into(),
            CodecFormat::VP9 => "VP9".into(),
            CodecFormat::H264 => "H264".into(),
            CodecFormat::H265 => "H265".into(),
            CodecFormat::Unknown => "Unknow".into(),
        }
    }
}
