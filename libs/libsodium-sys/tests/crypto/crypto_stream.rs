// crypto_stream.h

use libsodium_sys::*;
use std::ffi::CStr;

#[test]
fn test_crypto_stream_keybytes() {
    assert!(unsafe { crypto_stream_keybytes() } == crypto_stream_KEYBYTES as usize)
}

#[test]
fn test_crypto_stream_noncebytes() {
    assert!(unsafe { crypto_stream_noncebytes() } == crypto_stream_NONCEBYTES as usize)
}

#[test]
fn test_crypto_stream_primitive() {
    unsafe {
        let s = crypto_stream_primitive();
        let s = CStr::from_ptr(s);
        let p = CStr::from_bytes_with_nul(crypto_stream_PRIMITIVE).unwrap();
        assert_eq!(s, p);
    }
}
