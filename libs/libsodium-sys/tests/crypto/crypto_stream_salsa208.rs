// crypto_stream_salsa208.h

use libsodium_sys::*;

#[test]
fn test_crypto_stream_salsa208_keybytes() {
    assert!(
        unsafe { crypto_stream_salsa208_keybytes() } == crypto_stream_salsa208_KEYBYTES as usize
    )
}
#[test]
fn test_crypto_stream_salsa208_noncebytes() {
    assert!(
        unsafe { crypto_stream_salsa208_noncebytes() }
            == crypto_stream_salsa208_NONCEBYTES as usize
    )
}
