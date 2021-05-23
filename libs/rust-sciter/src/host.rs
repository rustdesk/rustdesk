//! Sciter host application helpers.

use ::{_API};
use capi::scdef::SCITER_RT_OPTIONS;
use capi::sctypes::*;
use capi::screquest::HREQUEST;
use capi::schandler::NativeHandler;
use dom::event::EventHandler;
use eventhandler::*;
use value::{Value};

pub use capi::scdef::{LOAD_RESULT, OUTPUT_SUBSYTEMS, OUTPUT_SEVERITY};
pub use capi::scdef::{SCN_LOAD_DATA, SCN_DATA_LOADED, SCN_ATTACH_BEHAVIOR, SCN_INVALIDATE_RECT};


/// A specialized `Result` type for Sciter host operations.
pub type Result<T> = ::std::result::Result<T, ()>;

macro_rules! ok_or {
	($ok:ident) => {
		if $ok != 0 {
			Ok(())
		} else {
			Err(())
		}
	};

	($ok:ident, $rv:expr) => {
		if $ok != 0 {
			Ok($rv)
		} else {
			Err(())
		}
	};

	($ok:ident, $rv:expr, $err:expr) => {
		if $ok != 0 {
			Ok($rv)
		} else {
			Err($err)
		}
	};
}


/** Sciter notification handler for [`Window.sciter_handler()`](../window/struct.Window.html#method.sciter_handler).

## Resource handling and custom resource loader

HTML loaded into Sciter may contain external resources: CSS (Cascading Style Sheets),
images, fonts, cursors and scripts.
To get any of such resources Sciter will first send a `on_data_load(SCN_LOAD_DATA)` notification
to your application using the callback handler registered with `sciter::Window.sciter_handler()` function.

Your application can provide your own data for such resources
(for example, from the resource section, database or other storage of your choice)
or delegate the resource loading to the built-in HTTP client or file loader, or discard the loading at all.

Note: This handler should be registered before any
[`load_html()`](struct.Host.html#method.load_html) or
[`load_file()`](struct.Host.html#method.load_file) calls
in order to send notifications while loading.

*/
#[allow(unused_variables)]
pub trait HostHandler {

	/// Notifies that Sciter is about to download the referred resource.
	///
	/// You can load or overload data immediately by calling `self.data_ready()` with parameters provided by `SCN_LOAD_DATA`,
	/// or save them (including `request_id`) for later usage and answer here with `LOAD_RESULT::LOAD_DELAYED` code.
	///
	/// Also you can discard the request (data will not be loaded at the document)
	/// or take care about this request completely by yourself (via the [request API](../request/index.html)).
	fn on_data_load(&mut self, pnm: &mut SCN_LOAD_DATA) -> Option<LOAD_RESULT> { return None; }

	/// This notification indicates that external data (for example, image) download process completed.
	fn on_data_loaded(&mut self, pnm: &SCN_DATA_LOADED) { }

	/// This notification is sent on parsing the document and while processing elements
	/// having non empty `behavior: ` style attribute value.
	fn on_attach_behavior(&mut self, pnm: &mut SCN_ATTACH_BEHAVIOR) -> bool { return false; }

	/// This notification is sent when instance of the engine is destroyed.
	fn on_engine_destroyed(&mut self) { }

	/// This notification is sent when the engine encounters critical rendering error: e.g. DirectX gfx driver error.
  /// Most probably bad gfx drivers.
	fn on_graphics_critical_failure(&mut self) { }

	/// This notification is sent when the engine needs some area to be redrawn.
	fn on_invalidate(&mut self, pnm: &SCN_INVALIDATE_RECT) {}

	/// This output function will be used for reporting problems found while loading html and css documents.
	fn on_debug_output(&mut self, subsystem: OUTPUT_SUBSYTEMS, severity: OUTPUT_SEVERITY, message: &str) {
		if !message.is_empty() {
			if severity == OUTPUT_SEVERITY::INFO {
				// e.g. `stdout.println` in TIScript
				println!("{:?}:{:?}: {}", severity, subsystem, message);
			} else {
				// e.g. `stderr.println` or CSS/script errors and warnings.
				eprintln!("{:?}:{:?}: {}", severity, subsystem, message);
			}
		}
	}

	/// This function is used as response to [`on_data_load`](#method.on_data_load) request.
	///
	/// Parameters here must be taken from [`SCN_LOAD_DATA`](struct.SCN_LOAD_DATA.html) structure. You can store them for later usage,
	/// but you must answer as [`LOAD_DELAYED`](enum.LOAD_RESULT.html#variant.LOAD_DELAYED) code and provide an `request_id` here.
	fn data_ready(&self, hwnd: HWINDOW, uri: &str, data: &[u8], request_id: Option<HREQUEST>) {
		let s = s2w!(uri);
		match request_id {
			Some(req) => {
				(_API.SciterDataReadyAsync)(hwnd, s.as_ptr(), data.as_ptr(), data.len() as UINT, req)
			},
			None => {
				(_API.SciterDataReady)(hwnd, s.as_ptr(), data.as_ptr(), data.len() as UINT)
			},
		};
	}

  /// This function is used as a response to the [`on_attach_behavior`](#method.on_attach_behavior) request
  /// to attach a newly created behavior `handler` to the requested element.
	fn attach_behavior<Handler: EventHandler>(&self, pnm: &mut SCN_ATTACH_BEHAVIOR, handler: Handler) {
		// make native handler
		let boxed = Box::new(handler);
		let ptr = Box::into_raw(boxed);	// dropped in `_event_handler_proc`
		pnm.elementProc = ::eventhandler::_event_handler_proc::<Handler>;
		pnm.elementTag = ptr as LPVOID;
	}
}


/// Default `HostHandler` implementation
#[derive(Default)]
struct DefaultHandler;

/// Default `HostHandler` implementation
impl HostHandler for DefaultHandler {

}

use std::rc::Rc;
use std::cell::RefCell;

type BehaviorList = Vec<(String, Box<dyn Fn() -> Box<dyn EventHandler>>)>;
type SharedBehaviorList = Rc<RefCell<BehaviorList>>;
type SharedArchive = Rc<RefCell<Option<Archive>>>;

#[repr(C)]
struct HostCallback<Callback> {
	sig: u32,
	behaviors: SharedBehaviorList,
	handler: Callback,
  archive: SharedArchive,
}

/// Sciter host runtime support.
pub struct Host {
	hwnd: HWINDOW,
	behaviors: SharedBehaviorList,
	handler: RefCell<NativeHandler>,
  archive: SharedArchive,
}

impl Host {

	/// Attach Sciter host to existing window.
	///
	/// Usually Sciter window created by a [`sciter::Window::create()`](../window/struct.Window.html#method.create),
	/// but you can attach Sciter to an existing native window.
	/// In this case you need to mix-in window events processing with `SciterProcND` (Windows only).
	/// Sciter engine will be initialized either on `WM_CREATE` or `WM_INITDIALOG` response
	/// or by calling `SciterCreateOnDirectXWindow` (again, Windows only).
	pub fn attach(hwnd: HWINDOW) -> Host {
		// Host with default debug handler installed
		let host = Host {
      hwnd,
      behaviors: Default::default(),
      handler: Default::default(),
      archive: Default::default(),
    };
		host.setup_callback(DefaultHandler::default());
		return host;
	}

	/// Attach Sciter host to an existing window with the given Host handler.
	pub fn attach_with<Handler: HostHandler>(hwnd: HWINDOW, handler: Handler) -> Host {
	  let host = Host {
      hwnd,
      behaviors: Default::default(),
      handler: Default::default(),
      archive: Default::default(),
    };
	  host.setup_callback(handler);
	  return host;
	}

	/// Attach [`dom::EventHandler`](../dom/event/trait.EventHandler.html) to the Sciter window.
	pub fn event_handler<Handler: EventHandler>(&self, handler: Handler) {
		self.attach_handler(handler)
	}

	/// Attach [`dom::EventHandler`](../dom/event/trait.EventHandler.html) to the Sciter window.
	#[doc(hidden)]
	pub fn attach_handler<Handler: EventHandler>(&self, handler: Handler) {
		let hwnd = self.get_hwnd();
		let boxed = Box::new( WindowHandler { hwnd, handler } );
		let ptr = Box::into_raw(boxed);	// dropped in `_event_handler_window_proc`
		// eprintln!("{}: {:?}", std::any::type_name::<Handler>(), ptr);

		let func = _event_handler_window_proc::<Handler>;
		let flags = ::dom::event::default_events();
		(_API.SciterWindowAttachEventHandler)(hwnd, func, ptr as LPVOID, flags as UINT);
	}

	/// Set callback for Sciter engine events.
	pub(crate) fn setup_callback<Callback: HostHandler>(&self, handler: Callback) {

		let payload: HostCallback<Callback> = HostCallback {
			sig: 17,
			behaviors: Rc::clone(&self.behaviors),
      archive: Rc::clone(&self.archive),
			handler: handler,
		};

		*self.handler.borrow_mut() = NativeHandler::from(payload);
		let ptr = self.handler.borrow().as_mut_ptr();

		(_API.SciterSetCallback)(self.get_hwnd(), _on_handle_notification::<Callback>, ptr);
		(_API.SciterSetupDebugOutput)(0 as HWINDOW, ptr, _on_debug_notification::<Callback>);
	}

	/// Register a native event handler for the specified behavior name.
	///
	/// See the [`Window::register_behavior`](../window/struct.Window.html#method.register_behavior) for an example.
	pub fn register_behavior<Factory>(&self, name: &str, factory: Factory)
	where
		Factory: Fn() -> Box<dyn EventHandler> + 'static
	{
		let make: Box<dyn Fn() -> Box<dyn EventHandler>> = Box::new(factory);
		let pair = (name.to_owned(), make);
		self.behaviors.borrow_mut().push(pair);
	}

  /// Register an archive produced by `packfolder`.
  ///
  /// See documentation of the [`Archive`](struct.Archive.html).
  pub fn register_archive(&self, resource: &[u8]) -> Result<()> {
    *self.archive.borrow_mut() = Some(Archive::open(resource)?);
    Ok(())
  }

	/// Set debug mode for this window.
	pub fn enable_debug(&self, enable: bool) {
		(_API.SciterSetOption)(self.hwnd, SCITER_RT_OPTIONS::SCITER_SET_DEBUG_MODE, enable as UINT_PTR);
	}

	/// Get native window handle.
	pub fn get_hwnd(&self) -> HWINDOW {
		self.hwnd
	}

	/// Get window root DOM element.
	pub fn get_root(&self) -> Option<::dom::Element> {
		::dom::Element::from_window(self.hwnd).ok()
	}

	/// Load an HTML document from file.
	pub fn load_file(&self, uri: &str) -> bool {
		// TODO: it should be `Result<()>` instead `bool`
		let s = s2w!(uri);
		(_API.SciterLoadFile)(self.hwnd, s.as_ptr()) != 0
	}

	/// Load an HTML document from memory.
	pub fn load_html(&self, html: &[u8], uri: Option<&str>) -> bool {
		match uri {
			Some(uri) => {
				let s = s2w!(uri);
				(_API.SciterLoadHtml)(self.hwnd, html.as_ptr(), html.len() as UINT, s.as_ptr()) != 0
			},
			None => {
				(_API.SciterLoadHtml)(self.hwnd, html.as_ptr(), html.len() as UINT, 0 as LPCWSTR) != 0
			}
		}
	}

	/// This function is used as response to [`HostHandler::on_data_load`](trait.HostHandler.html#method.on_data_load) request.
	pub fn data_ready(&self, uri: &str, data: &[u8]) {
		let s = s2w!(uri);
		(_API.SciterDataReady)(self.hwnd, s.as_ptr(), data.as_ptr(), data.len() as UINT);
	}

	/// Use this function outside of [`HostHandler::on_data_load`](trait.HostHandler.html#method.on_data_load) request.
	///
	/// It can be used for two purposes:
	///
	/// 1. Asynchronious resource loading in respect of [`on_data_load`](trait.HostHandler.html#method.on_data_load)
	/// requests (you must use `request_id` in this case).
	/// 2. Refresh of an already loaded resource (for example, dynamic image updates).
	pub fn data_ready_async(&self, uri: &str, data: &[u8], request_id: Option<HREQUEST>) {
		let s = s2w!(uri);
		let req = request_id.unwrap_or(::std::ptr::null_mut());
		(_API.SciterDataReadyAsync)(self.hwnd, s.as_ptr(), data.as_ptr(), data.len() as UINT, req);
	}

	/// Evaluate the given script in context of the current document.
	///
	/// This function returns `Result<Value,Value>` with script function result value or with Sciter script error.
	pub fn eval_script(&self, script: &str) -> ::std::result::Result<Value, Value> {
		let (s,n) = s2wn!(script);
		let mut rv = Value::new();
		let ok = (_API.SciterEval)(self.hwnd, s.as_ptr(), n, rv.as_ptr());
		ok_or!(ok, rv, rv)
	}

	/// Call a script function defined in the global namespace.
	///
	/// This function returns `Result<Value,Value>` with script function result value or with Sciter script error.
	///
	/// You can use the [`&make_args!(args...)`](../macro.make_args.html) macro which helps you
	/// to construct script arguments from Rust types.
	pub fn call_function(&self, name: &str, args: &[Value]) -> ::std::result::Result<Value, Value> {
		let mut rv = Value::new();
		let s = s2u!(name);
		let argv = Value::pack_args(args);
		let ok = (_API.SciterCall)(self.hwnd, s.as_ptr(), argv.len() as UINT, argv.as_ptr(), rv.as_ptr());
		ok_or!(ok, rv, rv)
	}

	/// Set home url for Sciter resources.
	///
	/// If you set it like `set_home_url("https://sciter.com/modules/")` then
	///
	///  `<script src="sciter:lib/root-extender.tis">` will load
	///  root-extender.tis from
	///
	/// `https://sciter.com/modules/lib/root-extender.tis`.
	pub fn set_home_url(&self, url: &str) -> Result<()> {
		let s = s2w!(url);
		let ok = (_API.SciterSetHomeURL)(self.hwnd, s.as_ptr());
		ok_or!(ok)
	}

	/// Set media type of this Sciter instance.
	///
	/// For example, media type can be "handheld", "projection", "screen", "screen-hires", etc.
	/// By default, Sciter window has the `"screen"` media type.
	///
	/// Media type name is used while loading and parsing style sheets in the engine,
	/// so you should call this function **before** loading document in it.
	///
	pub fn set_media_type(&self, media_type: &str) -> Result<()> {
		let s = s2w!(media_type);
		let ok = (_API.SciterSetMediaType)(self.hwnd, s.as_ptr());
		ok_or!(ok)
	}

	/// Set media variables (dictionary) for this Sciter instance.
	///
	/// By default Sciter window has `"screen:true"` and `"desktop:true"/"handheld:true"` media variables.
	///
	/// Media variables can be changed in runtime. This will cause styles of the document to be reset.
	///
	/// ## Example
	///
	/// ```rust,no_run
	/// # use sciter::vmap;
	/// # let mut host = sciter::Host::attach(0 as sciter::types::HWINDOW);
	/// host.set_media_vars( &vmap! {
	///   "screen" => true,
	///   "handheld" => true,
	/// }).unwrap();
	/// ```
	pub fn set_media_vars(&self, media: &Value) -> Result<()> {
		let ok = (_API.SciterSetMediaVars)(self.hwnd, media.as_cptr());
		ok_or!(ok)
	}

	/// Set or append the [master](https://sciter.com/css-extensions-in-h-smile-engine-part-i-style-sets/)
	/// style sheet styles (**globally**, for all windows).
	pub fn set_master_css(&self, css: &str, append: bool) -> Result<()> {
		let s = s2u!(css);
		let b = s.as_bytes();
		let n = b.len() as UINT;
		let ok = if append {
			(_API.SciterAppendMasterCSS)(b.as_ptr(), n)
		} else {
			(_API.SciterSetMasterCSS)(b.as_ptr(), n)
		};
		ok_or!(ok)
	}

	/// Set (reset) style sheet of the **current** document.
	///
	/// Will reset styles for all elements according to given CSS.
	pub fn set_window_css(&self, css: &str, base_url: &str, media_type: &str) -> Result<()> {
		let s = s2u!(css);
		let url = s2w!(base_url);
		let media = s2w!(media_type);
		let b = s.as_bytes();
		let n = b.len() as UINT;
		let ok = (_API.SciterSetCSS)(self.hwnd, b.as_ptr(), n, url.as_ptr(), media.as_ptr());
		ok_or!(ok)
	}

}


// Sciter notification handler.
// This comes as free function due to https://github.com/rust-lang/rust/issues/32364
extern "system" fn _on_handle_notification<T: HostHandler>(pnm: *mut ::capi::scdef::SCITER_CALLBACK_NOTIFICATION, param: LPVOID) -> UINT
{
	use capi::scdef::{SCITER_NOTIFICATION, SCITER_CALLBACK_NOTIFICATION};

	// reconstruct pointer to Handler
	let callback = NativeHandler::get_data::<HostCallback<T>>(&param);
	let me: &mut T = &mut callback.handler;

	// process notification
	let nm: &mut SCITER_CALLBACK_NOTIFICATION = unsafe { &mut *pnm };
	let code: SCITER_NOTIFICATION = unsafe { ::std::mem::transmute(nm.code) };


	let result: UINT = match code {
		SCITER_NOTIFICATION::SC_LOAD_DATA => {
			let scnm = pnm as *mut SCN_LOAD_DATA;
      let scnm = unsafe { &mut *scnm };
			let mut re = me.on_data_load(scnm);
      if re.is_none() {
        if let Some(archive) = callback.archive.borrow().as_ref() {
          let uri = w2s!(scnm.uri);
          if uri.starts_with("this://app/") {
            if let Some(data) = archive.get(&uri) {
              me.data_ready(scnm.hwnd, &uri, data, None);
            } else {
              eprintln!("[sciter] error: can't load {:?}", uri);
            }
          }
          re = Some(LOAD_RESULT::LOAD_DEFAULT);
        }
      }
			re.unwrap_or(LOAD_RESULT::LOAD_DEFAULT) as UINT
		},

		SCITER_NOTIFICATION::SC_DATA_LOADED => {
			let scnm = pnm as *mut SCN_DATA_LOADED;
			me.on_data_loaded(unsafe { &mut *scnm } );
			0 as UINT
		},

		SCITER_NOTIFICATION::SC_ATTACH_BEHAVIOR => {
			let scnm = pnm as *mut SCN_ATTACH_BEHAVIOR;
			let scnm = unsafe { &mut *scnm };
			let mut re = me.on_attach_behavior(scnm);
			if !re {
				let name = u2s!(scnm.name);
				let behavior = callback.behaviors
					.borrow()
					.iter()
					.find(|x| x.0 == name)
					.map(|x| x.1());

				if let Some(behavior) = behavior {
					let boxed = Box::new( BoxedHandler { handler: behavior } );
					let ptr = Box::into_raw(boxed);	// dropped in `_event_handler_behavior_proc`

					scnm.elementProc = ::eventhandler::_event_handler_behavior_proc;
					scnm.elementTag = ptr as LPVOID;
					re = true;
				}
			}
			re as UINT
		},

		SCITER_NOTIFICATION::SC_ENGINE_DESTROYED => {
			me.on_engine_destroyed();
			0 as UINT
		},

		SCITER_NOTIFICATION::SC_GRAPHICS_CRITICAL_FAILURE => {
			me.on_graphics_critical_failure();
			0 as UINT
		},

		SCITER_NOTIFICATION::SC_INVALIDATE_RECT => {
			let scnm = pnm as *const SCN_INVALIDATE_RECT;
			me.on_invalidate(unsafe { &*scnm });
			0 as UINT
		}

		_ => 0,
	};

	return result;
}

// Sciter debug output handler.
extern "system" fn _on_debug_notification<T: HostHandler>(param: LPVOID, subsystem: OUTPUT_SUBSYTEMS, severity: OUTPUT_SEVERITY,
	text: LPCWSTR, _text_length: UINT)
{
	// reconstruct pointer to Handler
	// let me = unsafe { &mut *(param as *mut HostCallback<T>) };
	let me = NativeHandler::get_data::<HostCallback<T>>(&param);
	let message = ::utf::w2s(text).replace("\r", "\n");
	me.handler.on_debug_output(subsystem, severity, message.trim_end());
}


/// Sciter compressed archive.
///
/// An archive is produced by `packfolder` tool (from SDK) that creates a single blob with compressed resources.
/// It allows to use the same resource pack uniformly across different platforms.
///
/// For example, app resource files (HTML/CSS/scripts) can be stored in an `assets` folder
/// that can be packed into a single archive by calling `packfolder.exe assets target/assets.rc -binary`.
/// And later it can be accessed via the Archive API explicitly:
///
/// ```rust,ignore
/// let archived = include_bytes!("target/assets.rc");
/// let assets = sciter::host::Archive::open(archived).expect("Unable to load archive.");
///
/// // access `assets/index.htm`
/// let html_data = assets.get("index.htm").unwrap();
/// ```
///
/// or implicitly via the `this://app/` URL after registering the archive via
/// [`Window::archive_handler`](../window/struct.Window.html#method.archive_handler):
///
/// ```rust,ignore
///
/// let archived = include_bytes!("target/assets.rc");
/// let mut frame = sciter::Window::new();
/// frame.archive_handler(archived).expect("Unable to load archive");
/// frame.load("this://app/index.htm");
/// ```
pub struct Archive(HSARCHIVE);

/// Close the archive.
impl Drop for Archive {
  fn drop(&mut self) {
    (_API.SciterCloseArchive)(self.0);
  }
}

impl Archive {
  /// Open an archive blob.
  pub fn open(archived: &[u8]) -> Result<Self> {
    let p = (_API.SciterOpenArchive)(archived.as_ptr(), archived.len() as u32);
    if !p.is_null() {
      Ok(Archive(p))
    } else {
      Err(())
    }
  }

  /// Get an archive item.
  ///
  /// Given a path, returns a reference to the contents of an archived item.
  pub fn get(&self, path: &str) -> Option<&[u8]> {
    // skip initial part of the path
    let skip = if path.starts_with("this://app/") {
      "this://app/".len()
    } else if path.starts_with("//") {
      "//".len()
    } else {
      0
    };

    let wname = s2w!(path);
    let name = &wname[skip..];

    let mut pb = ::std::ptr::null();
    let mut cb = 0;
    let ok = (_API.SciterGetArchiveItem)(self.0, name.as_ptr(), &mut pb, &mut cb);
    if ok != 0 && !pb.is_null() {
      let data = unsafe { ::std::slice::from_raw_parts(pb, cb as usize) };
      Some(data)
    } else {
      None
    }
  }
}
