/// Serialization.
use serde::ser::{self, Serialize};

use error::{Error, Result};
use sciter::{Value};


/// Serialize the given data structure into Sciter value.
pub fn to_value<T: ?Sized + Serialize>(value: &T) -> Result<Value> {
	let mut p = Serializer { output: Value::new() };
	value.serialize(&mut p)?;
	Ok(p.output)
}

/// Implementation of serialization.
pub struct Serializer {
	output: Value,
}

// Helper structure for serialization of sequence data types (array, map, tuple ans so on).
#[doc(hidden)]
pub struct SeqSerializer<'a> {
	ser: &'a mut Serializer,
	output: Value,
	key: Option<Value>,
	outer: Option<Value>,
}

impl<'a> SeqSerializer<'a> {
	fn typed(ser: &'a mut Serializer, typed: Value) -> Self {
		SeqSerializer {
			ser: ser,
			output: typed,
			key: None,
			outer: None,
		}
	}

	fn with_outer(ser: &'a mut Serializer, outer: Value, typed: Value) -> Self {
		SeqSerializer {
			ser: ser,
			output: typed,
			key: None,
			outer: Some(outer),
		}
	}
}

// serde traits implementation

impl<'a> ser::SerializeSeq for SeqSerializer<'a> {
	type Ok = ();
	type Error = Error;

	fn end(self) -> Result<()> {
		self.ser.output = self.output;
		Ok(())
	}

	fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<()> {
		let dst = to_value(value)?;
		self.output.push(dst);
		Ok(())
	}
}

impl<'a> ser::SerializeMap for SeqSerializer<'a> {
	type Ok = ();
	type Error = Error;

	fn end(self) -> Result<()> {
		ser::SerializeSeq::end(self)
	}

	fn serialize_entry<K, V>(&mut self, key: &K, value: &V) -> Result<()> where K: ?Sized + Serialize, V: ?Sized + Serialize {
		self.output.set_item(to_value(key)?, to_value(value)?);
		Ok(())
	}

	fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> Result<()> {
		self.key = Some(to_value(key)?);
		Ok(())
	}

	fn serialize_value<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<()> {
		let key = self.key.take();
		self.output.set_item(key.unwrap(), to_value(value)?);
		Ok(())
	}
}

impl<'a> ser::SerializeStruct for SeqSerializer<'a> {
	type Ok = ();
	type Error = Error;

	fn end(self) -> Result<()> {
		ser::SerializeSeq::end(self)
	}

	fn serialize_field<T: ?Sized + Serialize>(&mut self, key: &'static str, value: &T) -> Result<()> {
		self.output.set_item(key, to_value(value)?);
		Ok(())
	}
}

impl<'a> ser::SerializeStructVariant for SeqSerializer<'a> {
	type Ok = ();
	type Error = Error;

	fn end(self) -> Result<()> {
		// self.output: map
		// self.outer: left key
		let mut result = Value::new();
		result.set_item(self.outer.unwrap(), self.output);
		self.ser.output = result;
		Ok(())
	}

	fn serialize_field<T: ?Sized + Serialize>(&mut self, key: &'static str, value: &T) -> Result<()> {
		self.output.set_item(key, to_value(value)?);
		Ok(())
	}
}

impl<'a> ser::SerializeTuple for SeqSerializer<'a> {
	type Ok = ();
	type Error = Error;

	fn end(self) -> Result<()> {
		ser::SerializeSeq::end(self)
	}

	fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<()> {
		ser::SerializeSeq::serialize_element(self, value)
	}
}

impl<'a> ser::SerializeTupleStruct for SeqSerializer<'a> {
	type Ok = ();
	type Error = Error;

	fn end(self) -> Result<()> {
		ser::SerializeSeq::end(self)
	}

	fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<()> {
		ser::SerializeSeq::serialize_element(self, value)
	}
}

impl<'a> ser::SerializeTupleVariant for SeqSerializer<'a> {
	type Ok = ();
	type Error = Error;

	fn end(self) -> Result<()> {
		// self.output: array
		// self.outer: left key
		let mut result = Value::new();
		result.set_item(self.outer.unwrap(), self.output);
		self.ser.output = result;
		Ok(())
	}

	fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<()> {
		ser::SerializeSeq::serialize_element(self, value)
	}
}


impl<'a> ser::Serializer for &'a mut Serializer {
	type Ok = ();
	type Error = Error;

  type SerializeSeq = SeqSerializer<'a>;
  type SerializeTuple = SeqSerializer<'a>;
  type SerializeTupleStruct = SeqSerializer<'a>;
  type SerializeTupleVariant = SeqSerializer<'a>;
  type SerializeMap = SeqSerializer<'a>;
  type SerializeStruct = SeqSerializer<'a>;
  type SerializeStructVariant = SeqSerializer<'a>;


  fn serialize_bool(self, v: bool) -> Result<()> {
  	self.output = v.into();
  	Ok(())
  }

  fn serialize_i8(self, v: i8) -> Result<()> {
  	self.serialize_i32(v as i32)
  }

  fn serialize_i16(self, v: i16) -> Result<()> {
  	self.serialize_i32(v as i32)
  }

  fn serialize_i32(self, v: i32) -> Result<()> {
  	self.output = v.into();
  	Ok(())
  }

  fn serialize_u8(self, v: u8) -> Result<()> {
  	self.serialize_i32(v as i32)
  }

  fn serialize_u16(self, v: u16) -> Result<()> {
  	self.serialize_i32(v as i32)
  }

  fn serialize_u32(self, v: u32) -> Result<()> {
  	if v <= i32::max_value() as u32 {
  		self.serialize_i32(v as i32)
  	} else {
  		self.serialize_f64(v as f64)
  	}
  }

  fn serialize_i64(self, _v: i64) -> Result<()> {
  	Err(Error::UnsupportedType)
  }

  fn serialize_u64(self, _v: u64) -> Result<()> {
  	Err(Error::UnsupportedType)
  }

  // Float values.
  fn serialize_f32(self, v: f32) -> Result<()> {
  	self.serialize_f64(v as f64)
  }

  fn serialize_f64(self, v: f64) -> Result<()> {
  	self.output = v.into();
  	Ok(())
  }

  // A single character is passed as a string.
  fn serialize_char(self, v: char) -> Result<()> {
  	self.serialize_str(&v.to_string())
  }

  // String itself.
  fn serialize_str(self, v: &str) -> Result<()> {
  	self.output = v.into();
  	Ok(())
  }

  // Binary bytes.
  fn serialize_bytes(self, v: &[u8]) -> Result<()> {
  	self.output = Value::from(v);
  	Ok(())
  }

  // A `None` value of `Option` type in Rust.
  fn serialize_none(self) -> Result<()> {
  	self.output = Value::null();
  	Ok(())
  }

  // Some value of `Option` type in Rust.
  fn serialize_some<T>(self, v: &T) -> Result<()> where T: ?Sized + Serialize {
  	v.serialize(self)
  }

  // The type of `()` in Rust.
  fn serialize_unit(self) -> Result<()> {
  	self.serialize_none()
  }

  // A named value containing no data, like `struct Unit;`.
  fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
  	self.serialize_none()
  }

  // Enums. Serialized as an externally tagged enum representation,
  // see https://serde.rs/enum-representations.html.

  // A unit variant of enum, like `E::A` of `enum E { A, B }`.
  fn serialize_unit_variant(self, _name: &'static str, _index: u32, value: &'static str)
  	-> Result<()>
  {
  	// `"A"`
  	self.serialize_str(value)
  }

  // For example the `E::N` in `enum E { N(u8) }`.
  fn serialize_newtype_variant<T>(self, _name: &'static str, _index: u32, variant: &'static str, value: &T)
  	-> Result<()> where T: ?Sized + Serialize
  {
  	// `{ "N": u8 }`
  	self.output.set_item(to_value(variant)?, to_value(value)?);
  	Ok(())
  }

  // For example the `E::T` in `enum E { T(u8, u8) }`.
  fn serialize_tuple_variant(self, _name: &'static str, _index: u32, value: &'static str, _len: usize)
  	 -> Result<Self::SerializeTupleVariant>
  {
  	// `{ "T": [u8, u8] }`
  	let left = to_value(value)?;
  	Ok(SeqSerializer::with_outer(self, left, Value::array(0)))
  }

  // For example the `E::S` in `enum E { S { r: u8, g: u8, b: u8 } }`.
  fn serialize_struct_variant(self, _name: &'static str, _index: u32, value: &'static str, _len: usize)
  	 -> Result<Self::SerializeStructVariant>
  {
  	// `{ "S": {r: u8, g: u8, b: u8} }`
  	let left = to_value(value)?;
  	Ok(SeqSerializer::with_outer(self, left, Value::map()))
  }

  // New-type struct, like `struct Celcius(u32)`.
  fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<()>
  	where T: ?Sized + Serialize
  {
  	// Serialize the inner itself.
  	value.serialize(self)
  }

  // A variably sized heterogeneous sequence of values, for example `Vec<T>` or `HashSet<T>`.
  fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
  	// let init = if let Some(size) = len { Value::array(size) } else { Value::new() };
  	Ok(SeqSerializer::typed(self, Value::array(0)))
  }

  // A statically sized heterogeneous sequence of values, `[u64; 10]`, `(u8,)` or `(String, u64, Vec<T>)`.
  fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple> {
  	self.serialize_seq(Some(len))
  }

  // A named tuple, for example `struct Rgb(u8, u8, u8)`.
  fn serialize_tuple_struct(self, _name: &'static str, len: usize) -> Result<Self::SerializeTupleStruct> {
  	self.serialize_seq(Some(len))
  }

  // A heterogeneous key-value pairing, for example `BTreeMap<K, V>`.
  fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
  	Ok(SeqSerializer::typed(self, Value::map()))
  }

  // A heterogeneous key-value pairing , for example `struct S { r: u8, g: u8, b: u8 }`.
  fn serialize_struct(self, _name: &'static str, len: usize) -> Result<Self::SerializeStruct> {
  	self.serialize_map(Some(len))
  }

}
