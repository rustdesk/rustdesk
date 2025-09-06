use std::rc::Rc;

use super::ffi::*;
use super::Server;
use crate::Pixfmt;

#[derive(Debug)]
pub struct Display {
    server: Rc<Server>,
    default: bool,
    rect: Rect,
    root: xcb_window_t,
    name: String,
    pixfmt: Pixfmt,
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
        name: String,
        pixfmt: Pixfmt,
    ) -> Display {
        Display {
            server,
            default,
            rect,
            root,
            name,
            pixfmt,
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

    pub fn name(&self) -> String {
        self.name.clone()
    }

    pub fn pixfmt(&self) -> Pixfmt {
        self.pixfmt
    }
}
