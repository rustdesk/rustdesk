//! Sciter interop with native code and vice versa.

#![allow(unused_variables)]
#![allow(non_snake_case)]

#[macro_use]
extern crate sciter;

use sciter::{HELEMENT, Element, Value};

struct EventHandler {
	root: Option<Element>,
}

impl Drop for EventHandler {
	fn drop(&mut self) {
		println!("interop::EventHandler: Bye bye, HTML!");
	}
}

impl EventHandler {

	fn script_call_test(&self, args: &[Value], root: &Element) -> Option<Value> {

		println!("root: {:?}", root);
		// return None;

		println!("calling 'hello'");
		let answer = root.call_function("hello", &make_args!("hello, rust!"));
		println!(" answer {:?}", answer);

		println!("get and call 'hello'");
		let answer = root.eval_script(r"hello");
		if answer.is_err() {
			return None;
		}
		let obj = answer.unwrap();
		let answer = obj.call(None, &make_args!("argument"), None);
		println!(" answer is {:?}", answer);

		println!("eval 'hello'");
		let answer = root.eval_script(r#"hello("42");"#);
		println!(" answer is {:?}", answer);

		println!("calling 'raise_error'; the following exceptions are expected then:");
		let answer = root.call_function("raise_error", &make_args!(17, "42", false));
		println!(" answer is {:?}", answer);

		println!("calling inexisting function");
		let answer = root.call_function("raise_error2", &[]);
		println!(" answer is {:?}", answer);

		Some(Value::from(true))
	}

	fn NativeCall(&mut self, arg: String) -> Value {
		Value::from(format!("Rust window ({})", arg))
	}

	fn GetNativeApi(&mut self) -> Value {

		fn on_add(args: &[Value]) -> Value {
			let ints = args.iter().map(|x| x.to_int().unwrap());
			// let sum: i32 = ints.sum();	// error: issue #27739
			let sum: i32 = ints.sum();
			Value::from(sum)
		}

		fn on_sub(args: &[Value]) -> Value {
			if args.len() != 2 || args.iter().any(|x| !x.is_int()) {
				return Value::error("sub requires 2 integer arguments!");
			}
			let ints: Vec<_> = args.iter().map(|x| x.to_int().unwrap()).collect();
			let (a,b) = (ints[0], ints[1]);
			Value::from(a - b)
		}

		let on_mul = |args: &[Value]|  -> Value {
			let prod: i32 = args.iter().map(|x| x.to_int().unwrap()).product();
			Value::from(prod)
		};

		let mut api = Value::new();

		api.set_item("add", on_add);
		api.set_item("sub", on_sub);
		api.set_item("mul", on_mul);

		println!("returning {:?}", api);

		api
	}

	fn calc_sum(&mut self, a: i32, b: i32) -> i32 {
		a + b
	}

}


impl sciter::EventHandler for EventHandler {

	fn attached(&mut self, root: HELEMENT) {
		self.root = Some(Element::from(root));
	}

	dispatch_script_call! {

		fn NativeCall(String);

		fn GetNativeApi();

		fn calc_sum(i32, i32);
	}

	fn on_script_call(&mut self, root: HELEMENT, name: &str, argv: &[Value]) -> Option<Value> {

		let args = argv.iter().map(|x| format!("{:?}", &x)).collect::<Vec<String>>().join(", ");
		println!("script->native: {}({}), root {:?}", name, args, Element::from(root));

		let handled = self.dispatch_script_call(root, name, argv);
		if handled.is_some() {
			return handled;
		}

		if name == "ScriptCallTest" {
			return self.script_call_test(argv, &Element::from(root));
		}

		None
	}

}

fn check_options() {
	sciter::set_options(sciter::RuntimeOptions::ScriptFeatures(
		sciter::SCRIPT_RUNTIME_FEATURES::ALLOW_SYSINFO as u8		// Enables `Sciter.machineName()`
		| sciter::SCRIPT_RUNTIME_FEATURES::ALLOW_FILE_IO as u8	// Enables opening file dialog (`view.selectFile()`)
	)).ok();

	for arg in std::env::args() {
		if arg.starts_with("--sciter-gfx=") {
			use sciter::GFX_LAYER;
			let backend = match arg.split_at("--sciter-gfx=".len()).1.trim() {
				"auto" => GFX_LAYER::AUTO,
				"cpu" => GFX_LAYER::CPU,
				"skia" | "skia-cpu" => GFX_LAYER::SKIA_CPU,
				"skia-opengl" => GFX_LAYER::SKIA_OPENGL,

				#[cfg(windows)]
				"d2d" => GFX_LAYER::D2D,
				#[cfg(windows)]
				"warp" => GFX_LAYER::WARP,

				_ => GFX_LAYER::AUTO,
			};
			println!("setting {:?} backend", backend);
			let ok = sciter::set_options(sciter::RuntimeOptions::GfxLayer(backend));
			if let Err(e) = ok {
				println!("failed to set backend: {:?}", e);
			}

		} else if arg.starts_with("--ux-theme") {
			#[cfg(windows)]
			sciter::set_options(sciter::RuntimeOptions::UxTheming(true)).ok();
		}
	}
}

fn main() {
	// interop --sciter-gfx=cpu --ux-theme
	check_options();

	let html = include_bytes!("interop.htm");
	let handler = EventHandler { root: None };
	let mut frame = sciter::Window::new();
	frame.event_handler(handler);
	frame.load_html(html, Some("example://interop.htm"));
	frame.run_app();
}
