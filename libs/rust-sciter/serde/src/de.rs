/// Deserialization.
use serde::de::{self, Deserialize, Visitor};

use error::{Error, Result};
use sciter::{Value};


/// Deserializes a Sciter value to the specific Rust type.
pub fn from_value<'a, T>(input: &'a Value) -> Result<T>
	where T: Deserialize<'a>
{
	let p = Deserializer::from_value(input.clone());
	T::deserialize(p)
}


/// Implementation of deserialization.
pub struct Deserializer {
	input: Value,
}


impl<'de> Deserializer {

	pub fn from_value(input: Value) -> Self {
		Deserializer { input: input }
	}
}


impl<'de, 'a> ::serde::de::Deserializer<'de> for Deserializer {
	type Error = Error;


	fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value>
	{
		use sciter::value::VALUE_TYPE;
		match self.input.get_type() {
			VALUE_TYPE::T_UNDEFINED|VALUE_TYPE::T_NULL => visitor.visit_none(),
			VALUE_TYPE::T_BOOL => visitor.visit_bool(self.input.to_bool().unwrap()),
			VALUE_TYPE::T_INT => visitor.visit_i32(self.input.to_int().unwrap()),
			VALUE_TYPE::T_FLOAT => visitor.visit_f64(self.input.to_float().unwrap()),
			VALUE_TYPE::T_STRING => visitor.visit_str(&self.input.as_string().unwrap()),
			VALUE_TYPE::T_ARRAY => visitor.visit_seq(SeqAccess::new(self)),
			VALUE_TYPE::T_MAP => self.deserialize_map(visitor),
			VALUE_TYPE::T_BYTES => visitor.visit_bytes(self.input.as_bytes().unwrap()),
			VALUE_TYPE::T_OBJECT => self.deserialize_map(visitor),
			_ => Err(Error::UnsupportedType),
		}
	}

	fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value>	{
		self.deserialize_any(visitor)
	}

	fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
		if let Some(v) = self.input.to_bool() {
			visitor.visit_bool(v)
		} else {
			Err(Error::ExpectedType(format!("expected {:?}, given {:?}", "bool", self.input)))
		}
	}

	forward_to_deserialize_any! {
		i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string bytes	byte_buf
	}

	fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
		if self.input.is_undefined() || self.input.is_null() {
			visitor.visit_none()
		} else {
			visitor.visit_some(self)
		}
	}

	fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
		if self.input.is_undefined() || self.input.is_null() {
			visitor.visit_unit()
		} else {
			Err(Error::ExpectedType(format!("expected {:?}, given {:?}", "null", self.input)))
		}
	}

	fn deserialize_unit_struct<V: Visitor<'de>>(self, _name: &'static str, visitor: V) -> Result<V::Value> {
		self.deserialize_unit(visitor)
	}

	fn deserialize_newtype_struct<V: Visitor<'de>>(self, _name: &str, visitor: V) -> Result<V::Value> {
		visitor.visit_newtype_struct(self)
	}

	fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value>	{
		if self.input.is_array() {
			let it = self.input.values();
			let sq = de::value::SeqDeserializer::new(it);
			visitor.visit_seq(sq)
		} else {
			Err(Error::ExpectedType(format!("expected {:?}, given {:?}", "sequence", self.input)))
		}
	}

	fn deserialize_tuple<V: Visitor<'de>>(self, _len: usize, visitor: V) -> Result<V::Value> {
		self.deserialize_seq(visitor)
	}

	fn deserialize_tuple_struct<V: Visitor<'de>>(self, _name: &'static str, _len: usize, visitor: V) -> Result<V::Value> {
		self.deserialize_seq(visitor)
	}

	fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
		if self.input.is_map() {
			let it = self.input.items().into_iter();
			let sq = de::value::MapDeserializer::new(it);
			visitor.visit_map(sq)
		} else {
			Err(Error::ExpectedType(format!("expected {:?}, given {:?}", "map", self.input)))
		}
	}

	fn deserialize_struct<V: Visitor<'de>>(self, _name: &'static str, _fields: &'static [&'static str], visitor: V) -> Result<V::Value>
	{
		self.deserialize_map(visitor)
	}

	fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value>
	{
		self.deserialize_str(visitor)
	}

	fn deserialize_enum<V: Visitor<'de>>(self, _name: &'static str, _fields: &'static [&'static str], visitor: V) -> Result<V::Value>
	{
		// it can be `"A"`, `{"T": u8}`, `{S: {"x": u8}}`
		match (self.input.is_string(), self.input.is_map()) {

			(true, _) => {
				use self::de::IntoDeserializer;
				visitor.visit_enum(self.input.as_string().unwrap().into_deserializer())
			},

			(_, true) => {
				visitor.visit_enum(SeqAccess::new(self))
			},

			_ => {
				Err(Error::ExpectedType(format!("expected enum (as string or map), given {:?}", self.input)))
			}
		}
	}
}


impl<'de> de::IntoDeserializer<'de, Error> for Value {
	type Deserializer = Deserializer;

	fn into_deserializer(self) -> Self::Deserializer {
		Deserializer::from_value(self)
	}
}


#[doc(hidden)]
struct SeqAccess {
	de: Deserializer,
	pos: usize,
	len: usize,
	key: Option<Value>,
}

impl SeqAccess {
	fn new(d: Deserializer) -> Self	{
		let len = d.input.len();
		SeqAccess {
			de: d,
			pos: 0,
			len: len,
			key: None,
		}
	}
}

impl<'de> de::SeqAccess<'de> for SeqAccess {
	type Error = Error;

	fn size_hint(&self) -> Option<usize> {
		Some(self.len)
	}

	fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
		where T: de::DeserializeSeed<'de>
	{
		if self.pos < self.len {
			self.pos += 1;
			let v = self.de.input.get(self.pos - 1);
			let inner = Deserializer::from_value(v);
			seed.deserialize(inner).map(Some)
		} else {
			Ok(None)
		}
	}
}

impl<'de> de::MapAccess<'de> for SeqAccess {
	type Error = Error;

	fn size_hint(&self) -> Option<usize> {
		Some(self.len)
	}

	fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
		where K: de::DeserializeSeed<'de>
	{
		if self.pos < self.len {
			self.pos += 1;
			let v = self.de.input.key_at(self.pos - 1);
			let inner = Deserializer::from_value(v);
			seed.deserialize(inner).map(Some)
		} else {
			Ok(None)
		}
	}

	fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
		where V: de::DeserializeSeed<'de>
	{
		let v = self.de.input.get(self.pos - 1);
		let inner = Deserializer::from_value(v);
		seed.deserialize(inner)
	}
}

impl<'de> de::EnumAccess<'de> for SeqAccess {
	type Error = Error;
	type Variant = Self;

	fn variant_seed<V>(mut self, seed: V) -> Result<(V::Value, Self::Variant)>
		where V: de::DeserializeSeed<'de>
	{
		// `{ "N": ... }`
		// Here I suppose to deserialize the variant key.
		let v = self.de.input.key_at(0);
		self.key = Some(v.clone());
		let vkey = seed.deserialize( Deserializer::from_value(v) )?;
		Ok((vkey, self))
	}
}

impl<'de> de::VariantAccess<'de> for SeqAccess {
	type Error = Error;

	fn unit_variant(self) -> Result<()> {
		de::Deserialize::deserialize(self.de)
	}

	fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value>
		where T: de::DeserializeSeed<'de>
	{
		// `{ "N": u8 }`
		let v = self.de.input.get_item(self.key.unwrap());
		seed.deserialize( Deserializer::from_value(v) )
	}

	fn tuple_variant<V>(self, len: usize, visitor: V) -> Result<V::Value>
		where V: de::Visitor<'de>
	{
		// `{ "T": [u8, u8] }`
		let v = self.de.input.get_item(self.key.unwrap());
		de::Deserializer::deserialize_tuple(Deserializer::from_value(v), len, visitor)
	}

	fn struct_variant<V>(self, fields: &'static [&'static str], visitor: V) -> Result<V::Value>
		where V: de::Visitor<'de>
	{
		// `{ "S": {r: u8, g: u8, b: u8} }`
		let v = self.de.input.get_item(self.key.unwrap());
		de::Deserializer::deserialize_struct(Deserializer::from_value(v), "", fields, visitor)
	}

}
