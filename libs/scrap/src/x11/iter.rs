use std::ptr;
use std::rc::Rc;

use hbb_common::libc;

use super::ffi::*;
use super::{Display, Rect, Server};

//TODO: Do I have to free the displays?

pub struct DisplayIter {
    outer: xcb_screen_iterator_t,
    inner: Option<(xcb_randr_monitor_info_iterator_t, xcb_window_t)>,
    server: Rc<Server>,
}

impl DisplayIter {
    pub unsafe fn new(server: Rc<Server>) -> DisplayIter {
        let mut outer = xcb_setup_roots_iterator(server.setup());
        let inner = Self::next_screen(&mut outer, &server);
        DisplayIter {
            outer,
            inner,
            server,
        }
    }

    fn next_screen(
        outer: &mut xcb_screen_iterator_t,
        server: &Server,
    ) -> Option<(xcb_randr_monitor_info_iterator_t, xcb_window_t)> {
        if outer.rem == 0 {
            return None;
        }

        unsafe {
            let root = (*outer.data).root;

            let cookie = xcb_randr_get_monitors_unchecked(
                server.raw(),
                root,
                1, //TODO: I don't know if this should be true or false.
            );

            let response = xcb_randr_get_monitors_reply(server.raw(), cookie, ptr::null_mut());

            let inner = xcb_randr_get_monitors_monitors_iterator(response);

            libc::free(response as *mut _);
            xcb_screen_next(outer);

            Some((inner, root))
        }
    }
}

impl Iterator for DisplayIter {
    type Item = Display;

    fn next(&mut self) -> Option<Display> {
        loop {
            if let Some((ref mut inner, root)) = self.inner {
                // If there is something in the current screen, return that.
                if inner.rem != 0 {
                    unsafe {
                        let data = &*inner.data;

                        let display = Display::new(
                            self.server.clone(),
                            data.primary != 0,
                            Rect {
                                x: data.x,
                                y: data.y,
                                w: data.width,
                                h: data.height,
                            },
                            root,
                        );

                        xcb_randr_monitor_info_next(inner);
                        return Some(display);
                    }
                }
            } else {
                // If there is no current screen, the screen iterator is empty.
                return None;
            }

            // The current screen was empty, so try the next screen.
            self.inner = Self::next_screen(&mut self.outer, &self.server);
        }
    }
}
