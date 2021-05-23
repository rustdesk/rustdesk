#![allow(unused_variables)]

extern crate sciter;
extern crate sciter_serde;

#[macro_use]
extern crate serde_derive;
extern crate serde_bytes;
extern crate serde;

use sciter_serde::{from_value, to_value};


// serialize, deserialize and compare with the original value.
// taken from [serde_bincode](https://github.com/TyOverby/bincode/blob/master/tests/test.rs)
fn the_same<V>(actual: V, expr: &'static str)
	where V: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug + 'static
{
	let sv = to_value(&actual).expect(&format!("to_value({})", expr));
	let dv = from_value(&sv).expect(&format!("from_value({})", expr));
	let decoded = dv;
	assert_eq!(actual, decoded, "the_same({:?})", expr);
}

macro_rules! the_same {
	($e:expr) => {
		the_same($e, stringify!($e))
	}
}

#[test]
fn basic_types() {
	the_same!(true);
	the_same!(false);

	the_same!(7i8);
	the_same!(7i16);
	the_same!(7i32);
	// the_same!(7i64); there are no 64-bit integers in Sciter, only floats.

	the_same!(7u8);
	the_same!(7u16);
	the_same!(7u32);
	// the_same!(7u64); ditto

	the_same!(7f32);
	the_same!(7f64);

	the_same!(-7i32);
	// the_same!(-7isize);


	the_same!(Box::new(7));
}

#[test]
fn strings() {
	the_same!("7".to_string());
}

#[test]
fn tuples() {
	the_same!( (1,) );
	the_same!( (1,2) );
	the_same!( (1,2,3) );

	the_same!( (1, "7".to_string(), ()) );
}

#[test]
fn structs() {

	#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
	struct Test {
		x: bool,
		y: i32,
		z: String,
	}

	#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
	struct Nested {
		inner: Test,
		payload: Option<String>,
	}

	let t = Test { x: true, y: 7, z: "42".to_string() };

	the_same!(t.clone());

	let n = Nested { inner: t.clone(), payload: Some("Some".to_string()) };
	the_same!(n.clone());

}

#[test]
fn newtypes() {
	#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
	struct Test(u32);

	the_same!(Test(7));
}

#[test]
fn newtuples() {
	#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
	struct Test(u32, bool);

	the_same!(Test(7, false));
}

#[test]
fn options() {
	the_same!(None::<bool>);
	the_same!(Some(true));
	the_same!(Some(false));
}

#[test]
fn enums() {
	#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
	enum Test {
		Zero,
		One(u32),
		Two(u32, u32),
		Three { x: u32, y: u32, z: u32 },
		Five,
	}

	the_same!(Test::Zero);
	// the_same!(Test::One(7));
	// the_same!(Test::Two(7, 7));
	// the_same!(Test::Three { x: 1, y: 2, z: 3});
	the_same!(Test::Five);
}

#[test]
fn arrays() {
	let v = [1,2,3];
	the_same!(v);

	let v = vec![1,2,3];
	the_same!(v);
}

#[test]
#[should_panic]
fn unsupported_u64() {
	the_same!(7u64);
}

#[test]
#[should_panic]
fn unsupported_i64() {
	the_same!(-7i64);
}

#[test]
#[should_panic]
fn unsupported_usize() {
	the_same!(7usize);
}

#[test]
fn newtype_variant() {
	#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
	enum Test {
		Zero,
		One(u32),
		Two(u32, u32),
		Three { x: u32, y: u32, z: u32 },
	}

	the_same!(Test::One(7));
}

#[test]
fn tuple_variant() {
	#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
	enum Test {
		Zero,
		One(u32),
		Two(u32, u32),
		Three { x: u32, y: u32, z: u32 },
	}

	the_same!(Test::Two(7, 7));
}


#[test]
fn struct_variant() {
	#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
	enum Test {
		Zero,
		One(u32),
		Two(u32, u32),
		Three { x: u32, y: u32, z: u32 },
	}

	the_same!(Test::Three { x: 1, y: 2, z: 3 });
}
