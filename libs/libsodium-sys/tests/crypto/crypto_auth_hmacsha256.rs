// crypto_auth_hmacsha256.h

use libsodium_sys::*;

#[test]
fn test_crypto_auth_hmacsha256_bytes() {
    assert!(unsafe { crypto_auth_hmacsha256_bytes() } == crypto_auth_hmacsha256_BYTES as usize)
}

#[test]
fn test_crypto_auth_hmacsha256_keybytes() {
    assert!(
        unsafe { crypto_auth_hmacsha256_keybytes() as usize }
            == crypto_auth_hmacsha256_KEYBYTES as usize
    )
}
