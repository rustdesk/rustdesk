use scrap::{
    quartz::{
        ffi::{CFRelease, IOSurfaceDecrementUseCount},
        Frame as SurfaceFrame,
    },
    Display, Frame, PixelBuffer, TraitCapturer,
};
use std::{
    ffi::{c_char, c_int, c_void, CStr},
    io,
    ptr::{self, NonNull},
    time::Duration,
};

extern "C" {
    fn RustDeskSckSupported() -> bool;
    fn RustDeskSckCreate(display: u32, width: u32, height: u32) -> *mut c_void;
    fn RustDeskSckTakeFrame(
        handle: *mut c_void,
        surface: *mut *mut c_void,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
    fn RustDeskSckDestroy(handle: *mut c_void) -> c_int;
}

pub struct SckCapturer {
    handle: NonNull<c_void>,
    width: usize,
    height: usize,
    saved_raw_data: Vec<u8>,
}

impl SckCapturer {
    pub fn is_supported() -> bool {
        unsafe { RustDeskSckSupported() }
    }

    pub fn new(display: &Display) -> io::Result<Self> {
        let display_id = display
            .name()
            .parse::<u32>()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        let width = u32::try_from(display.width())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        let height = u32::try_from(display.height())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        if display_id == 0 || !(1..=4096).contains(&width) || !(1..=4096).contains(&height) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid ScreenCaptureKit display or dimensions",
            ));
        }
        let handle = NonNull::new(unsafe { RustDeskSckCreate(display_id, width, height) })
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::Other,
                    "Failed to create ScreenCaptureKit stream",
                )
            })?;
        Ok(Self {
            handle,
            width: width as usize,
            height: height as usize,
            saved_raw_data: Vec::new(),
        })
    }
}

impl TraitCapturer for SckCapturer {
    fn frame<'a>(&'a mut self, _timeout: Duration) -> io::Result<Frame<'a>> {
        let mut surface = ptr::null_mut();
        let mut error = [0u8; 512];
        let status = unsafe {
            RustDeskSckTakeFrame(
                self.handle.as_ptr(),
                &mut surface,
                error.as_mut_ptr().cast(),
                error.len(),
            )
        };
        if status == 0 {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        if status != 1 || surface.is_null() {
            let message = if status == 1 {
                "ScreenCaptureKit returned an empty surface".to_owned()
            } else {
                match CStr::from_bytes_until_nul(&error) {
                    Ok(message) if !message.to_bytes().is_empty() => {
                        message.to_string_lossy().into_owned()
                    }
                    _ => format!("ScreenCaptureKit capture failed with status {status}"),
                }
            };
            hbb_common::throttled_log!(Duration::from_secs(60), warn, "{message}");
            return Err(io::Error::new(io::ErrorKind::ConnectionReset, message));
        }
        let frame = unsafe {
            let frame = SurfaceFrame::new(surface);
            // Frame owns its own retain and use count; release the native handoff.
            IOSurfaceDecrementUseCount(surface);
            CFRelease(surface);
            frame
        };
        scrap::would_block_if_equal(&mut self.saved_raw_data, frame.inner())?;
        Ok(Frame::PixelBuffer(PixelBuffer::from_frame(
            frame,
            self.width,
            self.height,
        )))
    }
}

impl Drop for SckCapturer {
    fn drop(&mut self) {
        let status = unsafe { RustDeskSckDestroy(self.handle.as_ptr()) };
        if status != 0 {
            hbb_common::throttled_log!(
                Duration::from_secs(60),
                warn,
                "ScreenCaptureKit stream stop returned status {status}"
            );
        }
    }
}

pub fn create_capturer(display: &Display) -> io::Result<Option<Box<dyn TraitCapturer>>> {
    if crate::virtual_display_manager::owns_display(&display.name()) && SckCapturer::is_supported()
    {
        Ok(Some(Box::new(SckCapturer::new(display)?)))
    } else {
        Ok(None)
    }
}
