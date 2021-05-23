//! Platform-dependent windows support.

use capi::sctypes::*;


pub trait BaseWindow {

	fn create(&mut self, rc: RECT, flags: UINT, parent: HWINDOW) -> HWINDOW;

	fn get_hwnd(&self) -> HWINDOW;

	fn collapse(&self, hide: bool);
	fn expand(&self, maximize: bool);
	fn dismiss(&self);

	fn set_title(&mut self, title: &str);
	fn get_title(&self) -> String;

	fn run_app(&self);
	fn quit_app(&self);
}

#[cfg(windows)]
mod windows {

	use ::{_API};
	use capi::sctypes::*;
	use capi::scdef::*;


	#[link(name="user32")]
	extern "system"
	{
		fn ShowWindow(hwnd: HWINDOW, show: INT) -> BOOL;
		fn PostMessageW(hwnd: HWINDOW, msg: UINT, w: WPARAM, l: LPARAM) -> BOOL;
		fn SetWindowTextW(hwnd: HWINDOW, s: LPCWSTR) -> BOOL;
		fn GetWindowTextLengthW(hwnd: HWINDOW) -> INT;
		fn GetWindowTextW(hwnd: HWINDOW, s: LPWSTR, l: INT) -> INT;
		fn GetMessageW(msg: LPMSG, hwnd: HWINDOW, min: UINT, max: UINT) -> BOOL;
		fn DispatchMessageW(msg: LPMSG) -> LRESULT;
		fn TranslateMessage(msg: LPMSG) -> BOOL;
		fn PostQuitMessage(code: INT);
	}

	#[link(name="ole32")]
	extern "system"
	{
		fn OleInitialize(pv: LPCVOID) -> i32;	// HRESULT
	}

	pub struct OsWindow
	{
		hwnd: HWINDOW,
		flags: UINT,
	}

	impl OsWindow {

		pub fn new() -> OsWindow {
			OsWindow { hwnd: 0 as HWINDOW, flags: 0 }
		}

		pub fn from(hwnd: HWINDOW) -> OsWindow {
			OsWindow { hwnd: hwnd, flags: 0 }
		}

		fn init_app() {
			unsafe { OleInitialize(::std::ptr::null()) };
		}

	}

	impl super::BaseWindow for OsWindow {

		/// Get native window handle.
		fn get_hwnd(&self) -> HWINDOW {
			return self.hwnd;
		}

		/// Create a new native window.
		fn create(&mut self, rc: RECT, flags: UINT, parent: HWINDOW) -> HWINDOW {

			if (flags & SCITER_CREATE_WINDOW_FLAGS::SW_MAIN as u32) != 0 {
				OsWindow::init_app();
			}

			self.flags = flags;

			#[cfg(not(feature = "windowless"))]
			{
				let cb = ::std::ptr::null();
				self.hwnd = (_API.SciterCreateWindow)(flags, &rc, cb, 0 as LPVOID, parent);
				if self.hwnd.is_null() {
					panic!("Failed to create window!");
				}
			}
			#[cfg(feature = "windowless")]
			{
				let _ = rc;
				let _ = parent;
				let _ = &(_API.SciterVersion);
			}

			return self.hwnd;
		}

		/// Minimize or hide window.
		fn collapse(&self, hide: bool) {
			let n: INT = if hide { 0 } else { 6 };
			unsafe { ShowWindow(self.hwnd, n) };
		}

		/// Show or maximize window.
		fn expand(&self, maximize: bool) {
			let n: INT = if maximize { 3 } else { 1 };
			unsafe { ShowWindow(self.hwnd, n) };
		}

		/// Close window.
		fn dismiss(&self) {
			unsafe { PostMessageW(self.hwnd, 0x0010, 0, 0) };
		}

		/// Set native window title.
		fn set_title(&mut self, title: &str) {
			let s = s2w!(title);
			unsafe { SetWindowTextW(self.hwnd, s.as_ptr()) };
		}

		/// Get native window title.
		fn get_title(&self) -> String {

			let n = unsafe { GetWindowTextLengthW(self.hwnd) + 1 };
			let mut title: Vec<u16> = Vec::new();
			title.resize(n as usize, 0);
			unsafe { GetWindowTextW(self.hwnd, title.as_mut_ptr(), n) };
			return ::utf::w2s(title.as_ptr());
		}

		/// Run the main app message loop until window been closed.
		fn run_app(&self) {
			let mut msg = MSG { hwnd: 0 as HWINDOW, message: 0, wParam: 0, lParam: 0, time: 0, pt: POINT { x: 0, y: 0 } };
			let pmsg: LPMSG = &mut msg;
			let null: HWINDOW = ::std::ptr::null_mut();
			unsafe {
				while GetMessageW(pmsg, null, 0, 0) != 0 {
					TranslateMessage(pmsg);
					DispatchMessageW(pmsg);
				}
			};
		}

		/// Post app quit message.
		fn quit_app(&self) {
			unsafe { PostQuitMessage(0) };
		}
	}

}

#[cfg(target_os="linux")]
mod windows {

	use ::{_API};
	use capi::sctypes::*;
	use capi::scdef::*;
	use super::BaseWindow;

	use ::std::ptr;

	#[link(name="gtk-3")]
	extern "C"
	{
		fn gtk_init(argc: *const i32, argv: *const *const LPCSTR);
		fn gtk_main();
		fn gtk_main_quit();
		fn gtk_widget_get_toplevel(view: HWINDOW) -> HWINDOW;
		fn gtk_window_present(hwnd: HWINDOW);
		fn gtk_widget_hide(hwnd: HWINDOW);
		fn gtk_window_maximize(hwnd: HWINDOW);
		fn gtk_window_iconify(hwnd: HWINDOW);
		fn gtk_window_close(hwnd: HWINDOW);
		fn gtk_window_set_title(hwnd: HWINDOW, title: LPCSTR);
		fn gtk_window_get_title(hwnd: HWINDOW) -> LPCSTR;
	}

	pub struct OsWindow
	{
		hwnd: HWINDOW,
		flags: UINT,
	}

	impl OsWindow {

		pub fn new() -> OsWindow {
			OsWindow { hwnd: 0 as HWINDOW, flags: 0 }
		}

		pub fn from(hwnd: HWINDOW) -> OsWindow {
			OsWindow { hwnd: hwnd, flags: 0 }
		}

		fn init_app() {
			unsafe { gtk_init(ptr::null(), ptr::null()) };
		}

		fn window(&self) -> HWINDOW {
			let hwnd = self.get_hwnd();
			if hwnd.is_null() {
				hwnd
			} else {
				unsafe { gtk_widget_get_toplevel(hwnd) }
			}
		}

	}

	impl super::BaseWindow for OsWindow {

		/// Get native window handle.
		fn get_hwnd(&self) -> HWINDOW {
			return self.hwnd;
		}

		/// Create a new native window.
		fn create(&mut self, rc: RECT, flags: UINT, parent: HWINDOW) -> HWINDOW {

			if (flags & SCITER_CREATE_WINDOW_FLAGS::SW_MAIN as u32) != 0 {
				OsWindow::init_app();
			}

			self.flags = flags;

			#[cfg(not(feature = "windowless"))]
			{
				let cb = ptr::null();
				self.hwnd = (_API.SciterCreateWindow)(flags, &rc, cb, 0 as LPVOID, parent);
				if self.hwnd.is_null() {
					panic!("Failed to create window!");
				}
			}
			#[cfg(feature = "windowless")]
			{
				let _ = rc;
				let _ = parent;
				let _ = &(_API.SciterVersion);
			}
			return self.hwnd;
		}

		/// Minimize or hide window.
		fn collapse(&self, hide: bool) {
			unsafe {
				if hide {
					gtk_widget_hide(self.get_hwnd())
				} else {
					gtk_window_iconify(self.window())
				}
			};
		}

		/// Show or maximize window.
		fn expand(&self, maximize: bool) {
			let wnd = self.window();
			unsafe {
				if maximize {
					gtk_window_maximize(wnd)
				} else {
					gtk_window_present(wnd)
				}
			};
		}

		/// Close window.
		fn dismiss(&self) {
			unsafe { gtk_window_close(self.window()) };
		}

		/// Set native window title.
		fn set_title(&mut self, title: &str) {
			let s = s2u!(title);
			unsafe { gtk_window_set_title(self.window(), s.as_ptr()) };
		}

		/// Get native window title.
		fn get_title(&self) -> String {
			let s = unsafe { gtk_window_get_title(self.window()) };
			return u2s!(s);
		}

		/// Run the main app message loop until window been closed.
		fn run_app(&self) {
			unsafe { gtk_main() };
		}

		/// Post app quit message.
		fn quit_app(&self) {
			unsafe { gtk_main_quit() };
		}
	}

}

#[cfg(target_os="macos")]
mod windows {

	extern crate objc_foundation;

	use objc::runtime::{Class, Object};
	use self::objc_foundation::{NSString, INSString};


	/// Activation policies that control whether and how an app may be activated.
	#[repr(C)]
	#[allow(dead_code)]
	enum NSApplicationActivationPolicy {
		Regular = 0,
		Accessory,
		Prohibited,
	}


	// Note: Starting some OSX version (perhaps, 10.13),
	// the AppKit framework isn't loaded implicitly.
	#[link(name="CoreFoundation", kind="framework")]
	extern {}

	#[link(name="AppKit", kind="framework")]
	extern {}

	use ::{_API};
	use capi::sctypes::*;
	use capi::scdef::*;
	use super::BaseWindow;

	pub struct OsWindow
	{
		hwnd: HWINDOW,
		flags: UINT,
	}

	impl OsWindow {

		pub fn new() -> OsWindow {
			OsWindow { hwnd: 0 as HWINDOW, flags: 0, }
		}

		pub fn from(hwnd: HWINDOW) -> OsWindow {
			OsWindow { hwnd: hwnd, flags: 0 }
		}

		fn get_app() -> *mut Object {
			let cls = Class::get("NSApplication").expect("`NSApplication` is not registered.");
			let obj = unsafe { msg_send!(cls, sharedApplication) };
			return obj;
		}

		fn init_app() {
			// By default, unbundled apps start with `NSApplicationActivationPolicyProhibited` (no dock, no menu).
			let app = OsWindow::get_app();
			let _: () = unsafe { msg_send!(app, setActivationPolicy:NSApplicationActivationPolicy::Regular) };
		}

		fn view(&self) -> *mut Object {
			let hwnd = self.get_hwnd();
			let hwnd: *mut Object = unsafe { ::std::mem::transmute(hwnd) };
			return hwnd;
		}

		fn window(&self) -> *mut Object {
			let hwnd = self.view();
			let obj: *mut Object = unsafe { msg_send!(hwnd, window) };
			assert!(!obj.is_null());
			return obj;
		}
	}

	impl super::BaseWindow for OsWindow {

		/// Get native window handle.
		fn get_hwnd(&self) -> HWINDOW {
			return self.hwnd;
		}

		/// Create a new native window.
		fn create(&mut self, rc: RECT, flags: UINT, parent: HWINDOW) -> HWINDOW {

			if (flags & SCITER_CREATE_WINDOW_FLAGS::SW_MAIN as u32) != 0 {
				OsWindow::init_app();
			}

			self.flags = flags;

			#[cfg(not(feature = "windowless"))]
			{
				let w = rc.right - rc.left;
				let h = rc.bottom - rc.top;
				let prc: *const RECT = if w > 0 && h > 0 {
					&rc
				} else {
					0 as *const RECT
				};

				let cb = 0 as *const SciterWindowDelegate;
				self.hwnd = (_API.SciterCreateWindow)(flags, prc, cb, 0 as LPVOID, parent);
				if self.hwnd.is_null() {
					panic!("Failed to create window!");
				}
			}
			#[cfg(feature = "windowless")]
			{
				let _ = rc;
				let _ = parent;
				let _ = &(_API.SciterVersion);
			}
			return self.hwnd;
		}

		/// Minimize or hide window.
		fn collapse(&self, hide: bool) {
			let wnd = self.window();
			if hide {
				let _: () = unsafe { msg_send!(wnd, orderOut:0) };
			} else {
				let hwnd = self.view();
				let _: () = unsafe { msg_send!(wnd, performMiniaturize:hwnd) };
			}
		}

		/// Show or maximize window.
		fn expand(&self, maximize: bool) {
			let wnd = self.window();
			if (self.flags & SCITER_CREATE_WINDOW_FLAGS::SW_TITLEBAR as UINT) != 0 {
				let app = OsWindow::get_app();
				let _: () = unsafe { msg_send!(app, activateIgnoringOtherApps:true) };
			}
			unsafe {
				let _: () = msg_send!(wnd, makeKeyAndOrderFront:0);
				// msg_send!(wnd, orderFrontRegardless);
			}
			if maximize {
				let _: () = unsafe { msg_send!(wnd, performZoom:0) };
			}
		}

		/// Close window.
		fn dismiss(&self) {
			let wnd = self.window();
			let _: () = unsafe { msg_send!(wnd, close) };
		}

		/// Set native window title.
		fn set_title(&mut self, title: &str) {
			let s = NSString::from_str(title);
			let wnd = self.window();
			let _: () = unsafe { msg_send!(wnd, setTitle:s) };
		}

		/// Get native window title.
		fn get_title(&self) -> String {
			String::new()
		}

		/// Run the main app message loop until window been closed.
		fn run_app(&self) {
			let app = OsWindow::get_app();
			let _: () = unsafe { msg_send!(app, finishLaunching) };
			let _: () = unsafe { msg_send!(app, run) };
		}

		/// Post app quit message.
		fn quit_app(&self) {
			let app = OsWindow::get_app();
			let _: () = unsafe { msg_send!(app, terminate:app) };
		}
	}

}

pub type OsWindow = windows::OsWindow;
