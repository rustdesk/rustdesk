use enigo::{Enigo, Key, KeyboardControllable};
use std::thread;
use std::time::Duration;
use std::time::Instant;

fn main() {
    thread::sleep(Duration::from_secs(2));
    let mut enigo = Enigo::new();

    let now = Instant::now();

    // write text
    enigo.key_sequence("Hello World! ❤️");

    let time = now.elapsed();
    println!("{:?}", time);

    // select all
    enigo.key_down(Key::Control).ok();
    enigo.key_click(Key::Layout('a'));
    enigo.key_up(Key::Control);
}
