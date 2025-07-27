extern crate repng;
extern crate scrap;

use scrap::Display;
#[cfg(windows)]
use scrap::{CapturerMag, TraitCapturer};
#[cfg(windows)]
use std::fs::File;

fn main() {
    let n = Display::all().unwrap().len();
    for _i in 0..n {
        #[cfg(windows)]
        record(_i);
    }
}

#[cfg(windows)]
fn get_display(i: usize) -> Display {
    Display::all().unwrap().remove(i)
}

#[cfg(windows)]
fn record(i: usize) {
    use std::time::Duration;

    use scrap::{Frame, TraitPixelBuffer};

    for d in Display::all().unwrap() {
        println!("{:?} {} {}", d.origin(), d.width(), d.height());
    }

    let display = get_display(i);
    let (w, h) = (display.width(), display.height());

    {
        let mut capture_mag = CapturerMag::new(display.origin(), display.width(), display.height())
            .expect("Couldn't begin capture.");
        let wnd_cls = "";
        let wnd_name = "RustDeskPrivacyWindow";
        if false == capture_mag.exclude(wnd_cls, wnd_name).unwrap() {
            println!("No window found for cls {} name {}", wnd_cls, wnd_name);
        } else {
            println!("Filter window for cls {} name {}", wnd_cls, wnd_name);
        }

        let frame = capture_mag.frame(Duration::from_millis(0)).unwrap();
        let Frame::PixelBuffer(frame) = frame else {
            return;
        };
        let frame = frame.data();
        println!("Capture data len: {}, Saving...", frame.len());

        let mut bitflipped = Vec::with_capacity(w * h * 4);
        let stride = frame.len() / h;

        for y in 0..h {
            for x in 0..w {
                let i = stride * y + 4 * x;
                bitflipped.extend_from_slice(&[frame[i + 2], frame[i + 1], frame[i], 255]);
            }
        }
        // Save the image.
        let name = format!("capture_mag_{}_1.png", i);
        repng::encode(
            File::create(name.clone()).unwrap(),
            w as u32,
            h as u32,
            &bitflipped,
        )
        .unwrap();
        println!("Image saved to `{}`.", name);
    }

    {
        let mut capture_mag = CapturerMag::new(display.origin(), display.width(), display.height())
            .expect("Couldn't begin capture.");
        let wnd_cls = "";
        let wnd_title = "RustDeskPrivacyWindow";
        if false == capture_mag.exclude(wnd_cls, wnd_title).unwrap() {
            println!("No window found for cls {} title {}", wnd_cls, wnd_title);
        } else {
            println!("Filter window for cls {} title {}", wnd_cls, wnd_title);
        }

        let frame = capture_mag.frame(Duration::from_millis(0)).unwrap();
        let Frame::PixelBuffer(frame) = frame else {
            return;
        };
        println!("Capture data len: {}, Saving...", frame.data().len());

        let mut raw = Vec::new();
        unsafe {
            scrap::ARGBToRAW(
                frame.data().as_ptr(),
                frame.stride()[0] as _,
                (&mut raw).as_mut_ptr(),
                (w * 3) as _,
                w as _,
                h as _,
            )
        };

        let mut bitflipped = Vec::with_capacity(w * h * 4);
        let stride = raw.len() / h;

        for y in 0..h {
            for x in 0..w {
                let i = stride * y + 3 * x;
                bitflipped.extend_from_slice(&[raw[i], raw[i + 1], raw[i + 2], 255]);
            }
        }
        let name = format!("capture_mag_{}_2.png", i);
        repng::encode(
            File::create(name.clone()).unwrap(),
            w as u32,
            h as u32,
            &bitflipped,
        )
        .unwrap();

        println!("Image saved to `{}`.", name);
    }
}
