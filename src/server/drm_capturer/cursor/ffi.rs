// The stable PipeWire 0.3/SPA C ABI; load it at runtime like the DRM capture library.
use hbb_common::{anyhow::anyhow, libloading::Library, ResultType};
use std::{
    ffi::{c_char, c_int, c_void},
    mem::size_of,
    sync::OnceLock,
};

pub type Handle = *mut c_void;
pub const META_CURSOR: u32 = 5;
pub const PARAM_FORMAT: u32 = 4;
pub const STREAM_ERROR: c_int = -1;
pub const STREAM_UNCONNECTED: c_int = 0;
pub const DIRECTION_INPUT: c_int = 0;
pub const AUTOCONNECT: u32 = 1;
pub const DONT_RECONNECT: u32 = 1 << 7;
const PARAM_ENUM_FORMAT: u32 = 3;
const PARAM_META: u32 = 6;
const TYPE_ID: u32 = 3;
const TYPE_INT: u32 = 4;
const TYPE_OBJECT: u32 = 15;
const TYPE_CHOICE: u32 = 19;
const CHOICE_RANGE: u32 = 1;
const OBJECT_FORMAT: u32 = 0x40003;
const OBJECT_META: u32 = 0x40005;
const FORMAT_MEDIA_TYPE: u32 = 1;
const FORMAT_MEDIA_SUBTYPE: u32 = 2;
const FORMAT_VIDEO: u32 = 0x20001;
const MEDIA_VIDEO: u32 = 2;
const MEDIA_RAW: u32 = 1;
const VIDEO_BGRA: u32 = 12;
const META_TYPE: u32 = 1;
const META_SIZE: u32 = 2;

macro_rules! api {
    ($($name:ident: $signature:ty),* $(,)?) => {
        pub struct Api {
            _library: Library,
            $(pub $name: $signature,)*
        }

        impl Api {
            pub fn get() -> ResultType<&'static Self> {
                static API: OnceLock<Result<Api, String>> = OnceLock::new();
                API.get_or_init(|| unsafe { Self::load() }.map_err(|e| e.to_string()))
                    .as_ref().map_err(|e| anyhow!("PipeWire cursor library: {e}"))
            }

            unsafe fn load() -> ResultType<Self> {
                let library = Library::new("libpipewire-0.3.so.0")?;
                let api = Self {
                    $($name: *library.get(concat!(stringify!($name), "\0").as_bytes())?,)*
                    _library: library,
                };
                (api.pw_init)(std::ptr::null_mut(), std::ptr::null_mut());
                Ok(api)
            }
        }
    };
}

api! {
    pw_init: unsafe extern "C" fn(*mut c_int, *mut *mut *mut c_char),
    pw_thread_loop_new: unsafe extern "C" fn(*const c_char, Handle) -> Handle,
    pw_thread_loop_get_loop: unsafe extern "C" fn(Handle) -> Handle,
    pw_thread_loop_start: unsafe extern "C" fn(Handle) -> c_int,
    pw_thread_loop_stop: unsafe extern "C" fn(Handle),
    pw_thread_loop_destroy: unsafe extern "C" fn(Handle),
    pw_thread_loop_lock: unsafe extern "C" fn(Handle),
    pw_thread_loop_unlock: unsafe extern "C" fn(Handle),
    pw_properties_new_string: unsafe extern "C" fn(*const c_char) -> Handle,
    pw_stream_new_simple: unsafe extern "C" fn(Handle, *const c_char, Handle, *const Events, Handle) -> Handle,
    pw_stream_connect: unsafe extern "C" fn(Handle, c_int, u32, u32, *const *const Pod, u32) -> c_int,
    pw_stream_update_params: unsafe extern "C" fn(Handle, *const *const Pod, u32) -> c_int,
    pw_stream_dequeue_buffer: unsafe extern "C" fn(Handle) -> *mut PwBuffer,
    pw_stream_queue_buffer: unsafe extern "C" fn(Handle, *mut PwBuffer) -> c_int,
    pw_stream_destroy: unsafe extern "C" fn(Handle),
}

#[repr(C)]
pub struct Events {
    pub version: u32,
    pub destroy: Option<unsafe extern "C" fn()>,
    pub state_changed: Option<unsafe extern "C" fn(Handle, c_int, c_int, *const c_char)>,
    pub control_info: Option<unsafe extern "C" fn()>,
    pub io_changed: Option<unsafe extern "C" fn()>,
    pub param_changed: Option<unsafe extern "C" fn(Handle, u32, *const Pod)>,
    pub add_buffer: Option<unsafe extern "C" fn()>,
    pub remove_buffer: Option<unsafe extern "C" fn()>,
    pub process: Option<unsafe extern "C" fn(Handle)>,
    pub drained: Option<unsafe extern "C" fn()>,
    pub command: Option<unsafe extern "C" fn()>,
    pub trigger_done: Option<unsafe extern "C" fn()>,
}

#[repr(C)]
pub struct PwBuffer {
    pub buffer: *const Buffer,
}

#[repr(C)]
pub struct Buffer {
    pub n_metas: u32,
    pub n_datas: u32,
    pub metas: *const Meta,
    pub datas: Handle,
}

#[repr(C)]
pub struct Meta {
    pub kind: u32,
    pub size: u32,
    pub data: Handle,
}

#[repr(C)]
pub struct Pod {
    size: u32,
    kind: u32,
}

#[repr(C)]
struct Property {
    key: u32,
    flags: u32,
    pod: Pod,
    value: u32,
    padding: u32,
}

impl Property {
    fn new(key: u32, kind: u32, value: u32) -> Self {
        Self {
            key,
            flags: 0,
            pod: Pod {
                size: size_of::<u32>() as u32,
                kind,
            },
            value,
            padding: 0,
        }
    }
}

#[repr(C, align(8))]
pub struct Object<const N: usize> {
    pub pod: Pod,
    kind: u32,
    id: u32,
    properties: [Property; N],
}

impl<const N: usize> Object<N> {
    fn new(kind: u32, id: u32, properties: [Property; N]) -> Self {
        Self {
            pod: Pod {
                size: (size_of::<Self>() - size_of::<Pod>()) as u32,
                kind: TYPE_OBJECT,
            },
            kind,
            id,
            properties,
        }
    }
}

pub fn video_format() -> Object<3> {
    Object::new(
        OBJECT_FORMAT,
        PARAM_ENUM_FORMAT,
        [
            Property::new(FORMAT_MEDIA_TYPE, TYPE_ID, MEDIA_VIDEO),
            Property::new(FORMAT_MEDIA_SUBTYPE, TYPE_ID, MEDIA_RAW),
            Property::new(FORMAT_VIDEO, TYPE_ID, VIDEO_BGRA),
        ],
    )
}

#[repr(C)]
struct SizeRange {
    key: u32,
    flags: u32,
    pod: Pod,
    choice: u32,
    choice_flags: u32,
    child: Pod,
    values: [u32; 3],
    padding: u32,
}

#[repr(C, align(8))]
pub struct CursorMeta {
    pub pod: Pod,
    kind: u32,
    id: u32,
    meta_type: Property,
    size: SizeRange,
}

pub fn cursor_meta() -> CursorMeta {
    CursorMeta {
        pod: Pod {
            size: (size_of::<CursorMeta>() - size_of::<Pod>()) as u32,
            kind: TYPE_OBJECT,
        },
        kind: OBJECT_META,
        id: PARAM_META,
        meta_type: Property::new(META_TYPE, TYPE_ID, META_CURSOR),
        size: SizeRange {
            key: META_SIZE,
            flags: 0,
            pod: Pod {
                size: (size_of::<[u32; 2]>() + size_of::<Pod>() + size_of::<[u32; 3]>()) as u32,
                kind: TYPE_CHOICE,
            },
            choice: CHOICE_RANGE,
            choice_flags: 0,
            child: Pod {
                size: size_of::<u32>() as u32,
                kind: TYPE_INT,
            },
            // Older Mutter allocates 64x64, newer versions 384x384. Let the producer choose.
            values: [
                super::metadata::META_BYTES,
                super::metadata::META_HEADER_BYTES,
                i32::MAX as u32,
            ],
            padding: 0,
        },
    }
}
