pub use self::capturer::Capturer;
pub use self::config::Config;
pub use self::display::Display;
pub use self::ffi::{CGError, PixelFormat};
pub use self::frame::Frame;

mod capturer;
mod config;
mod display;
pub mod ffi;
mod frame;

use std::sync::{Arc, Mutex};

lazy_static::lazy_static! {
    pub static ref ENABLE_RETINA: Arc<Mutex<bool>> = Arc::new(Mutex::new(true));
}
