// crypto_hash_sha256.h

use libsodium_sys::*;

#[test]
fn test_crypto_hash_sha256_bytes() {
    assert!(unsafe { crypto_hash_sha256_bytes() } == crypto_hash_sha256_BYTES as usize)
}
