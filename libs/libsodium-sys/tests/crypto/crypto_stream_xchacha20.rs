// crypto_stream_xchacha20.h

use libsodium_sys::*;

#[test]
fn test_crypto_stream_xchacha20_keybytes() {
    assert!(
        unsafe { crypto_stream_xchacha20_keybytes() } == crypto_stream_xchacha20_KEYBYTES as usize
    )
}

#[test]
fn test_crypto_stream_xchacha20_noncebytes() {
    assert!(
        unsafe { crypto_stream_xchacha20_noncebytes() }
            == crypto_stream_xchacha20_NONCEBYTES as usize
    )
}
