#![allow(dead_code)]

use block::RcBlock;
use hbb_common::libc::c_void;

pub type CGDisplayStreamRef = *mut c_void;
pub type CFDictionaryRef = *mut c_void;
pub type CFBooleanRef = *mut c_void;
pub type CFNumberRef = *mut c_void;
pub type CFStringRef = *mut c_void;
pub type CGDisplayStreamUpdateRef = *mut c_void;
pub type IOSurfaceRef = *mut c_void;
pub type DispatchQueue = *mut c_void;
pub type DispatchQueueAttr = *mut c_void;
pub type CFAllocatorRef = *mut c_void;

#[repr(C)]
pub struct CFDictionaryKeyCallBacks {
    callbacks: [usize; 5],
    version: i32,
}

#[repr(C)]
pub struct CFDictionaryValueCallBacks {
    callbacks: [usize; 4],
    version: i32,
}

macro_rules! pixel_format {
    ($a:expr, $b:expr, $c:expr, $d:expr) => {
        ($a as i32) << 24 | ($b as i32) << 16 | ($c as i32) << 8 | ($d as i32)
    };
}

pub const SURFACE_LOCK_READ_ONLY: u32 = 0x0000_0001;
pub const SURFACE_LOCK_AVOID_SYNC: u32 = 0x0000_0002;

pub fn cfbool(x: bool) -> CFBooleanRef {
    unsafe {
        if x {
            kCFBooleanTrue
        } else {
            kCFBooleanFalse
        }
    }
}

#[repr(i32)]
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum CGDisplayStreamFrameStatus {
    /// A new frame was generated.
    FrameComplete = 0,
    /// A new frame was not generated because the display did not change.
    FrameIdle = 1,
    /// A new frame was not generated because the display has gone blank.
    FrameBlank = 2,
    /// The display stream was stopped.
    Stopped = 3,
    #[doc(hidden)]
    __Nonexhaustive,
}

#[repr(i32)]
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum CFNumberType {
    /* Fixed-width types */
    SInt8 = 1,
    SInt16 = 2,
    SInt32 = 3,
    SInt64 = 4,
    Float32 = 5,
    Float64 = 6,
    /* 64-bit IEEE 754 */
    /* Basic C types */
    Char = 7,
    Short = 8,
    Int = 9,
    Long = 10,
    LongLong = 11,
    Float = 12,
    Double = 13,
    /* Other */
    CFIndex = 14,
    NSInteger = 15,
    CGFloat = 16,
}

#[repr(i32)]
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
#[must_use]
pub enum CGError {
    Success = 0,
    Failure = 1000,
    IllegalArgument = 1001,
    InvalidConnection = 1002,
    InvalidContext = 1003,
    CannotComplete = 1004,
    NotImplemented = 1006,
    RangeCheck = 1007,
    TypeCheck = 1008,
    InvalidOperation = 1010,
    NoneAvailable = 1011,
    #[doc(hidden)]
    __Nonexhaustive,
}

#[repr(i32)]
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum PixelFormat {
    /// Packed Little Endian ARGB8888
    Argb8888 = pixel_format!('B', 'G', 'R', 'A'),
    /// Packed Little Endian ARGB2101010
    Argb2101010 = pixel_format!('l', '1', '0', 'r'),
    /// 2-plane "video" range YCbCr 4:2:0
    YCbCr420Video = pixel_format!('4', '2', '0', 'v'),
    /// 2-plane "full" range YCbCr 4:2:0
    YCbCr420Full = pixel_format!('4', '2', '0', 'f'),
    #[doc(hidden)]
    __Nonexhaustive,
}

pub type CGDisplayStreamFrameAvailableHandler = *const c_void;

pub type FrameAvailableHandler = RcBlock<
    (
        CGDisplayStreamFrameStatus, // status
        u64,                        // displayTime
        IOSurfaceRef,               // frameSurface
        CGDisplayStreamUpdateRef,   // updateRef
    ),
    (),
>;

#[cfg(target_pointer_width = "64")]
pub type CGFloat = f64;
#[cfg(not(target_pointer_width = "64"))]
pub type CGFloat = f32;
#[repr(C)]
pub struct CGPoint {
    pub x: CGFloat,
    pub y: CGFloat,
}
#[repr(C)]
pub struct CGSize {
    pub width: CGFloat,
    pub height: CGFloat,
}
#[repr(C)]
pub struct CGRect {
    pub origin: CGPoint,
    pub size: CGSize,
}

#[link(name = "System", kind = "dylib")]
#[link(name = "CoreGraphics", kind = "framework")]
#[link(name = "CoreFoundation", kind = "framework")]
#[link(name = "IOSurface", kind = "framework")]
extern "C" {
    // CoreGraphics

    pub static kCGDisplayStreamShowCursor: CFStringRef;
    pub static kCGDisplayStreamPreserveAspectRatio: CFStringRef;
    pub static kCGDisplayStreamMinimumFrameTime: CFStringRef;
    pub static kCGDisplayStreamQueueDepth: CFStringRef;

    pub fn CGDisplayStreamCreateWithDispatchQueue(
        display: u32,
        output_width: usize,
        output_height: usize,
        pixel_format: PixelFormat,
        properties: CFDictionaryRef,
        queue: DispatchQueue,
        handler: CGDisplayStreamFrameAvailableHandler,
    ) -> CGDisplayStreamRef;

    pub fn CGDisplayStreamStart(displayStream: CGDisplayStreamRef) -> CGError;

    pub fn CGDisplayStreamStop(displayStream: CGDisplayStreamRef) -> CGError;

    pub fn CGMainDisplayID() -> u32;
    pub fn CGDisplayPixelsWide(display: u32) -> usize;
    pub fn CGDisplayPixelsHigh(display: u32) -> usize;

    pub fn CGGetOnlineDisplayList(
        max_displays: u32,
        online_displays: *mut u32,
        display_count: *mut u32,
    ) -> CGError;

    pub fn CGDisplayIsBuiltin(display: u32) -> i32;
    pub fn CGDisplayIsMain(display: u32) -> i32;
    pub fn CGDisplayIsActive(display: u32) -> i32;
    pub fn CGDisplayIsOnline(display: u32) -> i32;

    pub fn CGDisplayBounds(display: u32) -> CGRect;
    pub fn BackingScaleFactor(display: u32) -> f32;

    // IOSurface

    pub fn IOSurfaceGetAllocSize(buffer: IOSurfaceRef) -> usize;
    pub fn IOSurfaceGetBaseAddress(buffer: IOSurfaceRef) -> *mut c_void;
    pub fn IOSurfaceIncrementUseCount(buffer: IOSurfaceRef);
    pub fn IOSurfaceDecrementUseCount(buffer: IOSurfaceRef);
    pub fn IOSurfaceLock(buffer: IOSurfaceRef, options: u32, seed: *mut u32) -> i32;
    pub fn IOSurfaceUnlock(buffer: IOSurfaceRef, options: u32, seed: *mut u32) -> i32;
    pub fn IOSurfaceGetBaseAddressOfPlane(buffer: IOSurfaceRef, index: usize) -> *mut c_void;
    pub fn IOSurfaceGetBytesPerRowOfPlane(buffer: IOSurfaceRef, index: usize) -> usize;

    // Dispatch

    pub fn dispatch_queue_create(label: *const i8, attr: DispatchQueueAttr) -> DispatchQueue;

    pub fn dispatch_release(object: DispatchQueue);

    // Core Foundation

    pub static kCFTypeDictionaryKeyCallBacks: CFDictionaryKeyCallBacks;
    pub static kCFTypeDictionaryValueCallBacks: CFDictionaryValueCallBacks;

    // EVEN THE BOOLEANS ARE REFERENCES.
    pub static kCFBooleanTrue: CFBooleanRef;
    pub static kCFBooleanFalse: CFBooleanRef;

    pub fn CFNumberCreate(
        allocator: CFAllocatorRef,
        theType: CFNumberType,
        valuePtr: *const c_void,
    ) -> CFNumberRef;

    pub fn CFDictionaryCreate(
        allocator: CFAllocatorRef,
        keys: *const *mut c_void,
        values: *const *mut c_void,
        numValues: i64,
        keyCallBacks: *const CFDictionaryKeyCallBacks,
        valueCallBacks: *const CFDictionaryValueCallBacks,
    ) -> CFDictionaryRef;

    pub fn CFRetain(cf: *const c_void);
    pub fn CFRelease(cf: *const c_void);
}
