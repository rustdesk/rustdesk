extern crate repng;
extern crate scrap;

use std::fs::File;
use std::io::ErrorKind::WouldBlock;
use std::thread;
use std::time::Duration;

use scrap::{i420_to_rgb, Capturer, Display, TraitCapturer};

fn main() {
    let n = Display::all().unwrap().len();
    for i in 0..n {
        record(i);
    }
}

fn get_display(i: usize) -> Display {
    Display::all().unwrap().remove(i)
}

fn record(i: usize) {
    let one_second = Duration::new(1, 0);
    let one_frame = one_second / 60;

    for d in Display::all().unwrap() {
        println!("{:?} {} {}", d.origin(), d.width(), d.height());
    }

    let display = get_display(i);
    let mut capturer = Capturer::new(display, false).expect("Couldn't begin capture.");
    let (w, h) = (capturer.width(), capturer.height());

    loop {
        // Wait until there's a frame.

        let buffer = match capturer.frame(Duration::from_millis(0)) {
            Ok(buffer) => buffer,
            Err(error) => {
                if error.kind() == WouldBlock {
                    // Keep spinning.
                    thread::sleep(one_frame);
                    continue;
                } else {
                    panic!("Error: {}", error);
                }
            }
        };
        println!("Captured data len: {}, Saving...", buffer.len());

        // Flip the BGRA image into a RGBA image.

        let mut bitflipped = Vec::with_capacity(w * h * 4);
        let stride = buffer.len() / h;

        for y in 0..h {
            for x in 0..w {
                let i = stride * y + 4 * x;
                bitflipped.extend_from_slice(&[buffer[i + 2], buffer[i + 1], buffer[i], 255]);
            }
        }

        // Save the image.

        let name = format!("screenshot{}_1.png", i);
        repng::encode(
            File::create(name.clone()).unwrap(),
            w as u32,
            h as u32,
            &bitflipped,
        )
        .unwrap();

        println!("Image saved to `{}`.", name);
        break;
    }

    drop(capturer);
    let display = get_display(i);
    let mut capturer = Capturer::new(display, true).expect("Couldn't begin capture.");
    let (w, h) = (capturer.width(), capturer.height());

    loop {
        // Wait until there's a frame.

        let buffer = match capturer.frame(Duration::from_millis(0)) {
            Ok(buffer) => buffer,
            Err(error) => {
                if error.kind() == WouldBlock {
                    // Keep spinning.
                    thread::sleep(one_frame);
                    continue;
                } else {
                    panic!("Error: {}", error);
                }
            }
        };
        println!("Captured data len: {}, Saving...", buffer.len());

        let mut frame = Default::default();
        i420_to_rgb(w, h, &buffer, &mut frame);

        let mut bitflipped = Vec::with_capacity(w * h * 4);
        let stride = frame.len() / h;

        for y in 0..h {
            for x in 0..w {
                let i = stride * y + 3 * x;
                bitflipped.extend_from_slice(&[frame[i], frame[i + 1], frame[i + 2], 255]);
            }
        }
        let name = format!("screenshot{}_2.png", i);
        repng::encode(
            File::create(name.clone()).unwrap(),
            w as u32,
            h as u32,
            &bitflipped,
        )
        .unwrap();

        println!("Image saved to `{}`.", name);
        break;
    }
}
