/*! Sciter Object Model (SOM passport), native C interface.

See https://sciter.com/native-code-exposure-to-script/
and https://sciter.com/developers/for-native-gui-programmers/sciter-object-model/.

*/

#![allow(non_snake_case, non_camel_case_types)]
#![allow(dead_code)]

use capi::sctypes::*;
use capi::scvalue::VALUE;

/// An atom value that uniquely identifies the name being registered.
pub type som_atom_t = u64;


/// `som_asset_t` is a structure that a custom native object must be derived from.
#[repr(C)]
#[derive(Debug)]
pub struct som_asset_t {
	pub(crate) isa: &'static som_asset_class_t,
}

impl som_asset_t {
	pub(crate) fn get_passport(&self) -> *const som_passport_t {
		(self.isa.get_passport)(self as *const _ as *mut _)
	}
}

/// Is a pack of 4 pointers to functions that define the life time of an asset.
#[repr(C)]
#[derive(Debug)]
pub(crate) struct som_asset_class_t {
	/// Increments the reference count for an interface on an object.
	pub add_ref: extern "C" fn(thing: *mut som_asset_t) -> i32,

	/// Decrements the reference count for an interface on an object.
	pub release: extern "C" fn(thing: *mut som_asset_t) -> i32,

	/// Retrieves a pointer to a supported interface of an object.
	pub get_interface: extern "C" fn(thing: *mut som_asset_t, name: LPCSTR, out: *mut *mut som_asset_t) -> bool,

	/// Retrieves a pointer to the passport declaration of an object.
	pub get_passport: extern "C" fn(thing: *mut som_asset_t) -> *const som_passport_t,
}


/// Defines properties and methods of an asset.
#[repr(C)]
pub struct som_passport_t {
	/// Flags of an asset, see [`som_passport_flags`](enum.som_passport_flags.html).
	pub flags: u64,

	/// The name of the class (asset type).
	pub name: som_atom_t,

	/// Properties: `asset.prop`.
	///
	/// Must be a pointer to an array of structures:
	///
	/// ```rust,no_run
	/// # use sciter::om::*;
	/// let mut pst = Box::new(som_passport_t::default());
	///
	/// type ObjectProps = [som_property_def_t; 2];
	/// let mut props = Box::new(ObjectProps::default());
	///
	/// let mut prop1 = &mut props[0];
	/// prop1.name = atom("age");
	///
	/// let mut prop2 = &mut props[1];
	/// prop2.name = atom("name");
	///
	/// pst.n_properties = 2;
	/// pst.properties = Box::into_raw(props) as *const _;
	/// ```

	pub properties: *const som_property_def_t,

	/// Properties count.
	pub n_properties: usize,

	/// Methods: `asset.func()`
	///
	/// Must be a pointer to an array of structures,
	/// see [`properties`](struct.som_passport_t.html#structfield.properties) for an example.
	pub methods: *const som_method_def_t,

	/// Methods count.
	pub n_methods: usize,

	/// Index access: `var item = asset[key]`.
	pub item_getter: Option<som_item_getter_t>,

	/// Index access: `asset[key] = item`.
	pub item_setter: Option<som_item_setter_t>,

	/// Enumeration: `for(var item in asset)`.
	pub item_next: Option<som_item_next_t>,

	/// Property access interceptor: `var val = asset.prop`.
	pub prop_getter: Option<som_any_prop_getter_t>,

	/// Property set interceptor: `asset.prop = val`.
	pub prop_setter: Option<som_any_prop_setter_t>,
}

/// Empty passport.
impl Default for som_passport_t {
	fn default() -> Self {
		use std::ptr;
		Self {
			flags: 0,
			name: 0,

			prop_getter: None,
			prop_setter: None,

			item_getter: None,
			item_setter: None,
			item_next: None,

			properties: ptr::null(),
			n_properties: 0,

			methods: ptr::null(),
			n_methods: 0,
		}
	}
}


/// [`som_passport_t`](struct.som_passport_t.html#structfield.flags) flags.
#[repr(u64)]
#[derive(Debug, PartialOrd, PartialEq)]
pub enum som_passport_flags {
	/// Not extendable.
	SEALED = 0,

	/// Extendable.
	///
	/// An asset may have new properties added by script.
	EXTENDABLE = 1,
}


/// Property of an asset.
#[repr(C)]
pub struct som_property_def_t {
	pub reserved: LPVOID,

	/// Property name.
	pub name: som_atom_t,

	/// Property getter: `var val = asset.prop`.
	pub getter: Option<som_prop_getter_t>,

	/// Property setter: `asset.prop = val`.
	pub setter: Option<som_prop_setter_t>,
}

/// Empty property.
impl Default for som_property_def_t {
	fn default() -> Self {
		Self {
			reserved: std::ptr::null_mut(),
			name: 0,
			getter: None,
			setter: None,
		}
	}
}

/// Method of an asset.
#[repr(C)]
pub struct som_method_def_t {
	pub reserved: LPVOID,

	/// Method name.
	pub name: som_atom_t,

	/// Parameters count.
	///
	/// The actual arguments count can be lesser then specified here:
	///
	/// ```tiscript,ignore
	/// function asset.func(a,b,c);  // native asset method accepts 3 parameters
	///
	/// asset.func("one"); // call with only one parameter.
	/// ```
	pub params: usize,

	/// Method body.
	pub func: Option<som_method_t>,
}

/// Empty method.
impl Default for som_method_def_t {
	fn default() -> Self {
		Self {
			reserved: std::ptr::null_mut(),
			name: 0,
			params: 0,
			func: None,
		}
	}
}

type som_dispose_t = extern "C" fn(thing: *mut som_asset_t);

type som_prop_getter_t = extern "C" fn(thing: *mut som_asset_t, p_value: &mut VALUE) -> BOOL;
type som_prop_setter_t = extern "C" fn(thing: *mut som_asset_t, p_value: &VALUE) -> BOOL;

type som_any_prop_getter_t = extern "C" fn(thing: *mut som_asset_t, propSymbol: som_atom_t, p_value: &mut VALUE) -> BOOL;
type som_any_prop_setter_t = extern "C" fn(thing: *mut som_asset_t, propSymbol: som_atom_t, p_value: &VALUE) -> BOOL;

type som_item_getter_t = extern "C" fn(thing: *mut som_asset_t, p_key: &VALUE, p_value: &mut VALUE) -> BOOL;
type som_item_setter_t = extern "C" fn(thing: *mut som_asset_t, p_key: &VALUE, p_value: &VALUE) -> BOOL;

type som_item_next_t = extern "C" fn(thing: *mut som_asset_t, p_idx: &mut VALUE, p_value: &mut VALUE) -> BOOL;

type som_method_t = extern "C" fn(thing: *mut som_asset_t, argc: u32, argv: *const VALUE, p_result: &mut VALUE) -> BOOL;
