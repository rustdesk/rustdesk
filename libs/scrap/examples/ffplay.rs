use std::time::Duration;

extern crate scrap;

fn main() {
    use scrap::{Capturer, Display, TraitCapturer};
    use std::io::ErrorKind::WouldBlock;
    use std::io::Write;
    use std::process::{Command, Stdio};

    let d = Display::primary().unwrap();
    let (w, h) = (d.width(), d.height());

    let child = Command::new("ffplay")
        .args(&[
            "-f",
            "rawvideo",
            "-pixel_format",
            "bgr0",
            "-video_size",
            &format!("{}x{}", w, h),
            "-framerate",
            "60",
            "-",
        ])
        .stdin(Stdio::piped())
        .spawn()
        .expect("This example requires ffplay.");

    let mut capturer = Capturer::new(d, false).unwrap();
    let mut out = child.stdin.unwrap();

    loop {
        match capturer.frame(Duration::from_millis(0)) {
            Ok(frame) => {
                // Write the frame, removing end-of-row padding.
                let stride = frame.len() / h;
                let rowlen = 4 * w;
                for row in frame.chunks(stride) {
                    let row = &row[..rowlen];
                    out.write_all(row).unwrap();
                }
            }
            Err(ref e) if e.kind() == WouldBlock => {
                // Wait for the frame.
            }
            Err(_) => {
                // We're done here.
                break;
            }
        }
    }
}
