extern crate libc;
extern crate x11;
use crate::linux::common::{convert, FALSE, KEYBOARD};
use crate::linux::keyboard::Keyboard;
use crate::rdev::{Event, ListenError};
use std::convert::TryInto;
use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_uchar, c_uint, c_ulong};
use std::ptr::null;
use x11::xlib;
use x11::xrecord;

static mut RECORD_ALL_CLIENTS: c_ulong = xrecord::XRecordAllClients;
static mut GLOBAL_CALLBACK: Option<Box<dyn FnMut(Event)>> = None;

pub fn listen<T>(callback: T) -> Result<(), ListenError>
where
    T: FnMut(Event) + 'static,
{
    let keyboard = Keyboard::new().ok_or(ListenError::KeyboardError)?;

    unsafe {
        KEYBOARD = Some(keyboard);
        GLOBAL_CALLBACK = Some(Box::new(callback));
        // Open displays
        let dpy_control = xlib::XOpenDisplay(null());
        if dpy_control.is_null() {
            return Err(ListenError::MissingDisplayError);
        }
        let extension_name = CStr::from_bytes_with_nul(b"RECORD\0")
            .map_err(|_| ListenError::XRecordExtensionError)?;
        let extension = xlib::XInitExtension(dpy_control, extension_name.as_ptr());
        if extension.is_null() {
            return Err(ListenError::XRecordExtensionError);
        }

        // Prepare record range
        let mut record_range: xrecord::XRecordRange = *xrecord::XRecordAllocRange();
        record_range.device_events.first = xlib::KeyPress as c_uchar;
        record_range.device_events.last = if crate::keyboard_only() {
            xlib::KeyRelease
        } else {
            xlib::MotionNotify
        } as c_uchar;

        // Create context
        let context = xrecord::XRecordCreateContext(
            dpy_control,
            0,
            &mut RECORD_ALL_CLIENTS,
            1,
            &mut &mut record_range as *mut &mut xrecord::XRecordRange
                as *mut *mut xrecord::XRecordRange,
            1,
        );

        if context == 0 {
            return Err(ListenError::RecordContextError);
        }

        xlib::XSync(dpy_control, FALSE);
        // Run
        let result =
            xrecord::XRecordEnableContext(dpy_control, context, Some(record_callback), &mut 0);
        if result == 0 {
            return Err(ListenError::RecordContextEnablingError);
        }
    }
    Ok(())
}

// No idea how to do that properly relevant doc lives here:
// https://www.x.org/releases/X11R7.7/doc/libXtst/recordlib.html#Datum_Flags
// https://docs.rs/xproto/1.1.5/xproto/struct._xEvent__bindgen_ty_1.html
// 0.4.2: xproto was removed for some reason and contained the real structs
// but we can't use it anymore.
#[repr(C)]
struct XRecordDatum {
    type_: u8,
    code: u8,
    _rest: u64,
    _1: bool,
    _2: bool,
    _3: bool,
    root_x: i16,
    root_y: i16,
    event_x: i16,
    event_y: i16,
    state: u16,
}

unsafe extern "C" fn record_callback(
    _null: *mut c_char,
    raw_data: *mut xrecord::XRecordInterceptData,
) {
    let Some(data) = raw_data.as_ref() else {
        return;
    };

    if data.category != xrecord::XRecordFromServer {
        return;
    }

    debug_assert!(data.data_len * 4 >= std::mem::size_of::<XRecordDatum>().try_into().unwrap());
    // Cast binary data
    #[allow(clippy::cast_ptr_alignment)]
    let Some(xdatum) = (data.data as *const XRecordDatum).as_ref() else {
        return;
    };

    let code: c_uint = xdatum.code.into();
    let type_: c_int = xdatum.type_.into();
    // let state = xdatum.state;

    let x = xdatum.root_x as f64;
    let y = xdatum.root_y as f64;

    if let Some(event) = convert(&mut KEYBOARD, code, type_, x, y) {
        if let Some(callback) = &mut GLOBAL_CALLBACK {
            callback(event);
        }
    }
    xrecord::XRecordFreeData(raw_data);
}
