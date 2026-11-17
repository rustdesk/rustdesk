// crypto_aead_xchacha20poly1305.h

use libsodium_sys::*;

#[test]
fn test_crypto_aead_xchacha20poly1305_ietf_keybytes() {
    assert!(unsafe { crypto_aead_xchacha20poly1305_ietf_keybytes() as usize } ==
            crypto_aead_xchacha20poly1305_ietf_KEYBYTES)
}
#[test]
fn test_crypto_aead_xchacha20poly1305_ietf_nsecbytes() {
    assert!(unsafe { crypto_aead_xchacha20poly1305_ietf_nsecbytes() as usize } ==
            crypto_aead_xchacha20poly1305_ietf_NSECBYTES)
}
#[test]
fn test_crypto_aead_xchacha20poly1305_ietf_npubbytes() {
    assert!(unsafe { crypto_aead_xchacha20poly1305_ietf_npubbytes() as usize } ==
            crypto_aead_xchacha20poly1305_ietf_NPUBBYTES)
}
#[test]
fn test_crypto_aead_xchacha20poly1305_ietf_abytes() {
    assert!(unsafe { crypto_aead_xchacha20poly1305_ietf_abytes() as usize } ==
            crypto_aead_xchacha20poly1305_ietf_ABYTES)
}
