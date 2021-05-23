/*! High-level native window wrapper.

To create an instance of Sciter you will need either to create a new Sciter window
or to attach (mix-in) the Sciter engine to an existing window.

The handle of the Sciter engine is defined as `HWINDOW` type which is:

* `HWND` handle on Microsoft Windows.
* `NSView*` – a pointer to [`NSView`](https://developer.apple.com/library/mac/documentation/Cocoa/Reference/ApplicationKit/Classes/NSView_Class/) instance that is a contentView of Sciter window on OS X.
* `GtkWidget*` – a pointer to [`GtkWidget`](https://developer.gnome.org/gtk3/stable/GtkWidget.html) instance
that is a root widget of Sciter window on Linux/GTK.

## Creation of a new window:

```no_run
extern crate sciter;

fn main() {
  let mut frame = sciter::Window::new();
  frame.load_file("minimal.htm");
  frame.run_app();
}
```

Also you can register a [host](../host/trait.HostHandler.html) and a [DOM](../dom/event/index.html) event handlers.

.
*/
use ::{_API};
use capi::sctypes::*;

use platform::{BaseWindow, OsWindow};
use host::{Host, HostHandler};
use dom::event::{EventHandler};

use std::rc::Rc;


/// `SCITER_CREATE_WINDOW_FLAGS` alias.
pub type Flags = SCITER_CREATE_WINDOW_FLAGS;

pub use capi::scdef::{SCITER_CREATE_WINDOW_FLAGS};


/// Per-window Sciter engine options.
///
/// Used by [`Window::set_options()`](struct.Window.html#method.set_options).
///
/// See also [global options](../enum.RuntimeOptions.html).
#[derive(Copy, Clone)]
pub enum Options {
	/// Enable smooth scrolling, enabled by default.
	SmoothScroll(bool),

	/// Font rendering, value: `0` - system default, `1` - no smoothing, `2` - standard smoothing, `3` - ClearType.
	FontSmoothing(u8),

	/// Windows Aero support, value: `false` - normal drawing, `true` - window has transparent background after calls
	/// [`DwmExtendFrameIntoClientArea()`](https://msdn.microsoft.com/en-us/library/windows/desktop/aa969512(v=vs.85).aspx)
	/// or [`DwmEnableBlurBehindWindow()`](https://msdn.microsoft.com/en-us/library/windows/desktop/aa969508(v=vs.85).aspx).
	TransparentWindow(bool),

	/// Transparent windows support. When enabled, window uses per pixel alpha
  /// (e.g. [`WS_EX_LAYERED`](https://msdn.microsoft.com/en-us/library/ms997507.aspx?f=255&MSPPError=-2147217396) window).
	AlphaWindow(bool),

  /// global or per-window; enables Sciter Inspector for this window, must be called before loading HTML.
  DebugMode(bool),

  /// global or per-window; value: combination of [`SCRIPT_RUNTIME_FEATURES`](../enum.SCRIPT_RUNTIME_FEATURES.html) flags.
	ScriptFeatures(u8),

	/// Window is main, will destroy all other dependent windows on close, since 4.3.0.12
	MainWindow(bool),
}


/// Sciter window.
pub struct Window
{
	base: OsWindow,
	host: Rc<Host>,
}

// `Window::new()` is rather expensive operation to make it default.
#[allow(clippy::new_without_default)]
impl Window {

	/// Create a new main window.
	// #[cfg(not(feature = "windowless"))]
	#[cfg_attr(feature = "windowless", deprecated = "Sciter.Lite doesn't have OS windows in windowless mode.")]
	pub fn new() -> Window {
		Builder::main_window().create()
	}

	/// Create a new window with the specified position, flags and an optional parent window.
	#[cfg_attr(feature = "windowless", deprecated = "Sciter.Lite doesn't have OS windows in windowless mode.")]
	pub fn create(rect: RECT, flags: Flags, parent: Option<HWINDOW>) -> Window {
		if cfg!(feature = "windowless")
		{
			panic!("Sciter.Lite doesn't have OS windows in windowless mode!");
		}

		let mut base = OsWindow::new();
		let hwnd = base.create(rect, flags as UINT, parent.unwrap_or(0 as HWINDOW));
		assert!(!hwnd.is_null());

		let wnd = Window { base: base, host: Rc::new(Host::attach(hwnd))};
		return wnd;
	}

	/// Attach Sciter to an existing native window.
	pub fn attach(hwnd: HWINDOW) -> Window {
		// suppress warnings about unused method
		let _ = &OsWindow::new;

		assert!(!hwnd.is_null());
		Window { base: OsWindow::from(hwnd), host: Rc::new(Host::attach(hwnd)) }
	}

	/// Obtain a reference to `Host` which allows you to control Sciter engine.
	pub fn get_host(&self) -> Rc<Host> {
		self.host.clone()
	}

	/// Set [callback](../host/trait.HostHandler.html) for Sciter engine events.
	pub fn sciter_handler<Callback: HostHandler + Sized>(&mut self, handler: Callback) {
		self.host.setup_callback(handler);
	}

	/// Attach [`dom::EventHandler`](../dom/event/trait.EventHandler.html) to the Sciter window.
	///
	/// You should install Window event handler only once - it will survive all document reloads.
	/// Also it can be registered on an empty window before the document is loaded.
	pub fn event_handler<Handler: EventHandler>(&mut self, handler: Handler) {
		self.host.attach_handler(handler);
	}

  /// Register an archive produced by `packfolder` tool.
	///
	/// The resources can be accessed via the `this://app/` URL.
	///
	/// See documentation of the [`Archive`](../host/struct.Archive.html).
	///
  pub fn archive_handler(&mut self, resource: &[u8]) -> Result<(), ()> {
    self.host.register_archive(resource)
  }

	/// Register a native event handler for the specified behavior name.
	///
	/// Behavior is a named event handler which is created for a particular DOM element.
	/// In Sciter’s sense, it is a function that is called for different UI events on the DOM element.
	/// Essentially it is an analog of the [WindowProc](https://en.wikipedia.org/wiki/WindowProc) in Windows.
	///
	/// In HTML, there is a `behavior` CSS property that defines the name of a native module
	/// that is responsible for the initialization and event handling on the element.
	/// For example, by defining `div { behavior:button; }` you are asking all `<div>` elements in your markup
	/// to behave as buttons: generate [`BUTTON_CLICK`](../dom/event/enum.BEHAVIOR_EVENTS.html#variant.BUTTON_CLICK)
	/// DOM events when the user clicks on that element, and be focusable.
	///
	/// When the engine discovers an element having `behavior: xyz;` defined in its style,
	/// it sends the [`SC_ATTACH_BEHAVIOR`](../host/trait.HostHandler.html#method.on_attach_behavior) host notification
	/// with the name `"xyz"` and an element handle to the application.
	/// You can consume the notification and respond to it yourself,
	/// or the default handler walks through the list of registered behavior factories
	/// and creates an instance of the corresponding [`dom::EventHandler`](../dom/event/trait.EventHandler.html).
	///
	/// ## Example:
	///
	/// ```rust,no_run
	/// struct Button;
	///
	/// impl sciter::EventHandler for Button {}
	///
	/// let mut frame = sciter::Window::new();
	///
	/// // register a factory method that creates a new event handler
	/// // for each element that has "custom-button" behavior:
	/// frame.register_behavior("custom-button", || { Box::new(Button) });
	/// ```
	///
	/// And in HTML it can be used as:
	///
	/// ```html
	/// <button style="behavior: custom-button">Rusty button</button>
	/// ```
	pub fn register_behavior<Factory>(&mut self, name: &str, factory: Factory)
	where
		Factory: Fn() -> Box<dyn EventHandler> + 'static
	{
		self.host.register_behavior(name, factory);
	}

	/// Load an HTML document from file.
	///
	/// The specified `uri` should be either an absolute file path
	/// or a full URL to the HTML to load.
	///
	/// Supported URL schemes are: `http://`, `file://`, `this://app/` (when used with [`archive_handler`](#archive_handler)).
	pub fn load_file(&mut self, uri: &str) -> bool {
		self.host.load_file(uri)
	}

	/// Load an HTML document from memory.
	pub fn load_html(&mut self, html: &[u8], uri: Option<&str>) -> bool {
		self.host.load_html(html, uri)
	}

	/// Get native window handle.
	pub fn get_hwnd(&self) -> HWINDOW {
		self.base.get_hwnd()
	}

	/// Minimize or hide window.
	pub fn collapse(&self, hide: bool) {
		self.base.collapse(hide)
	}

	/// Show or maximize window.
	pub fn expand(&self, maximize: bool) {
		self.base.expand(maximize)
	}

	/// Close window.
	pub fn dismiss(&self) {
		self.base.dismiss()
	}

	/// Set title of native window.
	pub fn set_title(&mut self, title: &str) {
		self.base.set_title(title)
	}

	/// Get native window title.
	pub fn get_title(&self) -> String {
		self.base.get_title()
	}

	/// Set various Sciter engine options, see the [`Options`](enum.Options.html).
	pub fn set_options(&self, options: Options) -> Result<(), ()> {
		use capi::scdef::SCITER_RT_OPTIONS::*;
		use self::Options::*;
		let (option, value) = match options {
			SmoothScroll(enable) => (SCITER_SMOOTH_SCROLL, enable as usize),
			FontSmoothing(technology) => (SCITER_FONT_SMOOTHING, technology as usize),
			TransparentWindow(enable) => (SCITER_TRANSPARENT_WINDOW, enable as usize),
			AlphaWindow(enable) => (SCITER_ALPHA_WINDOW, enable as usize),
			MainWindow(enable) => (SCITER_SET_MAIN_WINDOW, enable as usize),
      DebugMode(enable) => (SCITER_SET_DEBUG_MODE, enable as usize),
			ScriptFeatures(mask) => (SCITER_SET_SCRIPT_RUNTIME_FEATURES, mask as usize),
		};
		let ok = (_API.SciterSetOption)(self.get_hwnd(), option, value);
		if ok != 0 {
			Ok(())
		} else {
			Err(())
		}
	}

	/// Show window and run the main app message loop until window been closed.
	pub fn run_app(self) {
		self.base.expand(false);
		self.base.run_app();
	}

	/// Run the main app message loop with already configured window.
	pub fn run_loop(&self) {
		self.base.run_app();
	}

	/// Post app quit message.
	pub fn quit_app(&self) {
		self.base.quit_app()
	}
}


/// Generic rectangle struct.
/// NOTE that this is different from the [`RECT`](../types/struct.RECT.html) type as it specifies width and height.
#[derive(Clone, Copy)]
pub struct Rectangle {
	pub x: i32,
	pub y: i32,
	pub width: i32,
	pub height: i32
}


/// Builder pattern for window creation.
///
/// For example,
///
/// ```rust,no_run
/// let mut frame = sciter::window::Builder::main_window()
///   .with_size((800,600))
///   .resizeable()
///   .glassy()
///   .create();
/// ```
#[derive(Default)]
pub struct Builder {
	flags: Flags,
	rect: RECT,
	parent: Option<HWINDOW>,
}

// Note: https://rust-lang-nursery.github.io/api-guidelines/type-safety.html#non-consuming-builders-preferred
impl Builder {

	/// Main application window (resizeable with min/max buttons and title).
	/// Will terminate the app on close.
	pub fn main_window() -> Self {
		Builder::main()
			.resizeable()
			.closeable()
			.with_title()
	}

	/// Popup window (with min/max buttons and title).
	pub fn popup_window() -> Self {
		Builder::popup()
			.closeable()
			.with_title()
	}

	/// Child window style. if this flag is set all other flags are ignored.
	pub fn child_window() -> Self {
		Builder::with_flags(SCITER_CREATE_WINDOW_FLAGS::SW_CHILD)
	}

	/// If you want to start from scratch.
	pub fn none() -> Self {
		Builder::with_flags(SCITER_CREATE_WINDOW_FLAGS::SW_CHILD)	// 0
	}

	/// Start with some flags.
	pub fn with_flags(flags: Flags) -> Self {
		let mut me = Builder::default();
		me.flags = flags;
		me
	}

	/// Main window style (appears in taskbar).
	/// Will terminate the app on close.
	pub fn main() -> Self {
		Builder::with_flags(SCITER_CREATE_WINDOW_FLAGS::SW_MAIN)
	}

	/// Popup style, window is created as topmost.
	pub fn popup() -> Self {
		Builder::with_flags(SCITER_CREATE_WINDOW_FLAGS::SW_POPUP)
	}

	/// Tool window style (with thin titlebar).
	pub fn tool() -> Self {
		Builder::with_flags(SCITER_CREATE_WINDOW_FLAGS::SW_TOOL)
	}

	/// Specify the parent window (e.g. for child creation).
	pub fn with_parent(mut self, parent: HWINDOW) -> Self {
		self.parent = Some(parent);
		self
	}

	/// Specify the precise window size in `(width, height)` form.
	pub fn with_size(mut self, size: (i32, i32)) -> Self {
		self.rect.right = self.rect.left + size.0;
		self.rect.bottom = self.rect.top + size.1;
		self
	}

	/// Specify the precise window position in `(X, Y)` form.
	pub fn with_pos(mut self, position: (i32, i32)) -> Self {
		let size = self.rect.size();
		self.rect.left = position.0;
		self.rect.top = position.1;
		self.rect.right = position.0 + size.cx;
		self.rect.bottom = position.1 + size.cy;
		self
	}

	/// Specify the exact window rectangle in `(X, Y, W, H)` form.
	pub fn with_rect(mut self, rect: Rectangle) -> Self {
		self.rect = RECT {
			left: rect.x,
			top: rect.y,
			right: rect.x + rect.width,
			bottom: rect.y + rect.height,
		};
		self
	}

	/// Top level window, has titlebar.
	pub fn with_title(self) -> Self {
		self.or(SCITER_CREATE_WINDOW_FLAGS::SW_TITLEBAR)
	}

	/// Can be resized.
	pub fn resizeable(self) -> Self {
		self.or(SCITER_CREATE_WINDOW_FLAGS::SW_RESIZEABLE)
	}

	/// Can not be resized.
	pub fn fixed(self) -> Self {
		self.and(SCITER_CREATE_WINDOW_FLAGS::SW_RESIZEABLE)
	}

	/// Has minimize / maximize buttons.
	pub fn closeable(self) -> Self {
		self.or(SCITER_CREATE_WINDOW_FLAGS::SW_CONTROLS)
	}

	/// Glassy window ("Acrylic" on Windows and "Vibrant" on macOS).
	pub fn glassy(self) -> Self {
		self.or(SCITER_CREATE_WINDOW_FLAGS::SW_GLASSY)
	}

	/// Transparent window.
	pub fn alpha(self) -> Self {
		self.or(SCITER_CREATE_WINDOW_FLAGS::SW_ALPHA)
	}

	/// Can be debugged with Inspector.
	pub fn debug(self) -> Self {
		self.or(SCITER_CREATE_WINDOW_FLAGS::SW_ENABLE_DEBUG)
	}

	fn or(mut self, flag: Flags) -> Self {
		self.flags = self.flags | flag;
		self
	}

	fn and(mut self, flag: Flags) -> Self {
		let masked = self.flags as u32 & !(flag as u32);
		self.flags = unsafe { ::std::mem::transmute(masked) };
		self
	}

	/// Consume the builder and call [`Window::create()`](struct.Window.html#method.create) with built parameters.
	#[cfg_attr(feature = "windowless", deprecated = "Sciter.Lite doesn't have OS windows in windowless mode.")]
	pub fn create(self) -> Window {
		Window::create(self.rect, self.flags, self.parent)
	}
}
