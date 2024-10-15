use std::mem;

use super::ffi::*;

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
#[repr(C)]
pub struct Display(u32);

impl Display {
    pub fn primary() -> Display {
        Display(unsafe { CGMainDisplayID() })
    }

    pub fn online() -> Result<Vec<Display>, CGError> {
        unsafe {
            #[allow(invalid_value)]
            let mut arr: [u32; 16] = mem::MaybeUninit::uninit().assume_init();
            let mut len: u32 = 0;

            match CGGetOnlineDisplayList(16, arr.as_mut_ptr(), &mut len) {
                CGError::Success => (),
                x => return Err(x),
            }

            let mut res = Vec::with_capacity(16);
            for i in 0..len as usize {
                res.push(Display(*arr.get_unchecked(i)));
            }
            Ok(res)
        }
    }

    pub fn id(self) -> u32 {
        self.0
    }

    pub fn width(self) -> usize {
        let w = unsafe { CGDisplayPixelsWide(self.0) };
        let s = self.scale();
        if s > 1.0 {
            ((w as f64) * s).round() as usize
        } else {
            w
        }
    }

    pub fn height(self) -> usize {
        let h = unsafe { CGDisplayPixelsHigh(self.0) };
        let s = self.scale();
        if s > 1.0 {
            ((h as f64) * s).round() as usize
        } else {
            h
        }
    }

    pub fn is_builtin(self) -> bool {
        unsafe { CGDisplayIsBuiltin(self.0) != 0 }
    }

    pub fn is_primary(self) -> bool {
        unsafe { CGDisplayIsMain(self.0) != 0 }
    }

    pub fn is_active(self) -> bool {
        unsafe { CGDisplayIsActive(self.0) != 0 }
    }

    pub fn is_online(self) -> bool {
        unsafe { CGDisplayIsOnline(self.0) != 0 }
    }

    pub fn scale(self) -> f64 {
        let s = unsafe { BackingScaleFactor(self.0) as _ };
        if s > 1. {
            let enable_retina = super::ENABLE_RETINA.lock().unwrap().clone();
            if enable_retina {
                return s;
            }
        }
        1.
    }

    pub fn bounds(self) -> CGRect {
        unsafe { CGDisplayBounds(self.0) }
    }
}
