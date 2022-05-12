use crate::android::ffi::*;
use crate::rgba_to_i420;
use lazy_static::lazy_static;
use std::io;
use std::sync::Mutex;

lazy_static! {
    static ref SCREEN_SIZE: Mutex<(u16, u16)> = Mutex::new((0, 0));
}

pub struct Capturer {
    display: Display,
    bgra: Vec<u8>,
    saved_raw_data: Vec<u128>, // for faster compare and copy
}

impl Capturer {
    pub fn new(display: Display, _yuv: bool) -> io::Result<Capturer> {
        Ok(Capturer {
            display,
            bgra: Vec::new(),
            saved_raw_data: Vec::new(),
        })
    }

    pub fn width(&self) -> usize {
        self.display.width() as usize
    }

    pub fn height(&self) -> usize {
        self.display.height() as usize
    }

    pub fn frame<'a>(&'a mut self, _timeout_ms: u32) -> io::Result<Frame<'a>> {
        if let Some(buf) = get_video_raw() {
            crate::would_block_if_equal(&mut self.saved_raw_data, buf)?;
            rgba_to_i420(self.width(), self.height(), buf, &mut self.bgra);
            Ok(Frame::RAW(&self.bgra))
        } else {
            return Err(io::ErrorKind::WouldBlock.into());
        }
    }
}

pub enum Frame<'a> {
    RAW(&'a [u8]),
    VP9(&'a [u8]),
    Empty,
}

pub struct Display {
    default: bool,
    rect: Rect,
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
struct Rect {
    pub x: i16,
    pub y: i16,
    pub w: u16,
    pub h: u16,
}

impl Display {
    pub fn primary() -> io::Result<Display> {
        let mut size = SCREEN_SIZE.lock().unwrap();
        if size.0 == 0 || size.1 == 0 {
            let (w, h) = get_size().unwrap_or((0, 0));
            size.0 = w;
            size.1 = h;
        }
        Ok(Display {
            default: true,
            rect: Rect {
                x: 0,
                y: 0,
                w: size.0,
                h: size.1,
            },
        })
    }

    pub fn all() -> io::Result<Vec<Display>> {
        Ok(vec![Display::primary()?])
    }

    pub fn width(&self) -> usize {
        self.rect.w as usize
    }

    pub fn height(&self) -> usize {
        self.rect.h as usize
    }

    pub fn origin(&self) -> (i32, i32) {
        let r = self.rect;
        (r.x as _, r.y as _)
    }

    pub fn is_online(&self) -> bool {
        true
    }

    pub fn is_primary(&self) -> bool {
        self.default
    }

    pub fn name(&self) -> String {
        "Android".into()
    }

    pub fn refresh_size() {
        let mut size = SCREEN_SIZE.lock().unwrap();
        let (w, h) = get_size().unwrap_or((0, 0));
        size.0 = w;
        size.1 = h;
    }
}

fn get_size() -> Option<(u16, u16)> {
    let res = call_main_service_get_by_name("screen_size").ok()?;
    if res.len() > 0 {
        let mut sp = res.split(":");
        let w = sp.next()?.parse::<u16>().ok()?;
        let h = sp.next()?.parse::<u16>().ok()?;
        return Some((w, h));
    }
    None
}
