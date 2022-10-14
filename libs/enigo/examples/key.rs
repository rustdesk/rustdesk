use enigo::{Enigo, Key, KeyboardControllable};
use std::thread;
use std::time::Duration;

fn main() {
    thread::sleep(Duration::from_secs(2));
    let mut enigo = Enigo::new();

    enigo.key_down(Key::Shift).ok();
    enigo.key_down(Key::Layout('.')).ok();
    enigo.key_up(Key::Layout('.'));
    enigo.key_up(Key::Shift);
    enigo.key_down(Key::Shift).ok();
    enigo.key_down(Key::Layout('-')).ok();
    enigo.key_up(Key::Layout('-'));
    enigo.key_up(Key::Shift);
}
