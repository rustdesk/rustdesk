use crate::common::x11::Frame;
use crate::wayland::{capturable::*, *};
use std::io;

pub struct Capturer(Display, Box<dyn Recorder>, bool, Vec<u8>);

fn map_err<E: ToString>(err: E) -> io::Error {
    io::Error::new(io::ErrorKind::Other, err.to_string())
}

impl Capturer {
    pub fn new(display: Display, yuv: bool) -> io::Result<Capturer> {
        let r = display.0.recorder(false).map_err(map_err)?;
        Ok(Capturer(display, r, yuv, Default::default()))
    }

    pub fn width(&self) -> usize {
        self.0.width()
    }

    pub fn height(&self) -> usize {
        self.0.height()
    }

    pub fn frame<'a>(&'a mut self, timeout_ms: u32) -> io::Result<Frame<'a>> {
        match self.1.capture(timeout_ms as _).map_err(map_err)? {
            PixelProvider::BGR0(w, h, x) => Ok(Frame(if self.2 {
                crate::common::bgra_to_i420(w as _, h as _, &x, &mut self.3);
                &self.3[..]
            } else {
                x
            })),
            PixelProvider::NONE => Err(std::io::ErrorKind::WouldBlock.into()),
            _ => Err(map_err("Invalid data")),
        }
    }
}

pub struct Display(pipewire::PipeWireCapturable);

impl Display {
    pub fn primary() -> io::Result<Display> {
        let mut all = Display::all()?;
        if all.is_empty() {
            return Err(io::ErrorKind::NotFound.into());
        }
        Ok(all.remove(0))
    }

    pub fn all() -> io::Result<Vec<Display>> {
        Ok(pipewire::get_capturables(false)
            .map_err(map_err)?
            .drain(..)
            .map(|x| Display(x))
            .collect())
    }

    pub fn width(&self) -> usize {
        self.0.size.0
    }

    pub fn height(&self) -> usize {
        self.0.size.1
    }

    pub fn origin(&self) -> (i32, i32) {
        self.0.position
    }

    pub fn is_online(&self) -> bool {
        true
    }

    pub fn is_primary(&self) -> bool {
        false
    }

    pub fn name(&self) -> String {
        "".to_owned()
    }
}
