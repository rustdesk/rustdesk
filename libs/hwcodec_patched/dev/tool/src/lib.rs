#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use std::os::raw::c_void;

include!(concat!(env!("OUT_DIR"), "/tool_ffi.rs"));

pub struct Tool {
    inner: *mut c_void,
}

impl Tool {
    pub fn new(luid: i64) -> Result<Self, ()> {
        let inner = unsafe { tool_new(luid) };
        if inner.is_null() {
            Err(())
        } else {
            Ok(Self { inner })
        }
    }

    pub fn device(&mut self) -> *mut c_void {
        unsafe { tool_device(self.inner) }
    }

    pub fn get_texture(&mut self, width: i32, height: i32) -> *mut c_void {
        unsafe { tool_get_texture(self.inner, width, height) }
    }

    pub fn get_texture_size(&mut self, texture: *mut c_void) -> (i32, i32) {
        let mut width = 0;
        let mut height = 0;
        unsafe { tool_get_texture_size(self.inner, texture, &mut width, &mut height) }
        (width, height)
    }
}

impl Drop for Tool {
    fn drop(&mut self) {
        unsafe { tool_destroy(self.inner) }
        self.inner = std::ptr::null_mut();
    }
}
