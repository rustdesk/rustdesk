// crypto_core_salsa20.h

use libsodium_sys::*;

#[test]
fn test_crypto_core_salsa20_outputbytes() {
    assert!(
        unsafe { crypto_core_salsa20_outputbytes() } == crypto_core_salsa20_OUTPUTBYTES as usize
    )
}

#[test]
fn test_crypto_core_salsa20_inputbytes() {
    assert!(unsafe { crypto_core_salsa20_inputbytes() } == crypto_core_salsa20_INPUTBYTES as usize)
}

#[test]
fn test_crypto_core_salsa20_keybytes() {
    assert!(unsafe { crypto_core_salsa20_keybytes() } == crypto_core_salsa20_KEYBYTES as usize)
}

#[test]
fn test_crypto_core_salsa20_constbytes() {
    assert!(unsafe { crypto_core_salsa20_constbytes() } == crypto_core_salsa20_CONSTBYTES as usize)
}
