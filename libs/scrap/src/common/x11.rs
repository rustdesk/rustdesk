use crate::{x11, common::TraitCapturer};
use std::{io, ops, time::Duration};

pub struct Capturer(x11::Capturer);

#[allow(non_upper_case_globals)]
pub const IS_CURSOR_EMBEDDED: bool = false;

impl Capturer {
    pub fn new(display: Display, yuv: bool) -> io::Result<Capturer> {
        x11::Capturer::new(display.0, yuv).map(Capturer)
    }

    pub fn width(&self) -> usize {
        self.0.display().rect().w as usize
    }

    pub fn height(&self) -> usize {
        self.0.display().rect().h as usize
    }
}

impl TraitCapturer for Capturer {
    fn set_use_yuv(&mut self, use_yuv: bool) {
        self.0.set_use_yuv(use_yuv);
    }

    fn frame<'a>(&'a mut self, _timeout: Duration) -> io::Result<Frame<'a>> {
        Ok(Frame(self.0.frame()?))
    }
}

pub struct Frame<'a>(pub(crate) &'a [u8]);

impl<'a> ops::Deref for Frame<'a> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        self.0
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
        "".to_owned()
    }
}
