use std::{ops, ptr, slice};

use super::ffi::*;

pub struct Frame {
    surface: IOSurfaceRef,
    inner: &'static [u8],
    i420: *mut u8,
    i420_len: usize,
}

impl Frame {
    pub unsafe fn new(surface: IOSurfaceRef) -> Frame {
        CFRetain(surface);
        IOSurfaceIncrementUseCount(surface);

        IOSurfaceLock(surface, SURFACE_LOCK_READ_ONLY, ptr::null_mut());

        let inner = slice::from_raw_parts(
            IOSurfaceGetBaseAddress(surface) as *const u8,
            IOSurfaceGetAllocSize(surface),
        );

        Frame {
            surface,
            inner,
            i420: ptr::null_mut(),
            i420_len: 0,
        }
    }

    #[inline]
    pub fn inner(&self) -> &[u8] {
        self.inner
    }

    pub fn nv12_to_i420<'a>(&'a mut self, w: usize, h: usize, i420: &'a mut Vec<u8>) {
        unsafe {
            let plane0 = IOSurfaceGetBaseAddressOfPlane(self.surface, 0);
            let stride0 = IOSurfaceGetBytesPerRowOfPlane(self.surface, 0);
            let plane1 = IOSurfaceGetBaseAddressOfPlane(self.surface, 1);
            let stride1 = IOSurfaceGetBytesPerRowOfPlane(self.surface, 1);
            crate::common::nv12_to_i420(
                plane0 as _,
                stride0 as _,
                plane1 as _,
                stride1 as _,
                w,
                h,
                i420,
            );
            self.i420 = i420.as_mut_ptr() as _;
            self.i420_len = i420.len();
        }
    }
}

impl ops::Deref for Frame {
    type Target = [u8];
    fn deref<'a>(&'a self) -> &'a [u8] {
        if self.i420.is_null() {
            self.inner
        } else {
            unsafe {
                let inner = slice::from_raw_parts(self.i420 as *const u8, self.i420_len);
                inner
            }
        }
    }
}

impl Drop for Frame {
    fn drop(&mut self) {
        unsafe {
            IOSurfaceUnlock(self.surface, SURFACE_LOCK_READ_ONLY, ptr::null_mut());

            IOSurfaceDecrementUseCount(self.surface);
            CFRelease(self.surface);
        }
    }
}
