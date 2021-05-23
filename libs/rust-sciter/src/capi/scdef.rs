//! Common Sciter declarations.

#![allow(non_camel_case_types, non_snake_case)]
#![allow(dead_code)]

use capi::sctypes::*;
use capi::scvalue::{VALUE};
use capi::screquest::{HREQUEST};
use capi::scdom::{HELEMENT};
use capi::scapi::ISciterAPI;

//////////////////////////////////////////////////////////////////////////////////
pub enum ID2D1RenderTarget {}
pub enum ID2D1Factory {}
pub enum IDWriteFactory {}
pub enum IDXGISwapChain {}
pub enum IDXGISurface {}


#[repr(C)]
#[derive(Debug, PartialOrd, PartialEq)]
/// `HostHandler::on_data_load()` result code.
///
/// This notification gives application a chance to override built-in loader and
/// implement loading of resources in its own way (for example, images can be loaded from
/// a database or other resource).
pub enum LOAD_RESULT {
	/// Do the default loading if data is not set.
  LOAD_DEFAULT,
  /// Discard the request completely (data will not be loaded at the document).
  LOAD_DISCARD,
  /// Data will be delivered later by the host application.
  LOAD_DELAYED,
  /// You return this result to indicate that your (the host) application took or
  /// will take care about `HREQUEST` in your code completely.
  LOAD_MYSELF,
}

/// Script runtime options.
#[repr(C)]
#[derive(Debug)]
#[allow(missing_docs)]
pub enum SCRIPT_RUNTIME_FEATURES
{
	ALLOW_FILE_IO = 0x1,
	ALLOW_SOCKET_IO = 0x2,
	ALLOW_EVAL = 0x4,
	ALLOW_SYSINFO = 0x8,
}

/// Sciter graphics rendering backend.
#[repr(C)]
#[derive(Debug)]
#[derive(Copy, Clone)]
#[allow(missing_docs)]
pub enum GFX_LAYER
{
	/// An auto-selected backend.
	AUTO = 0xFFFF,

	/// Depends on OS: GDI, Cairo or CoreGraphics.
	CPU = 1,

	/// A software rasterizer for Direct2D (Windows only).
	#[cfg(windows)]
	WARP = 2,

	/// A hardware Direct2D mode (Windows only).
	#[cfg(windows)]
	D2D = 3,

	/// Skia backend with CPU rasterization mode.
	SKIA_CPU = 4,

	/// Skia backend with OpenGL rendering.
	SKIA_OPENGL = 5,
}

#[repr(C)]
#[derive(Debug, PartialOrd, PartialEq)]
/// Various Sciter engine options (global or per-window).
pub enum SCITER_RT_OPTIONS
{
	/// value:TRUE - enable, value:FALSE - disable, enabled by default.
  SCITER_SMOOTH_SCROLL = 1,
  /// global; value: milliseconds, connection timeout of http client.
  SCITER_CONNECTION_TIMEOUT = 2,
  /// global; value: 0 - drop connection, 1 - use builtin dialog, 2 - accept connection silently.
  SCITER_HTTPS_ERROR = 3,
  /// value: 0 - system default, 1 - no smoothing, 2 - std smoothing, 3 - clear type.
  SCITER_FONT_SMOOTHING = 4,
	/// Windows Aero support, value:
	/// 0 - normal drawing,
	/// 1 - window has transparent background after calls `DwmExtendFrameIntoClientArea()` or `DwmEnableBlurBehindWindow()`.
  SCITER_TRANSPARENT_WINDOW = 6,
  /// global; value = LPCBYTE, json - GPU black list, see: gpu-blacklist.json resource.
  /// Note: is not used since Sciter 4.
  #[deprecated(since="4.0.1", note="This option isn't working since Sciter 4.0.1.1.")]
  SCITER_SET_GPU_BLACKLIST  = 7,
  /// global or per-window; value - combination of [SCRIPT_RUNTIME_FEATURES](enum.SCRIPT_RUNTIME_FEATURES.html) flags.
  SCITER_SET_SCRIPT_RUNTIME_FEATURES = 8,
  /// global (must be called before any window creation); value - [GFX_LAYER](enum.GFX_LAYER.html).
  SCITER_SET_GFX_LAYER = 9,
  /// global or per-window; value - TRUE/FALSE
  SCITER_SET_DEBUG_MODE = 10,
  /// global; value - BOOL, TRUE - the engine will use "unisex" theme that is common for all platforms.
  /// That UX theme is not using OS primitives for rendering input elements.
  /// Use it if you want exactly the same (modulo fonts) look-n-feel on all platforms.
  SCITER_SET_UX_THEMING = 11,
  /// value - TRUE/FALSE - window uses per pixel alpha (e.g. `WS_EX_LAYERED`/`UpdateLayeredWindow()` window).
  SCITER_ALPHA_WINDOW  = 12,
  /// global; value: UTF-8 encoded script source to be loaded into each view before any other script execution.
	SCITER_SET_INIT_SCRIPT = 13,
	/// per-window; value - TRUE/FALSE - window is main, will destroy all other dependent windows on close.
	SCITER_SET_MAIN_WINDOW = 14,
	/// global; value - max request length in megabytes (1024*1024 bytes).
	SCITER_SET_MAX_HTTP_DATA_LENGTH = 15,
}

/// Window flags
#[repr(C)]
pub enum SCITER_CREATE_WINDOW_FLAGS {
	/// child window only, if this flag is set all other flags ignored.
  SW_CHILD      = 1,
  /// toplevel window, has titlebar.
  SW_TITLEBAR   = 1 << 1,
  /// has resizeable frame.
  SW_RESIZEABLE = 1 << 2,
  /// is tool window.
  SW_TOOL       = 1 << 3,
  /// has minimize / maximize buttons.
  SW_CONTROLS   = 1 << 4,
  /// glassy window - "Acrylic" on Windows and "Vibrant" on macOS.
  SW_GLASSY     = 1 << 5,
  /// transparent window (e.g. `WS_EX_LAYERED` on Windows, macOS is supported too).
  SW_ALPHA      = 1 << 6,
  /// main window of the app, will terminate the app on close.
  SW_MAIN       = 1 << 7,
  /// the window is created as topmost window.
  SW_POPUP      = 1 << 8,
  /// make this window inspector ready.
  SW_ENABLE_DEBUG = 1 << 9,
  /// it has its own script VM.
  SW_OWNS_VM      = 1 << 10,
}

impl Default for SCITER_CREATE_WINDOW_FLAGS {
	fn default() -> Self {
		SCITER_CREATE_WINDOW_FLAGS::SW_CHILD
	}
}

/// Flags can be OR'ed as `SW_MAIN|SW_ALPHA`.
impl ::std::ops::BitOr for SCITER_CREATE_WINDOW_FLAGS {
  type Output = SCITER_CREATE_WINDOW_FLAGS;
  fn bitor(self, rhs: Self::Output) -> Self::Output {
    let rn = (self as UINT) | (rhs as UINT);
    unsafe { ::std::mem::transmute(rn) }
  }
}

#[repr(C)]
#[derive(Debug, PartialOrd, PartialEq)]
pub enum SCITER_NOTIFICATION {
  SC_LOAD_DATA = 1,
  SC_DATA_LOADED = 2,
  SC_ATTACH_BEHAVIOR = 4,
  SC_ENGINE_DESTROYED = 5,
  SC_POSTED_NOTIFICATION = 6,
	SC_GRAPHICS_CRITICAL_FAILURE = 7,
	SC_KEYBOARD_REQUEST = 8,
	SC_INVALIDATE_RECT = 9,
}

#[repr(C)]
#[derive(Debug, PartialEq)]
/// The type of a loaded resource.
pub enum RESOURCE_TYPE {
	/// HTML document.
	HTML = 0,
	/// Image.
	IMAGE = 1,
	/// CSS.
	STYLE = 2,
	/// Mouse cursor image.
	CURSOR = 3,
	/// TIScript document.
	SCRIPT = 4,
	/// Any data.
	RAW = 5,
	/// Font.
	FONT,
	/// Sound (wav bytes).
	SOUND,
}

/// The type of a loaded resource.
pub type SCITER_RESOURCE_TYPE = RESOURCE_TYPE;


#[repr(C)]
#[derive(Debug)]
/// Notifies that Sciter is about to download a referred resource.
pub struct SCN_LOAD_DATA
{
	/// `SC_LOAD_DATA` here.
  pub code: UINT,
  /// `HWINDOW` of the window this callback was attached to.
  pub hwnd: HWINDOW,

  /// [in] Zero terminated string, fully qualified uri, for example, "http://server/folder/file.ext".
  pub uri: LPCWSTR,

  /// [in,out] pointer to loaded data to return. If data exists in the cache then this field contain pointer to it.
  pub outData: LPCBYTE,
  /// [in,out] loaded data size to return.
  pub outDataSize: UINT,

  /// [in] resource type category
  pub dataType: RESOURCE_TYPE,

  /// [in] request handle that can be used with Sciter request API.
  pub request_id: HREQUEST,

  /// [in] destination element for request.
  pub principal: HELEMENT,
  /// [in] source element for request.
  pub initiator: HELEMENT,
}

#[repr(C)]
#[derive(Debug)]
/// This notification indicates that external data (for example, image) download process has been completed.
pub struct SCN_DATA_LOADED
{
	/// `SC_DATA_LOADED` here.
  pub code: UINT,
  /// `HWINDOW` of the window this callback was attached to.
  pub hwnd: HWINDOW,
  /// [in] zero terminated string, fully qualified uri, for example, "http://server/folder/file.ext".
  pub uri: LPCWSTR,
  /// [in] pointer to loaded data.
  pub data: LPCBYTE,
  /// [in] loaded data size (in bytes).
  pub dataSize: UINT,
  /// [in] resource type category
  pub dataType: RESOURCE_TYPE,
  /// Download status code:
  ///
  /// * status = 0 and `dataSize == 0` - unknown error.
  /// * status = 100..505 - http response status, note: 200 - OK!
  /// * status > 12000 - wininet error code, see `ERROR_INTERNET_***` in wininet.h
  pub status: UINT,
}

#[repr(C)]
/// This notification is sent on parsing the document and while processing elements
/// having non empty `behavior: ` style attribute value.
pub struct SCN_ATTACH_BEHAVIOR
{
	/// `SC_ATTACH_BEHAVIOR` here.
  pub code: UINT,
  /// `HWINDOW` of the window this callback was attached to.
  pub hwnd: HWINDOW,

  /// [in] target DOM element handle
  pub element: HELEMENT,
  /// [in] zero terminated string, string appears as value of CSS `behavior: ` attribute.
  pub name: LPCSTR,
  /// [out] pointer to ElementEventProc function.
  pub elementProc: ElementEventProc,
  /// [out] tag value, passed as is into pointer ElementEventProc function.
  pub elementTag: LPVOID,
}

#[repr(C)]
/// This notification is issued when keyboard needs to be shown –
/// mobiles can show soft keyboard by handling it.
pub struct SCN_KEYBOARD_REQUEST
{
	/// `SC_KEYBOARD_REQUEST` here.
  pub code: UINT,
  /// `HWINDOW` of the window this callback was attached to.
	pub hwnd: HWINDOW,

	pub keyboard_mode: UINT,
}

#[repr(C)]
/// This notification is sent when a specific window area
/// needs to be redrawn.
pub struct SCN_INVALIDATE_RECT
{
	/// `SC_INVALIDATE_RECT` here.
  pub code: UINT,
  /// `HWINDOW` of the window this callback was attached to.
  pub hwnd: HWINDOW,

	/// Coordinates of the invalidated area.
	pub invalid_rect: RECT,
}

#[repr(C)]
pub struct SCITER_CALLBACK_NOTIFICATION
{
	pub code: UINT,
	pub hwnd: HWINDOW,
}
pub type LPSCITER_CALLBACK_NOTIFICATION = *mut SCITER_CALLBACK_NOTIFICATION;

pub type SciterHostCallback = extern "system" fn (pns: LPSCITER_CALLBACK_NOTIFICATION, callbackParam: LPVOID) -> UINT;

pub type SciterWindowDelegate = extern "system" fn (hwnd: HWINDOW, msg: UINT, wParam: WPARAM, lParam: LPARAM, pParam: LPVOID, handled: * mut BOOL) -> LRESULT;

pub type ElementEventProc = extern "system" fn (tag: LPVOID, he: HELEMENT, evtg: UINT, prms: LPVOID) -> BOOL;

#[repr(C)]
#[derive(Debug, PartialOrd, PartialEq)]
/// Debug output categories.
pub enum OUTPUT_SUBSYTEMS
{
	/// html parser & runtime
  DOM = 0,
  /// csss! parser & runtime
  CSSS,
  /// css parser
  CSS,
  /// TIS parser & runtime
  TIS,
}

#[repr(C)]
#[derive(Debug, PartialOrd, PartialEq)]
#[allow(missing_docs)]
/// Debug output severity.
pub enum OUTPUT_SEVERITY
{
  INFO,
  WARNING,
  ERROR,
}

pub type DEBUG_OUTPUT_PROC = extern "system" fn (param: LPVOID, subsystem: OUTPUT_SUBSYTEMS, severity: OUTPUT_SEVERITY, text: LPCWSTR, text_length: UINT);

pub type LPCWSTR_RECEIVER = extern "system" fn (szstr: LPCWSTR, str_length: UINT, param: LPVOID);
pub type LPCSTR_RECEIVER  = extern "system" fn (szstr: LPCSTR,  str_length: UINT, param: LPVOID);
pub type LPCBYTE_RECEIVER = extern "system" fn (szstr: LPCBYTE, str_length: UINT, param: LPVOID);

pub type ELEMENT_BITMAP_RECEIVER = extern "system" fn (rgba: LPCBYTE, x: INT, y: INT, width: UINT, height: UINT, param: LPVOID);

pub type KeyValueCallback = extern "system" fn (param: LPVOID, pkey: *const VALUE, pval: *const VALUE) -> BOOL;

/// Signature of Sciter extension library.
///
/// * `api` - Sciter API to be used inside the extension.
/// * `exported` - extension object, it can be [asset](https://sciter.com/developers/for-native-gui-programmers/sciter-object-model/),
/// function, or other `sciter::Value` supported type.
///
/// Return `true` if the `exported` object was initialized.
///
/// The extension should be placed in the same folder as "sciter.dll"
/// and export a `SciterLibraryInit` function:
///
/// ```rust,no_run
/// use sciter::types::{BOOL, VALUE};
/// use sciter::Value;
///
/// #[no_mangle]
/// pub extern "system"
/// fn SciterLibraryInit(api: &'static sciter::ISciterAPI, exported: &mut VALUE) -> BOOL
/// {
///   sciter::set_host_api(api);
///
///   unimplemented!("export some extension functions");
///
///   true as BOOL
/// }
/// ```
///
/// In script such extension library can be imported as:
///
/// ```javascript
/// const exported = include library "library-name";
/// ```
///
/// See the [blog post](https://sciter.com/include-library-name-native-extensions/) for more details.
///
pub type SciterLibraryInit = extern "system" fn (api: &'static ISciterAPI, exported: &mut VALUE) -> BOOL;
