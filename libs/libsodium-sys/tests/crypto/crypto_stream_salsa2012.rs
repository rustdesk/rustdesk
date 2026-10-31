// crypto_stream_salsa2012.h

use libsodium_sys::*;

#[test]
fn test_crypto_stream_salsa2012_keybytes() {
    assert!(
        unsafe { crypto_stream_salsa2012_keybytes() } == crypto_stream_salsa2012_KEYBYTES as usize
    )
}

#[test]
fn test_crypto_stream_salsa2012_noncebytes() {
    assert!(
        unsafe { crypto_stream_salsa2012_noncebytes() }
            == crypto_stream_salsa2012_NONCEBYTES as usize
    )
}
