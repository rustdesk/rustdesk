// crypto_box_curve25519xsalsa20poly1305.h

use libsodium_sys::*;

#[test]
fn test_crypto_box_curve25519xsalsa20poly1305_seedbytes() {
    assert!(
        unsafe { crypto_box_curve25519xsalsa20poly1305_seedbytes() }
            == crypto_box_curve25519xsalsa20poly1305_SEEDBYTES as usize
    )
}

#[test]
fn test_crypto_box_curve25519xsalsa20poly1305_publickeybytes() {
    assert!(
        unsafe { crypto_box_curve25519xsalsa20poly1305_publickeybytes() }
            == crypto_box_curve25519xsalsa20poly1305_PUBLICKEYBYTES as usize
    )
}

#[test]
fn test_crypto_box_curve25519xsalsa20poly1305_secretkeybytes() {
    assert!(
        unsafe { crypto_box_curve25519xsalsa20poly1305_secretkeybytes() }
            == crypto_box_curve25519xsalsa20poly1305_SECRETKEYBYTES as usize
    )
}

#[test]
fn test_crypto_box_curve25519xsalsa20poly1305_beforenmbytes() {
    assert!(
        unsafe { crypto_box_curve25519xsalsa20poly1305_beforenmbytes() }
            == crypto_box_curve25519xsalsa20poly1305_BEFORENMBYTES as usize
    )
}

#[test]
fn test_crypto_box_curve25519xsalsa20poly1305_noncebytes() {
    assert!(
        unsafe { crypto_box_curve25519xsalsa20poly1305_noncebytes() }
            == crypto_box_curve25519xsalsa20poly1305_NONCEBYTES as usize
    )
}

#[test]
fn test_crypto_box_curve25519xsalsa20poly1305_zerobytes() {
    assert!(
        unsafe { crypto_box_curve25519xsalsa20poly1305_zerobytes() }
            == crypto_box_curve25519xsalsa20poly1305_ZEROBYTES as usize
    )
}

#[test]
fn test_crypto_box_curve25519xsalsa20poly1305_boxzerobytes() {
    assert!(
        unsafe { crypto_box_curve25519xsalsa20poly1305_boxzerobytes() }
            == crypto_box_curve25519xsalsa20poly1305_BOXZEROBYTES as usize
    )
}

#[test]
fn test_crypto_box_curve25519xsalsa20poly1305_macbytes() {
    assert!(
        unsafe { crypto_box_curve25519xsalsa20poly1305_macbytes() }
            == crypto_box_curve25519xsalsa20poly1305_MACBYTES as usize
    )
}
