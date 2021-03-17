//! Test that supplying empty packets does forward error correction.

extern crate magnum_opus;
use magnum_opus::*;

#[test]
fn blah() {
    let mut magnum_opus = Decoder::new(48000, Channels::Mono).unwrap();

    let mut output = vec![0i16; 5760];
    let size = magnum_opus.decode(&[], &mut output[..], true).unwrap();
    assert_eq!(size, 5760);
}