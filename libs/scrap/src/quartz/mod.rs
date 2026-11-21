pub use self::capturer::Capturer;
pub use self::config::Config;
pub use self::display::Display;
pub use self::ffi::{CGError, PixelFormat};
pub use self::frame::Frame;
pub use self::sck::Capturer as SckCapturer;

mod capturer;
mod config;
mod display;
pub mod ffi;
mod frame;
mod sck;

use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};

lazy_static::lazy_static! {
    pub static ref ENABLE_RETINA: Arc<Mutex<bool>> = Arc::new(Mutex::new(true));
    static ref EXCLUDED_WINDOW_IDS: Mutex<Vec<u32>> = Mutex::new(Vec::new());
}

static EXCLUDED_WINDOW_IDS_GENERATION: AtomicU64 = AtomicU64::new(0);

pub fn set_excluded_window_ids(mut window_ids: Vec<u32>) {
    window_ids.sort_unstable();
    window_ids.dedup();
    let mut current = EXCLUDED_WINDOW_IDS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if *current == window_ids {
        return;
    }
    *current = window_ids;
    EXCLUDED_WINDOW_IDS_GENERATION.fetch_add(1, Ordering::AcqRel);
}

pub(crate) fn excluded_window_ids() -> (Vec<u32>, u64) {
    let window_ids = EXCLUDED_WINDOW_IDS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let generation = EXCLUDED_WINDOW_IDS_GENERATION.load(Ordering::Acquire);
    (window_ids.clone(), generation)
}
