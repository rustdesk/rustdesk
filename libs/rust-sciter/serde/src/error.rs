use std;
use std::fmt::{self, Display};

use serde::{ser, de};


/// Result type for serialization.
pub type Result<T> = std::result::Result<T, Error>;

/// Error type for serialization.
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
	Message(String),
	Unimplemented,
	UnsupportedType,
	ExpectedType(String),
}

impl ser::Error for Error {
	fn custom<T: Display>(msg: T) -> Self {
		Error::Message(msg.to_string())
	}
}

impl de::Error for Error {
	fn custom<T: Display>(msg: T) -> Self {
		Error::Message(msg.to_string())
	}
}

impl std::error::Error for Error {
	fn description(&self) -> &str {
		match *self {
			Error::Message(ref msg) => msg,
			Error::ExpectedType(ref msg) => msg,
			Error::Unimplemented => "unimplemented",
			Error::UnsupportedType => "unsupported",
		}
	}
}

impl Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Error::Message(ref msg) => write!(f, "error: {}", msg),
			Error::ExpectedType(ref msg) => write!(f, "expected: {}", msg),
			Error::UnsupportedType => write!(f, "unsupported type"),
			Error::Unimplemented => write!(f, "unimplemented"),
		}
	}
}
