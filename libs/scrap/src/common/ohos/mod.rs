use super::{Frame, Pixfmt, TraitCapturer, TraitPixelBuffer};
use std::{io, sync::Mutex, time::Duration};

pub mod avcodec;
pub mod direct_render;
pub use direct_render::{
    lookup_direct_render_target, register_direct_render_target_lookup, register_render_context,
    register_render_stats_callback, DirectRenderTarget, DirectRenderTargetLookup,
    RenderStatsCallback,
};

#[derive(Clone, Default)]
struct ScreenFrame {
    rgba: Vec<u8>,
    width: usize,
    height: usize,
    sequence: u64,
}

lazy_static::lazy_static! {
    static ref LATEST_SCREEN_FRAME: Mutex<ScreenFrame> = Default::default();
    static ref CONFIGURED_SCREEN_SIZE: Mutex<(usize, usize)> = Default::default();
}

/// Configure the host display geometry independently from captured frame delivery.
/// Returns `None` for invalid geometry, otherwise whether the geometry changed.
pub fn configure_screen_size(width: usize, height: usize) -> Option<bool> {
    if width == 0 || height == 0 || width.checked_mul(height)?.checked_mul(4).is_none() {
        return None;
    }
    let mut size = CONFIGURED_SCREEN_SIZE.lock().unwrap();
    let changed = *size != (width, height);
    *size = (width, height);
    Some(changed)
}

/// Replace the latest host screen frame supplied by the HarmonyOS frontend.
/// The buffer must contain tightly packed RGBA8888 pixels.
pub fn push_screen_frame_rgba(rgba: &[u8], width: usize, height: usize) -> bool {
    let Some(expected) = width.checked_mul(height).and_then(|v| v.checked_mul(4)) else {
        return false;
    };
    if width == 0 || height == 0 || rgba.len() != expected {
        return false;
    }
    let _ = configure_screen_size(width, height);
    let mut frame = LATEST_SCREEN_FRAME.lock().unwrap();
    frame.rgba.clear();
    frame.rgba.extend_from_slice(rgba);
    frame.width = width;
    frame.height = height;
    frame.sequence = frame.sequence.wrapping_add(1);
    true
}

pub fn screen_size() -> (usize, usize) {
    *CONFIGURED_SCREEN_SIZE.lock().unwrap()
}

/// Clear captured pixels and geometry between host generations so a newly
/// connected viewer can never receive a frame retained from a prior session.
pub fn reset_screen_state() {
    *LATEST_SCREEN_FRAME.lock().unwrap() = ScreenFrame::default();
    *CONFIGURED_SCREEN_SIZE.lock().unwrap() = (0, 0);
}

pub struct Capturer {
    display: Display,
    rgba: Vec<u8>,
    sequence: u64,
}

impl Capturer {
    pub fn new(display: Display) -> io::Result<Self> {
        Ok(Self {
            display,
            rgba: Vec::new(),
            sequence: 0,
        })
    }

    pub fn width(&self) -> usize {
        self.display.width()
    }
    pub fn height(&self) -> usize {
        self.display.height()
    }
}

impl TraitCapturer for Capturer {
    fn frame<'a>(&'a mut self, _timeout: Duration) -> io::Result<Frame<'a>> {
        let frame = LATEST_SCREEN_FRAME.lock().unwrap();
        if frame.sequence == 0 || frame.sequence == self.sequence {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        self.rgba.clone_from(&frame.rgba);
        self.sequence = frame.sequence;
        self.display.width = frame.width;
        self.display.height = frame.height;
        Ok(Frame::PixelBuffer(PixelBuffer::new(
            &self.rgba,
            frame.width,
            frame.height,
            vec![frame.width * 4],
            Pixfmt::RGBA,
        )))
    }
}

#[derive(Clone)]
pub struct Display {
    width: usize,
    height: usize,
}

impl Display {
    pub fn primary() -> io::Result<Self> {
        let (width, height) = screen_size();
        Ok(Self { width, height })
    }
    pub fn all() -> io::Result<Vec<Self>> {
        Ok(vec![Self::primary()?])
    }
    pub fn width(&self) -> usize {
        self.width
    }
    pub fn height(&self) -> usize {
        self.height
    }
    pub fn origin(&self) -> (i32, i32) {
        (0, 0)
    }
    pub fn is_online(&self) -> bool {
        self.width > 0 && self.height > 0
    }
    pub fn is_primary(&self) -> bool {
        true
    }
    pub fn name(&self) -> String {
        "HarmonyOS".to_owned()
    }
    pub fn refresh_size() {}
    pub fn fix_quality() -> u16 {
        1
    }
}
pub struct PixelBuffer<'a> {
    data: &'a [u8],
    width: usize,
    height: usize,
    stride: Vec<usize>,
    pixfmt: Pixfmt,
}

impl<'a> PixelBuffer<'a> {
    pub fn new(
        data: &'a [u8],
        width: usize,
        height: usize,
        stride: Vec<usize>,
        pixfmt: Pixfmt,
    ) -> Self {
        Self {
            data,
            width,
            height,
            stride,
            pixfmt,
        }
    }
}

impl<'a> TraitPixelBuffer for PixelBuffer<'a> {
    fn data(&self) -> &[u8] {
        self.data
    }

    fn width(&self) -> usize {
        self.width
    }

    fn height(&self) -> usize {
        self.height
    }

    fn stride(&self) -> Vec<usize> {
        self.stride.clone()
    }

    fn pixfmt(&self) -> Pixfmt {
        self.pixfmt
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_requires_a_new_frame_for_a_new_host_generation() {
        reset_screen_state();
        assert!(push_screen_frame_rgba(&[1, 2, 3, 4], 1, 1));
        let mut first = Capturer::new(Display::primary().unwrap()).unwrap();
        assert!(first.frame(Duration::ZERO).is_ok());

        reset_screen_state();
        assert_eq!(screen_size(), (0, 0));
        let mut restarted = Capturer::new(Display::primary().unwrap()).unwrap();
        match restarted.frame(Duration::ZERO) {
            Err(error) => assert_eq!(error.kind(), io::ErrorKind::WouldBlock),
            Ok(_) => panic!("a reset host generation must not expose a stale frame"),
        }

        assert!(push_screen_frame_rgba(&[5, 6, 7, 8], 1, 1));
        assert!(restarted.frame(Duration::ZERO).is_ok());
        reset_screen_state();
    }
}
