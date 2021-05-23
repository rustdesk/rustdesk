//! Native handler wrappers.

#![allow(dead_code)]

use capi::sctypes::{LPCVOID, LPVOID};

type Opaque = LPCVOID;


/// Native wrapper for handlers which can be passed to foreign functions.
#[repr(C)]
#[derive(Debug)]
pub struct NativeHandler {
	// pointer to handler
	handler: Opaque,

	// pointer to handler destructor
	dtor: fn(param: Opaque),
}

impl Drop for NativeHandler {
	fn drop(&mut self) {
		if !self.handler.is_null() {
			(self.dtor)(self.handler);
		}
	}
}

impl Default for NativeHandler {
	fn default() -> Self {
		NativeHandler { handler: ::std::ptr::null(), dtor: NativeHandler::drop_it::<i32> }
	}
}

impl NativeHandler {

	/// Construct boxed wrapper from handler object.
	pub fn from<T>(handler: T) -> NativeHandler {
		let boxed = Box::new(handler);
		let ptr = Box::into_raw(boxed);
		let dtor = NativeHandler::drop_it::<T>;
		return NativeHandler { handler: ptr as Opaque, dtor: dtor };
	}

	/// Return a native pointer to handler wrapper.
	pub fn as_ptr(&self) -> LPCVOID {
		self.handler as LPCVOID
	}

	/// Return a native pointer to handler wrapper.
	pub fn as_mut_ptr(&self) -> LPVOID {
		self.handler as LPVOID
	}

	/// Access handler by reference.
	pub fn as_ref<T>(&self) -> &T {
		let pobj = self.handler as *const T;
		let boxed = unsafe { &*pobj };
		return boxed;
	}

	/// Access handler by mutable reference.
	pub fn as_mut<T>(&mut self) -> &mut T {
		let pobj = self.handler as *mut T;
		let boxed = unsafe { &mut *pobj };
		return boxed;
	}

  #[allow(clippy::mut_from_ref)]
	pub fn get_data<T>(ptr: &LPVOID) -> &mut T {
		assert!(!ptr.is_null());
		let obj = *ptr as *mut T;
		unsafe { &mut *obj}
	}

	// Call destructor of handler.
	fn drop_it<T>(param: Opaque) {
		// reconstruct pointer to Box
		let pobj = param as *mut T;
		if !pobj.is_null() {
			// and drop it
			unsafe { Box::from_raw(pobj) };
		}
	}
}

#[cfg(test)]
mod test {
	use super::NativeHandler;

	struct Handler {
		pub i: i32,
	}

	impl Drop for Handler {
		fn drop(&mut self) {
			println!("Handler::drop");
		}
	}



	#[test]
	fn test1() {
		{
			println!("\ncreate");
			let h = Handler { i: 7 };
			let p = NativeHandler::from(h);

			println!("handler i {:?}", p.as_ref::<Handler>().i);
			println!("quit");
		}
		println!("done.");

		// assert!(false);
	}

}
