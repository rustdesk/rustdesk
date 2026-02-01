use core::ffi::{c_int, c_void};

#[link(name = "avcodec")]
extern "C" {
    fn av_jni_set_java_vm(
       vm:  *mut c_void,
       ctx: *mut c_void,
    ) -> c_int;
}

pub fn ffmpeg_set_java_vm(vm: *mut c_void) {
    unsafe {
        av_jni_set_java_vm(
            vm as _,
            std::ptr::null_mut() as _,
        );
    }
}
