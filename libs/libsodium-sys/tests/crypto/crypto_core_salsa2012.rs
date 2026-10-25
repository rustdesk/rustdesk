// crypto_core_salsa2012.h

use libsodium_sys::*;

#[test]
fn test_crypto_core_salsa2012_outputbytes() {
    assert!(
        unsafe { crypto_core_salsa2012_outputbytes() }
            == crypto_core_salsa2012_OUTPUTBYTES as usize
    )
}

#[test]
fn test_crypto_core_salsa2012_inputbytes() {
    assert!(
        unsafe { crypto_core_salsa2012_inputbytes() } == crypto_core_salsa2012_INPUTBYTES as usize
    )
}

#[test]
fn test_crypto_core_salsa2012_keybytes() {
    assert!(unsafe { crypto_core_salsa2012_keybytes() } == crypto_core_salsa2012_KEYBYTES as usize)
}

#[test]
fn test_crypto_core_salsa2012_constbytes() {
    assert!(
        unsafe { crypto_core_salsa2012_constbytes() } == crypto_core_salsa2012_CONSTBYTES as usize
    )
}
