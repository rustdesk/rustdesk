use crate::{
    common::{
        wayland,
        x11::{self},
        TraitCapturer,
    },
    Frame,
};
use std::{io, time::Duration};

#[cfg(all(target_os = "linux", feature = "drm"))]
use super::drm;
#[cfg(all(target_os = "linux", feature = "drm"))]
use hbb_common::log;

pub enum Capturer {
    X11(x11::Capturer),
    WAYLAND(wayland::Capturer),
    #[cfg(all(target_os = "linux", feature = "drm"))]
    DRM(drm::Capturer),
}

impl Capturer {
    pub fn new(display: Display) -> io::Result<Capturer> {
        Ok(match display {
            Display::X11(d) => Capturer::X11(x11::Capturer::new(d)?),
            Display::WAYLAND(d) => Capturer::WAYLAND(wayland::Capturer::new(d)?),
            #[cfg(all(target_os = "linux", feature = "drm"))]
            Display::DRM(d) => Capturer::DRM(drm::Capturer::new(d)?),
        })
    }

    pub fn width(&self) -> usize {
        match self {
            Capturer::X11(d) => d.width(),
            Capturer::WAYLAND(d) => d.width(),
            #[cfg(all(target_os = "linux", feature = "drm"))]
            Capturer::DRM(d) => d.width(),
        }
    }

    pub fn height(&self) -> usize {
        match self {
            Capturer::X11(d) => d.height(),
            Capturer::WAYLAND(d) => d.height(),
            #[cfg(all(target_os = "linux", feature = "drm"))]
            Capturer::DRM(d) => d.height(),
        }
    }
}

impl TraitCapturer for Capturer {
    fn frame<'a>(&'a mut self, timeout: Duration) -> io::Result<Frame<'a>> {
        match self {
            Capturer::X11(d) => d.frame(timeout),
            Capturer::WAYLAND(d) => d.frame(timeout),
            #[cfg(all(target_os = "linux", feature = "drm"))]
            Capturer::DRM(d) => d.frame(timeout),
        }
    }
}

pub enum Display {
    X11(x11::Display),
    WAYLAND(wayland::Display),
    #[cfg(all(target_os = "linux", feature = "drm"))]
    DRM(drm::Display),
}

impl Display {
    pub fn primary() -> io::Result<Display> {
        // On Wayland: try DRM/KMS first — no portal consent dialog, works at
        // the login screen. Falls back to PipeWire/portal if DRM is unavailable
        // (helper missing, no active CRTC, etc.).
        #[cfg(all(target_os = "linux", feature = "drm"))]
        if !super::is_x11() {
            if let Ok(d) = drm::Display::primary() {
                log::info!("DRM/KMS capture active");
                return Ok(Display::DRM(d));
            }
        }

        Ok(if super::is_x11() {
            Display::X11(x11::Display::primary()?)
        } else {
            Display::WAYLAND(wayland::Display::primary()?)
        })
    }

    pub fn all() -> io::Result<Vec<Display>> {
        // On Wayland: try DRM/KMS first (see primary() for rationale).
        #[cfg(all(target_os = "linux", feature = "drm"))]
        if !super::is_x11() {
            if let Ok(displays) = drm::Display::all() {
                if !displays.is_empty() {
                    log::info!("DRM/KMS capture active ({} display(s))", displays.len());
                    return Ok(displays.into_iter().map(Display::DRM).collect());
                }
            }
        }

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
            #[cfg(all(target_os = "linux", feature = "drm"))]
            Display::DRM(d) => d.width(),
        }
    }

    pub fn height(&self) -> usize {
        match self {
            Display::X11(d) => d.height(),
            Display::WAYLAND(d) => d.height(),
            #[cfg(all(target_os = "linux", feature = "drm"))]
            Display::DRM(d) => d.height(),
        }
    }

    pub fn scale(&self) -> f64 {
        match self {
            Display::X11(_d) => 1.0,
            Display::WAYLAND(d) => d.scale(),
            #[cfg(all(target_os = "linux", feature = "drm"))]
            Display::DRM(d) => d.scale(),
        }
    }

    pub fn logical_width(&self) -> usize {
        match self {
            Display::X11(d) => d.width(),
            Display::WAYLAND(d) => d.logical_width(),
            #[cfg(all(target_os = "linux", feature = "drm"))]
            Display::DRM(d) => d.logical_width(),
        }
    }

    pub fn logical_height(&self) -> usize {
        match self {
            Display::X11(d) => d.height(),
            Display::WAYLAND(d) => d.logical_height(),
            #[cfg(all(target_os = "linux", feature = "drm"))]
            Display::DRM(d) => d.logical_height(),
        }
    }

    pub fn origin(&self) -> (i32, i32) {
        match self {
            Display::X11(d) => d.origin(),
            Display::WAYLAND(d) => d.origin(),
            #[cfg(all(target_os = "linux", feature = "drm"))]
            Display::DRM(d) => d.origin(),
        }
    }

    pub fn is_online(&self) -> bool {
        match self {
            Display::X11(d) => d.is_online(),
            Display::WAYLAND(d) => d.is_online(),
            #[cfg(all(target_os = "linux", feature = "drm"))]
            Display::DRM(d) => d.is_online(),
        }
    }

    pub fn is_primary(&self) -> bool {
        match self {
            Display::X11(d) => d.is_primary(),
            Display::WAYLAND(d) => d.is_primary(),
            #[cfg(all(target_os = "linux", feature = "drm"))]
            Display::DRM(d) => d.is_primary(),
        }
    }

    pub fn name(&self) -> String {
        match self {
            Display::X11(d) => d.name(),
            Display::WAYLAND(d) => d.name(),
            #[cfg(all(target_os = "linux", feature = "drm"))]
            Display::DRM(d) => d.name(),
        }
    }
}
