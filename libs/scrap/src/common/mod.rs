pub use self::codec::*;

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
            } else {
                mod x11;
                pub use self::x11::*;
            }
        }
    } else if #[cfg(dxgi)] {
        mod dxgi;
        pub use self::dxgi::*;
    } else {
        //TODO: Fallback implementation.
    }
}

pub mod codec;
mod convert;
pub use self::convert::*;
pub const STRIDE_ALIGN: usize = 16; // commonly used in libvpx vpx_img_alloc caller

mod vpx;
