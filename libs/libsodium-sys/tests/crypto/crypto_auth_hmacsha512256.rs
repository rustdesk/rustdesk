// crypto_auth_hmacsha512256.h

use libsodium_sys::*;

#[test]
fn test_crypto_auth_hmacsha512256_bytes() {
    assert!(
        unsafe { crypto_auth_hmacsha512256_bytes() } == crypto_auth_hmacsha512256_BYTES as usize
    )
}

#[test]
fn test_crypto_auth_hmacsha512256_keybytes() {
    assert!(
        unsafe { crypto_auth_hmacsha512256_keybytes() }
            == crypto_auth_hmacsha512256_KEYBYTES as usize
    )
}
