// crypto_sign_ed25519.h

use libsodium_sys::*;

#[test]
fn test_crypto_sign_ed25519_bytes() {
    assert!(unsafe { crypto_sign_ed25519_bytes() } == crypto_sign_ed25519_BYTES as usize)
}

#[test]
fn test_crypto_sign_ed25519_seedbytes() {
    assert!(unsafe { crypto_sign_ed25519_seedbytes() } == crypto_sign_ed25519_SEEDBYTES as usize)
}

#[test]
fn test_crypto_sign_ed25519_publickeybytes() {
    assert!(
        unsafe { crypto_sign_ed25519_publickeybytes() }
            == crypto_sign_ed25519_PUBLICKEYBYTES as usize
    )
}

#[test]
fn test_crypto_sign_ed25519_secretkeybytes() {
    assert!(
        unsafe { crypto_sign_ed25519_secretkeybytes() }
            == crypto_sign_ed25519_SECRETKEYBYTES as usize
    )
}
