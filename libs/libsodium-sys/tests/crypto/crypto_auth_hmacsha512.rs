// crypto_auth_hmacsha512.h

use libsodium_sys::*;

#[test]
fn test_crypto_auth_hmacsha512_bytes() {
    assert!(unsafe { crypto_auth_hmacsha512_bytes() } == crypto_auth_hmacsha512_BYTES as usize)
}

#[test]
fn test_crypto_auth_hmacsha512_keybytes() {
    assert!(
        unsafe { crypto_auth_hmacsha512_keybytes() } == crypto_auth_hmacsha512_KEYBYTES as usize
    )
}
