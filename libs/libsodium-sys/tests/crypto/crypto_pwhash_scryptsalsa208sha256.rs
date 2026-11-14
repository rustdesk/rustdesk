// crypto_pwhash_scryptsalsa208sha256.h

use libc::{c_ulonglong, size_t};
use libsodium_sys::*;
use std::ffi::CStr;

#[test]
fn test_crypto_pwhash_scryptsalsa208sha256_saltbytes() {
    assert!(
        unsafe { crypto_pwhash_scryptsalsa208sha256_saltbytes() }
            == crypto_pwhash_scryptsalsa208sha256_SALTBYTES as usize
    )
}

#[test]
fn test_crypto_pwhash_scryptsalsa208sha256_strbytes() {
    assert!(
        unsafe { crypto_pwhash_scryptsalsa208sha256_strbytes() }
            == crypto_pwhash_scryptsalsa208sha256_STRBYTES as usize
    )
}

#[test]
fn test_crypto_pwhash_scryptsalsa208sha256_opslimit_interactive() {
    assert!(
        unsafe { crypto_pwhash_scryptsalsa208sha256_opslimit_interactive() }
            == crypto_pwhash_scryptsalsa208sha256_OPSLIMIT_INTERACTIVE as usize
    )
}

#[test]
fn test_crypto_pwhash_scryptsalsa208sha256_memlimit_interactive() {
    assert!(
        unsafe { crypto_pwhash_scryptsalsa208sha256_memlimit_interactive() }
            == crypto_pwhash_scryptsalsa208sha256_MEMLIMIT_INTERACTIVE as usize
    )
}

#[test]
fn test_crypto_pwhash_scryptsalsa208sha256_opslimit_sensitive() {
    assert!(
        unsafe { crypto_pwhash_scryptsalsa208sha256_opslimit_sensitive() }
            == crypto_pwhash_scryptsalsa208sha256_OPSLIMIT_SENSITIVE as usize
    )
}

#[test]
fn test_crypto_pwhash_scryptsalsa208sha256_memlimit_sensitive() {
    assert!(
        unsafe { crypto_pwhash_scryptsalsa208sha256_memlimit_sensitive() }
            == crypto_pwhash_scryptsalsa208sha256_MEMLIMIT_SENSITIVE as usize
    )
}

#[test]
fn test_crypto_pwhash_scryptsalsa208sha256_strprefix() {
    unsafe {
        let s = crypto_pwhash_scryptsalsa208sha256_strprefix();
        let s = CStr::from_ptr(s);
        let p = CStr::from_bytes_with_nul(crypto_pwhash_scryptsalsa208sha256_STRPREFIX).unwrap();
        assert_eq!(s, p);
    }
}

#[test]
fn test_crypto_pwhash_scryptsalsa208sha256_str() {
    let password = "Correct Horse Battery Staple";
    let mut hashed_password = [0; crypto_pwhash_scryptsalsa208sha256_STRBYTES as usize];
    let ret_hash = unsafe {
        crypto_pwhash_scryptsalsa208sha256_str(
            hashed_password.as_mut_ptr(),
            password.as_ptr() as *const _,
            password.len() as c_ulonglong,
            u64::from(crypto_pwhash_scryptsalsa208sha256_OPSLIMIT_INTERACTIVE),
            crypto_pwhash_scryptsalsa208sha256_MEMLIMIT_INTERACTIVE as size_t,
        )
    };
    assert!(ret_hash == 0);
    let ret_verify = unsafe {
        crypto_pwhash_scryptsalsa208sha256_str_verify(
            hashed_password.as_ptr(),
            password.as_ptr() as *const _,
            password.len() as c_ulonglong,
        )
    };
    assert!(ret_verify == 0);
}

#[test]
fn test_crypto_pwhash_scryptsalsa208sha256_ll_1() {
    // See https://www.tarsnap.com/scrypt/scrypt.pdf Page 16
    let password = "";
    let salt = "";
    let n = 16;
    let r = 1;
    let p = 1;
    let mut buf = [0u8; 64];
    let expected = [
        0x77, 0xd6, 0x57, 0x62, 0x38, 0x65, 0x7b, 0x20, 0x3b, 0x19, 0xca, 0x42, 0xc1, 0x8a, 0x04,
        0x97, 0xf1, 0x6b, 0x48, 0x44, 0xe3, 0x07, 0x4a, 0xe8, 0xdf, 0xdf, 0xfa, 0x3f, 0xed, 0xe2,
        0x14, 0x42, 0xfc, 0xd0, 0x06, 0x9d, 0xed, 0x09, 0x48, 0xf8, 0x32, 0x6a, 0x75, 0x3a, 0x0f,
        0xc8, 0x1f, 0x17, 0xe8, 0xd3, 0xe0, 0xfb, 0x2e, 0x0d, 0x36, 0x28, 0xcf, 0x35, 0xe2, 0x0c,
        0x38, 0xd1, 0x89, 0x06,
    ];
    let ret = unsafe {
        crypto_pwhash_scryptsalsa208sha256_ll(
            password.as_ptr(),
            password.len() as size_t,
            salt.as_ptr(),
            salt.len() as size_t,
            n,
            r,
            p,
            buf.as_mut_ptr(),
            buf.len() as size_t,
        )
    };
    assert!(ret == 0);
    assert!(buf[0..] == expected[0..]);
}

#[test]
fn test_crypto_pwhash_scryptsalsa208sha256_ll_2() {
    // See https://www.tarsnap.com/scrypt/scrypt.pdf Page 16
    let password = "password";
    let salt = "NaCl";
    let n = 1024;
    let r = 8;
    let p = 16;
    let mut buf = [0u8; 64];
    let expected = [
        0xfd, 0xba, 0xbe, 0x1c, 0x9d, 0x34, 0x72, 0x00, 0x78, 0x56, 0xe7, 0x19, 0x0d, 0x01, 0xe9,
        0xfe, 0x7c, 0x6a, 0xd7, 0xcb, 0xc8, 0x23, 0x78, 0x30, 0xe7, 0x73, 0x76, 0x63, 0x4b, 0x37,
        0x31, 0x62, 0x2e, 0xaf, 0x30, 0xd9, 0x2e, 0x22, 0xa3, 0x88, 0x6f, 0xf1, 0x09, 0x27, 0x9d,
        0x98, 0x30, 0xda, 0xc7, 0x27, 0xaf, 0xb9, 0x4a, 0x83, 0xee, 0x6d, 0x83, 0x60, 0xcb, 0xdf,
        0xa2, 0xcc, 0x06, 0x40,
    ];
    let ret = unsafe {
        crypto_pwhash_scryptsalsa208sha256_ll(
            password.as_ptr(),
            password.len() as size_t,
            salt.as_ptr(),
            salt.len() as size_t,
            n,
            r,
            p,
            buf.as_mut_ptr(),
            buf.len() as size_t,
        )
    };
    assert!(ret == 0);
    assert!(buf[0..] == expected[0..]);
}

#[test]
fn test_crypto_pwhash_scryptsalsa208sha256_ll_3() {
    // See https://www.tarsnap.com/scrypt/scrypt.pdf Page 16
    let password = "pleaseletmein";
    let salt = "SodiumChloride";
    let n = 16384;
    let r = 8;
    let p = 1;
    let mut buf = [0u8; 64];
    let expected = [
        0x70, 0x23, 0xbd, 0xcb, 0x3a, 0xfd, 0x73, 0x48, 0x46, 0x1c, 0x06, 0xcd, 0x81, 0xfd, 0x38,
        0xeb, 0xfd, 0xa8, 0xfb, 0xba, 0x90, 0x4f, 0x8e, 0x3e, 0xa9, 0xb5, 0x43, 0xf6, 0x54, 0x5d,
        0xa1, 0xf2, 0xd5, 0x43, 0x29, 0x55, 0x61, 0x3f, 0x0f, 0xcf, 0x62, 0xd4, 0x97, 0x05, 0x24,
        0x2a, 0x9a, 0xf9, 0xe6, 0x1e, 0x85, 0xdc, 0x0d, 0x65, 0x1e, 0x40, 0xdf, 0xcf, 0x01, 0x7b,
        0x45, 0x57, 0x58, 0x87,
    ];
    let ret = unsafe {
        crypto_pwhash_scryptsalsa208sha256_ll(
            password.as_ptr(),
            password.len() as size_t,
            salt.as_ptr(),
            salt.len() as size_t,
            n,
            r,
            p,
            buf.as_mut_ptr(),
            buf.len() as size_t,
        )
    };
    assert!(ret == 0);
    assert!(buf[0..] == expected[0..]);
}

#[test]
fn test_crypto_pwhash_scryptsalsa208sha256_ll_4() {
    // See https://www.tarsnap.com/scrypt/scrypt.pdf Page 16
    let password = "pleaseletmein";
    let salt = "SodiumChloride";
    let n = 1_048_576;
    let r = 8;
    let p = 1;
    let mut buf = [0u8; 64];
    let expected = [
        0x21, 0x01, 0xcb, 0x9b, 0x6a, 0x51, 0x1a, 0xae, 0xad, 0xdb, 0xbe, 0x09, 0xcf, 0x70, 0xf8,
        0x81, 0xec, 0x56, 0x8d, 0x57, 0x4a, 0x2f, 0xfd, 0x4d, 0xab, 0xe5, 0xee, 0x98, 0x20, 0xad,
        0xaa, 0x47, 0x8e, 0x56, 0xfd, 0x8f, 0x4b, 0xa5, 0xd0, 0x9f, 0xfa, 0x1c, 0x6d, 0x92, 0x7c,
        0x40, 0xf4, 0xc3, 0x37, 0x30, 0x40, 0x49, 0xe8, 0xa9, 0x52, 0xfb, 0xcb, 0xf4, 0x5c, 0x6f,
        0xa7, 0x7a, 0x41, 0xa4,
    ];
    let ret = unsafe {
        crypto_pwhash_scryptsalsa208sha256_ll(
            password.as_ptr(),
            password.len() as size_t,
            salt.as_ptr(),
            salt.len() as size_t,
            n,
            r,
            p,
            buf.as_mut_ptr(),
            buf.len() as size_t,
        )
    };
    assert!(ret == 0);
    assert!(buf[0..] == expected[0..]);
}
