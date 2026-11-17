// crypto_stream_xsalsa20.h

use libsodium_sys::*;

#[test]
fn test_crypto_stream_xsalsa20_keybytes() {
    assert!(
        unsafe { crypto_stream_xsalsa20_keybytes() } == crypto_stream_xsalsa20_KEYBYTES as usize
    )
}

#[test]
fn test_crypto_stream_xsalsa20_noncebytes() {
    assert!(
        unsafe { crypto_stream_xsalsa20_noncebytes() }
            == crypto_stream_xsalsa20_NONCEBYTES as usize
    )
}
