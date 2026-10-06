use std::{ops, ptr, slice};

use super::ffi::*;

pub struct Frame {
    surface: Option<IOSurfaceRef>,
    inner: &'static [u8],
    pub(crate) bgra: Vec<u8>,
    bgra_stride: usize,
}

// Frame is shared between the poll thread and the caller through
// Arc<Mutex<Option<Frame>>>. The IOSurfaceRef is only accessed while
// holding the lock, and all refcounting (CFRetain/CFRelease) is done
// inside Frame::new/Drop, so it is safe to send/sync between threads.
unsafe impl Send for Frame {}
unsafe impl Sync for Frame {}

impl Frame {
    /// Create a Frame from an IOSurface (CGDisplayStream path)
    pub unsafe fn new(surface: IOSurfaceRef) -> Frame {
        CFRetain(surface);
        IOSurfaceIncrementUseCount(surface);

        IOSurfaceLock(surface, SURFACE_LOCK_READ_ONLY, ptr::null_mut());

        let inner = slice::from_raw_parts(
            IOSurfaceGetBaseAddress(surface) as *const u8,
            IOSurfaceGetAllocSize(surface),
        );

        Frame {
            surface: Some(surface),
            inner,
            bgra: Vec::new(),
            bgra_stride: 0,
        }
    }

    /// Create a Frame from raw BGRA pixel data (CGWindowList fallback path)
    /// No IOSurface is associated; the data lives in `bgra` directly.
    pub fn from_bytes(data: Vec<u8>, stride: usize, _height: usize) -> Frame {
        Frame {
            surface: None,
            inner: &[],
            bgra: data,
            bgra_stride: stride,
        }
    }

    #[inline]
    pub fn inner(&self) -> &[u8] {
        &self.inner
    }

    pub fn stride(&self) -> usize {
        self.bgra_stride
    }

    pub fn surface_to_bgra<'a>(&'a mut self, h: usize) {
        if let Some(surface) = self.surface {
            unsafe {
                let plane0 = IOSurfaceGetBaseAddressOfPlane(surface, 0) as *const u8;
                self.bgra_stride = IOSurfaceGetBytesPerRowOfPlane(surface, 0);
                self.bgra.resize(self.bgra_stride * h, 0);
                std::ptr::copy_nonoverlapping(
                    plane0,
                    self.bgra.as_mut_ptr(),
                    self.bgra_stride * h,
                );
            }
        }
        // If surface is None, bgra already holds the data from CGWindowList
    }
}

impl ops::Deref for Frame {
    type Target = [u8];
    fn deref<'a>(&'a self) -> &'a [u8] {
        &self.bgra
    }
}

impl Drop for Frame {
    fn drop(&mut self) {
        if let Some(surface) = self.surface {
            unsafe {
                IOSurfaceUnlock(surface, SURFACE_LOCK_READ_ONLY, ptr::null_mut());

                IOSurfaceDecrementUseCount(surface);
                CFRelease(surface);
            }
        }
    }
}