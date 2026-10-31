// crypto_core_salsa208.h

use libsodium_sys::*;

#[test]
fn test_crypto_core_salsa208_outputbytes() {
    assert!(
        unsafe { crypto_core_salsa208_outputbytes() } == crypto_core_salsa208_OUTPUTBYTES as usize
    )
}

#[test]
fn test_crypto_core_salsa208_inputbytes() {
    assert!(
        unsafe { crypto_core_salsa208_inputbytes() } == crypto_core_salsa208_INPUTBYTES as usize
    )
}

#[test]
fn test_crypto_core_salsa208_keybytes() {
    assert!(unsafe { crypto_core_salsa208_keybytes() } == crypto_core_salsa208_KEYBYTES as usize)
}

#[test]
fn test_crypto_core_salsa208_constbytes() {
    assert!(
        unsafe { crypto_core_salsa208_constbytes() } == crypto_core_salsa208_CONSTBYTES as usize
    )
}
