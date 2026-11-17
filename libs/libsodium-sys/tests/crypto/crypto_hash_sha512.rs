// crypto_hash_sha512.h

use libsodium_sys::*;

#[test]
fn test_crypto_hash_sha512_bytes() {
    assert!(unsafe { crypto_hash_sha512_bytes() } == crypto_hash_sha512_BYTES as usize)
}
