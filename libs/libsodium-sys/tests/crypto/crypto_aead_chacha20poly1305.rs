// crypto_aead_chacha20poly1305.h

use libsodium_sys::*;

#[test]
fn test_crypto_aead_chacha20poly1305_keybytes() {
    assert!(
        unsafe { crypto_aead_chacha20poly1305_keybytes() as usize }
            == crypto_aead_chacha20poly1305_KEYBYTES as usize
    )
}

#[test]
fn test_crypto_aead_chacha20poly1305_nsecbytes() {
    assert!(
        unsafe { crypto_aead_chacha20poly1305_nsecbytes() as usize }
            == crypto_aead_chacha20poly1305_NSECBYTES as usize
    )
}

#[test]
fn test_crypto_aead_chacha20poly1305_npubbytes() {
    assert!(
        unsafe { crypto_aead_chacha20poly1305_npubbytes() as usize }
            == crypto_aead_chacha20poly1305_NPUBBYTES as usize
    )
}

#[test]
fn test_crypto_aead_chacha20poly1305_abytes() {
    assert!(
        unsafe { crypto_aead_chacha20poly1305_abytes() as usize }
            == crypto_aead_chacha20poly1305_ABYTES as usize
    )
}

#[test]
fn test_crypto_aead_chacha20poly1305_ietf_keybytes() {
    assert!(
        unsafe { crypto_aead_chacha20poly1305_ietf_keybytes() }
            == crypto_aead_chacha20poly1305_ietf_KEYBYTES as usize
    )
}

#[test]
fn test_crypto_aead_chacha20poly1305_ietf_nsecbytes() {
    assert!(
        unsafe { crypto_aead_chacha20poly1305_ietf_nsecbytes() }
            == crypto_aead_chacha20poly1305_ietf_NSECBYTES as usize
    )
}

#[test]
fn test_crypto_aead_chacha20poly1305_ietf_npubbytes() {
    assert!(
        unsafe { crypto_aead_chacha20poly1305_ietf_npubbytes() }
            == crypto_aead_chacha20poly1305_ietf_NPUBBYTES as usize
    )
}

#[test]
fn test_crypto_aead_chacha20poly1305_ietf_abytes() {
    assert!(
        unsafe { crypto_aead_chacha20poly1305_ietf_abytes() }
            == crypto_aead_chacha20poly1305_ietf_ABYTES as usize
    )
}
