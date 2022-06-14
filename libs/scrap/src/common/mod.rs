pub use self::vpxcodec::*;

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
    } else if #[cfg(target_os = "android")] {
        mod android;
        pub use self::android::*;
    }else {
        //TODO: Fallback implementation.
    }
}

pub mod codec;
mod convert;
#[cfg(feature = "hwcodec")]
pub mod hwcodec;
pub mod vpxcodec;
pub use self::convert::*;
pub const STRIDE_ALIGN: usize = 64; // commonly used in libvpx vpx_img_alloc caller
pub const HW_STRIDE_ALIGN: usize = 0; // recommended by av_frame_get_buffer

mod vpx;

#[inline]
pub fn would_block_if_equal(old: &mut Vec<u128>, b: &[u8]) -> std::io::Result<()> {
    let b = unsafe { std::slice::from_raw_parts::<u128>(b.as_ptr() as _, b.len() / 16) };
    if b == &old[..] {
        return Err(std::io::ErrorKind::WouldBlock.into());
    }
    old.resize(b.len(), 0);
    old.copy_from_slice(b);
    Ok(())
}
