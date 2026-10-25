// crypto_generichash.h

use libsodium_sys::*;
use std::ffi::CStr;

#[test]
fn test_crypto_generichash_bytes_min() {
    assert_eq!(
        unsafe { crypto_generichash_bytes_min() },
        crypto_generichash_BYTES_MIN as usize
    )
}

#[test]
fn test_crypto_generichash_bytes_max() {
    assert_eq!(
        unsafe { crypto_generichash_bytes_max() },
        crypto_generichash_BYTES_MAX as usize
    )
}

#[test]
fn test_crypto_generichash_bytes() {
    assert_eq!(
        unsafe { crypto_generichash_bytes() },
        crypto_generichash_BYTES as usize
    )
}

#[test]
fn test_crypto_generichash_keybytes_min() {
    assert_eq!(
        unsafe { crypto_generichash_keybytes_min() },
        crypto_generichash_KEYBYTES_MIN as usize
    )
}

#[test]
fn test_crypto_generichash_keybytes_max() {
    assert_eq!(
        unsafe { crypto_generichash_keybytes_max() },
        crypto_generichash_KEYBYTES_MAX as usize
    )
}

#[test]
fn test_crypto_generichash_keybytes() {
    assert_eq!(
        unsafe { crypto_generichash_keybytes() },
        crypto_generichash_KEYBYTES as usize
    )
}
#[test]
fn test_crypto_generichash_primitive() {
    unsafe {
        let s = crypto_generichash_primitive();
        let s = CStr::from_ptr(s);
        let p = CStr::from_bytes_with_nul(crypto_generichash_PRIMITIVE).unwrap();
        assert_eq!(s, p);
    }
}

#[test]
fn test_crypto_generichash_statebytes() {
    assert!(unsafe { crypto_generichash_statebytes() } > 0);
}

#[test]
fn test_crypto_generichash() {
    let mut out = [0u8; crypto_generichash_BYTES as usize];
    let m = [0u8; 64];
    let key = [0u8; crypto_generichash_KEYBYTES as usize];

    assert_eq!(
        unsafe {
            crypto_generichash(
                out.as_mut_ptr(),
                out.len(),
                m.as_ptr(),
                m.len() as u64,
                key.as_ptr(),
                key.len(),
            )
        },
        0
    );
}

#[cfg(test)]
use std::mem;

#[test]
fn test_crypto_generichash_multipart() {
    let mut out = [0u8; crypto_generichash_BYTES as usize];
    let m = [0u8; 64];
    let key = [0u8; crypto_generichash_KEYBYTES as usize];

    let mut pst = mem::MaybeUninit::<crypto_generichash_state>::uninit();

    assert_eq!(
        unsafe { crypto_generichash_init(pst.as_mut_ptr(), key.as_ptr(), key.len(), out.len()) },
        0
    );

    let mut pst = unsafe { pst.assume_init() };

    assert_eq!(
        unsafe { crypto_generichash_update(&mut pst, m.as_ptr(), m.len() as u64) },
        0
    );

    assert_eq!(
        unsafe { crypto_generichash_update(&mut pst, m.as_ptr(), m.len() as u64) },
        0
    );

    assert_eq!(
        unsafe { crypto_generichash_final(&mut pst, out.as_mut_ptr(), out.len()) },
        0
    );
}
