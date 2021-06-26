// This component uses Sciter Engine,
// copyright Terra Informatica Software, Inc.
// (http://terrainformatica.com/).

/*!
# Rust bindings library for Sciter engine.

[Sciter](http://sciter.com) is an embeddable [multiplatform](https://sciter.com/sciter/crossplatform/) HTML/CSS/script engine
with GPU accelerated rendering designed to render modern desktop application UI.
It's a compact, single dll/dylib/so file (4-8 mb) engine without any additional dependencies.

Check the [screenshot gallery](https://github.com/oskca/sciter#sciter-desktop-ui-examples)
of the desktop UI examples.

Sciter supports all standard elements defined in HTML5 specification
[with some additions](https://sciter.com/developers/for-web-programmers/).
CSS is extended to better support the Desktop UI development,
e.g. flow and flex units, vertical and horizontal alignment, OS theming.

[Sciter SDK](https://sciter.com/download/) comes with a demo "browser" with builtin DOM inspector,
script debugger and documentation viewer:

![Sciter tools](https://sciter.com/images/sciter-tools.png)

Check <https://sciter.com> website and its [documentation resources](https://sciter.com/developers/)
for engine principles, architecture and more.


## Brief look:

Here is a minimal sciter app:

```no_run
extern crate sciter;

fn main() {
    let mut frame = sciter::Window::new();
    frame.load_file("minimal.htm");
    frame.run_app();
}
```

It looks similar like this:

![Minimal sciter sample](https://i.imgur.com/ojcM5JJ.png)

Check [rust-sciter/examples](https://github.com/sciter-sdk/rust-sciter/tree/master/examples)
folder for more complex usage and module-level sections for the guides about:

* [Window](window/index.html) creation.
* [Behaviors](dom/event/index.html) and event handling.
* [DOM](dom/index.html) access methods.
* Sciter [Value](value/index.html) interface.

*/

#![doc(html_logo_url = "https://sciter.com/screenshots/slide-sciter-osx.png",
       html_favicon_url = "https://sciter.com/wp-content/themes/sciter/!images/favicon.ico")]

// documentation test:
// #![warn(missing_docs)]


/* Clippy lints */

#![allow(clippy::needless_return, clippy::let_and_return)] // past habits
#![allow(clippy::redundant_field_names)] // since Rust 1.17 and less readable
#![allow(clippy::unreadable_literal)] // C++ SDK constants
// #![allow(clippy::cast_ptr_alignment)] // 0.0.195 only


/* Macros */

#[cfg(target_os = "macos")]
#[macro_use] extern crate objc;
#[macro_use] extern crate lazy_static;


#[macro_use] pub mod macros;

mod capi;

#[doc(hidden)]
pub use capi::scdom::{HELEMENT};
pub use capi::scdef::{GFX_LAYER, SCRIPT_RUNTIME_FEATURES};

/* Rust interface */
mod platform;
mod eventhandler;

pub mod dom;
pub mod graphics;
pub mod host;
pub mod om;
pub mod request;
pub mod types;
pub mod utf;
pub mod value;
pub mod video;
pub mod window;
pub mod windowless;

pub use dom::Element;
pub use dom::event::EventHandler;
pub use host::{Archive, Host, HostHandler};
pub use value::{Value, FromValue};
pub use window::Window;


/// Builder pattern for window creation. See [`window::Builder`](window/struct.Builder.html) documentation.
///
/// For example,
///
/// ```rust,no_run
/// let mut frame = sciter::WindowBuilder::main_window()
///   .with_size((800,600))
///   .glassy()
///   .fixed()
///   .create();
/// ```
pub type WindowBuilder = window::Builder;


/* Loader */
pub use capi::scapi::{ISciterAPI};
use capi::scgraphics::SciterGraphicsAPI;
use capi::screquest::SciterRequestAPI;

#[cfg(all(windows, not(feature = "dynamic")))]
mod ext {
	#[link(name = "sciter.static")]
	extern "system" { pub fn SciterAPI() -> *const ::capi::scapi::ISciterAPI;	}
}

#[cfg(all(windows, feature = "dynamic"))]
mod ext {
	// Note:
	// Sciter 4.x shipped with universal "sciter.dll" library for different builds:
	// bin/32, bin/64, bin/skia32, bin/skia64
	// However it is quite inconvenient now (e.g. we can not put x64 and x86 builds in %PATH%)
	//
	#![allow(non_snake_case, non_camel_case_types)]
	use capi::scapi::{ISciterAPI};
	use capi::sctypes::{LPCSTR, LPCVOID, BOOL};

  type ApiType = *const ISciterAPI;
	type FuncType = extern "system" fn () -> *const ISciterAPI;

  pub static mut CUSTOM_DLL_PATH: Option<String> = None;

	extern "system"
	{
		fn LoadLibraryA(lpFileName: LPCSTR) -> LPCVOID;
    fn FreeLibrary(dll: LPCVOID) -> BOOL;
		fn GetProcAddress(hModule: LPCVOID, lpProcName: LPCSTR) -> LPCVOID;
	}

  pub fn try_load_library(permanent: bool) -> ::std::result::Result<ApiType, String> {
    use std::ffi::CString;
    use std::path::Path;

    fn try_load(path: &Path) -> Option<LPCVOID> {
      let path = CString::new(format!("{}", path.display())).expect("invalid library path");
      let dll = unsafe { LoadLibraryA(path.as_ptr()) };
      if !dll.is_null() {
        Some(dll)
      } else {
        None
      }
    }

    fn in_global() -> Option<LPCVOID> {
      // modern dll name
      let mut dll = unsafe { LoadLibraryA(b"sciter.dll\0".as_ptr() as LPCSTR) };
      if dll.is_null() {
        // try to load with old names
        let alternate = if cfg!(target_arch = "x86_64") { b"sciter64.dll\0" } else { b"sciter32.dll\0" };
        dll = unsafe { LoadLibraryA(alternate.as_ptr() as LPCSTR) };
      }
      if !dll.is_null() {
        Some(dll)
      } else {
        None
      }
    }

    // try specified path first (and only if present)
    // and several paths to lookup then
    let dll = if let Some(path) = unsafe { CUSTOM_DLL_PATH.as_ref() } {
      try_load(Path::new(path))
    } else {
      in_global()
    };

    if let Some(dll) = dll {
      // get the "SciterAPI" exported symbol
      let sym = unsafe { GetProcAddress(dll, b"SciterAPI\0".as_ptr() as LPCSTR) };
      if sym.is_null() {
        return Err("\"SciterAPI\" function was expected in the loaded library.".to_owned());
      }

      if !permanent {
        unsafe { FreeLibrary(dll) };
        return Ok(0 as ApiType);
      }

      let get_api: FuncType = unsafe { std::mem::transmute(sym) };
      return Ok(get_api());
    }
    let sdkbin = if cfg!(target_arch = "x86_64") { "bin/64" } else { "bin/32" };
    let msg = format!("Please verify that Sciter SDK is installed and its binaries (from SDK/{}) are available in PATH.", sdkbin);
    Err(format!("error: '{}' was not found neither in PATH nor near the current executable.\n  {}", "sciter.dll", msg))
  }

	pub unsafe fn SciterAPI() -> *const ISciterAPI {
    match try_load_library(true) {
      Ok(api) => api,
      Err(error) => panic!(error),
    }
	}
}

#[cfg(all(feature = "dynamic", unix))]
mod ext {
  #![allow(non_snake_case, non_camel_case_types)]
  extern crate libc;

  pub static mut CUSTOM_DLL_PATH: Option<String> = None;

  #[cfg(target_os = "linux")]
  const DLL_NAMES: &'static [&'static str] = &[ "libsciter-gtk.so" ];

  #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
  const DLL_NAMES: &'static [&'static str] = &[ "sciter-osx-64.dylib" ];

  use capi::scapi::ISciterAPI;
  use capi::sctypes::{LPVOID, LPCSTR};

  type FuncType = extern "system" fn () -> *const ISciterAPI;
  type ApiType = *const ISciterAPI;


  pub fn try_load_library(permanent: bool) -> ::std::result::Result<ApiType, String> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    use std::path::{Path, PathBuf};


    // Try to load the library from a specified absolute path.
    fn try_load(path: &Path) -> Option<LPVOID> {
      let bytes = path.as_os_str().as_bytes();
      if let Ok(cstr) = CString::new(bytes) {
        let dll = unsafe { libc::dlopen(cstr.as_ptr(), libc::RTLD_LOCAL | libc::RTLD_LAZY) };
        if !dll.is_null() {
          return Some(dll)
        }
      }
      None
    }

    // Try to find a library (by one of its names) in a specified path.
    fn try_load_from(dir: Option<&Path>) -> Option<LPVOID> {

      let dll = DLL_NAMES.iter()
        .map(|name| {
          let mut path = dir.map(Path::to_owned).unwrap_or(PathBuf::new());
          path.push(name);
          path
        })
        .map(|path| try_load(&path))
        .filter(|dll| dll.is_some())
        .nth(0)
        .map(|o| o.unwrap());

      if dll.is_some() {
        return dll;
      }

      None
    }

    // Try to load from the current directory.
    fn in_current_dir() -> Option<LPVOID> {
      if let Ok(dir) = ::std::env::current_exe() {
        if let Some(dir) = dir.parent() {
          let dll = try_load_from(Some(dir));
          if dll.is_some() {
            return dll;
          }

          if cfg!(target_os = "macos") {
            // "(bundle folder)/Contents/Frameworks/"
            let mut path = dir.to_owned();
            path.push("../Frameworks/sciter-osx-64.dylib");
            return try_load(&path);
          }
        }
      }
      None
    }

    // Try to load indirectly via `dlopen("dll.so")`.
    fn in_global() -> Option<LPVOID> {
      try_load_from(None)
    }

    // Try to find in $PATH.
    fn in_paths() -> Option<LPVOID> {
      use std::env;
      if let Some(paths) = env::var_os("PATH") {
        for path in env::split_paths(&paths) {
          if let Some(dll) = try_load_from(Some(&path)) {
            return Some(dll);
          }
        }
      }
      None
    }

    // try specified path first (and only if present)
    // and several paths to lookup then
    let dll = if let Some(path) = unsafe { CUSTOM_DLL_PATH.as_ref() } {
      try_load(Path::new(path))
    } else {
      in_current_dir().or_else(in_paths).or_else(in_global)
    };

    if let Some(dll) = dll {
      // get the "SciterAPI" exported symbol
      let sym = unsafe { libc::dlsym(dll, b"SciterAPI\0".as_ptr() as LPCSTR) };
      if sym.is_null() {
        return Err("\"SciterAPI\" function was expected in the loaded library.".to_owned());
      }

      if !permanent {
        unsafe { libc::dlclose(dll) };
        return Ok(0 as ApiType);
      }

      let get_api: FuncType = unsafe { std::mem::transmute(sym) };
      return Ok(get_api());
    }

    let sdkbin = if cfg!(target_os = "macos") { "bin.osx" } else { "bin.lnx" };
    let msg = format!("Please verify that Sciter SDK is installed and its binaries (from {}) are available in PATH.", sdkbin);
    Err(format!("error: '{}' was not found neither in PATH nor near the current executable.\n  {}", DLL_NAMES[0], msg))
  }

  pub fn SciterAPI() -> *const ISciterAPI {
    match try_load_library(true) {
      Ok(api) => api,
      Err(error) => panic!("{}", error),
    }
  }
}


#[cfg(all(target_os = "linux", not(feature = "dynamic")))]
mod ext {
	// Note:
	// Since 4.1.4 library name has been changed to "libsciter-gtk" (without 32/64 suffix).
	// Since 3.3.1.6 library name was changed to "libsciter".
	// However CC requires `-l sciter` form.
	#[link(name = "sciter-gtk")]
	extern "system" { pub fn SciterAPI() -> *const ::capi::scapi::ISciterAPI;	}
}

#[cfg(all(target_os = "macos", target_arch = "x86_64", not(feature = "dynamic")))]
mod ext {
	#[link(name = "sciter-osx-64", kind = "dylib")]
	extern "system" { pub fn SciterAPI() -> *const ::capi::scapi::ISciterAPI;	}
}

/// Getting ISciterAPI reference, can be used for manual API calling.
#[doc(hidden)]
#[allow(non_snake_case)]
pub fn SciterAPI<'a>() -> &'a ISciterAPI {
	let ap = unsafe {
		if cfg!(feature="extension") {
			// TODO: it's not good to raise a panic inside `lazy_static!`,
      // because it wents into recursive panicing.
      //
			// Somehow, `cargo test --all` tests all the features,
      // also sometimes it comes even without `cfg!(test)`.
      // Well, the culprit is "examples/extensions" which uses the "extension" feature,
      // but how on earth it builds without `cfg(test)`?
      //
			if cfg!(test) {
				&*ext::SciterAPI()
			} else {
				EXT_API
					//.or_else(|| Some(&*ext::SciterAPI()))
					.expect("Sciter API is not available yet, call `sciter::set_api()` first.")
			}
		} else {
			&*ext::SciterAPI()
		}
	};

	let abi_version = ap.version;

	if cfg!(feature = "windowless") {
		assert!(abi_version >= 0x0001_0001, "Incompatible Sciter build and \"windowless\" feature");
	}
	if cfg!(not(feature = "windowless")) {
		assert!(abi_version < 0x0001_0000, "Incompatible Sciter build and \"windowless\" feature");
	}

	return ap;
}

/// Getting ISciterAPI reference, can be used for manual API calling.
///
/// Bypasses ABI compatability checks.
#[doc(hidden)]
#[allow(non_snake_case)]
pub fn SciterAPI_unchecked<'a>() -> &'a ISciterAPI {
	let ap = unsafe {
		if cfg!(feature="extension") {
			EXT_API.expect("Sciter API is not available yet, call `sciter::set_api()` first.")
		} else {
			&*ext::SciterAPI()
		}
	};

	return ap;
}


lazy_static! {
	static ref _API: &'static ISciterAPI = SciterAPI();
	static ref _GAPI: &'static SciterGraphicsAPI = {
		if version_num() < 0x0401_0A00 {
			panic!("Graphics API is incompatible since 4.1.10 (your version is {})", version());
		}
		unsafe { &*(SciterAPI().GetSciterGraphicsAPI)() }
	};
	static ref _RAPI: &'static SciterRequestAPI = unsafe { &*(SciterAPI().GetSciterRequestAPI)() };
}

/// Set a custom path to the Sciter dynamic library.
///
/// Note: Must be called first before any other function.
///
/// Returns error if the specified library can not be loaded.
///
/// # Example
///
/// ```rust
/// if sciter::set_library("~/lib/sciter/bin.gtk/x64/libsciter-gtk.so").is_ok() {
///   println!("loaded Sciter version {}", sciter::version());
/// }
/// ```
pub fn set_library(custom_path: &str) -> ::std::result::Result<(), String> {
  #[cfg(not(feature = "dynamic"))]
  fn set_impl(_: &str) -> ::std::result::Result<(), String> {
    Err("Don't use `sciter::set_library()` in static builds.\n  Build with the feature \"dynamic\" instead.".to_owned())
  }

  #[cfg(feature = "dynamic")]
  fn set_impl(path: &str) -> ::std::result::Result<(), String> {
    unsafe {
      ext::CUSTOM_DLL_PATH = Some(path.to_owned());
    }
    ext::try_load_library(false).map(|_| ())
  }

  set_impl(custom_path)
}

static mut EXT_API: Option<&'static ISciterAPI> = None;

/// Set the Sciter API coming from `SciterLibraryInit`.
///
/// Note: Must be called first before any other function.
pub fn set_host_api(api: &'static ISciterAPI) {
	if cfg!(feature="extension") {
		unsafe {
			EXT_API.replace(api);
		}
	}
}

/// Sciter engine version number (e.g. `0x03030200`).
///
/// Note: does not return the `build` part because it doesn't fit in `0..255` byte range.
/// Use [`sciter::version()`](fn.version.html) instead which returns the complete version string.
pub fn version_num() -> u32 {
	use types::BOOL;
	let v1 = (_API.SciterVersion)(true as BOOL);
	let v2 = (_API.SciterVersion)(false as BOOL);
	let (major, minor, revision, _build) = (v1 >> 16 & 0xFF, v1 & 0xFF, v2 >> 16 & 0xFF, v2 & 0xFF);
	let num = (major << 24) | (minor << 16) | (revision << 8);
	// let num = ((v1 >> 16) << 24) | ((v1 & 0xFFFF) << 16) | ((v2 >> 16) << 8) | (v2 & 0xFFFF);
	return num;
}

/// Sciter engine version string (e.g. "`3.3.2.0`").
pub fn version() -> String {
	use types::BOOL;
	let v1 = (_API.SciterVersion)(true as BOOL);
	let v2 = (_API.SciterVersion)(false as BOOL);
	let num = [v1 >> 16, v1 & 0xFFFF, v2 >> 16, v2 & 0xFFFF];
	let version = format!("{}.{}.{}.{}", num[0], num[1], num[2], num[3]);
	return version;
}

/// Sciter API version.
///
/// Returns:
///
///	* `0x0000_0001` for regular builds, `0x0001_0001` for windowless builds.
/// * `0x0000_0002` since 4.4.2.14 (a breaking change in assets with [SOM builds](https://sciter.com/native-code-exposure-to-script/))
/// * `0x0000_0003` since 4.4.2.16
/// * `0x0000_0004` since 4.4.2.17 (a breaking change in SOM passport)
/// * `0x0000_0005` since 4.4.3.20 (a breaking change in `INITIALIZATION_PARAMS`, SOM in event handlers fix)
/// * `0x0000_0006` since 4.4.3.24 (TIScript native API is gone, use SOM instead)
///
/// Since 4.4.0.3.
pub fn api_version() -> u32 {
	_API.version
}

/// Returns true for windowless builds.
pub fn is_windowless() -> bool {
	api_version() >= 0x0001_0001
}

/// Various global Sciter engine options.
///
/// Used by [`sciter::set_options()`](fn.set_options.html).
///
/// See also [per-window options](window/enum.Options.html).
#[derive(Copy, Clone)]
pub enum RuntimeOptions<'a> {

  /// global; value: the full path to the Sciter dynamic library (dll/dylib/so),
  /// must be called before any other Sciter function.
  LibraryPath(&'a str),
  /// global; value: [`GFX_LAYER`](enum.GFX_LAYER.html), must be called before any window creation.
  GfxLayer(GFX_LAYER),
  /// global; value: `true` - the engine will use a "unisex" theme that is common for all platforms.
  /// That UX theme is not using OS primitives for rendering input elements.
  /// Use it if you want exactly the same (modulo fonts) look-n-feel on all platforms.
  UxTheming(bool),
  /// global or per-window; enables Sciter Inspector for all windows, must be called before any window creation.
  DebugMode(bool),
  /// global or per-window; value: combination of [`SCRIPT_RUNTIME_FEATURES`](enum.SCRIPT_RUNTIME_FEATURES.html) flags.
  ///
  /// Note that these features have been disabled by default
  /// since [4.2.5.0](https://rawgit.com/c-smile/sciter-sdk/7036a9c7912ac30d9f369d9abb87b278d2d54c6d/logfile.htm).
  ScriptFeatures(u8),
	/// global; value: milliseconds, connection timeout of http client.
	ConnectionTimeout(u32),
	/// global; value: `0` - drop connection, `1` - use builtin dialog, `2` - accept connection silently.
	OnHttpsError(u8),
	// global; value: json with GPU black list, see the `gpu-blacklist.json` resource.
	// Not used in Sciter 4, in fact: https://sciter.com/forums/topic/how-to-use-the-gpu-blacklist/#post-59338
	// GpuBlacklist(&'a str),
	/// global; value: script source to be loaded into each view before any other script execution.
	InitScript(&'a str),
	/// global; value - max request length in megabytes (1024*1024 bytes), since 4.3.0.15.
	MaxHttpDataLength(usize),
}

/// Set various global Sciter engine options, see the [`RuntimeOptions`](enum.RuntimeOptions.html).
pub fn set_options(options: RuntimeOptions) -> std::result::Result<(), ()> {
	use RuntimeOptions::*;
	use capi::scdef::SCITER_RT_OPTIONS::*;
	let (option, value) = match options {
		ConnectionTimeout(ms) => (SCITER_CONNECTION_TIMEOUT, ms as usize),
		OnHttpsError(behavior) => (SCITER_HTTPS_ERROR, behavior as usize),
		// GpuBlacklist(json) => (SCITER_SET_GPU_BLACKLIST, json.as_bytes().as_ptr() as usize),
		InitScript(script) => (SCITER_SET_INIT_SCRIPT, script.as_bytes().as_ptr() as usize),
		ScriptFeatures(mask) => (SCITER_SET_SCRIPT_RUNTIME_FEATURES, mask as usize),
		GfxLayer(backend) => (SCITER_SET_GFX_LAYER, backend as usize),
		DebugMode(enable) => (SCITER_SET_DEBUG_MODE, enable as usize),
		UxTheming(enable) => (SCITER_SET_UX_THEMING, enable as usize),
		MaxHttpDataLength(value) => (SCITER_SET_MAX_HTTP_DATA_LENGTH, value),
    LibraryPath(path) => {
      return set_library(path).map_err(|_|());
    }
	};
	let ok = (_API.SciterSetOption)(std::ptr::null_mut(), option, value);
	if ok != 0 {
		Ok(())
	} else {
		Err(())
	}
}
