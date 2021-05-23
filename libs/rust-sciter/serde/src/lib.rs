// This component uses Sciter Engine,
// copyright Terra Informatica Software, Inc.
// (http://terrainformatica.com/).

/*!

[Serde](https://docs.rs/serde) support for [Sciter](https://docs.rs/sciter-rs) engine.

While technically you could just use the `serde_json` crate and perform serialization via
an intermediate string (something like `sciter::Value::from_str(&serde_json::to_string(<your data>)?)?`),
you can also use direct serialization between your data and `sciter::Value`.

## Supported types of Sciter value

+ Bool (`bool`)
+ Integer (`i8`-`i32`)
+	Float (`f32`-`f64`)
+ String (`&str`, `String`)
+ Bytes (`&[u8]`)
+ Array (`&[T]`, `Vec<T>`)
+ Object (key-value mapping like `struct` or `HashMap`, `BTreeMap`, etc.)

Unsupported:

- Date
- Currency
- Length
- Range
- Duration
- Angle
- Color

## Supported types of the Serde data model

* [x] `bool`
* [x] integer types except the following:
* [-] `i64`/`u64` - 64-bit integers stored as `f64` in Sciter
* [x] strings
* [x] byte arrays
* [x] option
* [x] unit (stored as `null`)
* [x] unit struct (stored as `null`)
* [x] unit variant (aka `enum`, stored just as enum index of `i32` type)
* [x] newtype struct (aka `struct Io(u32)`, stored as underlaying value)
* [-] newtype variant
* [x] seq, like vector (stored as array)
* [x] tuple (stored as array)
* [x] tuple struct (stored as array)
* [-] tuple variant
* [x] map (stored as map)
* [x] struct (stored as map)
* [-] struct variant

See the [Serde data model](https://serde.rs/data-model.html) for reference.

# Examples

```rust
extern crate sciter;
extern crate sciter_serde;

use sciter::Value;
use sciter_serde::{from_value, to_value};

fn back_and_forth() {
	let v: Value = to_value(&true).unwrap();
	let b: bool = from_value(&v).unwrap();
	assert_eq!(b, true);
}

fn main() {

	// bool
	let v: Value = to_value(&true).unwrap();
	assert!(v.is_bool());
	assert_eq!(v, Value::from(true));

	// numbers
	let v = to_value(&12u32).unwrap();
	assert_eq!(v, 12.into());

	let v = to_value(& 42.0f64).unwrap();
	assert_eq!(v, 42.0f64.into());

	// strings
	let v = to_value("hello").unwrap();
	assert_eq!(v, "hello".into());

	// arrays
	let a = [1,2,3];
	let v = to_value(&a).unwrap();
	assert_eq!(v, a.iter().cloned().collect());

	// maps
	let m = {
		use std::collections::BTreeMap;
		let mut m = BTreeMap::new();
		m.insert("17", 17);
		m.insert("42", 42);
		m
	};
	let v = to_value(&m).unwrap();
	assert_eq!(v, Value::parse(r#"{ "17": 17, "42": 42 }"#).unwrap());
}
```

With derived serialization:

```rust
# #![doc(test(no_crate_inject))]
#[macro_use]
extern crate serde_derive;
extern crate serde;

extern crate sciter;
extern crate sciter_serde;

use sciter::Value;
use sciter_serde::to_value;

fn main() {

	// structs
	#[derive(Serialize)]
	struct Test {
		x: i32,
		y: i32,
	}

	let v = to_value(&Test {x: 1, y: 2}).unwrap();
	assert_eq!(v, Value::parse(r#"{ "x": 1, "y": 2 }"#).unwrap());
}

```

*/
#![allow(clippy::redundant_field_names)]
#![allow(clippy::tabs_in_doc_comments)]

#[macro_use]
extern crate serde;
extern crate sciter;


mod error;
mod ser;
mod de;

#[doc(inline)]
pub use ser::to_value;

#[doc(inline)]
pub use de::from_value;

pub use error::{Result, Error};
