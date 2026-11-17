// crypto_scalarmult_curve25519.h

use libsodium_sys::*;

#[test]
fn test_crypto_scalarmult_curve25519_bytes() {
    assert_eq!(
        unsafe { crypto_scalarmult_curve25519_bytes() },
        crypto_scalarmult_curve25519_BYTES as usize
    );
}

#[test]
fn test_crypto_scalarmult_curve25519_scalarbytes() {
    assert_eq!(
        unsafe { crypto_scalarmult_curve25519_scalarbytes() },
        crypto_scalarmult_curve25519_SCALARBYTES as usize
    );
}
