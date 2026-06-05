/// SAFETY: the returned Vec must not be resized or reserverd
pub unsafe fn aligned_u8_vec(cap: usize, align: usize) -> Vec<u8> {
    use std::alloc::*;

    let layout =
        Layout::from_size_align(cap, align).expect("invalid aligned value, must be power of 2");
    unsafe {
        let ptr = alloc(layout);
        if ptr.is_null() {
            panic!("failed to allocate {} bytes", cap);
        }
        Vec::from_raw_parts(ptr, 0, cap)
    }
}
