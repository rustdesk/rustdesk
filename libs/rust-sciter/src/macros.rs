//! Macros

/// Rust string to UTF-8 conversion. See also `utf::u2s`.
///
/// # Example:
///
/// ```ignore
/// let cstr = s2u!("hello"); // ffi::CString
/// libc::printf("%hs", cstr.as_ptr());
/// ```
///
#[macro_export]
macro_rules! s2u {
	($s:expr) => ( $crate::utf::s2un($s.as_ref()).0 )
}

/// Rust string to UTF-8 conversion. See also `utf::u2s`.
///
/// # Example:
///
/// ```ignore
/// let (cstr, len) = s2un!("hello"); // ffi::CString
/// libc::printf("%.*hs", len, cstr.as_ptr());
/// ```
///
#[macro_export]
macro_rules! s2un {
	($s:expr) => ( $crate::utf::s2un($s.as_ref()) )
}


/// Rust string to UTF-16 conversion. See also `utf::w2s`.
///
/// # Example:
///
/// ```ignore
/// let cwstr = s2w!("hello"); // Vec<u16>
/// libc::printf("%ws", cwstr.as_ptr());
/// ```
///
#[macro_export]
macro_rules! s2w {
	($s:expr) => ( $crate::utf::s2vec($s.as_ref()) )
}

/// Rust string to UTF-16 conversion. See also `utf::w2s`.
///
/// # Example:
///
/// ```ignore
/// let (cwstr, len) = s2wn!("hello"); // Vec<u16>
/// libc::printf("%.*ws", len, cwstr.as_ptr());
/// ```
///
#[macro_export]
macro_rules! s2wn {
	($s:expr) => ( $crate::utf::s2vecn($s.as_ref()) )
}


/// UTF-16 to `String` conversion.
#[macro_export]
macro_rules! w2s {
	($s:expr) => ( $crate::utf::w2s($s) )
}


/// UTF-8 to `String` conversion.
#[macro_export]
macro_rules! u2s {
	($s:expr) => ( $crate::utf::u2s($s) )
}


/// Pack arguments to call the Sciter script function.
#[doc(hidden)]
#[macro_export]
macro_rules! pack_args {
	() => { $crate::value::Value::pack_args(&[]) };

	( $($s:expr),* ) => {
		{
			let args = [
			$(
				$crate::value::Value::from($s)
			 ),*
			];
			$crate::value::Value::pack_args(&args)
		}
	};
}

/// Pack arguments into a `[Value]` array to call Sciter script functions.
///
/// Used in [`Element.call_function()`](dom/struct.Element.html#method.call_function),
/// [`Element.call_method()`](dom/struct.Element.html#method.call_method),
/// [`Host.call_function()`](host/struct.Host.html#method.call_function),
/// [`Value.call()`](value/struct.Value.html#method.call).
///
/// ## Example:
///
/// ```rust,ignore
/// # #![doc(test(no_crate_inject))]
/// # #[macro_use] extern crate sciter;
/// let value = sciter::Value::new();
/// let result = value.call(None, &make_args!(1, "2", 3.0), Some(file!())).unwrap();
/// ```
#[macro_export]
macro_rules! make_args {
	() => { { let args : [$crate::value::Value; 0] = []; args } };

	( $($s:expr),* ) => {
		{
			let args = [
			$(
				$crate::value::Value::from($s)
			 ),*
			];
			args
		}
	};
}

#[doc(hidden)]
#[macro_export]
/// Declare handle type (native pointer).
macro_rules! MAKE_HANDLE {
	($(#[$attrs:meta])* $name:ident, $inner:ident) => {
		#[repr(C)] #[doc(hidden)]
		pub struct $inner { _unused: usize }
    $(#[$attrs])*
    pub type $name = *mut $inner;
	};
}

/// Dispatch script calls to native code. Used in [`dom::EventHandler`](dom/event/trait.EventHandler.html) implementations.
///
/// This macro generates a new function which dispatches incoming script calls to the corresponding native functions
/// with arguments unpacking and type checking.
///
/// Note: unstable, will be improved.
#[macro_export]
macro_rules! dispatch_script_call {

	(
		$(
			fn $name:ident ( $( $argt:ident ),* );
		 )*
	) => {

		fn dispatch_script_call(&mut self, _root: $crate::HELEMENT, name: &str, argv: &[$crate::Value]) -> Option<$crate::Value>
		{
			match name {
				$(
					stringify!($name) => {

						// args count
						let mut _i = 0;
						$(
							let _: $argt;
							_i += 1;
						)*
						let argc = _i;

						if argv.len() != argc {
							return Some($crate::Value::error(&format!("{} error: {} of {} arguments provided.", stringify!($name), argv.len(), argc)));
						}

						// call function
						let mut _i = 0;
						let rv = self.$name(
							$(
								{
									match $crate::FromValue::from_value(&argv[_i]) {
										Some(arg) => { _i += 1; arg },
										None => {
											// invalid type
											return Some($crate::Value::error(&format!("{} error: invalid type of {} argument ({} expected, {:?} provided).",
												stringify!($name), _i, stringify!($argt), argv[_i])));
										},
									}
								}
							 ),*
						);

						// return result value
						return Some($crate::Value::from(rv));
					},
				 )*

				_ => ()
			};

			// script call not handled
			return None;
		}
	};
}


/// Create a `sciter::Value` (of map type) from a list of key-value pairs.
///
/// # Example
///
/// ```rust
/// # #[macro_use] extern crate sciter;
/// # fn main() {
/// let v: sciter::Value = vmap! {
///   "one" => 1,
///   "two" => 2.0,
///   "three" => "",
/// };
/// assert!(v.is_map());
/// assert_eq!(v.len(), 3);
/// # }
/// ```
#[macro_export]
macro_rules! vmap {
  ( $($key:expr => $value:expr,)+ ) => { vmap!($($key => $value),+)  };
  ( $($key:expr => $value:expr),* ) => {
    {
      let mut _v = $crate::Value::map();
      $(
        _v.set_item($key, $value);
      )*
      _v
    }
  };
}

/// Creates a `sciter::Value` (of array type) containing the arguments.
///
/// # Example
///
/// ```rust
/// # #[macro_use] extern crate sciter;
/// # fn main() {
/// let v: sciter::Value = varray![1, 2.0, "three"];
/// assert!(v.is_array());
/// assert_eq!(v.len(), 3);
/// # }
/// ```
#[macro_export]
macro_rules! varray {
  ( $($value:expr,)+ ) => { varray!($($value),+) };
  ( $($value:expr),* ) => {
    {
      // args count
      let mut _i = 0;
      $(
        let _ = &$value;
        _i += 1;
      )*
      let argc = _i;
      let mut _v = $crate::Value::array(argc);
      let mut _i = 0;
      $(
        _v.set(_i, $value);
        _i += 1;
      )*
      _v
    }
  };
}
