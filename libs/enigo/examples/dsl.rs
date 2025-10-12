use enigo::{Enigo, KeyboardControllable};
use std::thread;
use std::time::Duration;

fn main() {
    thread::sleep(Duration::from_secs(2));
    let mut enigo = Enigo::new();

    // write text and select all
    enigo.key_sequence_parse("{+UNICODE}{{Hello World!}} ❤️{-UNICODE}{+CTRL}a{-CTRL}");
}
