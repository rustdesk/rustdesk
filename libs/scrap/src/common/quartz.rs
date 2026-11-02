use crate::{quartz, Frame, Pixfmt};
use std::marker::PhantomData;
use std::sync::{Arc, Mutex, TryLockError};
use std::{io, mem};

pub fn uses_screen_capture_kit() -> bool {
    quartz::SckCapturer::is_available()
}

enum Backend {
    ScreenCaptureKit(quartz::SckCapturer),
    DisplayStream(quartz::Capturer),
}

impl Backend {
    fn width(&self) -> usize {
        match self {
            Self::ScreenCaptureKit(capturer) => capturer.width(),
            Self::DisplayStream(capturer) => capturer.width(),
        }
    }

    fn height(&self) -> usize {
        match self {
            Self::ScreenCaptureKit(capturer) => capturer.height(),
            Self::DisplayStream(capturer) => capturer.height(),
        }
    }
}

pub struct Capturer {
    inner: Backend,
    frame: Arc<Mutex<Option<quartz::Frame>>>,
    saved_raw_data: Vec<u8>, // for faster compare and copy
    excluded_window_ids_generation: u64,
}

impl Capturer {
    pub fn new(display: Display) -> io::Result<Capturer> {
        let frame = Arc::new(Mutex::new(None));

        let (excluded_window_ids, excluded_window_ids_generation) = quartz::excluded_window_ids();
        let inner = if uses_screen_capture_kit() {
            let f = frame.clone();
            match quartz::SckCapturer::new(
                display.0,
                display.width(),
                display.height(),
                quartz::PixelFormat::Argb8888,
                false,
                &excluded_window_ids,
                move |inner| {
                    *f.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(inner);
                },
            ) {
                Ok(capturer) => Backend::ScreenCaptureKit(capturer),
                // A legacy fallback would request the same missing permission on each retry.
                Err(err) if err.kind() == io::ErrorKind::PermissionDenied => return Err(err),
                Err(err) => {
                    hbb_common::throttled_log!(
                        std::time::Duration::from_secs(5),
                        warn,
                        "Failed to start ScreenCaptureKit, falling back to CGDisplayStream: {err}"
                    );
                    let f = frame.clone();
                    Backend::DisplayStream(
                        quartz::Capturer::new(
                            display.0,
                            display.width(),
                            display.height(),
                            quartz::PixelFormat::Argb8888,
                            Default::default(),
                            move |inner| {
                                if let Ok(mut f) = f.lock() {
                                    *f = Some(inner);
                                }
                            },
                        )
                        .map_err(|_| io::Error::from(io::ErrorKind::Other))?,
                    )
                }
            }
        } else {
            let f = frame.clone();
            Backend::DisplayStream(
                quartz::Capturer::new(
                    display.0,
                    display.width(),
                    display.height(),
                    quartz::PixelFormat::Argb8888,
                    Default::default(),
                    move |inner| {
                        if let Ok(mut f) = f.lock() {
                            *f = Some(inner);
                        }
                    },
                )
                .map_err(|_| io::Error::from(io::ErrorKind::Other))?,
            )
        };

        Ok(Capturer {
            inner,
            frame,
            saved_raw_data: Vec::new(),
            excluded_window_ids_generation,
        })
    }

    pub fn width(&self) -> usize {
        self.inner.width()
    }

    pub fn height(&self) -> usize {
        self.inner.height()
    }
}

impl crate::TraitCapturer for Capturer {
    fn frame<'a>(&'a mut self, _timeout_ms: std::time::Duration) -> io::Result<Frame<'a>> {
        if let Backend::ScreenCaptureKit(capturer) = &self.inner {
            if let Some(error) = capturer.stream_error() {
                return Err(io::Error::new(io::ErrorKind::Other, error));
            }
            let (window_ids, generation) = quartz::excluded_window_ids();
            if generation != self.excluded_window_ids_generation {
                if let Err(err) = capturer.update_excluded_window_ids(&window_ids) {
                    hbb_common::throttled_log!(
                        std::time::Duration::from_secs(5),
                        warn,
                        "Failed to exclude screen frame windows from capture: {err}"
                    );
                }
                self.excluded_window_ids_generation = generation;
            }
        }
        match self.frame.try_lock() {
            Ok(mut handle) => {
                let mut frame = None;
                mem::swap(&mut frame, &mut handle);

                match frame {
                    Some(mut frame) => {
                        crate::would_block_if_equal(&mut self.saved_raw_data, frame.inner())?;
                        frame.surface_to_bgra(self.height());
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

pub fn set_excluded_window_ids(window_ids: Vec<u32>) {
    quartz::set_excluded_window_ids(window_ids);
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
