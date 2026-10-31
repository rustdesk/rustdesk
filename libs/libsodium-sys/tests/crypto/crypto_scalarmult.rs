// crypto_scalarmult.h

use libsodium_sys::*;
use std::ffi::CStr;

#[test]
fn test_crypto_scalarmult_bytes() {
    assert_eq!(
        unsafe { crypto_scalarmult_bytes() },
        crypto_scalarmult_BYTES as usize
    );
}

#[test]
fn test_crypto_scalarmult_scalarbytes() {
    assert_eq!(
        unsafe { crypto_scalarmult_scalarbytes() },
        crypto_scalarmult_SCALARBYTES as usize
    );
}

#[test]
fn test_crypto_scalarmult_primitive() {
    unsafe {
        let s = crypto_scalarmult_primitive();
        let s = CStr::from_ptr(s);
        let p = CStr::from_bytes_with_nul(crypto_scalarmult_PRIMITIVE).unwrap();
        assert_eq!(s, p);
    }
}
