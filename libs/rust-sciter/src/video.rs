/*! Sciter video rendering.

Host application can render custom video streams using `<video>` infrastructure.

*/


use capi::sctypes::{UINT, LPCBYTE, LPCSTR};
use capi::scom::som_passport_t;

/// A type alias for Sciter functions that return `bool`.
pub type Result<T> = ::std::result::Result<T, ()>;


/// Color space for video frame.
#[repr(C)]
pub enum COLOR_SPACE {
	Unknown,

	Yv12,
	/// I420
	Iyuv,
	Nv12,
	Yuy2,

	Rgb24,
	Rgb555,
	Rgb565,
	Rgb32,
}

macro_rules! cppcall {
	// self.func()
	($this:ident . $func:ident ()) => {
		unsafe {
			((*$this.vtbl).$func)($this as *mut _)
		}
	};
  (const $this:ident . $func:ident ()) => {
    unsafe {
      ((*$this.vtbl).$func)($this as *const _)
    }
  };

	// self.func(args...)
	($this:ident . $func:ident ( $( $arg:expr ),* )) => {
		unsafe {
			((*$this.vtbl).$func)($this as *mut _, $($arg),* )
		}
	};
  (const $this:ident . $func:ident ( $( $arg:expr ),* )) => {
    unsafe {
      ((*$this.vtbl).$func)($this as *const _, $($arg),* )
    }
  };
}

macro_rules! cppresult {
	( $( $t:tt )* ) => {
		if cppcall!( $($t)* ) {
			Ok(())
		} else {
			Err(())
		}
	}
}

#[doc(hidden)]
pub trait NamedInterface {
	fn get_interface_name() -> &'static [u8];

	fn query_interface(from: &mut iasset) -> Option<* mut iasset> {
		let mut out: *mut iasset = ::std::ptr::null_mut();
		from.get_interface(Self::get_interface_name().as_ptr() as LPCSTR, &mut out as *mut _);
		if !out.is_null() {
			Some(out)
		} else {
			None
		}
	}
}

impl NamedInterface for video_source {
	fn get_interface_name() -> &'static [u8] {
		b"source.video.sciter.com\0"
	}
}

impl NamedInterface for video_destination {
	fn get_interface_name() -> &'static [u8] {
		b"destination.video.sciter.com\0"
	}
}

impl NamedInterface for fragmented_video_destination {
	fn get_interface_name() -> &'static [u8] {
		b"fragmented.destination.video.sciter.com\0"
	}
}


/// COM `IUnknown` alike thing.
#[repr(C)]
struct iasset_vtbl {
	/// Increments the reference count for an interface on an object.
	pub add_ref: extern "C" fn(this: *mut iasset) -> i32,

	/// Decrements the reference count for an interface on an object.
	pub release: extern "C" fn(this: *mut iasset) -> i32,

	/// Retrieves pointers to the supported interfaces on an object.
	pub get_interface: extern "C" fn(this: *mut iasset, name: LPCSTR, out: *mut *mut iasset) -> bool,

	/// Retrieves a pointer to the passport declaration of an object.
	pub get_passport: extern "C" fn(thing: *mut iasset) -> *const som_passport_t,
}

/// COM `IUnknown` alike thing.
#[repr(C)]
pub struct iasset {
	vtbl: *const iasset_vtbl,
}

impl iasset {
	/// Increments the reference count for an interface on an object.
	fn add_ref(&mut self) -> i32 {
		cppcall!(self.add_ref())
	}

	/// Decrements the reference count for an interface on an object.
	fn release(&mut self) -> i32 {
		cppcall!(self.release())
	}

	/// Retrieves pointers to the supported interfaces on an object.
	pub fn get_interface(&mut self, name: LPCSTR, out: *mut *mut iasset) -> bool {
		cppcall!(self.get_interface(name, out))
	}
}


/// Video source interface, used by engine to query video state.
#[repr(C)]
struct video_source_vtbl {
	// <-- iasset:
	/// Increments the reference count for an interface on an object.
	pub add_ref: extern "C" fn(this: *mut video_source) -> i32,

	/// Decrements the reference count for an interface on an object.
	pub release: extern "C" fn(this: *mut video_source) -> i32,

	/// Retrieves pointers to the supported interfaces on an object.
	pub get_interface: extern "C" fn(this: *mut video_source, name: *const u8, out: *mut *mut iasset) -> bool,

	/// Retrieves a pointer to the passport declaration of an object.
	pub get_passport: extern "C" fn(thing: *mut iasset) -> *const som_passport_t,
	// -->

	// <-- video_source
	pub play: extern "C" fn(this: *mut video_source) -> bool,
	pub pause: extern "C" fn(this: *mut video_source) -> bool,
	pub stop: extern "C" fn(this: *mut video_source) -> bool,

	pub get_is_ended: extern "C" fn(this: *const video_source, is_end: *mut bool) -> bool,

	pub get_position: extern "C" fn(this: *const video_source, seconds: *mut f64) -> bool,
	pub set_position: extern "C" fn(this: *mut video_source, seconds: f64) -> bool,

	pub get_duration: extern "C" fn(this: *const video_source, seconds: *mut f64) -> bool,

	pub get_volume: extern "C" fn(this: *const video_source, volume: *mut f64) -> bool,
	pub set_volume: extern "C" fn(this: *mut video_source, volume: f64) -> bool,

	pub get_balance: extern "C" fn(this: *const video_source, balance: *mut f64) -> bool,
	pub set_balance: extern "C" fn(this: *mut video_source, balance: f64) -> bool,
	// -->
}

/// Video source interface to query video state.
#[repr(C)]
pub struct video_source {
	vtbl: *const video_source_vtbl,
}

impl video_source {
	/// Starts playback from the current position.
	pub fn play(&mut self) -> Result<()> {
		cppresult!(self.play())
	}

	/// Pauses playback.
	pub fn pause(&mut self) -> Result<()> {
		cppresult!(self.pause())
	}

	/// Stops playback.
	pub fn stop(&mut self) -> Result<()> {
		cppresult!(self.stop())
	}

	/// Whether playback has reached the end of the video.
	pub fn is_ended(&self) -> Result<bool> {
		let mut r = false;
		cppresult!(const self.get_is_ended(&mut r as *mut _)).map(|_| r)
	}

	/// Reports the current playback position.
	pub fn get_position(&self) -> Result<f64> {
		let mut r = 0f64;
		cppresult!(const self.get_position(&mut r as *mut _)).map(|_| r)
	}

	/// Sets the current playback position.
	pub fn set_position(&mut self, seconds: f64) -> Result<()> {
		cppresult!(self.set_position(seconds))
	}

	/// Reports the duration of the video in seconds.
	///
	/// If duration is not available, returns `0`.
	pub fn get_duration(&self) -> Result<f64> {
		let mut r = 0f64;
		cppresult!(const self.get_duration(&mut r as *mut _)).map(|_| r)
	}

	/// Reports the current volume level of an audio track of the movie.
	///
	/// `1.0` corresponds to `0db`, `0.0` (mute) to `-100db`.
	pub fn get_volume(&self) -> Result<f64> {
		let mut r = 0f64;
		cppresult!(const self.get_volume(&mut r as *mut _)).map(|_| r)
	}

	/// Sets the current volume level between `0.0` (mute) and `1.0` (`0db`).
	pub fn set_volume(&mut self, volume: f64) -> Result<()> {
		cppresult!(self.set_volume(volume))
	}

	/// Reports the current stereo balance.
	pub fn get_balance(&self) -> Result<f64> {
		let mut r = 0f64;
		cppresult!(const self.get_balance(&mut r as *mut _)).map(|_| r)
	}

	/// Sets a new value of the stereo balance.
	pub fn set_balance(&mut self, balance: f64) -> Result<()> {
		cppresult!(self.set_balance(balance))
	}
}


/// Video destination interface, represents video rendering site.
#[repr(C)]
struct video_destination_vtbl {
	// <-- iasset:
	/// Increments the reference count for an interface on an object.
	pub add_ref: extern "C" fn(this: *mut video_destination) -> i32,

	/// Decrements the reference count for an interface on an object.
	pub release: extern "C" fn(this: *mut video_destination) -> i32,

	/// Retrieves pointers to the supported interfaces on an object.
	pub get_interface: extern "C" fn(this: *mut video_destination, name: *const u8, out: *mut *mut iasset) -> bool,

	/// Retrieves a pointer to the passport declaration of an object.
	pub get_passport: extern "C" fn(thing: *mut iasset) -> *const som_passport_t,
	// -->

	// <-- video_destination
	/// Whether this instance of `video_renderer` is attached to a DOM element and is capable of playing.
	pub is_alive: extern "C" fn(this: *const video_destination) -> bool,

	/// Start streaming/rendering.
	pub start_streaming: extern "C" fn(this: *mut video_destination, frame_width: i32, frame_height: i32, color_space: COLOR_SPACE, src: *const video_source) -> bool,

	/// Stop streaming.
	pub stop_streaming: extern "C" fn(this: *mut video_destination) -> bool,

	/// Render the next frame.
	pub render_frame: extern "C" fn(this: *mut video_destination, data: LPCBYTE, size: UINT) -> bool,
	// -->
}

/// Video destination interface, represents video rendering site.
#[repr(C)]
pub struct video_destination {
	vtbl: *const video_destination_vtbl,
}

impl video_destination {

	/// Whether this instance of `video_renderer` is attached to a DOM element and is capable of playing.
	pub fn is_alive(&self) -> bool {
		cppcall!(const self.is_alive())
	}

	/// Start streaming/rendering.
	///
	/// * `frame_size` - the width and the height of the video frame.
	/// * `color_space` - the color space format of the video frame.
	/// * `src` - an optional custom [`video_source`](struct.video_source.html) interface implementation, provided by the application.
	pub fn start_streaming(&mut self, frame_size: (i32, i32), color_space: COLOR_SPACE, src: Option<&video_source>) -> Result<()> {
		let src_ptr = if let Some(ptr) = src { ptr as *const _ } else { ::std::ptr::null() };
		cppresult!(self.start_streaming(frame_size.0, frame_size.1, color_space, src_ptr))
	}

	/// Stop streaming.
	pub fn stop_streaming(&mut self) -> Result<()> {
		cppresult!(self.stop_streaming())
	}

	/// Render the next frame.
	pub fn render_frame(&mut self, data: &[u8]) -> Result<()> {
		cppresult!(self.render_frame(data.as_ptr(), data.len() as UINT))
	}
}


/// Fragmented destination interface, used for partial updates.
#[repr(C)]
struct fragmented_video_destination_vtbl {
	// <-- iasset:
	/// Increments the reference count for an interface on an object.
	pub add_ref: extern "C" fn(this: *mut fragmented_video_destination) -> i32,

	/// Decrements the reference count for an interface on an object.
	pub release: extern "C" fn(this: *mut fragmented_video_destination) -> i32,

	/// Retrieves pointers to the supported interfaces on an object.
	pub get_interface: extern "C" fn(this: *mut fragmented_video_destination, name: *const u8, out: *mut *mut iasset) -> bool,

	/// Retrieves a pointer to the passport declaration of an object.
	pub get_passport: extern "C" fn(thing: *mut iasset) -> *const som_passport_t,
	// -->

	// <-- video_destination
	/// Whether this instance of `video_renderer` is attached to a DOM element and is capable of playing.
	pub is_alive: extern "C" fn(this: *const fragmented_video_destination) -> bool,

	/// Start streaming/rendering.
	pub start_streaming: extern "C" fn(this: *mut fragmented_video_destination, frame_width: i32, frame_height: i32, color_space: COLOR_SPACE, src: *const video_source) -> bool,

	/// Stop streaming.
	pub stop_streaming: extern "C" fn(this: *mut fragmented_video_destination) -> bool,

	/// Render the next frame.
	pub render_frame: extern "C" fn(this: *mut fragmented_video_destination, data: LPCBYTE, size: UINT) -> bool,
	// -->

	// <-- fragmented_video_destination
	/// Render the specified part of the current frame.
	pub render_frame_part: extern "C" fn(this: *mut fragmented_video_destination, data: LPCBYTE, size: UINT, x: i32, y: i32, width: i32, height: i32) -> bool,
	// -->
}

/// Fragmented destination interface, used for partial updates.
#[repr(C)]
pub struct fragmented_video_destination {
	vtbl: *const fragmented_video_destination_vtbl,
}

impl fragmented_video_destination {

	/// Whether this instance of `video_renderer` is attached to a DOM element and is capable of playing.
	pub fn is_alive(&self) -> bool {
		cppcall!(const self.is_alive())
	}

	/// Start streaming/rendering.
	///
	/// * `frame_size` - the width and the height of the video frame.
	/// * `color_space` - the color space format of the video frame.
	/// * `src` - an optional custom [`video_source`](struct.video_source.html) interface implementation, provided by the application.
	pub fn start_streaming(&mut self, frame_size: (i32, i32), color_space: COLOR_SPACE, src: Option<&video_source>) -> Result<()> {
		let src_ptr = if let Some(ptr) = src { ptr as *const _ } else { ::std::ptr::null() };
		cppresult!(self.start_streaming(frame_size.0, frame_size.1, color_space, src_ptr))
	}

	/// Stop streaming.
	pub fn stop_streaming(&mut self) -> Result<()> {
		cppresult!(self.stop_streaming())
	}

	/// Render the next frame.
	pub fn render_frame(&mut self, data: &[u8]) -> Result<()> {
		cppresult!(self.render_frame(data.as_ptr(), data.len() as UINT))
	}

	/// Render the specified part of the current frame.
	///
	/// * `update_point` - X and Y coordinates of the update portion.
	/// * `update_size` - width and height of the update portion.
	pub fn render_frame_part(&mut self, data: &[u8], update_point: (i32, i32), update_size: (i32, i32)) -> Result<()> {
		cppresult!(self.render_frame_part(data.as_ptr(), data.len() as UINT, update_point.0, update_point.1, update_size.0, update_size.1))
	}
}

/// A managed `iasset` pointer.
pub struct AssetPtr<T> {
	ptr: *mut T,
}

/// It's okay to transfer video pointers between threads.
unsafe impl<T> Send for AssetPtr<T> {}

use ::std::ops::{Deref, DerefMut};

impl Deref for AssetPtr<video_destination> {
	type Target = video_destination;

	fn deref(&self) -> &Self::Target {
		unsafe { &*self.ptr }
	}
}

impl DerefMut for AssetPtr<video_destination> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		unsafe { &mut *self.ptr }
	}
}

impl Deref for AssetPtr<fragmented_video_destination> {
	type Target = fragmented_video_destination;

	fn deref(&self) -> &Self::Target {
		unsafe { &*self.ptr }
	}
}

impl DerefMut for AssetPtr<fragmented_video_destination> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		unsafe { &mut *self.ptr }
	}
}

/// Decrements the reference count of a managed pointer.
impl<T> Drop for AssetPtr<T> {
	fn drop(&mut self) {
		self.get().release();
	}
}

impl<T> AssetPtr<T> {
	/// Attach to an existing `iasset` pointer without reference increment.
	fn attach(lp: *mut T) -> Self {
		assert!(!lp.is_null());
		Self {
			ptr: lp
		}
	}

	/// Attach to an `iasset` pointer and increment its reference count.
	pub fn adopt(lp: *mut T) -> Self {
		let mut me = Self::attach(lp);
		me.get().add_ref();
		me
	}

	/// Get as an `iasset` type.
	fn get(&mut self) -> &mut iasset {
		let ptr = self.ptr as *mut iasset;
		unsafe { &mut *ptr }
	}
}


/// Attach to an `iasset` pointer.
impl<T> From<*mut T> for AssetPtr<T> {
	/// Attach to a pointer and increment its reference count.
	fn from(lp: *mut T) -> Self {
		AssetPtr::adopt(lp)
	}
}


/// Attempt to construct `Self` via a conversion.
impl<T: NamedInterface> AssetPtr<T> {

	/// Retrieve a supported interface of the managed pointer.
	///
	/// Example:
	///
	/// ```rust,no_run
	/// # use sciter::video::{AssetPtr, iasset, video_source};
	/// # let external_ptr: *mut iasset = ::std::ptr::null_mut();
	/// let mut site = AssetPtr::adopt(external_ptr);
	/// let source = AssetPtr::<video_source>::try_from(&mut site);
	/// assert!(source.is_ok());
	/// ```
	pub fn try_from<U>(other: &mut AssetPtr<U>) -> Result<Self> {
		let me = T::query_interface(other.get());
		me.map(|p| AssetPtr::adopt(p as *mut T)).ok_or(())
	}
}
