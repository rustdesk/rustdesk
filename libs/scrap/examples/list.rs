extern crate scrap;

use scrap::Display;

fn main() {
    let displays = Display::all().unwrap();

    for (i, display) in displays.iter().enumerate() {
        println!(
            "Display {} [{}x{}]",
            i + 1,
            display.width(),
            display.height()
        );
    }
}
