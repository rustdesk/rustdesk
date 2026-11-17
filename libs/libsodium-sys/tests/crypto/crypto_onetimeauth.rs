// crypto_onetimeauth.h

use libsodium_sys::*;
use std::ffi::CStr;

#[test]
fn test_crypto_onetimeauth_bytes() {
    assert!(unsafe { crypto_onetimeauth_bytes() } == crypto_onetimeauth_BYTES as usize)
}

#[test]
fn test_crypto_onetimeauth_keybytes() {
    assert!(unsafe { crypto_onetimeauth_keybytes() } == crypto_onetimeauth_KEYBYTES as usize)
}

#[test]
fn test_crypto_onetimeauth_primitive() {
    unsafe {
        let s = crypto_onetimeauth_primitive();
        let s = CStr::from_ptr(s);
        let p = CStr::from_bytes_with_nul(crypto_onetimeauth_PRIMITIVE).unwrap();
        assert_eq!(s, p);
    }
}
