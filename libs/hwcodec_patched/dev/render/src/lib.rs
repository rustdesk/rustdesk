#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use std::os::raw::c_void;

include!(concat!(env!("OUT_DIR"), "/render_ffi.rs"));

pub struct Render {
    inner: *mut c_void,
}

impl Render {
    pub fn new(luid: i64, input_shared_handle: bool) -> Result<Self, ()> {
        let inner = unsafe { CreateDXGIRender(luid, input_shared_handle) };
        if inner.is_null() {
            Err(())
        } else {
            Ok(Self { inner })
        }
    }

    pub unsafe fn render(&mut self, tex: *mut c_void) -> Result<(), i32> {
        let result = DXGIRenderTexture(self.inner, tex);
        if result == 0 {
            Ok(())
        } else {
            Err(result)
        }
    }

    pub unsafe fn device(&mut self) -> *mut c_void {
        DXGIDevice(self.inner)
    }

    pub unsafe fn drop(&mut self) {
        DestroyDXGIRender(self.inner);
    }
}
