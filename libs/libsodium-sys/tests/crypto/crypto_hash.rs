// crypto_hash.h

use libsodium_sys::*;
use std::ffi::CStr;

#[test]
fn test_crypto_hash_bytes() {
    assert!(unsafe { crypto_hash_bytes() } == crypto_hash_BYTES as usize)
}

#[test]
fn test_crypto_hash_primitive() {
    unsafe {
        let s = crypto_hash_primitive();
        let s = CStr::from_ptr(s);
        let p = CStr::from_bytes_with_nul(crypto_hash_PRIMITIVE).unwrap();
        assert_eq!(s, p);
    }
}
