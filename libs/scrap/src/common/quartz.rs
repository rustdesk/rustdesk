use crate::{quartz, Frame, Pixfmt};
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, TryLockError};
use std::{io, mem, time};

/// Probing duration: if CGDisplayStream produces zero frames within this
/// window after start, we switch to the CGWindowList fallback capturer.
const PROBE_DURATION: time::Duration = time::Duration::from_secs(3);

pub struct Capturer {
    /// None when CGDisplayStream creation failed outright (direct fallback mode)
    inner: Option<quartz::Capturer>,
    width: usize,
    height: usize,
    frame: Arc<Mutex<Option<quartz::Frame>>>,
    saved_raw_data: Vec<u8>,
    /// True after fallback switch (CGDisplayStream → CGWindowList)
    degraded: Arc<AtomicBool>,
    /// Frame counter for the main (CGDisplayStream) capturer
    frame_count: Arc<AtomicU64>,
    /// Timestamp when the capturer was created (for probe window)
    start_time: time::Instant,
    /// Fallback frame channel (used when degraded == true)
    fallback_frame: Arc<Mutex<Option<quartz::Frame>>>,
    /// The CGWindowList fallback capturer (None until degradation triggers)
    fallback: Option<quartz::CGWindowListCapturer>,
    /// Bounds of the selected display in global screen coordinates; the
    /// CGWindowList fallback captures exactly this rect.
    display_bounds: quartz::ffi::CGRect,
}

impl Capturer {
    pub fn new(display: Display) -> io::Result<Capturer> {
        let frame = Arc::new(Mutex::new(None));
        let fallback_frame = Arc::new(Mutex::new(None));
        let degraded = Arc::new(AtomicBool::new(false));
        let frame_count = Arc::new(AtomicU64::new(0));
        let start_time = time::Instant::now();
        let (w, h) = (display.width(), display.height());
        let display_bounds = display.0.bounds();

        let f = frame.clone();
        let inner_result = quartz::Capturer::new(
            display.0,
            w,
            h,
            quartz::PixelFormat::Argb8888,
            Default::default(),
            frame_count.clone(),
            move |inner| {
                if let Ok(mut f) = f.lock() {
                    *f = Some(inner);
                }
            },
        );

        let inner = match inner_result {
            Ok(c) => {
                hbb_common::log::info!(
                    "CGDisplayStream created ok ({}x{}), starting 3s frame probe",
                    w,
                    h
                );
                // Normal mode: launch the probe thread
                let f_deg = degraded.clone();
                let f_fc = frame_count.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(PROBE_DURATION);
                    let count = f_fc.load(Ordering::Relaxed);
                    if count == 0 {
                        hbb_common::log::warn!(
                            "CGDisplayStream produced 0 frames in {}s, switching to CGWindowList fallback",
                            PROBE_DURATION.as_secs()
                        );
                        f_deg.store(true, Ordering::Relaxed);
                    }
                });
                Some(c)
            }
            Err(e) => {
                // CGDisplayStream creation failed outright (e.g. constructor
                // returned NULL on some platforms) — start in fallback mode.
                hbb_common::log::warn!(
                    "CGDisplayStream creation failed (err: {:?}), starting CGWindowList fallback directly",
                    e
                );
                degraded.store(true, Ordering::Relaxed);
                None
            }
        };

        Ok(Capturer {
            inner,
            width: w,
            height: h,
            frame,
            saved_raw_data: Vec::new(),
            degraded,
            frame_count,
            start_time,
            fallback_frame,
            fallback: None,
            display_bounds,
        })
    }

    pub fn width(&self) -> usize {
        self.inner.as_ref().map(|c| c.width()).unwrap_or(self.width)
    }

    pub fn height(&self) -> usize {
        self.inner.as_ref().map(|c| c.height()).unwrap_or(self.height)
    }
}

impl crate::TraitCapturer for Capturer {
    fn frame<'a>(&'a mut self, _timeout_ms: std::time::Duration) -> io::Result<Frame<'a>> {
        // Lazy-init fallback capturer on first use after degradation
        if self.degraded.load(Ordering::Relaxed) {
            if self.fallback.is_none() {
                self.fallback = Some(quartz::CGWindowListCapturer::new(
                    self.width(),
                    self.height(),
                    self.display_bounds,
                    self.fallback_frame.clone(),
                ));
            }
            // Read from the fallback frame channel
            return match self.fallback_frame.try_lock() {
                Ok(mut handle) => {
                    let mut frame = None;
                    mem::swap(&mut frame, &mut handle);
                    match frame {
                        Some(mut frame) => {
                            frame.surface_to_bgra(self.height());
                            crate::would_block_if_equal(&mut self.saved_raw_data, &frame.bgra)?;
                            return Ok(Frame::PixelBuffer(PixelBuffer {
                                frame,
                                data: PhantomData,
                                width: self.width(),
                                height: self.height(),
                            }));
                        }
                        None => Err(io::ErrorKind::WouldBlock.into()),
                    }
                }
                Err(TryLockError::WouldBlock) => Err(io::ErrorKind::WouldBlock.into()),
                Err(TryLockError::Poisoned(..)) => Err(io::ErrorKind::Other.into()),
            };
        }

        // Normal path: read from the main frame channel
        match self.frame.try_lock() {
            Ok(mut handle) => {
                let mut frame = None;
                mem::swap(&mut frame, &mut handle);
                match frame {
                    Some(mut frame) => {
                        frame.surface_to_bgra(self.height());
                        crate::would_block_if_equal(&mut self.saved_raw_data, &frame.bgra)?;
                        Ok(Frame::PixelBuffer(PixelBuffer {
                            frame,
                            data: PhantomData,
                            width: self.width(),
                            height: self.height(),
                        }))
                    }
                    None => Err(io::ErrorKind::WouldBlock.into()),
                }
            }
            Err(TryLockError::WouldBlock) => Err(io::ErrorKind::WouldBlock.into()),
            Err(TryLockError::Poisoned(..)) => Err(io::ErrorKind::Other.into()),
        }
    }
}

pub struct PixelBuffer<'a> {
    frame: quartz::Frame,
    data: PhantomData<&'a [u8]>,
    width: usize,
    height: usize,
}

impl<'a> crate::TraitPixelBuffer for PixelBuffer<'a> {
    fn data(&self) -> &[u8] {
        &*self.frame
    }

    fn width(&self) -> usize {
        self.width
    }

    fn height(&self) -> usize {
        self.height
    }

    fn stride(&self) -> Vec<usize> {
        let mut v = Vec::new();
        v.push(self.frame.stride());
        v
    }

    fn pixfmt(&self) -> Pixfmt {
        Pixfmt::BGRA
    }
}

pub struct Display(quartz::Display);

impl Display {
    pub fn primary() -> io::Result<Display> {
        Ok(Display(quartz::Display::primary()))
    }

    pub fn all() -> io::Result<Vec<Display>> {
        Ok(quartz::Display::online()
            .map_err(|_| io::Error::from(io::ErrorKind::Other))?
            .into_iter()
            .map(Display)
            .collect())
    }

    pub fn width(&self) -> usize {
        self.0.width()
    }

    pub fn height(&self) -> usize {
        self.0.height()
    }

    pub fn scale(&self) -> f64 {
        self.0.scale()
    }

    pub fn name(&self) -> String {
        self.0.id().to_string()
    }

    pub fn is_online(&self) -> bool {
        self.0.is_online()
    }

    pub fn origin(&self) -> (i32, i32) {
        let o = self.0.bounds().origin;
        (o.x as _, o.y as _)
    }

    pub fn is_primary(&self) -> bool {
        self.0.is_primary()
    }
}