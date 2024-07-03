use crate::{common::TraitCapturer, x11, Frame, Pixfmt, TraitPixelBuffer};
use std::{io, time::Duration};

pub struct Capturer(x11::Capturer);

pub const IS_CURSOR_EMBEDDED: bool = false;

impl Capturer {
    pub fn new(display: Display) -> io::Result<Capturer> {
        x11::Capturer::new(display.0).map(Capturer)
    }

    pub fn width(&self) -> usize {
        self.0.display().rect().w as usize
    }

    pub fn height(&self) -> usize {
        self.0.display().rect().h as usize
    }
}

impl TraitCapturer for Capturer {
    fn frame<'a>(&'a mut self, _timeout: Duration) -> io::Result<Frame<'a>> {
        let width = self.width();
        let height = self.height();
        let pixfmt = self.0.display().pixfmt();
        Ok(Frame::PixelBuffer(PixelBuffer::new(
            self.0.frame()?,
            pixfmt,
            width,
            height,
        )))
    }
}

pub struct PixelBuffer<'a> {
    data: &'a [u8],
    pixfmt: Pixfmt,
    width: usize,
    height: usize,
    stride: Vec<usize>,
}

impl<'a> PixelBuffer<'a> {
    pub fn new(data: &'a [u8], pixfmt: Pixfmt, width: usize, height: usize) -> Self {
        let stride0 = data.len() / height;
        let mut stride = Vec::new();
        stride.push(stride0);
        Self {
            data,
            pixfmt,
            width,
            height,
            stride,
        }
    }
}

impl<'a> TraitPixelBuffer for PixelBuffer<'a> {
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

    fn pixfmt(&self) -> crate::Pixfmt {
        self.pixfmt
    }
}

pub struct Display(x11::Display);

impl Display {
    pub fn primary() -> io::Result<Display> {
        let server = match x11::Server::default() {
            Ok(server) => server,
            Err(_) => return Err(io::ErrorKind::ConnectionRefused.into()),
        };

        let mut displays = x11::Server::displays(server);
        let mut best = displays.next();
        if best.as_ref().map(|x| x.is_default()) == Some(false) {
            best = displays.find(|x| x.is_default()).or(best);
        }

        match best {
            Some(best) => Ok(Display(best)),
            None => Err(io::ErrorKind::NotFound.into()),
        }
    }

    pub fn all() -> io::Result<Vec<Display>> {
        let server = match x11::Server::default() {
            Ok(server) => server,
            Err(_) => return Err(io::ErrorKind::ConnectionRefused.into()),
        };

        Ok(x11::Server::displays(server).map(Display).collect())
    }

    pub fn width(&self) -> usize {
        self.0.rect().w as usize
    }

    pub fn height(&self) -> usize {
        self.0.rect().h as usize
    }

    pub fn origin(&self) -> (i32, i32) {
        let r = self.0.rect();
        (r.x as _, r.y as _)
    }

    pub fn is_online(&self) -> bool {
        true
    }

    pub fn is_primary(&self) -> bool {
        self.0.is_default()
    }

    pub fn name(&self) -> String {
        self.0.name()
    }

    pub fn get_shm_status(&self) -> Result<(), x11::Error> {
        self.0.server().get_shm_status()
    }
}
