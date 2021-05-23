#![allow(unused_variables)]

extern crate sciter;
extern crate sciter_serde;

#[macro_use]
extern crate serde_derive;
extern crate serde_bytes;
extern crate serde;

use sciter::{Value};
use sciter_serde::{to_value};


#[test]
fn basic_types() {
	// bool
	let v = to_value(&true).unwrap();
	assert!(v.is_bool());
	assert_eq!(v, Value::from(true));

	// integer types
	let v = to_value(&0).unwrap();
	assert!(v.is_int());
	assert_eq!(v.to_int(), Some(0));

	let v = to_value(&7u8).unwrap();
	assert_eq!(v, Value::from(7));

	let v = to_value(&7u16).unwrap();
	assert_eq!(v, Value::from(7));

	let v = to_value(&7u32).unwrap();
	assert_eq!(v, Value::from(7));

	let v = to_value(&7i8).unwrap();
	assert_eq!(v, Value::from(7));

	let v = to_value(&7i16).unwrap();
	assert_eq!(v, Value::from(7));

	let v = to_value(&7i32).unwrap();
	assert_eq!(v, Value::from(7));

	let v = to_value(&7.0).unwrap();
	assert!(v.is_float());

	// 64-bit
	// let v = to_value(&7u64).unwrap();
	// assert!(v.is_float());
	// assert_eq!(v, Value::from(7.0));

	// Option
	// let v = to_value(&Some(7)).unwrap();
	// assert!(v.is_int());

	// let v = to_value(&None).unwrap();
	// assert!(v.is_null());
}

#[test]
fn strings() {
	// strings
	let v = to_value(&'h').unwrap();
	assert!(v.is_string());
	assert_eq!(v, Value::from("h"));

	let v = to_value("hello").unwrap();
	assert!(v.is_string());
	assert_eq!(v, Value::from("hello"));

	// doesn't work because Rust doesn't have specialization yet (https://github.com/rust-lang/rust#31844)
	// let v = to_value(b"hello").unwrap();
	// println!("b'hello': {:?}", v);
	// assert!(v.is_bytes());
	// assert_eq!(v.as_bytes(), Some(b"hello".as_ref()));

	use serde_bytes::Bytes;

	let v = to_value(&Bytes::new(b"hello")).unwrap();
	assert!(v.is_bytes());
	assert_eq!(v.as_bytes(), Some(b"hello".as_ref()));
}

#[test]
fn arrays() {
	let a = [1,2,3];
	let v = to_value(&a).unwrap();
	assert!(v.is_array());
	assert_eq!(v.len(), a.len());

	let a = vec![1,2,3];
	let v = to_value(&a).unwrap();
	assert!(v.is_array());
	assert_eq!(v.len(), a.len());
}

#[test]
fn structs() {

	#[derive(Serialize)]
	struct Test {
		int: u32,
		seq: Vec<&'static str>,
	}

	let a = Test { int: 7, seq: vec!["a", "b"]};
	let v = to_value(&a).unwrap();
	assert!(v.is_map());
	assert_eq!(v.len(), 2);
	assert_eq!(v.get_item("int"), Value::from(7) );
	assert_eq!(v.get_item("seq").len(), 2);
}
