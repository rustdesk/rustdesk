use hbb_common::libc;
use std::ptr;
use std::rc::Rc;

use super::ffi::*;
use super::DisplayIter;

#[derive(Debug)]
pub struct Server {
    raw: *mut xcb_connection_t,
    screenp: i32,
    setup: *const xcb_setup_t,
}

/*
use std::cell::RefCell;
thread_local! {
    static SERVER: RefCell<Option<Rc<Server>>> = RefCell::new(None);
}
*/

impl Server {
    pub fn displays(slf: Rc<Server>) -> DisplayIter {
        unsafe { DisplayIter::new(slf) }
    }

    pub fn default() -> Result<Rc<Server>, Error> {
        Ok(Rc::new(Server::connect(ptr::null())?))
        /*
        let mut res = Err(Error::from(0));
        SERVER.with(|xdo| {
            if let Ok(mut server) = xdo.try_borrow_mut() {
                if server.is_some() {
                    unsafe {
                        if 0 != xcb_connection_has_error(server.as_ref().unwrap().raw) {
                            *server = None;
                            println!("Reset x11 connection");
                        }
                    }
                }
                if server.is_none() {
                    println!("New x11 connection");
                    match Server::connect(ptr::null()) {
                        Ok(s) => {
                            let s = Rc::new(s);
                            res = Ok(s.clone());
                            *server = Some(s);
                        }
                        Err(err) => {
                            res = Err(err);
                        }
                    }
                } else {
                    res = Ok(server.as_ref().map(|x| x.clone()).unwrap());
                }
            }
        });
        res
        */
    }

    pub fn connect(addr: *const i8) -> Result<Server, Error> {
        unsafe {
            let mut screenp = 0;
            let raw = xcb_connect(addr, &mut screenp);

            let error = xcb_connection_has_error(raw);
            if error != 0 {
                xcb_disconnect(raw);
                Err(Error::from(error))
            } else {
                let setup = xcb_get_setup(raw);
                Ok(Server {
                    raw,
                    screenp,
                    setup,
                })
            }
        }
    }

    pub fn raw(&self) -> *mut xcb_connection_t {
        self.raw
    }
    pub fn screenp(&self) -> i32 {
        self.screenp
    }
    pub fn setup(&self) -> *const xcb_setup_t {
        self.setup
    }
    pub fn get_shm_status(&self) -> Result<(), Error> {
        unsafe { check_x11_shm_available(self.raw) }
    }
}

unsafe fn check_x11_shm_available(c: *mut xcb_connection_t) -> Result<(), Error> {
    let cookie = xcb_shm_query_version(c);
    let mut e: *mut xcb_generic_error_t = std::ptr::null_mut();
    let reply = xcb_shm_query_version_reply(c, cookie, &mut e as _);
    if reply.is_null() {
        // TODO: Should seperate SHM disabled from SHM not supported?
        return Err(Error::UnsupportedExtension);
    } else {
        // https://github.com/FFmpeg/FFmpeg/blob/6229e4ac425b4566446edefb67d5c225eb397b58/libavdevice/xcbgrab.c#L229
        libc::free(reply as *mut _);
        if e.is_null() {
            return Ok(());
        } else {
            libc::free(e as *mut _);
            // TODO: Does "This request does never generate any errors" in manual means `e` is never set, so we would never reach here?
            return Err(Error::Generic);
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        unsafe {
            xcb_disconnect(self.raw);
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Error {
    Generic,
    UnsupportedExtension,
    InsufficientMemory,
    RequestTooLong,
    ParseError,
    InvalidScreen,
}

impl From<i32> for Error {
    fn from(x: i32) -> Error {
        use self::Error::*;
        match x {
            2 => UnsupportedExtension,
            3 => InsufficientMemory,
            4 => RequestTooLong,
            5 => ParseError,
            6 => InvalidScreen,
            _ => Generic,
        }
    }
}
