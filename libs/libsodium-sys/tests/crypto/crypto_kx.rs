// crypto_kx.h

use libsodium_sys::*;
use std::ffi::CStr;

#[test]
fn test_crypto_kx_publickeybytes() {
    assert!(unsafe { crypto_kx_publickeybytes() } == crypto_kx_PUBLICKEYBYTES as usize)
}

#[test]
fn test_crypto_kx_secretkeybytes() {
    assert!(unsafe { crypto_kx_secretkeybytes() } == crypto_kx_SECRETKEYBYTES as usize)
}

#[test]
fn test_crypto_kx_seedbytes() {
    assert!(unsafe { crypto_kx_seedbytes() } == crypto_kx_SEEDBYTES as usize)
}

#[test]
fn test_crypto_kx_sessionkeybytes() {
    assert!(unsafe { crypto_kx_sessionkeybytes() } == crypto_kx_SESSIONKEYBYTES as usize)
}

#[test]
fn test_crypto_kx_primitive() {
    unsafe {
        let s = crypto_kx_primitive();
        let s = CStr::from_ptr(s);
        let p = CStr::from_bytes_with_nul(crypto_kx_PRIMITIVE).unwrap();
        assert_eq!(s, p);
    }
}
