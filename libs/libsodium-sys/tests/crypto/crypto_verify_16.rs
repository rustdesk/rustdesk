// crypto_verify_16.h

use libsodium_sys::*;

#[test]
fn test_crypto_verify_16_bytes() {
    assert_eq!(
        unsafe { crypto_verify_16_bytes() },
        crypto_verify_16_BYTES as usize
    );
}
