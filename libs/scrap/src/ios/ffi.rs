use std::os::raw::{c_uint, c_uchar, c_void};
use std::sync::{Arc, Mutex};
use std::ptr;

#[link(name = "ScreenCapture", kind = "static")]
extern "C" {
    fn ios_capture_init();
    fn ios_capture_start() -> bool;
    fn ios_capture_stop();
    fn ios_capture_is_active() -> bool;
    fn ios_capture_get_frame(
        buffer: *mut c_uchar,
        buffer_size: c_uint,
        out_width: *mut c_uint,
        out_height: *mut c_uint,
    ) -> c_uint;
    fn ios_capture_get_display_info(width: *mut c_uint, height: *mut c_uint);
    fn ios_capture_set_callback(callback: Option<extern "C" fn(*const c_uchar, c_uint, c_uint, c_uint)>);
    fn ios_capture_show_broadcast_picker();
    fn ios_capture_is_broadcasting() -> bool;
    fn ios_capture_set_audio_enabled(enable_mic: bool, enable_app_audio: bool);
    fn ios_capture_set_audio_callback(callback: Option<extern "C" fn(*const c_uchar, c_uint, bool)>);
}

lazy_static::lazy_static! {
    static ref FRAME_BUFFER: Arc<Mutex<FrameBuffer>> = Arc::new(Mutex::new(FrameBuffer::new()));
    static ref INITIALIZED: Mutex<bool> = Mutex::new(false);
}

struct FrameBuffer {
    data: Vec<u8>,
    width: u32,
    height: u32,
    updated: bool,
}

impl FrameBuffer {
    fn new() -> Self {
        FrameBuffer {
            data: Vec::new(),
            width: 0,
            height: 0,
            updated: false,
        }
    }
    
    fn update(&mut self, data: &[u8], width: u32, height: u32) {
        self.data.clear();
        self.data.extend_from_slice(data);
        self.width = width;
        self.height = height;
        self.updated = true;
    }
    
    fn get(&mut self) -> Option<(Vec<u8>, u32, u32)> {
        if self.updated && !self.data.is_empty() {
            self.updated = false; // Reset flag after consuming
            Some((self.data.clone(), self.width, self.height))
        } else {
            None
        }
    }
}

extern "C" fn frame_callback(data: *const c_uchar, size: c_uint, width: c_uint, height: c_uint) {
    if !data.is_null() && size > 0 {
        let slice = unsafe { std::slice::from_raw_parts(data, size as usize) };
        let mut buffer = FRAME_BUFFER.lock().unwrap();
        buffer.update(slice, width, height);
    }
}

pub fn init() {
    let mut initialized = INITIALIZED.lock().unwrap();
    if !*initialized {
        unsafe {
            ios_capture_init();
            ios_capture_set_callback(Some(frame_callback));
        }
        *initialized = true;
        log::info!("iOS screen capture initialized");
    }
}

pub fn start_capture() -> bool {
    init();
    unsafe { ios_capture_start() }
}

pub fn stop_capture() {
    unsafe { ios_capture_stop() }
}

pub fn is_capturing() -> bool {
    unsafe { ios_capture_is_active() }
}

lazy_static::lazy_static! {
    static ref TEMP_BUFFER: Mutex<Vec<u8>> = Mutex::new(vec![0u8; 4096 * 2160 * 4]);
}

pub fn get_frame() -> Option<(Vec<u8>, u32, u32)> {
    // Try callback-based frame first
    if let Ok(mut buffer) = FRAME_BUFFER.try_lock() {
        if let Some(frame) = buffer.get() {
            return Some(frame);
        }
    }
    
    // Fallback to polling
    let mut width: c_uint = 0;
    let mut height: c_uint = 0;
    
    let mut temp_buffer = TEMP_BUFFER.lock().unwrap();
    
    let size = unsafe {
        ios_capture_get_frame(
            temp_buffer.as_mut_ptr(),
            temp_buffer.len() as c_uint,
            &mut width,
            &mut height,
        )
    };
    
    if size > 0 && width > 0 && height > 0 {
        // Only allocate new Vec for the actual data
        let frame_data = temp_buffer[..size as usize].to_vec();
        Some((frame_data, width, height))
    } else {
        None
    }
}

pub fn get_display_info() -> (u32, u32) {
    let mut width: c_uint = 0;
    let mut height: c_uint = 0;
    unsafe {
        ios_capture_get_display_info(&mut width, &mut height);
    }
    (width, height)
}

pub fn show_broadcast_picker() {
    unsafe {
        ios_capture_show_broadcast_picker();
    }
}

pub fn is_broadcasting() -> bool {
    unsafe {
        ios_capture_is_broadcasting()
    }
}

pub fn enable_audio(mic: bool, app_audio: bool) {
    unsafe {
        ios_capture_set_audio_enabled(mic, app_audio);
    }
}

pub fn set_audio_callback(callback: Option<extern "C" fn(*const c_uchar, c_uint, bool)>) {
    unsafe {
        ios_capture_set_audio_callback(callback);
    }
}