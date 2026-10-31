// crypto_secretbox_xsalsa20poly1305.h

use libsodium_sys::*;

#[test]
fn test_crypto_secretbox_xsalsa20poly1305_keybytes() {
    assert!(
        unsafe { crypto_secretbox_xsalsa20poly1305_keybytes() }
            == crypto_secretbox_xsalsa20poly1305_KEYBYTES as usize
    )
}

#[test]
fn test_crypto_secretbox_xsalsa20poly1305_noncebytes() {
    assert!(
        unsafe { crypto_secretbox_xsalsa20poly1305_noncebytes() }
            == crypto_secretbox_xsalsa20poly1305_NONCEBYTES as usize
    )
}

#[test]
fn test_crypto_secretbox_xsalsa20poly1305_zerobytes() {
    assert!(
        unsafe { crypto_secretbox_xsalsa20poly1305_zerobytes() }
            == crypto_secretbox_xsalsa20poly1305_ZEROBYTES as usize
    )
}

#[test]
fn test_crypto_secretbox_xsalsa20poly1305_boxzerobytes() {
    assert!(
        unsafe { crypto_secretbox_xsalsa20poly1305_boxzerobytes() }
            == crypto_secretbox_xsalsa20poly1305_BOXZEROBYTES as usize
    )
}

#[test]
fn test_crypto_secretbox_xsalsa20poly1305_macbytes() {
    assert!(
        unsafe { crypto_secretbox_xsalsa20poly1305_macbytes() }
            == crypto_secretbox_xsalsa20poly1305_MACBYTES as usize
    )
}
