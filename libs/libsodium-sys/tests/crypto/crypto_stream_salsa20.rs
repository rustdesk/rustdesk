// crypto_stream_salsa20.h

use libsodium_sys::*;

#[test]
fn test_crypto_stream_salsa20_keybytes() {
    assert!(unsafe { crypto_stream_salsa20_keybytes() } == crypto_stream_salsa20_KEYBYTES as usize)
}

#[test]
fn test_crypto_stream_salsa20_noncebytes() {
    assert!(
        unsafe { crypto_stream_salsa20_noncebytes() } == crypto_stream_salsa20_NONCEBYTES as usize
    )
}
