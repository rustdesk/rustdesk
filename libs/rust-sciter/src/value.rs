/*! Rust interface to the [`sciter::value`](https://github.com/c-smile/sciter-sdk/blob/master/include/value.h).

Sciter [`Value`](struct.Value.html) holds superset of JSON objects.

It can contain as pure JSON objects (numbers, strings, maps and arrays) as internal objects like DOM elements,
proxies of script functions, objects and arrays.


## Basic usage

You can create an empty (undefined) Sciter value with [`Value::new()`](struct.Value.html):

```
use sciter::Value;

let v = Value::new();
assert!(v.is_undefined());
assert!(!v.is_null());
```

Or explicitly create `Value` of the specified type:

```
use sciter::Value;

let v = Value::null();
assert!(v.is_null());

let v = Value::symbol("hello");
assert!(v.is_symbol());
assert!(v.is_string());

let v = Value::error("hello");
assert!(v.is_error_string());
assert!(v.is_string());

// allocate a new array with 4 empty elements
let v = Value::array(4);
assert!(v.is_array());
assert!(v.len() == 4);

// allocate a new value with map type
let v = Value::map();
assert!(v.is_map());
assert!(v.len() == 0);

```

Also there is conversion from Rust types:

```
use sciter::Value;

let v = Value::from(true);
assert!(v.is_bool());

let v = Value::from(1);
assert!(v.is_int());

let v = Value::from(1.0);
assert!(v.is_float());

let v = Value::from("hello");
assert!(v.is_string());

let v = Value::from(b"123".as_ref());
assert!(v.is_bytes());
```

And from a sequence of objects:

```
use sciter::Value;

let v: Value = ["1","2","3"].iter().cloned().collect();
assert!(v.is_array());
assert_eq!(v.len(), 3);

assert_eq!(v[0], Value::from("1"));
```

And also there is a couple of macros for container literals with specific type
which are just shorthands for manual construction using the
[`set_item()`](struct.Value.html#method.set_item) and [`push()`](struct.Value.html#method.push)
methods:

```
# #[macro_use] extern crate sciter;
# fn main() {
let map = vmap! {
  "one" => 1,
  "two" => 2.0,
  "three" => "",
};
assert!(map.is_map());
assert_eq!(map.len(), 3);

let array = varray![1, 2.0, "three"];
assert!(array.is_array());
assert_eq!(array.len(), 3);
# }
```

To access its contents you should use one of [`to_`](struct.Value.html#method.to_int) methods:

```
use sciter::Value;

let v = Value::from(4);
assert_eq!(v.to_int(), Some(4));
```

Note that there are two functions that convert `Value` to JSON and back:

```
use sciter::Value;

let mut v: Value = "[1, 2, 3, 4]".parse().unwrap();
let json_str = v.into_string();
```

Array access:

```
use sciter::Value;

let mut v: Value = "[10, 20]".parse().unwrap();
assert_eq!(v[0], Value::from(10));

// explicit arguments:
v.set(1, Value::from(21));
v.set(2, Value::from(22));

// implicit arguments:
v.set(1, 21);
v.set(2, 22);

assert_eq!(v.len(), 3);

assert!(v.get(0).is_int());
```

Map access:

```
use sciter::Value;

let mut v: Value = "{one: 1, two: 2}".parse().unwrap();
assert_eq!(v["one"], 1.into());
assert_eq!(v.get_item("one"), 1.into());
assert_eq!(v[Value::from("one")], Value::from(1));

v.set_item("three", 3);
assert!(v.get_item("one").is_int());
```

.
*/

use ::{_API};
use capi::sctypes::*;
use capi::scvalue::{VALUE_UNIT_TYPE_STRING, VALUE_UNIT_TYPE_OBJECT, VALUE_UNIT_UNDEFINED};
pub use capi::scvalue::{VALUE_RESULT, VALUE_STRING_CVT_TYPE, VALUE_TYPE};
use capi::scvalue::VALUE;
use ::om::IAsset;


// TODO: `get`, `get_item` methods should return `Option<Value>`

/// `sciter::value` wrapper.
///
/// See the [module-level](index.html) documentation.
pub struct Value
{
	data: VALUE,
	tmp: * mut Value,
}

/// `sciter::Value` can be transferred across thread boundaries.
unsafe impl Send for Value {}

impl Value {

	/// Return a new Sciter value object ([`undefined`](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/undefined)).
	pub fn new() -> Value {
		Value { data: VALUE::default(), tmp: ::std::ptr::null_mut() }
	}

	/// Make an explicit [array](https://sciter.com/docs/content/script/Array.htm) value with the given length.
	pub fn array(length: usize) -> Value {
		let mut me = Value::new();
		(_API.ValueIntDataSet)(me.as_ptr(), length as i32, VALUE_TYPE::T_ARRAY as UINT, 0);
		return me;
	}

	/// Make an explicit [map](https://sciter.com/docs/content/script/Object.htm) value.
	pub fn map() -> Value {
		let mut me = Value::new();
		(_API.ValueIntDataSet)(me.as_ptr(), 0i32, VALUE_TYPE::T_MAP as UINT, 0);
		return me;
	}

	/// Make an explicit json [null](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/null) value.
	pub fn null() -> Value {
		let mut me = Value::new();
		me.data.t = VALUE_TYPE::T_NULL;
		return me;
	}
	/// Make an explicit `nothing` (where did it come from?).
	pub fn nothing() -> Value {
		let mut me = Value::new();
		me.data.t = VALUE_TYPE::T_UNDEFINED;
		me.data.u = VALUE_UNIT_UNDEFINED::UT_NOTHING as UINT;
		return me;
	}

	/// Make Sciter [symbol](https://sciter.com/docs/content/script/language/Syntax.htm#symbol-literals) value.
	pub fn symbol(val: &str) -> Value {
		let mut me = Value::new();
		me.assign_str(val, VALUE_UNIT_TYPE_STRING::SYMBOL);
		return me;
	}

	/// Make Sciter [error](https://sciter.com/docs/content/script/Error.htm) value.
	pub fn error(val: &str) -> Value {
		let mut me = Value::new();
		me.assign_str(val, VALUE_UNIT_TYPE_STRING::ERROR);
		return me;
	}

	/// Make Sciter [color](https://sciter.com/docs/content/script/Color.htm) value, in `0xAABBGGRR` form.
	pub fn color(val: u32) -> Value {
		let mut me = Value::new();
		(_API.ValueIntDataSet)(me.as_ptr(), val as i32, VALUE_TYPE::T_COLOR as u32, 0);
		return me;
	}

	/// Make Sciter [duration](https://sciter.com/docs/content/script/language/Types.htm) value, in seconds.
	pub fn duration(val: f64) -> Value {
		let mut me = Value::new();
		(_API.ValueFloatDataSet)(me.as_ptr(), val, VALUE_TYPE::T_DURATION as u32, 0);
		return me;
	}

	/// Make Sciter [angle](https://sciter.com/docs/content/script/Angle.htm) value, in radians.
	pub fn angle(val: f64) -> Value {
		let mut me = Value::new();
		(_API.ValueFloatDataSet)(me.as_ptr(), val, VALUE_TYPE::T_ANGLE as u32, 0);
		return me;
	}

	/// Parse a json string into value. Returns the number of chars left unparsed in case of error.
	pub fn parse(val: &str) -> Result<Value, usize> {
		return Value::parse_as(val, VALUE_STRING_CVT_TYPE::JSON_LITERAL);
	}

	/// Parse a json string into value. Returns the number of chars left unparsed in case of error.
	pub fn parse_as(val: &str, how: VALUE_STRING_CVT_TYPE) -> Result<Value, usize> {
		let mut me = Value::new();
		let (s,n) = s2wn!(val);
		let ok: u32 = (_API.ValueFromString)(me.as_ptr(), s.as_ptr(), n, how);
		if ok == 0 {
			Ok(me)
		} else {
			Err(ok as usize)
		}
	}

	/// Value to asset.
	pub fn to_asset<T>(&self) -> Option<&mut IAsset<T>> {
		if self.is_asset() {
			let mut val = 0_i64;
			if (_API.ValueInt64Data)(self.as_cptr(), &mut val)  == VALUE_RESULT::OK {
				let ptr = val as usize as *mut IAsset<T>;
				let asset = unsafe { &mut *ptr };
				return Some(asset);
			}
		}
		return None;
	}

	#[doc(hidden)]
	pub fn as_ptr(&mut self) -> *mut VALUE {
		&mut self.data as *mut VALUE
	}

	#[doc(hidden)]
	pub fn as_cptr(&self) -> *const VALUE {
		&self.data as *const VALUE
	}

	/// Get the inner type of the value.
	pub fn get_type(&self) -> VALUE_TYPE {
		return self.data.t;
	}

	/// Get the inner type and its subtype (e.g. units) of the value.
	pub fn full_type(&self) -> (VALUE_TYPE, UINT) {
		return (self.data.t, self.data.u);
	}

	/// Convert `T_OBJECT` value type to JSON `T_MAP` or `T_ARRAY` types.
	///
	/// Also must be used if you need to pass values between different threads.
	pub fn isolate(&mut self) {
		(_API.ValueIsolate)(self.as_ptr());
	}

	/// Clear the value. It deallocates all assosiated structures that are not used anywhere else.
	pub fn clear(&mut self) -> &mut Value {
		(_API.ValueClear)(self.as_ptr());
		self
	}

	/// Return the number of items in the `T_ARRAY`, `T_MAP`, `T_FUNCTION` and `T_OBJECT` value types.
	pub fn len(&self) -> usize {
		let mut n: INT = 0;
		(_API.ValueElementsCount)(self.as_cptr(), &mut n);
		return n as usize;
	}

	/// Append another value to the end of `T_ARRAY` value.
	pub fn push<T: Into<Value>>(&mut self, src: T) {
		(_API.ValueNthElementValueSet)(self.as_ptr(), self.len() as INT, src.into().as_cptr());
	}

	/// Insert or set value at the given `index` of `T_ARRAY`, `T_MAP`, `T_FUNCTION` and `T_OBJECT` value.
	pub fn set<T: Into<Value>>(&mut self, index: usize, src: T) {
		(_API.ValueNthElementValueSet)(self.as_ptr(), index as INT, src.into().as_cptr());
	}

	/// Retreive value of sub-element at `index`.
	///
	/// * `T_ARRAY` - nth element of the array;
	/// * `T_MAP` - value of the nth key/value pair in the map;
	/// * `T_FUNCTION` - value of the nth argument of the function.
	///
	pub fn get(&self, index: usize) -> Value {
		let mut v = Value::new();
		(_API.ValueNthElementValue)(self.as_cptr(), index as INT, v.as_ptr());
		return v;
	}

	/// Insert or set value of the sub-element by `key`.
	///
	/// * `T_MAP` - sets named value in the map;
	/// * `T_OBJECT` - sets value of property of the object;
	/// * `T_FUNCTION` - sets named argument of the function;
	/// * otherwise it converts self to the map type and adds the key/value to it.
	///
	pub fn set_item<TKey: Into<Value>, TValue: Into<Value>>(&mut self, key: TKey, value: TValue) {
		(_API.ValueSetValueToKey)(self.as_ptr(), key.into().as_cptr(), value.into().as_cptr());
	}

	/// Retrieve the value of a sub-element by key.
	pub fn get_item<T: Into<Value>>(&self, key: T) -> Value {
		let mut v = Value::new();
		(_API.ValueGetValueOfKey)(self.as_cptr(), key.into().as_cptr(), v.as_ptr());
		return v;
	}

	/// Retrieve the key of a sub-element by `index`.
	pub fn key_at(&self, index: usize) -> Value {
		let mut v = Value::new();
		(_API.ValueNthElementKey)(self.as_cptr(), index as INT, v.as_ptr());
		return v;
	}

	/// An iterator visiting all keys of key/value pairs in the map.
	///
	/// * `T_MAP` - keys of key/value pairs in the map;
	/// * `T_OBJECT` - names of key/value properties in the object;
	/// * `T_FUNCTION` - names of arguments of the function (if any).
	///
  /// The iterator element type is `Value` (as a key).
	pub fn keys(&self) -> KeyIterator {
		KeyIterator {
			base: self,
			index: 0,
			count: self.len(),
		}
	}

	/// An iterator visiting all values in arbitrary order.
	///
	/// * `T_ARRAY` - elements of the array;
	/// * `T_MAP` - values of key/value pairs in the map;
	/// * `T_OBJECT` - values of key/value properties in the object;
	/// * `T_FUNCTION` - values of arguments of the function.
	///
  /// The iterator element type is `Value`.
	pub fn values(&self) -> SeqIterator {
		SeqIterator {
			base: self,
			index: 0,
			count: self.len(),
		}
	}

	/// An iterator visiting all key-value pairs in arbitrary order.
	///
  /// The `Value` must has a key-value type (map, object, function).
  ///
	/// The iterator element type is `(Value, Value)`.
	pub fn items(&self) -> Vec<(Value, Value)> {
		type VecType = Vec<(Value, Value)>;
		let mut result = Vec::with_capacity(self.len());

		extern "system" fn on_pair(param: LPVOID, pkey: *const VALUE, pval: *const VALUE) -> BOOL {
			assert!(!param.is_null());
			unsafe {
				let result = param as *mut VecType;
				let result = &mut *result;
				let item = (Value::copy_from(pkey), Value::copy_from(pval));
				result.push(item);
			}
			return true as BOOL;
		}

		let ptr = &mut result as *mut VecType;
		(_API.ValueEnumElements)(self.as_cptr(), on_pair, ptr as LPVOID);

		return result;
	}

	/// Value to integer.
	pub fn to_int(&self) -> Option<i32> {
		let mut val = 0i32;
		match (_API.ValueIntData)(self.as_cptr(), &mut val) {
			VALUE_RESULT::OK => Some(val),
			_ => None
		}
	}

	/// Value to bool.
	pub fn to_bool(&self) -> Option<bool> {
		let mut val = 0i32;
		match (_API.ValueIntData)(self.as_cptr(), &mut val) {
			VALUE_RESULT::OK => Some(val != 0),
			_ => None
		}
	}

	/// Value to float.
	pub fn to_float(&self) -> Option<f64> {
		let mut val = 0f64;
		match (_API.ValueFloatData)(self.as_cptr(), &mut val) {
			VALUE_RESULT::OK => Some(val),
			_ => None
		}
	}

	/// Value to color.
	pub fn to_color(&self) -> Option<u32> {
		let mut val = 0i32;
		match (_API.ValueIntData)(self.as_cptr(), &mut val) {
			VALUE_RESULT::OK => Some(val as u32),
			_ => None
		}
	}

	/// Value to duration.
	pub fn to_duration(&self) -> Option<f64> {
		let mut val = 0f64;
		match (_API.ValueFloatData)(self.as_cptr(), &mut val) {
			VALUE_RESULT::OK => Some(val),
			_ => None
		}
	}

	/// Value to angle.
	pub fn to_angle(&self) -> Option<f64> {
		let mut val = 0f64;
		match (_API.ValueFloatData)(self.as_cptr(), &mut val) {
			VALUE_RESULT::OK => Some(val),
			_ => None
		}
	}

	/// Value as string for `T_STRING` type.
	pub fn as_string(&self) -> Option<String> {
		let mut s = 0 as LPCWSTR;
		let mut n = 0 as UINT;
		match (_API.ValueStringData)(self.as_cptr(), &mut s, &mut n) {
			VALUE_RESULT::OK => Some(::utf::w2sn(s, n as usize)),
			_ => None
		}
	}

	/// Value to json string (converted in-place). _Subject to change._
	pub fn into_string(mut self) -> String {
		(_API.ValueToString)(self.as_ptr(), VALUE_STRING_CVT_TYPE::JSON_LITERAL);
		return self.as_string().unwrap();
	}

	/// Value as a byte slice for `T_BYTES` type.
	pub fn as_bytes(&self) -> Option<&[u8]> {
		let mut s = 0 as LPCBYTE;
		let mut n = 0 as UINT;
		match (_API.ValueBinaryData)(self.as_cptr(), &mut s, &mut n) {
			VALUE_RESULT::OK => Some(unsafe { ::std::slice::from_raw_parts(s, n as usize) }),
			_ => None
		}
	}

	/// Value to byte vector for `T_BYTES` type.
	pub fn to_bytes(&self) -> Option<Vec<u8>> {
		match self.as_bytes() {
			Some(r) => Some(r.to_owned()),
			None => None,
		}
	}

	/// Function invocation for `T_OBJECT` with `UT_OBJECT_FUNCTION` value type.
	///
	/// Calls the tiscript function or method holded at `Value` with context of `this` object
	/// that will be known as _this_ inside that function (it is optional for global functions).
	///
	/// The `name` here is an url or a name of the script - used for error reporting in script.
	///
	/// You can use the [`make_args!(args...)`](../macro.make_args.html) macro which helps you
	/// to construct script arguments from Rust types.
	pub fn call(&self, this: Option<Value>, args: &[Value], name: Option<&str>) -> Result<Value, VALUE_RESULT> {
		let mut rv = Value::new();
		let argv = Value::pack_args(args);
		let name = s2w!(name.unwrap_or(""));
		let ok = (_API.ValueInvoke)(self.as_cptr(), this.unwrap_or_default().as_ptr(),
			argv.len() as UINT, argv.as_ptr(), rv.as_ptr(), name.as_ptr());
		match ok {
			VALUE_RESULT::OK => Ok(rv),
			_ => Err(ok)
		}
	}

	#[doc(hidden)]
	pub fn pack_to(&self, dst: &mut VALUE) {
		(_API.ValueCopy)(dst, self.as_cptr());
	}

	#[doc(hidden)]
	pub fn pack_args(args: &[Value]) -> Vec<VALUE> {
		let argc = args.len();
		let mut argv: Vec<VALUE> = Vec::with_capacity(argc);
		argv.resize(argc, VALUE::default());
		for i in 0..argc {
			args[i].pack_to(&mut argv[i]);
		}
		return argv;
	}

	#[doc(hidden)]
	pub unsafe fn unpack_from(args: * const VALUE, count: UINT) -> Vec<Value> {
		let argc = count as usize;
		let mut argv: Vec<Value> = Vec::with_capacity(argc);
		assert!(argc == 0 || !args.is_null());
		let args = ::std::slice::from_raw_parts(args, argc);
		for arg in args {
			argv.push(Value::copy_from(arg));
		}
		return argv;
	}

	#[doc(hidden)]
	unsafe fn copy_from(ptr: *const VALUE) -> Value {
		assert!(!ptr.is_null());
		let mut v = Value::new();
		(_API.ValueCopy)(v.as_ptr(), ptr);
		return v;
	}

  #[allow(clippy::mut_from_ref)]
	fn ensure_tmp_mut(&self) -> &mut Value {
		let cp = self as *const Value;
		let mp = cp as *mut Value;
		let me = unsafe { &mut *mp };
		return me.ensure_tmp();
	}

	fn ensure_tmp(&mut self) -> &mut Value {
		if self.tmp.is_null() {
			let tmp = Box::new(Value::new());
			self.tmp = Box::into_raw(tmp);
		}
		return unsafe { &mut *self.tmp };
	}

	/// Returns `true` is `self` is `undefined` or has zero elements.
	pub fn is_empty(&self) -> bool {
		self.is_undefined() || self.len() == 0
	}

	#[allow(missing_docs)]
	pub fn is_undefined(&self) -> bool {
		self.data.t == VALUE_TYPE::T_UNDEFINED && self.data.u == 0
	}
	#[allow(missing_docs)]
	pub fn is_null(&self) -> bool {
		self.data.t == VALUE_TYPE::T_NULL
	}
	#[allow(missing_docs)]
	pub fn is_nothing(&self) -> bool {
		self.data.t == VALUE_TYPE::T_UNDEFINED && self.data.u == VALUE_UNIT_UNDEFINED::UT_NOTHING as UINT
	}
	#[allow(missing_docs)]
	pub fn is_bool(&self) -> bool {
		self.data.t == VALUE_TYPE::T_BOOL
	}
	#[allow(missing_docs)]
	pub fn is_int(&self) -> bool {
		self.data.t == VALUE_TYPE::T_INT
	}
	#[allow(missing_docs)]
	pub fn is_float(&self) -> bool {
		self.data.t == VALUE_TYPE::T_FLOAT
	}
	#[allow(missing_docs)]
	pub fn is_bytes(&self) -> bool {
		self.data.t == VALUE_TYPE::T_BYTES
	}
	#[allow(missing_docs)]
	pub fn is_string(&self) -> bool {
		self.data.t == VALUE_TYPE::T_STRING
	}
	#[allow(missing_docs)]
	pub fn is_symbol(&self) -> bool {
		self.data.t == VALUE_TYPE::T_STRING && self.data.u == VALUE_UNIT_TYPE_STRING::SYMBOL as UINT
	}
	#[allow(missing_docs)]
	pub fn is_error_string(&self) -> bool {
		self.data.t == VALUE_TYPE::T_STRING && self.data.u == VALUE_UNIT_TYPE_STRING::ERROR as UINT
	}
	#[allow(missing_docs)]
	pub fn is_date(&self) -> bool {
		self.data.t == VALUE_TYPE::T_DATE
	}
	#[allow(missing_docs)]
	pub fn is_currency(&self) -> bool {
		self.data.t == VALUE_TYPE::T_CURRENCY
	}
	#[allow(missing_docs)]
	pub fn is_color(&self) -> bool {
		self.data.t == VALUE_TYPE::T_COLOR
	}
	#[allow(missing_docs)]
	pub fn is_duration(&self) -> bool {
		self.data.t == VALUE_TYPE::T_DURATION
	}
	#[allow(missing_docs)]
	pub fn is_angle(&self) -> bool {
		self.data.t == VALUE_TYPE::T_ANGLE
	}
	#[allow(missing_docs)]
	pub fn is_map(&self) -> bool {
		self.data.t == VALUE_TYPE::T_MAP
	}
	#[allow(missing_docs)]
	pub fn is_array(&self) -> bool {
		self.data.t == VALUE_TYPE::T_ARRAY
	}
	#[allow(missing_docs)]
	pub fn is_function(&self) -> bool {
		self.data.t == VALUE_TYPE::T_FUNCTION
	}
	#[allow(missing_docs)]
	pub fn is_native_function(&self) -> bool {
		(_API.ValueIsNativeFunctor)(self.as_cptr()) != 0
	}
	#[allow(missing_docs)]
	pub fn is_object(&self) -> bool {
		self.data.t == VALUE_TYPE::T_OBJECT
	}
	#[allow(missing_docs)]
	pub fn is_asset(&self) -> bool {
		self.data.t == VALUE_TYPE::T_ASSET
	}

  // script types:
  #[allow(missing_docs)]
  pub fn is_object_array(&self) -> bool {
    self.data.t == VALUE_TYPE::T_OBJECT && self.data.u == VALUE_UNIT_TYPE_OBJECT::ARRAY as UINT
  }
  #[allow(missing_docs)]
  pub fn is_object_map(&self) -> bool {
    self.data.t == VALUE_TYPE::T_OBJECT && self.data.u == VALUE_UNIT_TYPE_OBJECT::OBJECT as UINT
  }
  #[allow(missing_docs)]
  pub fn is_object_class(&self) -> bool {
    self.data.t == VALUE_TYPE::T_OBJECT && self.data.u == VALUE_UNIT_TYPE_OBJECT::OBJECT as UINT
  }
  #[allow(missing_docs)]
  pub fn is_object_native(&self) -> bool {
    self.data.t == VALUE_TYPE::T_OBJECT && self.data.u == VALUE_UNIT_TYPE_OBJECT::NATIVE as UINT
  }
  #[allow(missing_docs)]
  pub fn is_object_function(&self) -> bool {
    self.data.t == VALUE_TYPE::T_OBJECT && self.data.u == VALUE_UNIT_TYPE_OBJECT::FUNCTION as UINT
  }
  #[allow(missing_docs)]
  pub fn is_object_error(&self) -> bool {
    self.data.t == VALUE_TYPE::T_OBJECT && self.data.u == VALUE_UNIT_TYPE_OBJECT::ERROR as UINT
  }
	#[allow(missing_docs)]
	pub fn is_dom_element(&self) -> bool {
		self.data.t == VALUE_TYPE::T_DOM_OBJECT
	}

  // generic check (native or object):
  #[allow(missing_docs)]
  pub fn is_varray(&self) -> bool {
    self.is_array() || self.is_object()
  }
  #[allow(missing_docs)]
  pub fn is_vmap(&self) -> bool {
    self.is_map() || self.is_object_map()
  }
  #[allow(missing_docs)]
  pub fn is_vfunction(&self) -> bool {
    self.is_function() || self.is_object_function() || self.is_native_function()
  }
  #[allow(missing_docs)]
  pub fn is_verror(&self) -> bool {
    self.is_error_string() || self.is_object_error()
  }

	fn assign_str(&mut self, val: &str, unit: VALUE_UNIT_TYPE_STRING) -> VALUE_RESULT {
		let (s,n) = s2wn!(val);
		return (_API.ValueStringDataSet)(self.as_ptr(), s.as_ptr(), n, unit as UINT);
	}
}


/// Print `Value` as json string
impl ::std::fmt::Display for Value {
	fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
		let copy = self.clone();
		let re = copy.into_string();
		f.write_str(&re)
	}
}

/// Print `Value` as json string with explicit type showed.
impl ::std::fmt::Debug for Value {
	fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {

		let mut tname = format!("{:?}", self.data.t);

		if self.is_undefined() || self.is_null() {
			return f.write_str(&tname[2..].to_lowercase());

		} else if self.is_nothing() {
			return f.write_str("nothing");

		} else if self.is_string() && self.data.u != 0 {
			// VALUE_UNIT_TYPE_STRING
			let units = [
				("file", 0xfffe), ("symbol", 0xffff),
				("string", 0), ("error", 1), ("secure", 2),
				("url", 3), ("selector", 4),
				];

			tname.push_str(":");
			if let Some(name) = units.iter().find(|&&x| x.1 == self.data.u) {
				tname.push_str(name.0);
			} else {
				tname.push_str(&self.data.u.to_string());
			}

		} else if self.is_object() {
			// VALUE_UNIT_TYPE_OBJECT
			let units = ["array", "object", "class", "native", "function", "error"];
			let u = self.data.u as usize;
			tname.push_str(":");
			if u < units.len() {
				tname.push_str(units[u]);
			} else {
				tname.push_str(&u.to_string());
			}

		} else if self.data.u != 0 {
			// VALUE_UNIT_TYPE
			// redundant? like "length:7:12px" instead of "length:12px" (7 == `UT_PX`).

			// tname.push_str(":");
			// tname.push_str(&self.data.u.to_string());
		}
		f.write_str(&tname[2..].to_lowercase())?;
		f.write_str(":")?;
		write!(f, "{}", &self)
	}
}

/// Destroy pointed value.
impl Drop for Value {
	fn drop(&mut self) {
		if !self.tmp.is_null() {
			unsafe { Box::from_raw(self.tmp) };
		}
		(_API.ValueClear)(self.as_ptr());
	}
}

/// Return default value (`undefined`).
impl Default for Value {
	fn default() -> Self {
		return Value::new();
	}
}

/// Copies value.
///
/// All allocated objects are reference counted so copying is just a matter of increasing reference counts.
impl Clone for Value {
	fn clone(&self) -> Self {
		let mut dst = Value::new();
		(_API.ValueCopy)(dst.as_ptr(), self.as_cptr());
		return dst;
	}
}

/// Compare two values.
impl ::std::cmp::PartialEq for Value {
	fn eq(&self, other: &Self) -> bool {
		match (_API.ValueCompare)(self.as_cptr(), other.as_cptr()) {
			VALUE_RESULT::OK_TRUE => true,
			// VALUE_RESULT::OK => false,
			_ => false
		}
	}
}

/// Get item by index for array type.
impl ::std::ops::Index<usize> for Value {
	type Output = Value;
	fn index(&self, index: usize) -> &Self::Output {
		let tmp = self.ensure_tmp_mut();
		(_API.ValueNthElementValue)(self.as_cptr(), index as INT, tmp.as_ptr());
		return tmp;
	}
}

/// Set item by index for array type.
#[cfg(notworking)]
impl ::std::ops::IndexMut<usize> for Value {
	fn index_mut(&mut self, index: usize) -> &mut Value {
		let tmp = self.ensure_tmp_mut();
		(_API.ValueNthElementValue)(self.as_cptr(), index as INT, tmp.as_ptr());
		return tmp;
	}
}

/// Get item by key for map type.
impl ::std::ops::Index<Value> for Value {
	type Output = Value;
	fn index(&self, key: Value) -> &Self::Output {
		let tmp = self.ensure_tmp_mut();
		(_API.ValueGetValueOfKey)(self.as_cptr(), key.as_cptr(), tmp.as_ptr());
		return tmp;
	}
}

/// Get item by string key for map type.
impl ::std::ops::Index<&'static str> for Value {
	type Output = Value;
	fn index(&self, key: &'static str) -> &Self::Output {
		let tmp = self.ensure_tmp_mut();
		(_API.ValueGetValueOfKey)(self.as_cptr(), Value::from(key).as_cptr(), tmp.as_ptr());
		return tmp;
	}
}

/// Set item by key for map type.
#[cfg(notworking)]
impl ::std::ops::IndexMut<Value> for Value {
	fn index_mut<'a>(&'a mut self, key: Value) -> &'a mut Value {
		let ptr = self.as_ptr();
		let tmp = self.ensure_tmp();
		(_API.ValueSetValueToKey)(ptr, key.as_cptr(), tmp.as_ptr());
		return tmp;
	}
}

/// Value from nothing (`()`) for empty return values.
///
/// Returns `undefined` value.
impl From<()> for Value {
	fn from(_: ()) -> Self {
		Value::new()
	}
}

/// Value from a native `VALUE` object.
impl<'a> From<&'a VALUE> for Value {
  fn from(val: &'a VALUE) -> Self {
    unsafe { Value::copy_from(val as *const _) }
  }
}

/// Value from integer.
impl From<i32> for Value {
	// Note that there is no generic 64-bit integers at Sciter, only Date/Currency types.
	// There is a double (f64) for large numbers as a workaround.
	fn from(val: i32) -> Self {
		let mut me = Value::new();
		(_API.ValueIntDataSet)(me.as_ptr(), val, VALUE_TYPE::T_INT as UINT, 0);
		return me;
	}
}

/// Value from integer.
impl From<&i32> for Value {
	// Note that there is no generic 64-bit integers at Sciter, only Date/Currency types.
	// There is a double (f64) for large numbers as a workaround.
	fn from(val: &i32) -> Self {
		let mut me = Value::new();
		(_API.ValueIntDataSet)(me.as_ptr(), *val, VALUE_TYPE::T_INT as UINT, 0);
		return me;
	}
}

/// Value from float.
impl From<f64> for Value {
	fn from(val: f64) -> Self {
		let mut me = Value::new();
		(_API.ValueFloatDataSet)(me.as_ptr(), val, VALUE_TYPE::T_FLOAT as UINT, 0);
		return me;
	}
}

/// Value from float.
impl From<&f64> for Value {
	fn from(val: &f64) -> Self {
		let mut me = Value::new();
		(_API.ValueFloatDataSet)(me.as_ptr(), *val, VALUE_TYPE::T_FLOAT as UINT, 0);
		return me;
	}
}

/// Value from bool.
impl From<bool> for Value {
	fn from(val: bool) -> Self {
		let mut me = Value::new();
		(_API.ValueIntDataSet)(me.as_ptr(), val as INT, VALUE_TYPE::T_BOOL as UINT, 0);
		return me;
	}
}

/// Value from bool.
impl From<&bool> for Value {
	fn from(val: &bool) -> Self {
		let mut me = Value::new();
		(_API.ValueIntDataSet)(me.as_ptr(), *val as INT, VALUE_TYPE::T_BOOL as UINT, 0);
		return me;
	}
}

/// Value from string.
impl<'a> From<&'a str> for Value {
	fn from(val: &'a str) -> Self {
		let mut me = Value::new();
		me.assign_str(val, VALUE_UNIT_TYPE_STRING::STRING);
		return me;
	}
}

/// Value from string.
impl From<String> for Value {
	fn from(val: String) -> Self {
		let mut me = Value::new();
		me.assign_str(&val, VALUE_UNIT_TYPE_STRING::STRING);
		return me;
	}
}

impl<'a> From<&'a String> for Value {
	fn from(val: &'a String) -> Self {
		let mut me = Value::new();
		me.assign_str(val, VALUE_UNIT_TYPE_STRING::STRING);
		return me;
	}
}

/// Value from json string.
impl ::std::str::FromStr for Value {
	type Err = VALUE_RESULT;
	fn from_str(val: &str) -> Result<Self, Self::Err> {
		Value::parse(val).or(Err(VALUE_RESULT::BAD_PARAMETER))
	}
}

/// Value from binary array (sequence of bytes).
impl<'a> From<&'a [u8]> for Value {
	fn from(val: &'a [u8]) -> Self {
		let mut me = Value::new();
		(_API.ValueBinaryDataSet)(me.as_ptr(), val.as_ptr(), val.len() as UINT, VALUE_TYPE::T_BYTES as UINT, 0);
		return me;
	}
}

impl<T, E> From<Result<T, E>> for Value where T: Into<Value>, E: std::fmt::Display {
	fn from(val: Result<T, E>) -> Self {
		match val {
			Ok(v) => v.into(),
			Err(e) => Value::error(&e.to_string()),
		}
	}
}

// /// Value from sequence of items satisfying `Into<Value>`.
// impl ::std::iter::FromIterator<Into<Value>> for Value {
//   fn from_iter<I: IntoIterator<Item=Into<Value>>>(iterator: I) -> Self {
//     let iterator = iterator.into_iter();
//     let capacity = iterator.size_hint().0;
//     let mut v = Value::array(capacity);
//     for i in iterator {
//       v.push(Value::from(i));
//     }
//     return v;
//   }
// }

/// Value from sequence of `Value`.
impl ::std::iter::FromIterator<Value> for Value {
  fn from_iter<I: IntoIterator<Item=Value>>(iterator: I) -> Self {
    let iterator = iterator.into_iter();
    let capacity = iterator.size_hint().0;
    let mut v = Value::array(capacity);
    for (i, m) in iterator.enumerate() {
      v.set(i, m);
    }
    return v;
  }
}

/// Value from sequence of `i32`.
impl ::std::iter::FromIterator<i32> for Value {
	fn from_iter<I: IntoIterator<Item=i32>>(iterator: I) -> Self {
    let iterator = iterator.into_iter();
    let capacity = iterator.size_hint().0;
		let mut v = Value::array(capacity);
    for (i, m) in iterator.enumerate() {
      v.set(i, Value::from(m));
		}
		return v;
	}
}

/// Value from sequence of `f64`.
impl ::std::iter::FromIterator<f64> for Value {
	fn from_iter<I: IntoIterator<Item=f64>>(iterator: I) -> Self {
    let iterator = iterator.into_iter();
    let capacity = iterator.size_hint().0;
    let mut v = Value::array(capacity);
    for (i, m) in iterator.enumerate() {
      v.set(i, Value::from(m));
		}
		return v;
	}
}

/// Value from sequence of `&str`.
impl<'a> ::std::iter::FromIterator<&'a str> for Value {
	fn from_iter<I: IntoIterator<Item=&'a str>>(iterator: I) -> Self {
    let iterator = iterator.into_iter();
    let capacity = iterator.size_hint().0;
    let mut v = Value::array(capacity);
    for (i, m) in iterator.enumerate() {
      v.set(i, Value::from(m));
		}
		return v;
	}
}

/// Value from sequence of `String`.
impl ::std::iter::FromIterator<String> for Value {
	fn from_iter<I: IntoIterator<Item=String>>(iterator: I) -> Self {
    let iterator = iterator.into_iter();
    let capacity = iterator.size_hint().0;
    let mut v = Value::array(capacity);
    for (i, m) in iterator.enumerate() {
      v.set(i, Value::from(m));
		}
		return v;
	}
}

// /// Value from function.
// impl<F> From<F> for Value
// 	where F: Fn(&[Value]) -> Value
// {
// 	fn from(f: F) -> Value {
// 		let mut v = Value::new();
// 		let boxed = Box::new(f);
// 		let ptr = Box::into_raw(boxed);	// dropped in `_functor_release`
// 		(_API.ValueNativeFunctorSet)(v.as_ptr(), _functor_invoke::<F>, _functor_release::<F>, ptr as LPVOID);
// 		return v;
// 	}
// }

/// Value from function.
impl<F, R> From<F> for Value
where
	F: Fn(&[Value]) -> R,
	R: Into<Value>,
{
	fn from(f: F) -> Value {
		let mut v = Value::new();
		let boxed = Box::new(f);
		let ptr = Box::into_raw(boxed);	// dropped in `_functor_release`
		(_API.ValueNativeFunctorSet)(v.as_ptr(), _functor_invoke::<F,R>, _functor_release::<F>, ptr as LPVOID);
		return v;
	}
}

/// Value from asset.
impl<T> From<Box<IAsset<T>>> for Value {
	fn from(ptr: Box<IAsset<T>>) -> Value {
		let ptr = Box::into_raw(ptr);
		let mut me = Value::new();
		(_API.ValueInt64DataSet)(me.as_ptr(), ptr as usize as i64, VALUE_TYPE::T_ASSET as u32, 0);
		return me;
	}
}

extern "C" fn _functor_release<F>(tag: LPVOID)
{
	// reconstruct handler from pointer
	let ptr = tag as *mut F;
	let boxed = unsafe { Box::from_raw(ptr) };
	// and forget it
	drop(boxed);
}

extern "C" fn _functor_invoke<F, R>(tag: LPVOID, argc: UINT, argv: *const VALUE, retval: *mut VALUE)
where
	F: Fn(&[Value]) -> R,
	R: Into<Value>,

{
	// reconstruct handler from pointer
	let ptr = tag as *mut F;
	let me = unsafe { &mut *ptr };
	let retval = unsafe { &mut *retval };
	let args = unsafe { Value::unpack_from(argv, argc) };
	let rv = me(&args);
	let rv: Value = rv.into();
	rv.pack_to(retval)
}


/// Helper trait
pub trait FromValue {
	/// Converts value to specified type.
	fn from_value(v: &Value) -> Option<Self> where Self: Sized;
}

impl FromValue for Value {
	fn from_value(v: &Value) -> Option<Self> {
		Some(v.clone())
	}
}

impl FromValue for bool {
	fn from_value(v: &Value) -> Option<Self> {
		v.to_bool()
	}
}

impl FromValue for i32 {
	fn from_value(v: &Value) -> Option<Self> {
		v.to_int()
	}
}

impl FromValue for f64 {
	fn from_value(v: &Value) -> Option<Self> {
		v.to_float()
	}
}

impl FromValue for Vec<u8> {
	fn from_value(v: &Value) -> Option<Self> {
		v.to_bytes()
	}
}

impl FromValue for String {
	fn from_value(v: &Value) -> Option<Self> {
		v.as_string()
	}
}


/// An iterator visiting all keys of key/value pairs in the map-like `Value` objects.
#[doc(hidden)]
pub struct KeyIterator<'a> {
	base: &'a Value,
	index: usize,
	count: usize,
}

impl<'a> ::std::iter::Iterator for KeyIterator<'a> {
	type Item = Value;

	fn next(&mut self) -> Option<Self::Item> {
		if self.index < self.count {
			self.index += 1;
			Some(self.base.key_at(self.index - 1))
		} else {
			None
		}
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		let remain = self.count - self.index;
		(remain, Some(remain))
	}

	fn count(self) -> usize {
		self.count
	}
}

/// An iterator able to yield keys from both `Value`'s ends.
impl<'a> ::std::iter::DoubleEndedIterator for KeyIterator<'a> {
	fn next_back(&mut self) -> Option<Self::Item> {
		if self.index == self.count || self.count == 0 {
			None
		} else {
			self.count -= 1;
			Some(self.base.key_at(self.count))
		}
	}
}



/// An iterator over the sub-elements of a `Value`.
#[doc(hidden)]
pub struct SeqIterator<'a> {
	base: &'a Value,
	index: usize,
	count: usize,
}

impl<'a> ::std::iter::Iterator for SeqIterator<'a> {
	type Item = Value;

	fn next(&mut self) -> Option<Self::Item> {
		if self.index < self.count {
			self.index += 1;
			Some(self.base.get(self.index - 1))
		} else {
			None
		}
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		let remain = self.count - self.index;
		(remain, Some(remain))
	}

	fn count(self) -> usize {
		self.count
	}
}

/// An iterator able to yield sub-elements from both `Value`'s ends.
impl<'a> ::std::iter::DoubleEndedIterator for SeqIterator<'a> {
	fn next_back(&mut self) -> Option<Self::Item> {
		if self.index == self.count || self.count == 0 {
			None
		} else {
			self.count -= 1;
			Some(self.base.get(self.count))
		}
	}
}

/// Conversion into an `Iterator`.
///
/// Adds the `for` loop syntax support: `for subitem in &value {]`.
impl<'a> ::std::iter::IntoIterator for &'a Value {
	type Item = Value;
	type IntoIter = SeqIterator<'a>;

	fn into_iter(self) -> Self::IntoIter {
		self.values()
	}
}




#[cfg(test)]
mod tests {
	#![allow(unused_imports)]

	use super::{Value, FromValue};
	use capi::scvalue::{VALUE, VALUE_TYPE};
	use std::mem;
	use ::{_API};

	fn check1(a: i32) {
		assert_eq!(a, 12);
	}

	#[test]
	fn test_from_value() {
		let v = Value::from(12);
		check1(
			match FromValue::from_value(&v) {
				Some(x) => { x },
				None => { return; }
			}
		);
	}

	#[test]
	fn test_value_layout() {
		assert_eq!(mem::size_of::<VALUE_TYPE>(), 4);
		assert_eq!(mem::size_of::<VALUE>(), 16);
	}

	#[test]
	fn test_abi() {

		let mut data = VALUE { t: VALUE_TYPE::T_UNDEFINED, u: 0, d: 0 };
		assert_eq!(data.t, VALUE_TYPE::T_UNDEFINED);

		(_API.ValueInit)(&mut data);
		assert_eq!(data.t, VALUE_TYPE::T_UNDEFINED);

		(_API.ValueClear)(&mut data);
		assert_eq!(data.t, VALUE_TYPE::T_UNDEFINED);
	}
}
