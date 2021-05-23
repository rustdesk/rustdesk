//! Sciter value, native C interface.

#![allow(non_snake_case, non_camel_case_types)]
#![allow(dead_code)]

use capi::sctypes::*;

/// A JSON value.
///
/// An opaque union that can hold different types of values: numbers, strings, arrays, objects, etc.
#[repr(C)]
#[derive(Debug, Clone)]
pub struct VALUE
{
	/// Value type.
	pub t: VALUE_TYPE,

	/// Value unit type.
	pub u: UINT,

	/// Value data.
	pub d: UINT64,
}

impl Default for VALUE {
	fn default() -> Self {
		VALUE { t: VALUE_TYPE::T_UNDEFINED, u: 0, d: 0 }
	}
}

#[repr(C)]
#[derive(Debug, PartialOrd, PartialEq)]
pub enum VALUE_RESULT
{
  OK_TRUE = -1,
  OK = 0,
  BAD_PARAMETER = 1,
  INCOMPATIBLE_TYPE = 2,
}

impl std::error::Error for VALUE_RESULT {}

impl std::fmt::Display for VALUE_RESULT {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		write!(f, "{:?}", self)
	}
}


#[repr(C)]
#[derive(Debug, PartialOrd, PartialEq)]
pub enum VALUE_STRING_CVT_TYPE {
	SIMPLE = 0,
	JSON_LITERAL = 1,
	JSON_MAP = 2,
	XJSON_LITERAL = 3,
}


/// Type identifier of the value.
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialOrd, PartialEq)]
pub enum VALUE_TYPE {
	T_UNDEFINED = 0,
	T_NULL = 1,
	T_BOOL,
	T_INT,
	T_FLOAT,
	T_STRING,
	T_DATE,
	T_CURRENCY,
	T_LENGTH,
	T_ARRAY,
	T_MAP,
	T_FUNCTION,
	T_BYTES,
	T_OBJECT,
	T_DOM_OBJECT,
	T_RESOURCE,
	T_RANGE,
	T_DURATION,
	T_ANGLE,
	T_COLOR,
	T_ENUM,
	T_ASSET,

	T_UNKNOWN,
}

#[repr(C)]
#[derive(Debug, PartialOrd, PartialEq)]
pub enum VALUE_UNIT_UNDEFINED
{
	UT_NOTHING = 1,
}

#[repr(C)]
#[derive(Debug, PartialOrd, PartialEq)]
pub enum VALUE_UNIT_TYPE_STRING
{
	STRING = 0,        // string
	ERROR  = 1,        // is an error string
	SECURE = 2,        // secure string ("wiped" on destroy)
	URL 	 = 3,				 // url(...)
	SELECTOR = 4,			 // selector(...)
	FILE = 0xfffe,     // file name
	SYMBOL = 0xffff,   // symbol in tiscript sense
}

// Sciter or TIScript specific
#[repr(C)]
#[derive(Debug, PartialOrd, PartialEq)]
pub enum VALUE_UNIT_TYPE_OBJECT
{
	ARRAY  = 0,   // type T_OBJECT of type Array
	OBJECT = 1,   // type T_OBJECT of type Object
	CLASS  = 2,   // type T_OBJECT of type Class (class or namespace)
	NATIVE = 3,   // type T_OBJECT of native Type with data slot (LPVOID)
	FUNCTION = 4, // type T_OBJECT of type Function
	ERROR = 5,    // type T_OBJECT of type Error
}

pub type NATIVE_FUNCTOR_INVOKE = extern "C" fn (tag: LPVOID, argc: UINT, argv: *const VALUE, retval: * mut VALUE);
pub type NATIVE_FUNCTOR_RELEASE = extern "C" fn (tag: LPVOID);
