// crypto_stream_chacha20.h

use libsodium_sys::*;

#[test]
fn test_crypto_stream_chacha20_keybytes() {
    assert!(
        unsafe { crypto_stream_chacha20_keybytes() } == crypto_stream_chacha20_KEYBYTES as usize
    )
}

#[test]
fn test_crypto_stream_chacha20_noncebytes() {
    assert!(
        unsafe { crypto_stream_chacha20_noncebytes() }
            == crypto_stream_chacha20_NONCEBYTES as usize
    )
}
