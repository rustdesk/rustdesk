use std::{
    ptr,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    sync::{Arc, Mutex},
};

use block::{Block, ConcreteBlock};
use hbb_common::libc::c_void;

use super::config::Config;
use super::display::Display;
use super::ffi::*;
use super::frame::Frame;

pub struct Capturer {
    stream: CGDisplayStreamRef,
    queue: DispatchQueue,

    width: usize,
    height: usize,
    format: PixelFormat,
    display: Display,
    stopped: Arc<Mutex<bool>>,
    /// Frame counter for fallback degradation detection
    frame_count: Arc<AtomicU64>,
}

impl Capturer {
    /// `frame_count` must be supplied by the caller (the common-layer wrapper
    /// shares this Arc with its 3s probe thread), otherwise the probe would
    /// always read 0 and degrade to the CGWindowList fallback even while the
    /// main path is producing frames.
    pub fn new<F: Fn(Frame) + 'static>(
        display: Display,
        width: usize,
        height: usize,
        format: PixelFormat,
        config: Config,
        frame_count: Arc<AtomicU64>,
        handler: F,
    ) -> Result<Capturer, CGError> {
        let stopped = Arc::new(Mutex::new(false));
        let cloned_stopped = stopped.clone();
        let fc = frame_count.clone();
        let handler: FrameAvailableHandler = ConcreteBlock::new(move |status, _, surface, _| {
            use self::CGDisplayStreamFrameStatus::*;
            if status == Stopped {
                let mut lock = cloned_stopped.lock().unwrap();
                *lock = true;
                return;
            }
            if status == FrameComplete {
                handler(unsafe { Frame::new(surface) });
                fc.fetch_add(1, Ordering::Relaxed);
            }
        })
        .copy();

        let queue = unsafe {
            dispatch_queue_create(
                b"quadrupleslap.scrap\0".as_ptr() as *const i8,
                ptr::null_mut(),
            )
        };

        let stream = unsafe {
            let config = config.build();
            let stream = CGDisplayStreamCreateWithDispatchQueue(
                display.id(),
                width,
                height,
                format,
                config,
                queue,
                &*handler as *const Block<_, _> as *const c_void,
            );
            CFRelease(config);
            stream
        };

        match unsafe { CGDisplayStreamStart(stream) } {
            CGError::Success => Ok(Capturer {
                stream,
                queue,
                width,
                height,
                format,
                display,
                stopped,
                frame_count,
            }),
            x => Err(x),
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }
    pub fn height(&self) -> usize {
        self.height
    }
    pub fn format(&self) -> PixelFormat {
        self.format
    }
    pub fn display(&self) -> Display {
        self.display
    }

    /// Get the current frame count (for fallback degradation detection)
    pub fn frame_count(&self) -> u64 {
        self.frame_count.load(Ordering::Relaxed)
    }

    /// Reset the frame counter
    pub fn reset_frame_count(&self) {
        self.frame_count.store(0, Ordering::Relaxed);
    }
}

impl Drop for Capturer {
    fn drop(&mut self) {
        unsafe {
            let _ = CGDisplayStreamStop(self.stream);
            loop {
                if *self.stopped.lock().unwrap() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
            CFRelease(self.stream);
            dispatch_release(self.queue);
        }
    }
}

// ─── CGWindowListCapturer ───
//
// Fallback screen capturer that polls CGWindowListCreateImage on a background
// thread.  Used when CGDisplayStream produces zero frames (silent failure on
// certain platforms / GPU drivers).
//
// The capturer accepts an external frame channel (Arc<Mutex<Option<Frame>>>)
// so the parent Capturer can read fallback frames transparently.

pub struct CGWindowListCapturer {
    width: usize,
    height: usize,
    /// External frame channel shared with the parent Capturer
    frame: Arc<Mutex<Option<Frame>>>,
    /// True when the polling thread should stop
    should_stop: Arc<AtomicBool>,
}

impl CGWindowListCapturer {
    pub fn new(
        width: usize,
        height: usize,
        bounds: CGRect,
        frame: Arc<Mutex<Option<Frame>>>,
    ) -> Self {
        let should_stop = Arc::new(AtomicBool::new(false));

        hbb_common::log::info!(
            "CGWindowList fallback capturer starting ({}x{})",
            width,
            height
        );

        let f_frame = frame.clone();
        let f_stop = should_stop.clone();
        std::thread::spawn(move || {
            Self::poll_loop(width, height, bounds, f_frame, f_stop);
        });

        CGWindowListCapturer {
            width,
            height,
            frame,
            should_stop,
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }
    pub fn height(&self) -> usize {
        self.height
    }

    fn poll_loop(
        width: usize,
        height: usize,
        bounds: CGRect,
        frame: Arc<Mutex<Option<Frame>>>,
        should_stop: Arc<AtomicBool>,
    ) {
        // Target ~30 fps (33ms per frame)
        let interval = std::time::Duration::from_millis(33);
        let mut first_frame_done = false;
        let mut consecutive_failures: u32 = 0;

        loop {
            if should_stop.load(Ordering::Relaxed) {
                return;
            }

            let data = match Self::capture_one(width, height, bounds) {
                Some(d) => {
                    if !first_frame_done {
                        first_frame_done = true;
                        hbb_common::log::info!(
                            "CGWindowList fallback: first frame captured ({} bytes)",
                            d.0.len()
                        );
                    }
                    if consecutive_failures >= 30 {
                        hbb_common::log::info!(
                            "CGWindowList fallback: recovered after {} consecutive failures",
                            consecutive_failures
                        );
                    }
                    consecutive_failures = 0;
                    d
                }
                None => {
                    consecutive_failures += 1;
                    // Log every ~10s of consecutive failures (300 frames * 33ms)
                    if consecutive_failures == 1 || consecutive_failures % 300 == 0 {
                        hbb_common::log::warn!(
                            "CGWindowList fallback: capture failed (attempt #{}, likely missing screen recording permission)",
                            consecutive_failures
                        );
                    }
                    std::thread::sleep(interval);
                    continue;
                }
            };

            // Publish the frame and drop the lock BEFORE sleeping: the parent
            // reads this slot with try_lock(), so holding it across the 33ms
            // poll sleep would starve the reader of (nearly) every frame.
            match frame.lock() {
                Ok(mut f_lock) => {
                    *f_lock = Some(Frame::from_bytes(data.0, data.1, height));
                }
                Err(_) => {
                    // Poisoned means a writer panicked mid-update; the slot is
                    // unusable and there is no point in polling further.
                    hbb_common::log::error!(
                        "CGWindowList fallback: frame lock poisoned, stopping"
                    );
                    break;
                }
            }

            std::thread::sleep(interval);
        }
    }

    fn capture_one(
        width: usize,
        height: usize,
        bounds: CGRect,
    ) -> Option<(Vec<u8>, usize)> {
        unsafe {
            // Create a CGColorSpace for the bitmap context
            let cs_name = CFStringCreateWithCString(
                kCFAllocatorDefault(),
                "kCGColorSpaceGenericRGB\0".as_ptr() as *const i8,
                kCFStringEncodingUTF8,
            );
            let color_space = CGColorSpaceCreateWithName(cs_name);
            CFRelease(cs_name);

            if color_space.is_null() {
                return None;
            }

            // Create a CGBitmapContext for one BGRA frame
            let bitmap_info: u32 = kCGImageAlphaPremultipliedFirst | kCGBitmapByteOrder32Little;
            let context = CGBitmapContextCreate(
                ptr::null_mut(),
                width,
                height,
                8,
                0,
                color_space,
                bitmap_info,
            );

            if context.is_null() {
                CGColorSpaceRelease(color_space);
                return None;
            }

            // Get the screen image: capture the selected display's bounds in
            // global screen coordinates. CGRectZero would only be correct for
            // a single display sitting at the origin; multi-display setups
            // would capture the wrong region (or nothing at all).
            let cg_image = CGWindowListCreateImage(
                bounds,
                kCGWindowListOptionOnScreenOnly,
                kCGNullWindowID,
                kCGWindowImageBestResolution,
            );

            if cg_image.is_null() {
                // No image available (e.g. screen locked)
                let _ = CGContextRelease(context);
                CGColorSpaceRelease(color_space);
                return None;
            }

            // Draw the CGImage into the bitmap context (scales if needed)
            CGContextDrawImage(
                context,
                CGRect {
                    origin: CGPoint { x: 0.0, y: 0.0 },
                    size: CGSize {
                        width: width as f64,
                        height: height as f64,
                    },
                },
                cg_image,
            );

            // Get the resulting CGImage from the context
            let result_image = CGBitmapContextCreateImage(context);
            if result_image.is_null() {
                CGImageRelease(cg_image);
                let _ = CGContextRelease(context);
                CGColorSpaceRelease(color_space);
                return None;
            }

            // Extract pixel data
            let provider = CGImageGetDataProvider(result_image);
            let cf_data = CGDataProviderCopyData(provider);

            let result = if cf_data.is_null() {
                CGImageRelease(result_image);
                CGImageRelease(cg_image);
                let _ = CGContextRelease(context);
                CGColorSpaceRelease(color_space);
                None
            } else {
                let ptr = CFDataGetBytePtr(cf_data);
                let length = CFDataGetLength(cf_data) as usize;
                let stride = CGImageGetBytesPerRow(result_image) as usize;

                let mut bgra = Vec::with_capacity(length);
                bgra.extend_from_slice(std::slice::from_raw_parts(ptr, length));

                CFRelease(cf_data);
                // No CGDataProviderRelease(provider): CGImageGetDataProvider
                // returns a reference owned by the image, which the release
                // below disposes of; releasing the provider here would
                // over-free a borrowed reference.
                CGImageRelease(result_image);
                CGImageRelease(cg_image);
                let _ = CGContextRelease(context);
                CGColorSpaceRelease(color_space);

                Some((bgra, stride))
            };

            result
        }
    }
}

impl Drop for CGWindowListCapturer {
    fn drop(&mut self) {
        self.should_stop.store(true, Ordering::Relaxed);
    }
}