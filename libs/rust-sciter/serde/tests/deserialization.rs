#![allow(unused_variables)]

extern crate sciter;
extern crate sciter_serde;

#[macro_use]
extern crate serde_derive;
extern crate serde_bytes;
extern crate serde;

use sciter::{Value};
use sciter_serde::{from_value, to_value};


#[test]
fn basic_types() {
	// bool
	let v: bool = from_value(&Value::from(true)).unwrap();
	assert_eq!(v, true);

	// integer types
	let v: i32 = from_value(&Value::from(0)).unwrap();
	assert_eq!(v, 0);

	let v: i32 = from_value(&Value::from(7i32)).unwrap();
	assert_eq!(v, 7i32);

	// float
	let v: f32 = from_value(&Value::from(7.0)).unwrap();
	assert_eq!(v, 7.0);

	let v: f64 = from_value(&Value::from(7.0)).unwrap();
	assert_eq!(v, 7.0);

	// Option
	let v = Value::null();
	let v: Option<i32> = from_value(&v).unwrap();
	assert_eq!(v, None);

	let v = Value::from(7);
	let v: Option<i32> = from_value(&v).unwrap();
	assert_eq!(v, Some(7));
}

#[test]
fn strings() {
	let v: char = from_value(&Value::from("7")).unwrap();
	assert_eq!(v, '7');

	let v: String = from_value(&Value::from("7")).unwrap();
	assert_eq!(v, "7");

	let v: serde_bytes::ByteBuf = from_value(&Value::from(b"hello".as_ref())).unwrap();
	let v: &[u8] = &v;
	assert_eq!(v, b"hello".as_ref());
}

#[test]
fn arrays() {
	let it = [1,2,3].iter();
	let v: Value = it.cloned().collect();
	let v: Vec<i32> = from_value(&v).unwrap();
	assert_eq!(v, &[1,2,3]);
}

#[test]
fn structs() {
	#[derive(Serialize, Deserialize, PartialEq, Debug)]
	struct Test {
		int: u32,
		seq: Vec<String>,
	}

	println!("");

	let a = Test { int: 7, seq: vec!["a".to_owned(), "b".to_owned()]};

	let v: Value = to_value(&a).unwrap();
	println!("serialized Test:\n  {:?}", v);

	println!("keys:");
	v.keys().inspect(|i| println!("  {:?}", i)).count();

	println!("values:");
	v.values().inspect(|i| println!("  {:?}", i)).count();

	println!("items:");
	v.items().iter().inspect(|i| println!("  {:?}", i)).count();

	let e: Test = from_value(&v).unwrap();
	println!("deserialized Test:\n  {:?}", e);

	assert_eq!(a, e);
}
