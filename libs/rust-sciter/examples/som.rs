// there are logs in console window
// #![windows_subsystem="windows"]
extern crate sciter;

use sciter::{HELEMENT, types::{BOOL, VALUE}};

#[derive(Default)]
pub struct Object {
	age: i32,
	name: String,
}

impl Object {
	pub fn print(&self) -> String {
		format!("name: {}, age: {}", self.name, self.age)
	}

	pub fn add_year(&mut self, v: i32) -> i32 {
		self.age += v;
		self.age
	}
}

// SOM Passport of the asset.
// TODO: should be auto-generated.
impl sciter::om::Passport for Object {
	fn get_passport(&self) -> &'static sciter::om::som_passport_t {
		use sciter::om::*;

		extern "C" fn on_print(thing: *mut som_asset_t, _argc: u32, _argv: *const VALUE, p_result: &mut VALUE) -> BOOL
		{
			let me = IAsset::<Object>::from_raw(&thing);
			let r = me.print();
			let r: sciter::Value = r.into();
			r.pack_to(p_result);
			return true as BOOL;
		}
		extern "C" fn on_add_year(thing: *mut som_asset_t, argc: u32, argv: *const VALUE, p_result: &mut VALUE) -> BOOL
		{
			let me = IAsset::<Object>::from_raw(&thing);

			let args = unsafe { sciter::Value::unpack_from(argv, argc) };
			let required = 1;
			if args.len() != required {
				let r = sciter::Value::error(&format!("{} error: {} of {} arguments provided.", "Object::add_year", args.len(), required));
				r.pack_to(p_result);
				return true as BOOL;
			}

			let r = me.add_year(
				match sciter::FromValue::from_value(&args[0]) {
					Some(arg) => arg,
					None => {
							let r = sciter::Value::error(&format!("{} error: invalid type of {} argument ({} expected, {:?} provided).",
								"Object::add_year", 0, "i32", &args[0]
						));
						r.pack_to(p_result);
						return true as BOOL;
					}
				},
			);
			let r: sciter::Value = r.into();
			r.pack_to(p_result);
			return true as BOOL;
		}

		extern "C" fn on_get_age(thing: *mut som_asset_t, p_value: &mut VALUE) -> BOOL
		{
			let me = IAsset::<Object>::from_raw(&thing);
			let r = sciter::Value::from(&me.age);
			r.pack_to(p_value);
			return true as BOOL;
		}
		extern "C" fn on_set_age(thing: *mut som_asset_t, p_value: &VALUE) -> BOOL
		{
			let me = IAsset::<Object>::from_raw(&thing);
			use sciter::FromValue;
			let v = sciter::Value::from(p_value);
			if let Some(v) = FromValue::from_value(&v) {
				me.age = v;
				true as BOOL
			} else {
				false as BOOL
			}
		}

		extern "C" fn on_get_name(thing: *mut som_asset_t, p_value: &mut VALUE) -> BOOL
		{
			let me = IAsset::<Object>::from_raw(&thing);
			let r = sciter::Value::from(&me.name);
			r.pack_to(p_value);
			return true as BOOL;
		}
		extern "C" fn on_set_name(thing: *mut som_asset_t, p_value: &VALUE) -> BOOL
		{
			let me = IAsset::<Object>::from_raw(&thing);
			use sciter::FromValue;
			let v = sciter::Value::from(p_value);
			if let Some(v) = FromValue::from_value(&v) {
				me.name = v;
				true as BOOL
			} else {
				false as BOOL
			}
		}

		type ObjectMethods = [som_method_def_t; 2];

		let mut methods = Box::new(ObjectMethods::default());

		let mut method = &mut methods[0];
		method.name = atom("print");
		method.func = Some(on_print);
		method.params = 0;

		let mut method = &mut methods[1];
		method.name = atom("add_year");
		method.func = Some(on_add_year);
		method.params = 1;

		type ObjectProps = [som_property_def_t; 2];

		let mut props = Box::new(ObjectProps::default());

		let mut prop = &mut props[0];
		prop.name = atom("age");
		prop.getter = Some(on_get_age);
		prop.setter = Some(on_set_age);

		let mut prop = &mut props[1];
		prop.name = atom("name");
		prop.getter = Some(on_get_name);
		prop.setter = Some(on_set_name);

		let mut pst = Box::new(som_passport_t::default());
		pst.name = atom("TestGlobal");

		pst.n_methods = 2;
		pst.methods = Box::into_raw(methods) as *const _;

		pst.n_properties = 2;
		pst.properties = Box::into_raw(props) as *const _;

		Box::leak(pst)
	}
}


#[derive(Debug)]
struct Handler {
	asset: sciter::om::IAssetRef<Object>,
}

impl sciter::EventHandler for Handler {
	fn attached(&mut self, _root: HELEMENT) {
		println!("attached");
	}
	fn detached(&mut self, _root: HELEMENT) {
		println!("detached");
	}
	fn document_complete(&mut self, _root: HELEMENT, _target: HELEMENT) {
		println!("loaded");
	}

	fn get_asset(&mut self) -> Option<&sciter::om::som_asset_t> {
		Some(self.asset.as_ref())
	}
}

fn main() {
	sciter::set_options(sciter::RuntimeOptions::DebugMode(true)).unwrap();

	let mut frame = sciter::Window::new();

	let object = Object::default();
	let object = sciter::om::IAsset::new(object);
	sciter::om::into_global(object);

	let object2 = Object::default();
	let object2 = sciter::om::IAsset::new(object2);
	let object2 = sciter::om::IAssetRef::from(object2);
	let ptr = object2.as_ptr();
	let psp = object2.get_passport();
	println!{"asset {:?} psp {:?}", ptr, psp as *const _};
	println!("asset: {:?}", object2);

	let handler = Handler { asset: object2 };
	frame.event_handler(handler);

	let html = include_bytes!("som.htm");
	frame.load_html(html, Some("example://som.htm"));
	frame.run_app();
}
