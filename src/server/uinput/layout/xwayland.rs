use super::{source::Source, XkbKeymap, DEFAULT_GROUP};
use hbb_common::{anyhow::anyhow, bail, libc, libloading::Library, ResultType};
use std::{
    ffi::{c_void, CStr, CString},
    ptr::NonNull,
};
use xkbcommon_dl::{self as xkb, x11};

struct Connection {
    pointer: NonNull<c_void>,
    disconnect: unsafe extern "C" fn(*mut c_void),
    _library: Library,
}

impl Connection {
    fn open() -> ResultType<Self> {
        let display = CString::new(std::env::var("DISPLAY")?)?;
        if display.as_bytes().is_empty() {
            bail!("Xwayland DISPLAY is empty");
        }
        if !display.as_bytes().starts_with(b":")
            && !display.as_bytes().starts_with(b"unix:")
            && !display.as_bytes().starts_with(b"unix/:")
        {
            bail!("Xwayland layout detection requires a local DISPLAY");
        }
        unsafe {
            let library = Library::new("libxcb.so.1")?;
            let connect = *library
                .get::<unsafe extern "C" fn(*const libc::c_char, *mut libc::c_int) -> *mut c_void>(
                    b"xcb_connect\0",
                )?;
            let disconnect =
                *library.get::<unsafe extern "C" fn(*mut c_void)>(b"xcb_disconnect\0")?;
            let has_error = *library.get::<unsafe extern "C" fn(*mut c_void) -> libc::c_int>(
                b"xcb_connection_has_error\0",
            )?;
            let pointer = NonNull::new(connect(display.as_ptr(), std::ptr::null_mut()))
                .ok_or_else(|| anyhow!("Cannot create Xwayland connection"))?;
            let connection = Self {
                pointer,
                disconnect,
                _library: library,
            };
            let error = has_error(connection.pointer.as_ptr());
            if error != 0 {
                bail!("Cannot connect to Xwayland (XCB error {})", error);
            }
            Ok(connection)
        }
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        unsafe { (self.disconnect)(self.pointer.as_ptr()) };
    }
}

pub(super) fn read() -> ResultType<Source> {
    // Xwayland receives the group through focus-scoped wl_keyboard.modifiers.
    // With a native window focused, both core and slave X11 keyboards can
    // retain the last X11-focused group (verified on GNOME and KWin).
    // Reopening XCB or waiting cannot request the missing compositor update.
    // Prefer native GNOME/KDE sources; do not steal focus to refresh Xwayland.
    let api = xkb::xkbcommon_option().ok_or_else(|| anyhow!("Cannot load libxkbcommon"))?;
    let x11 = x11::xkbcommon_x11_option().ok_or_else(|| anyhow!("Cannot load libxkbcommon-x11"))?;
    let connection = Connection::open()?;
    let device = keyboard_device(x11, &connection)?;
    let keymap = read_keymap(x11, &connection, device)?;
    let state = unsafe {
        (x11.xkb_x11_state_new_from_device)(
            keymap.map.as_ptr(),
            connection.pointer.as_ptr(),
            device,
        )
    };
    let state =
        NonNull::new(state).ok_or_else(|| anyhow!("Cannot read Xwayland keyboard state"))?;
    let group = unsafe {
        (api.xkb_state_serialize_layout)(
            state.as_ptr(),
            xkb::xkb_state_component::XKB_STATE_LAYOUT_EFFECTIVE,
        )
    };
    unsafe { (api.xkb_state_unref)(state.as_ptr()) };
    let text = unsafe {
        (api.xkb_keymap_get_as_string)(
            keymap.map.as_ptr(),
            xkb::xkb_keymap_format::XKB_KEYMAP_FORMAT_TEXT_V1,
        )
    };
    let text = NonNull::new(text.cast_mut())
        .ok_or_else(|| anyhow!("Cannot serialize Xwayland keyboard map"))?;
    let bytes = unsafe { CStr::from_ptr(text.as_ptr()).to_bytes_with_nul().to_vec() };
    unsafe { libc::free(text.as_ptr().cast()) };
    Ok(Source::Xwayland {
        keymap: bytes,
        group: Some(group),
    })
}

fn read_keymap(
    x11: &x11::XkbCommonX11,
    connection: &Connection,
    device: i32,
) -> ResultType<XkbKeymap> {
    let api = xkb::xkbcommon_option().ok_or_else(|| anyhow!("Cannot load libxkbcommon"))?;
    let context =
        unsafe { (api.xkb_context_new)(xkb::xkb_context_flags::XKB_CONTEXT_NO_ENVIRONMENT_NAMES) };
    let context = NonNull::new(context).ok_or_else(|| anyhow!("Cannot create XKB context"))?;
    let map = unsafe {
        (x11.xkb_x11_keymap_new_from_device)(
            context.as_ptr(),
            connection.pointer.as_ptr(),
            device,
            xkb::xkb_keymap_compile_flags::XKB_KEYMAP_COMPILE_NO_FLAGS,
        )
    };
    unsafe { (api.xkb_context_unref)(context.as_ptr()) };
    let map = NonNull::new(map).ok_or_else(|| anyhow!("Cannot read Xwayland keyboard map"))?;
    Ok(XkbKeymap {
        api,
        map,
        group: DEFAULT_GROUP,
    })
}

fn keyboard_device(api: &x11::XkbCommonX11, connection: &Connection) -> ResultType<i32> {
    let supported = unsafe {
        (api.xkb_x11_setup_xkb_extension)(
            connection.pointer.as_ptr(),
            x11::XKB_X11_MIN_MAJOR_XKB_VERSION,
            x11::XKB_X11_MIN_MINOR_XKB_VERSION,
            x11::xkb_x11_setup_xkb_extension_flags::XKB_X11_SETUP_XKB_EXTENSION_NO_FLAGS,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if supported == 0 {
        bail!("Xwayland does not support the required XKB extension");
    }
    let device = unsafe { (api.xkb_x11_get_core_keyboard_device_id)(connection.pointer.as_ptr()) };
    if device < 0 {
        bail!("Xwayland has no core keyboard device");
    }
    Ok(device)
}
