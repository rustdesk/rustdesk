// Controlled-side capture source: an OHOS mirror of `common/android.rs`.
// Frames arrive RGBA8888 from AVScreenCapture through `crate::ohos::ffi`.
use crate::{ohos::{ffi, screencap}, Frame, Pixfmt};
use std::{io, time::Duration};

pub struct Capturer {
    display: Display,
    rgba: Vec<u8>,
    saved_raw_data: Vec<u8>, // for faster compare and copy
}

impl Capturer {
    pub fn new(display: Display) -> io::Result<Capturer> {
        Ok(Capturer {
            display,
            rgba: Vec::new(),
            saved_raw_data: Vec::new(),
        })
    }

    pub fn width(&self) -> usize {
        self.display.width() as usize
    }

    pub fn height(&self) -> usize {
        self.display.height() as usize
    }
}

impl crate::TraitCapturer for Capturer {
    fn frame<'a>(&'a mut self, _timeout: Duration) -> io::Result<Frame<'a>> {
        if ffi::get_video_raw(&mut self.rgba, &mut self.saved_raw_data).is_some() {
            Ok(Frame::PixelBuffer(PixelBuffer::new(
                &self.rgba,
                self.width(),
                self.height(),
            )))
        } else {
            return Err(io::ErrorKind::WouldBlock.into());
        }
    }
}

pub struct PixelBuffer<'a> {
    data: &'a [u8],
    width: usize,
    height: usize,
    stride: Vec<usize>,
}

impl<'a> PixelBuffer<'a> {
    pub fn new(data: &'a [u8], width: usize, height: usize) -> Self {
        let stride0 = data.len() / height;
        let mut stride = Vec::new();
        stride.push(stride0);
        PixelBuffer {
            data,
            width,
            height,
            stride,
        }
    }
}

impl<'a> crate::TraitPixelBuffer for PixelBuffer<'a> {
    fn data(&self) -> &[u8] {
        self.data
    }

    fn width(&self) -> usize {
        self.width
    }

    fn height(&self) -> usize {
        self.height
    }

    fn stride(&self) -> Vec<usize> {
        self.stride.clone()
    }

    fn pixfmt(&self) -> Pixfmt {
        Pixfmt::RGBA
    }
}

pub struct Display {
    default: bool,
    rect: Rect,
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
struct Rect {
    pub x: i16,
    pub y: i16,
    pub w: u16,
    pub h: u16,
}

impl Display {
    pub fn primary() -> io::Result<Display> {
        let (w, h) = screencap::screen_size();
        if w <= 0 || h <= 0 {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "screen capture not started or size unknown",
            ));
        }
        Ok(Display {
            default: true,
            rect: Rect {
                x: 0,
                y: 0,
                w: w as _,
                h: h as _,
            },
        })
    }

    pub fn all() -> io::Result<Vec<Display>> {
        Ok(vec![Display::primary()?])
    }

    pub fn width(&self) -> usize {
        self.rect.w as usize
    }

    pub fn height(&self) -> usize {
        self.rect.h as usize
    }

    pub fn origin(&self) -> (i32, i32) {
        let r = self.rect;
        (r.x as _, r.y as _)
    }

    pub fn is_online(&self) -> bool {
        true
    }

    pub fn is_primary(&self) -> bool {
        self.default
    }

    pub fn name(&self) -> String {
        "HarmonyOS".into()
    }

    pub fn refresh_size() {}

    pub fn fix_quality() -> u16 {
        1
    }
}
