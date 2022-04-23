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
pub const STRIDE_ALIGN: usize = 64; // commonly used in libvpx vpx_img_alloc caller

mod vpx;

#[inline]
pub fn would_block_if_equal(old: &mut Vec<u128>, b: &[u8]) -> std::io::Result<()> {
    let b = unsafe {
        std::slice::from_raw_parts::<u128>(b.as_ptr() as _, b.len() / 16)
    };
    if b == &old[..] {
        return Err(std::io::ErrorKind::WouldBlock.into());
    }
    old.resize(b.len(), 0);
    old.copy_from_slice(b);
    Ok(())
}
