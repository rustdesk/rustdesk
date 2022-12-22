use std::boxed::Box;
use std::error::Error;

pub enum PixelProvider<'a> {
    // 8 bits per color
    RGB(usize, usize, &'a [u8]),
    BGR0(usize, usize, &'a [u8]),
    // width, height, stride
    BGR0S(usize, usize, usize, &'a [u8]),
    NONE,
}

impl<'a> PixelProvider<'a> {
    pub fn size(&self) -> (usize, usize) {
        match self {
            PixelProvider::RGB(w, h, _) => (*w, *h),
            PixelProvider::BGR0(w, h, _) => (*w, *h),
            PixelProvider::BGR0S(w, h, _, _) => (*w, *h),
            PixelProvider::NONE => (0, 0),
        }
    }
}

pub trait Recorder {
    fn capture(&mut self, timeout_ms: u64) -> Result<PixelProvider, Box<dyn Error>>;
}

pub trait BoxCloneCapturable {
    fn box_clone(&self) -> Box<dyn Capturable>;
}

impl<T> BoxCloneCapturable for T
where
    T: Clone + Capturable + 'static,
{
    fn box_clone(&self) -> Box<dyn Capturable> {
        Box::new(self.clone())
    }
}

pub trait Capturable: Send + BoxCloneCapturable {
    /// Name of the Capturable, for example the window title, if it is a window.
    fn name(&self) -> String;
    /// Return x, y, width, height of the Capturable as floats relative to the absolute size of the
    /// screen. For example x=0.5, y=0.0, width=0.5, height=1.0 means the right half of the screen.
    fn geometry_relative(&self) -> Result<(f64, f64, f64, f64), Box<dyn Error>>;
    /// Callback that is called right before input is simulated.
    /// Useful to focus the window on input.
    fn before_input(&mut self) -> Result<(), Box<dyn Error>>;
    /// Return a Recorder that can record the current capturable.
    fn recorder(&self, capture_cursor: bool) -> Result<Box<dyn Recorder>, Box<dyn Error>>;
}

impl Clone for Box<dyn Capturable> {
    fn clone(&self) -> Self {
        self.box_clone()
    }
}
