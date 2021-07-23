use crate::common::{
    wayland,
    x11::{self, Frame},
};
use std::io;

pub enum Capturer {
    X11(x11::Capturer),
    WAYLAND(wayland::Capturer),
}

impl Capturer {
    pub fn new(display: Display, yuv: bool) -> io::Result<Capturer> {
        Ok(match display {
            Display::X11(d) => Capturer::X11(x11::Capturer::new(d, yuv)?),
            Display::WAYLAND(d) => Capturer::WAYLAND(wayland::Capturer::new(d, yuv)?),
        })
    }

    pub fn width(&self) -> usize {
        match self {
            Capturer::X11(d) => d.width(),
            Capturer::WAYLAND(d) => d.width(),
        }
    }

    pub fn height(&self) -> usize {
        match self {
            Capturer::X11(d) => d.height(),
            Capturer::WAYLAND(d) => d.height(),
        }
    }

    pub fn frame<'a>(&'a mut self, timeout_ms: u32) -> io::Result<Frame<'a>> {
        match self {
            Capturer::X11(d) => d.frame(timeout_ms),
            Capturer::WAYLAND(d) => d.frame(timeout_ms),
        }
    }
}

pub enum Display {
    X11(x11::Display),
    WAYLAND(wayland::Display),
}

#[inline]
fn is_wayland() -> bool {
    std::env::var("IS_WAYLAND").is_ok()
        || std::env::var("XDG_SESSION_TYPE") == Ok("wayland".to_owned())
}

impl Display {
    pub fn primary() -> io::Result<Display> {
        Ok(if is_wayland() {
            Display::WAYLAND(wayland::Display::primary()?)
        } else {
            Display::X11(x11::Display::primary()?)
        })
    }

    pub fn all() -> io::Result<Vec<Display>> {
        Ok(if is_wayland() {
            wayland::Display::all()?
                .drain(..)
                .map(|x| Display::WAYLAND(x))
                .collect()
        } else {
            x11::Display::all()?
                .drain(..)
                .map(|x| Display::X11(x))
                .collect()
        })
    }

    pub fn width(&self) -> usize {
        match self {
            Display::X11(d) => d.width(),
            Display::WAYLAND(d) => d.width(),
        }
    }

    pub fn height(&self) -> usize {
        match self {
            Display::X11(d) => d.height(),
            Display::WAYLAND(d) => d.height(),
        }
    }

    pub fn origin(&self) -> (i32, i32) {
        match self {
            Display::X11(d) => d.origin(),
            Display::WAYLAND(d) => d.origin(),
        }
    }

    pub fn is_online(&self) -> bool {
        match self {
            Display::X11(d) => d.is_online(),
            Display::WAYLAND(d) => d.is_online(),
        }
    }

    pub fn is_primary(&self) -> bool {
        match self {
            Display::X11(d) => d.is_primary(),
            Display::WAYLAND(d) => d.is_primary(),
        }
    }

    pub fn name(&self) -> String {
        match self {
            Display::X11(d) => d.name(),
            Display::WAYLAND(d) => d.name(),
        }
    }
}
