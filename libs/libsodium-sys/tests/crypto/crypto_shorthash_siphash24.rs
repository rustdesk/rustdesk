// crypto_shorthash_siphash24.h

use libsodium_sys::*;

#[test]
fn test_crypto_shorthash_siphash24_bytes() {
    assert!(
        unsafe { crypto_shorthash_siphash24_bytes() } == crypto_shorthash_siphash24_BYTES as usize
    )
}

#[test]
fn test_crypto_shorthash_siphash24_keybytes() {
    assert!(
        unsafe { crypto_shorthash_siphash24_keybytes() }
            == crypto_shorthash_siphash24_KEYBYTES as usize
    )
}
