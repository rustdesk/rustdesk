use crate::{
    common::{
        wayland,
        x11::{self},
        TraitCapturer,
    },
    Frame,
};
use std::{io, time::Duration};

pub enum Capturer {
    X11(x11::Capturer),
    WAYLAND(wayland::Capturer),
}

impl Capturer {
    pub fn new(display: Display) -> io::Result<Capturer> {
        Ok(match display {
            Display::X11(d) => Capturer::X11(x11::Capturer::new(d)?),
            Display::WAYLAND(d) => Capturer::WAYLAND(wayland::Capturer::new(d)?),
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
}

impl TraitCapturer for Capturer {
    fn frame<'a>(&'a mut self, timeout: Duration) -> io::Result<Frame<'a>> {
        match self {
            Capturer::X11(d) => d.frame(timeout),
            Capturer::WAYLAND(d) => d.frame(timeout),
        }
    }
}

pub enum Display {
    X11(x11::Display),
    WAYLAND(wayland::Display),
}

impl Display {
    pub fn primary() -> io::Result<Display> {
        Ok(if super::is_x11() {
            Display::X11(x11::Display::primary()?)
        } else {
            Display::WAYLAND(wayland::Display::primary()?)
        })
    }

    // Currently, wayland need to call wayland::clear() before call Display::all()
    pub fn all() -> io::Result<Vec<Display>> {
        Ok(if super::is_x11() {
            x11::Display::all()?
                .drain(..)
                .map(|x| Display::X11(x))
                .collect()
        } else {
            wayland::Display::all()?
                .drain(..)
                .map(|x| Display::WAYLAND(x))
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
