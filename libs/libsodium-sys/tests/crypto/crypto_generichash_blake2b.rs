// crypto_generichash_blake2b.h

use libsodium_sys::*;

#[test]
fn test_crypto_generichash_blake2b_state_alignment() {
    // this asserts the alignment applied that was broken with old
    // versions of bindgen
    assert_eq!(64, std::mem::align_of::<crypto_generichash_blake2b_state>());
}

#[test]
fn test_crypto_generichash_blake2b_bytes_min() {
    assert_eq!(
        unsafe { crypto_generichash_blake2b_bytes_min() },
        crypto_generichash_blake2b_BYTES_MIN as usize
    )
}

#[test]
fn test_crypto_generichash_blake2b_bytes_max() {
    assert_eq!(
        unsafe { crypto_generichash_blake2b_bytes_max() },
        crypto_generichash_blake2b_BYTES_MAX as usize
    )
}

#[test]
fn test_crypto_generichash_blake2b_bytes() {
    assert_eq!(
        unsafe { crypto_generichash_blake2b_bytes() },
        crypto_generichash_blake2b_BYTES as usize
    )
}

#[test]
fn test_crypto_generichash_blake2b_keybytes_min() {
    assert_eq!(
        unsafe { crypto_generichash_blake2b_keybytes_min() },
        crypto_generichash_blake2b_KEYBYTES_MIN as usize
    )
}

#[test]
fn test_crypto_generichash_blake2b_keybytes_max() {
    assert_eq!(
        unsafe { crypto_generichash_blake2b_keybytes_max() },
        crypto_generichash_blake2b_KEYBYTES_MAX as usize
    )
}

#[test]
fn test_crypto_generichash_blake2b_keybytes() {
    assert_eq!(
        unsafe { crypto_generichash_blake2b_keybytes() },
        crypto_generichash_blake2b_KEYBYTES as usize
    )
}

#[test]
fn test_crypto_generichash_blake2b_saltbytes() {
    assert_eq!(
        unsafe { crypto_generichash_blake2b_saltbytes() },
        crypto_generichash_blake2b_SALTBYTES as usize
    )
}

#[test]
fn test_crypto_generichash_blake2b_personalbytes() {
    assert_eq!(
        unsafe { crypto_generichash_blake2b_personalbytes() },
        crypto_generichash_blake2b_PERSONALBYTES as usize
    )
}

#[test]
fn test_crypto_generichash_blake2b() {
    let mut out = [0u8; crypto_generichash_blake2b_BYTES as usize];
    let m = [0u8; 64];
    let key = [0u8; crypto_generichash_blake2b_KEYBYTES as usize];

    assert_eq!(
        unsafe {
            crypto_generichash_blake2b(
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

#[test]
fn test_crypto_generichash_blake2b_salt_personal() {
    let mut out = [0u8; crypto_generichash_blake2b_BYTES as usize];
    let m = [0u8; 64];
    let key = [0u8; crypto_generichash_blake2b_KEYBYTES as usize];
    let salt = [0u8; crypto_generichash_blake2b_SALTBYTES as usize];
    let personal = [0u8; crypto_generichash_blake2b_PERSONALBYTES as usize];

    assert_eq!(
        unsafe {
            crypto_generichash_blake2b_salt_personal(
                out.as_mut_ptr(),
                out.len(),
                m.as_ptr(),
                m.len() as u64,
                key.as_ptr(),
                key.len(),
                salt.as_ptr(),
                personal.as_ptr(),
            )
        },
        0
    );
}
