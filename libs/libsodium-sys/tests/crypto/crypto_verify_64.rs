// crypto_verify_64.h

use libsodium_sys::*;

#[test]
fn test_crypto_verify_64_bytes() {
    assert_eq!(
        unsafe { crypto_verify_64_bytes() },
        crypto_verify_64_BYTES as usize
    );
}
