// crypto_core_hsalsa20.h

use libsodium_sys::*;

#[test]
fn test_crypto_core_hsalsa20_outputbytes() {
    assert!(
        unsafe { crypto_core_hsalsa20_outputbytes() } == crypto_core_hsalsa20_OUTPUTBYTES as usize
    )
}

#[test]
fn test_crypto_core_hsalsa20_inputbytes() {
    assert!(
        unsafe { crypto_core_hsalsa20_inputbytes() } == crypto_core_hsalsa20_INPUTBYTES as usize
    )
}

#[test]
fn test_crypto_core_hsalsa20_keybytes() {
    assert!(unsafe { crypto_core_hsalsa20_keybytes() } == crypto_core_hsalsa20_KEYBYTES as usize)
}

#[test]
fn test_crypto_core_hsalsa20_constbytes() {
    assert!(
        unsafe { crypto_core_hsalsa20_constbytes() } == crypto_core_hsalsa20_CONSTBYTES as usize
    )
}
