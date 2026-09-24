use block::{Block, ConcreteBlock};
use hbb_common::{libc::c_void, log};
use std::{
    ptr,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Condvar, Mutex,
    },
    time::Duration,
};

use super::config::Config;
use super::display::Display;
use super::ffi::*;
use super::frame::Frame;

const STOP_TIMEOUT: Duration = Duration::from_secs(2);
// Display::online enumerates at most 16 displays. Allow one replacement set
// while unacknowledged streams remain retained as required by CoreGraphics.
const MAX_NATIVE_STREAMS: usize = 32;
static NATIVE_STREAMS: AtomicUsize = AtomicUsize::new(0);

struct StreamPermit(&'static AtomicUsize);

impl StreamPermit {
    fn acquire(count: &'static AtomicUsize) -> Option<Self> {
        count
            .fetch_update(Ordering::AcqRel, Ordering::Relaxed, |count| {
                (count < MAX_NATIVE_STREAMS).then(|| count + 1)
            })
            .ok()
            .map(|_| Self(count))
    }
}

impl Drop for StreamPermit {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

struct NativeStream {
    stream: CGDisplayStreamRef,
    queue: DispatchQueue,
    // Preserve deferred ownership even if CoreGraphics discards its callback.
    _shutdown: Arc<Shutdown>,
    _permit: StreamPermit,
}

// Ownership may move between shutdown callers and the callback; native release
// always runs on the stream's serial callback queue after Stopped.
unsafe impl Send for NativeStream {}

impl NativeStream {
    fn release_later(self) {
        extern "C" fn release(context: *mut c_void) {
            let native = unsafe { Box::from_raw(context as *mut NativeStream) };
            unsafe {
                if !native.stream.is_null() {
                    CFRelease(native.stream);
                }
                dispatch_release(native.queue);
            }
            log::info!("Released stopped display stream {:p}", native.stream);
        }

        let queue = self.queue;
        let context = Box::into_raw(Box::new(self)) as *mut c_void;
        unsafe { dispatch_async_f(queue, context, release) };
    }
}

#[derive(Default)]
struct ShutdownState {
    stopped: bool,
    retired: Option<NativeStream>,
}

#[derive(Default)]
struct Shutdown {
    state: Mutex<ShutdownState>,
    changed: Condvar,
    closing: AtomicBool,
}

impl Shutdown {
    fn did_stop(&self) {
        let retired = {
            let mut state = self.state.lock().unwrap();
            state.stopped = true;
            self.changed.notify_all();
            state.retired.take()
        };
        if let Some(native) = retired {
            native.release_later();
        }
    }

    fn finish(&self, native: NativeStream, timeout: Duration) -> bool {
        let (mut state, _) = self
            .changed
            .wait_timeout_while(self.state.lock().unwrap(), timeout, |state| !state.stopped)
            .unwrap();
        log::info!("Retiring display stream {:p}", native.stream);
        if state.stopped {
            drop(state);
            native.release_later();
            true
        } else {
            // Stop can return success without delivering Stopped. Retain both
            // the state and resources until that notification makes release safe.
            state.retired = Some(native);
            false
        }
    }
}

pub struct Capturer {
    stream: CGDisplayStreamRef,
    queue: DispatchQueue,

    width: usize,
    height: usize,
    format: PixelFormat,
    display: Display,
    shutdown: Arc<Shutdown>,
    permit: Option<StreamPermit>,
}

impl Capturer {
    pub fn new<F: Fn(Frame) + 'static>(
        display: Display,
        width: usize,
        height: usize,
        format: PixelFormat,
        config: Config,
        handler: F,
    ) -> Result<Capturer, CGError> {
        let permit = StreamPermit::acquire(&NATIVE_STREAMS).ok_or_else(|| {
            log::error!("Cannot create display stream: native stream limit reached, waiting for stopped streams to be reclaimed");
            CGError::CannotComplete
        })?;
        let shutdown = Arc::new(Shutdown::default());
        let callback_shutdown = shutdown.clone();
        let handler: FrameAvailableHandler = ConcreteBlock::new(move |status, _, surface, _| {
            use self::CGDisplayStreamFrameStatus::*;
            if status == Stopped {
                callback_shutdown.did_stop();
                return;
            }
            if status == FrameComplete && !callback_shutdown.closing.load(Ordering::Acquire) {
                handler(unsafe { Frame::new(surface) });
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

        if stream.is_null() {
            unsafe { dispatch_release(queue) };
            return Err(CGError::Failure);
        }
        match unsafe { CGDisplayStreamStart(stream) } {
            CGError::Success => Ok(Capturer {
                stream,
                queue,
                width,
                height,
                format,
                display,
                shutdown,
                permit: Some(permit),
            }),
            error => {
                unsafe {
                    CFRelease(stream);
                    dispatch_release(queue);
                }
                Err(error)
            }
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

    pub(crate) fn is_stopped(&self) -> bool {
        self.shutdown.state.lock().unwrap().stopped
    }
}

impl Drop for Capturer {
    fn drop(&mut self) {
        self.shutdown.closing.store(true, Ordering::Release);
        let result = unsafe { CGDisplayStreamStop(self.stream) };
        if result != CGError::Success {
            log::warn!(
                "Failed to stop display {} stream {:p}: {:?}",
                self.display.id(),
                self.stream,
                result
            );
        }
        if let Some(permit) = self.permit.take() {
            let native = NativeStream {
                stream: self.stream,
                queue: self.queue,
                _shutdown: self.shutdown.clone(),
                _permit: permit,
            };
            if !self.shutdown.finish(native, STOP_TIMEOUT) {
                log::warn!("Timed out waiting for display {} stream {:p} to stop; retaining native resources until Stopped", self.display.id(), self.stream);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{thread, time::Instant};

    fn native_stream(count: &'static AtomicUsize, shutdown: &Arc<Shutdown>) -> NativeStream {
        NativeStream {
            stream: ptr::null_mut(),
            queue: unsafe {
                dispatch_queue_create(
                    b"rustdesk.test.capture-stop\0".as_ptr() as _,
                    ptr::null_mut(),
                )
            },
            _shutdown: shutdown.clone(),
            _permit: StreamPermit::acquire(count).unwrap(),
        }
    }

    fn wait_for_count(count: &AtomicUsize, expected: usize) {
        let started = Instant::now();
        while count.load(Ordering::Acquire) != expected {
            assert!(started.elapsed() < Duration::from_secs(2));
            thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn shutdown_timeout_retains_resources_until_late_stop_once() {
        static COUNT: AtomicUsize = AtomicUsize::new(0);
        static CALLBACK_COUNT: AtomicUsize = AtomicUsize::new(0);
        extern "C" fn late_stop(context: *mut c_void) {
            let shutdown = unsafe { Box::from_raw(context as *mut Arc<Shutdown>) };
            shutdown.did_stop();
            shutdown.did_stop();
            CALLBACK_COUNT.store(COUNT.load(Ordering::Acquire), Ordering::Release);
        }

        let shutdown = Arc::new(Shutdown::default());
        let retained = Arc::downgrade(&shutdown);
        let native = native_stream(&COUNT, &shutdown);
        let queue = native.queue;
        let started = Instant::now();
        assert!(!shutdown.finish(native, Duration::from_millis(20)));
        assert!(started.elapsed() < Duration::from_secs(2));
        drop(shutdown);
        assert_eq!(COUNT.load(Ordering::Acquire), 1);
        let context = Box::into_raw(Box::new(retained.upgrade().unwrap())) as *mut c_void;
        unsafe { dispatch_async_f(queue, context, late_stop) };
        wait_for_count(&COUNT, 0);
        assert_eq!(CALLBACK_COUNT.load(Ordering::Acquire), 1);
        assert!(retained.upgrade().is_none());
    }

    #[test]
    fn stopped_before_shutdown_releases_without_waiting() {
        static COUNT: AtomicUsize = AtomicUsize::new(0);
        let shutdown = Arc::new(Shutdown::default());
        shutdown.did_stop();
        assert!(shutdown.finish(native_stream(&COUNT, &shutdown), Duration::ZERO));
        wait_for_count(&COUNT, 0);
    }
}
