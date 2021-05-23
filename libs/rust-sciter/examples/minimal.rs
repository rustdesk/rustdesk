//! Minimalistic Sciter sample.

// Specify the Windows subsystem to eliminate console window.
// Requires Rust 1.18.
#![windows_subsystem="windows"]

extern crate sciter;

fn main() {
	// Step 1: Include the 'minimal.html' file as a byte array.
	// Hint: Take a look into 'minimal.html' which contains some tiscript code.
	let html = include_bytes!("minimal.htm");

	// Step 2: Enable the features we need in our tiscript code.
	sciter::set_options(sciter::RuntimeOptions::ScriptFeatures(
		sciter::SCRIPT_RUNTIME_FEATURES::ALLOW_SYSINFO as u8		// Enables `Sciter.machineName()`
		| sciter::SCRIPT_RUNTIME_FEATURES::ALLOW_FILE_IO as u8	// Enables opening file dialog (`view.selectFile()`)
		)).unwrap();

	// Enable debug mode for all windows, so that we can inspect them via Inspector.
	sciter::set_options(sciter::RuntimeOptions::DebugMode(true)).unwrap();

	// Step 3: Create a new main sciter window of type `sciter::Window`.
	// Hint: The sciter Window wrapper (src/window.rs) contains more
	// interesting functions to open or attach to another existing window.
	let mut frame = sciter::Window::new();

	if cfg!(target_os="macos") {
		// a temporary workaround for OSX, see
		// https://sciter.com/forums/topic/global-sciter_set_debug_mode-does-not-work-in-osx/
		frame.set_options(sciter::window::Options::DebugMode(true)).unwrap();
	}

	// Step 4: Load HTML byte array from memory to `sciter::Window`.
	// Hint: second parameter is an optional uri, it can be `None` in simple cases,
	// but it is useful for debugging purposes (check the Inspector tool from the Sciter SDK).
	// Also you can use a `load_file` method, but it requires an absolute path
	// of the main document to resolve HTML resources properly.
	frame.load_html(html, Some("example://minimal.htm"));

	// Step 5: Show window and run the main app message loop until window been closed.
	frame.run_app();
}
