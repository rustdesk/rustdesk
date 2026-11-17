// crypto_box.h

use libsodium_sys::*;
use std::ffi::CStr;
#[test]
fn test_crypto_box_seedbytes() {
    assert!(unsafe { crypto_box_seedbytes() } == crypto_box_SEEDBYTES as usize)
}

#[test]
fn test_crypto_box_publickeybytes() {
    assert!(unsafe { crypto_box_publickeybytes() } == crypto_box_PUBLICKEYBYTES as usize)
}

#[test]
fn test_crypto_box_secretkeybytes() {
    assert!(unsafe { crypto_box_secretkeybytes() } == crypto_box_SECRETKEYBYTES as usize)
}

#[test]
fn test_crypto_box_beforenmbytes() {
    assert!(unsafe { crypto_box_beforenmbytes() } == crypto_box_BEFORENMBYTES as usize)
}

#[test]
fn test_crypto_box_noncebytes() {
    assert!(unsafe { crypto_box_noncebytes() } == crypto_box_NONCEBYTES as usize)
}

#[test]
fn test_crypto_box_zerobytes() {
    assert!(unsafe { crypto_box_zerobytes() } == crypto_box_ZEROBYTES as usize)
}

#[test]
fn test_crypto_box_boxzerobytes() {
    assert!(unsafe { crypto_box_boxzerobytes() } == crypto_box_BOXZEROBYTES as usize)
}

#[test]
fn test_crypto_box_macbytes() {
    assert!(unsafe { crypto_box_macbytes() } == crypto_box_MACBYTES as usize)
}

#[test]
fn test_crypto_box_primitive() {
    unsafe {
        let s = crypto_box_primitive();
        let s = CStr::from_ptr(s);
        let p = CStr::from_bytes_with_nul(crypto_box_PRIMITIVE).unwrap();
        assert_eq!(s, p);
    }
}

#[test]
fn test_crypto_box_sealbytes() {
    assert!(unsafe { crypto_box_sealbytes() } == crypto_box_SEALBYTES as usize)
}
