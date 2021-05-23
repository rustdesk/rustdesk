/*! Windowless Sciter.

Windowless here means that Sciter does not use any `HWND`, `NSView*` or whatever OS uses for window designation.
You just need to provide something of size `void*` that will be associated with the instance of the engine.

Check out [this article](https://sciter.com/sciter-lite-is-published/) on sciter.com that explains
the difference between the desktop and the windowless Sciter engine versions.

*/

use ::{_API};
use capi::scdef::{GFX_LAYER};
use capi::scdom::HELEMENT;
use capi::sctypes::{HWINDOW, POINT, UINT, BOOL, RECT, LPCBYTE, LPVOID, INT};
use capi::scmsg::*;

pub use capi::scmsg::key_codes;
pub use capi::scbehavior::{MOUSE_BUTTONS, MOUSE_EVENTS, KEYBOARD_STATES, KEY_EVENTS};


/// Application-provided events to notify Sciter.
#[derive(Debug)]
pub enum Message {
	/// Creates an instance of Sciter assotiated with the given handle.
	Create {
		/// Graphics backend for rendering.
		backend: GFX_LAYER,
		/// Background transparency option.
		transparent: bool,
	},

	/// Destroys the engine instance.
	Destroy,

	/// Window size changes.
	Size {
		/// Width of the rendering surface.
		width: u32,
		/// Height of the rendering surface.
		height: u32,
	},

	/// Screen resolution changes.
	Resolution {
		/// Pixels per inch.
		ppi: u32,
	},

	/// Window focus event.
	Focus {
		/// Whether the window has got or lost the input focus.
		enter: bool,
	},

	/// Time changes in order to process animations, timers and other timed things.
	Heartbit {
		/// Absolute steady clock value, e.g. `GetTickCount()` or `glfwGetTime()`.
		milliseconds: u32,
	},

	/// Redraw the whole document.
	Redraw,

	/// Redraw the specific layer.
	Paint(PaintLayer),

	/// Render to a bitmap.
	RenderTo(RenderEvent),

	#[cfg(any(windows, doc))]
	/// Render to a DXGI surface (Windows only, since 4.4.3.27).
	RenderToDxgiSurface(DxgiRenderEvent),

	/// Mouse input.
	Mouse(MouseEvent),

	/// Keyboard input.
	Keyboard(KeyboardEvent),
}

/// Events describing the mouse input.
#[derive(Debug)]
pub struct MouseEvent {
	/// A specific mouse event, like "mouse down" or "mouse move".
	pub event: MOUSE_EVENTS,
	/// Which mouse button is pressed.
	pub button: MOUSE_BUTTONS,
	/// Which keyboard modifier (e.g. Ctrl or Alt) is pressed.
	pub modifiers: KEYBOARD_STATES,
	/// Mouse cursor position.
	pub pos: POINT,
}

/// Events describing the keyboard input.
#[derive(Debug)]
pub struct KeyboardEvent {
	/// A specific key event, like "key down" or "key up".
	pub event: KEY_EVENTS,
	/// A key code:
	///
	/// * a keyboard [scan-code](key_codes/index.html)
	/// for [`KEY_DOWN`](enum.KEY_EVENTS.html#variant.KEY_DOWN)
	/// and [`KEY_UP`](enum.KEY_EVENTS.html#variant.KEY_UP) events;
	/// * a Unicode code point for [`KEY_CHAR`](enum.KEY_EVENTS.html#variant.KEY_CHAR).
	pub code: UINT,
	/// Which keyboard modifier (e.g. Ctrl or Alt) is pressed.
	pub modifiers: KEYBOARD_STATES,
}

/// A specific UI layer to redraw.
#[derive(Debug)]
pub struct PaintLayer {
	/// A DOM element (layer) to render.
	pub element: HELEMENT,

	/// Whether the `element` is the topmost layer or a background one.
	pub is_foreground: bool,
}

/// Events for rendering UI to a bitmap.
pub struct RenderEvent
{
	/// Which layer to render (or the whole document if `None`).
	pub layer: Option<PaintLayer>,

	/// The callback that receives a rendered bitmap.
	///
	/// The first argument contains a rectangle with the coordinates (position and size) of the rendered bitmap.
	///
	/// The second ardument is the rendered bitmap in the `BGRA` form. The size of the bitmap equals to `width * height * 4`.
	pub callback: Box<dyn Fn(&RECT, &[u8])>,
}

impl std::fmt::Debug for RenderEvent {
	fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
		fmt
			.debug_struct("RenderEvent")
			.field("layer", &self.layer)
			.field("callback", &"Box<dyn Fn>")
			.finish()
	}
}

#[cfg(any(windows, doc))]
#[derive(Debug)]
/// Events for rendering UI to a DXGI surface.
///
/// Since 4.4.3.27.
pub struct DxgiRenderEvent {
	/// Which layer to render (or the whole document if `None`).
	pub layer: Option<PaintLayer>,

	/// [`IDXGISurface`](https://docs.microsoft.com/en-us/windows/win32/api/dxgi/nn-dxgi-idxgisurface) pointer.
	pub surface: LPVOID,
}


/// Notify Sciter about UI-specific events.
///
/// `wnd` here is not a window handle but rather a window instance (pointer).
pub fn handle_message(wnd: HWINDOW, event: Message) -> bool
{
	let ok = match event {
		Message::Create { backend, transparent } => {
			let msg = SCITER_X_MSG_CREATE {
				header: SCITER_X_MSG_CODE::SXM_CREATE.into(),
				backend,
				transparent: transparent as BOOL,
			};
			(_API.SciterProcX)(wnd, &msg.header as *const _)
		},

		Message::Destroy => {
			let msg = SCITER_X_MSG_DESTROY {
				header: SCITER_X_MSG_CODE::SXM_DESTROY.into(),
			};
			(_API.SciterProcX)(wnd, &msg.header as *const _)
		},

		Message::Size { width, height} => {
			let msg = SCITER_X_MSG_SIZE {
				header: SCITER_X_MSG_CODE::SXM_SIZE.into(),
				width,
				height,
			};
			(_API.SciterProcX)(wnd, &msg.header as *const _)
		},

		Message::Resolution { ppi } => {
			let msg = SCITER_X_MSG_RESOLUTION {
				header: SCITER_X_MSG_CODE::SXM_RESOLUTION.into(),
				ppi,
			};
			(_API.SciterProcX)(wnd, &msg.header as *const _)
		},

		Message::Focus { enter } => {
			let msg = SCITER_X_MSG_FOCUS {
				header: SCITER_X_MSG_CODE::SXM_FOCUS.into(),
				enter: enter as BOOL,
			};
			(_API.SciterProcX)(wnd, &msg.header as *const _)
		},

		Message::Heartbit { milliseconds } => {
			let msg = SCITER_X_MSG_HEARTBIT {
				header: SCITER_X_MSG_CODE::SXM_HEARTBIT.into(),
				time: milliseconds,
			};
			(_API.SciterProcX)(wnd, &msg.header as *const _)
		},

		Message::Mouse(params) => {
			let msg = SCITER_X_MSG_MOUSE {
				header: SCITER_X_MSG_CODE::SXM_MOUSE.into(),

				event: params.event,
				button: params.button,
				modifiers: params.modifiers as u32,
				pos: params.pos,
			};
			(_API.SciterProcX)(wnd, &msg.header as *const _)
		},

		Message::Keyboard(params) => {
			let msg = SCITER_X_MSG_KEY {
				header: SCITER_X_MSG_CODE::SXM_KEY.into(),

				event: params.event,
				code: params.code,
				modifiers: params.modifiers as u32,
			};
			(_API.SciterProcX)(wnd, &msg.header as *const _)
		},

		Message::Redraw => {
			use std::ptr;
			let msg = SCITER_X_MSG_PAINT {
				header: SCITER_X_MSG_CODE::SXM_PAINT.into(),
				element: ptr::null_mut(),
				isFore: true as BOOL,
				targetType: SCITER_PAINT_TARGET_TYPE::SPT_DEFAULT,
				context: ptr::null_mut(),
				callback: None,
			};
			(_API.SciterProcX)(wnd, &msg.header as *const _)
		},

		Message::Paint(paint) => {
			let msg = SCITER_X_MSG_PAINT {
				header: SCITER_X_MSG_CODE::SXM_PAINT.into(),
				element: paint.element,
				isFore: paint.is_foreground as BOOL,
				targetType: SCITER_PAINT_TARGET_TYPE::SPT_DEFAULT,
				context: std::ptr::null_mut(),
				callback: None,
			};
			(_API.SciterProcX)(wnd, &msg.header as *const _)
		},

		#[cfg(windows)]
		Message::RenderToDxgiSurface(paint) => {
			let layer = paint.layer.unwrap_or(PaintLayer {
				element: std::ptr::null_mut(),
				is_foreground: false,
			});

			let msg = SCITER_X_MSG_PAINT {
				header: SCITER_X_MSG_CODE::SXM_PAINT.into(),
				element: layer.element,
				isFore: layer.is_foreground as BOOL,
				targetType: SCITER_PAINT_TARGET_TYPE::SPT_SURFACE,
				context: paint.surface,
				callback: None,
			};
			(_API.SciterProcX)(wnd, &msg.header as *const _)
		},

		Message::RenderTo(paint) => {

			struct Callback {
				callback: Box<dyn Fn(&RECT, &[u8])>,
			}

			extern "system" fn inner(rgba: LPCBYTE, x: INT, y: INT, width: UINT, height: UINT, param: LPVOID)
			{
				assert!(!param.is_null());
				assert!(!rgba.is_null());
				if param.is_null() || rgba.is_null() { return; }

				let bitmap_area = RECT {
					left: x,
					top: y,
					right: x + width as INT,
					bottom: y + height as INT,
				};

				let bitmap_size = width * height * 4;
				let bitmap_data = unsafe { std::slice::from_raw_parts(rgba, bitmap_size as usize) };

				let param = param as *const Callback;
				let wrapper = unsafe { &*param };
				(wrapper.callback)(&bitmap_area, bitmap_data);
			}

			let wrapper = Callback {
				callback: paint.callback,
			};
			let param = &wrapper as *const _ as LPVOID;

			let layer = paint.layer.unwrap_or(PaintLayer {
				element: std::ptr::null_mut(),
				is_foreground: false,
			});

			let msg = SCITER_X_MSG_PAINT {
				header: SCITER_X_MSG_CODE::SXM_PAINT.into(),
				element: layer.element,
				isFore: layer.is_foreground as BOOL,
				targetType: SCITER_PAINT_TARGET_TYPE::SPT_RECEIVER,
				context: param,
				callback: Some(inner),
			};
			(_API.SciterProcX)(wnd, &msg.header as *const _)
		},

	};

	ok != 0
}
