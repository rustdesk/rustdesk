//! Sciter platform-dependent types.

#![allow(non_camel_case_types, non_snake_case)]

extern crate libc;

use self::libc::*;


// common
MAKE_HANDLE!(#[doc = "Window native handle."] HWINDOW, _HWINDOW); // HWND or NSView* or GtkWidget*
MAKE_HANDLE!(#[doc = "Archive native handle."] HSARCHIVE, _HSARCHIVE);

pub type BYTE = u8;
pub type INT = i32;
pub type LONG = i32;
pub type UINT = u32;
pub type INT64 = i64;
pub type UINT64 = u64;

pub type FLOAT_VALUE = f64;

pub type WPARAM = size_t;
pub type LPARAM = ssize_t;

pub type UINT_PTR = uintptr_t;
pub type LRESULT = ssize_t;

pub type CHAR = c_char;
pub type LPSTR = *mut CHAR;
pub type LPCSTR = *const CHAR;

pub type WCHAR = u16;
pub type LPWSTR = *mut WCHAR;
pub type LPCWSTR = *const WCHAR;

pub type LPCBYTE = *const BYTE;
pub type LPUINT = *mut UINT;

pub type VOID = c_void;
pub type LPVOID = *mut VOID;
pub type LPCVOID = *const VOID;

#[cfg(windows)]
pub type BOOL = i32;

#[cfg(not(windows))]
pub type BOOL = i8;

pub type PBOOL = *mut BOOL;

/// Defines the coordinates of the upper-left and lower-right corners of a rectangle.
#[repr(C)]
#[derive(Clone, Copy, PartialEq)]
#[derive(Default, Debug)]
pub struct RECT {
    pub left: LONG,
    pub top: LONG,
    pub right: LONG,
    pub bottom: LONG,
}
pub type LPRECT = *mut RECT;
pub type LPCRECT = *const RECT;

impl RECT {
	/// Calculate the height of the rect.
	pub fn height(&self) -> LONG {
		self.bottom - self.top
	}

	/// Calculate the width of the rect.
	pub fn width(&self) -> LONG {
		self.right - self.left
	}

	/// Return the size of the rect in width and height form.
	pub fn size(&self) -> SIZE {
		SIZE {
			cx: self.width(),
			cy: self.height(),
		}
	}

	/// Returns the top-left point of the rect.
	pub fn topleft(&self) -> POINT {
		POINT {
			x: self.left,
			y: self.top,
		}
	}
}

/// Defines the `x` and `y` coordinates of a point.
#[repr(C)]
#[derive(Clone, Copy, PartialEq)]
#[derive(Default, Debug)]
pub struct POINT {
    pub x: LONG,
    pub y: LONG,
}
pub type LPPOINT = *mut POINT;

/// Specifies the width and height of a rectangle.
#[repr(C)]
#[derive(Clone, Copy, PartialEq)]
#[derive(Default, Debug)]
pub struct SIZE {
    pub cx: LONG,
    pub cy: LONG,
}
pub type LPSIZE = *mut SIZE;


#[cfg(windows)]
#[repr(C)]
#[derive(Debug)]
pub struct MSG {
    pub hwnd: HWINDOW,
    pub message: UINT,
    pub wParam: WPARAM,
    pub lParam: LPARAM,
    pub time: UINT,
    pub pt: POINT,
}
#[cfg(windows)]
pub type LPMSG = *mut MSG;

