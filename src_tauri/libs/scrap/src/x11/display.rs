use std::rc::Rc;

use super::ffi::*;
use super::Server;

#[derive(Debug)]
pub struct Display {
    server: Rc<Server>,
    default: bool,
    rect: Rect,
    root: xcb_window_t,
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct Rect {
    pub x: i16,
    pub y: i16,
    pub w: u16,
    pub h: u16,
}

impl Display {
    pub unsafe fn new(
        server: Rc<Server>,
        default: bool,
        rect: Rect,
        root: xcb_window_t,
    ) -> Display {
        Display {
            server,
            default,
            rect,
            root,
        }
    }

    pub fn server(&self) -> &Rc<Server> {
        &self.server
    }
    pub fn is_default(&self) -> bool {
        self.default
    }
    pub fn rect(&self) -> Rect {
        self.rect
    }
    pub fn w(&self) -> usize {
        self.rect.w as _
    }
    pub fn h(&self) -> usize {
        self.rect.h as _
    }
    pub fn root(&self) -> xcb_window_t {
        self.root
    }
}
