use enigo::{Enigo, Key, KeyboardControllable};
use std::thread;
use std::time::Duration;

fn main() {
    thread::sleep(Duration::from_secs(2));
    let mut enigo = Enigo::new();

    enigo.key_down(Key::Layout('a')).ok();
    thread::sleep(Duration::from_secs(1));
    enigo.key_up(Key::Layout('a'));
}
