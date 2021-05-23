//! UTF-8 <> UTF-16 conversion support.

// Since Rust haven't stable support of UTF-16, I've ported this code
// from Sciter SDK (aux-cvt.h)

// (C) 2003-2015, Andrew Fedoniouk (andrew@terrainformatica.com)


#![allow(dead_code)]

use std::ffi::{CStr, CString};
use capi::sctypes::{LPCSTR, LPCWSTR, LPCBYTE};


/// UTF-8 to UTF-16* converter.
#[allow(unused_parens)]
fn towcs(utf: &[u8], outbuf: &mut Vec<u16>) -> bool
{
	let errc = 0x003F; // '?'
	let mut num_errors = 0;

	let last = utf.len();
	let mut pc = 0;
	while (pc < last) {
		let mut b = u32::from(utf[pc]); pc += 1;
		if (b == 0) { break; }

		if ((b & 0x80) == 0) {
			// 1-BYTE sequence: 000000000xxxxxxx = 0xxxxxxx

		} else if ((b & 0xE0) == 0xC0) {
			// 2-BYTE sequence: 00000yyyyyxxxxxx = 110yyyyy 10xxxxxx
			if (pc == last) {
				outbuf.push(errc);
				num_errors += 1;
				break;
			}

			b = (b & 0x1f) << 6;
			b |= (u32::from(utf[pc]) & 0x3f); pc += 1;

		} else if ((b & 0xf0) == 0xe0) {
			// 3-BYTE sequence: zzzzyyyyyyxxxxxx = 1110zzzz 10yyyyyy 10xxxxxx
			if (pc >= last - 1) {
				outbuf.push(errc);
				num_errors += 1;
				break;
			}

			b = (b & 0x0f) << 12;
			b |= (u32::from(utf[pc]) & 0x3f) << 6; pc += 1;
			b |= (u32::from(utf[pc]) & 0x3f); pc += 1;

			if (b == 0xFEFF && outbuf.is_empty()) { // bom at start
				continue; // skip it
			}

		} else if ((b & 0xf8) == 0xf0) {
			// 4-BYTE sequence: 11101110wwwwzzzzyy + 110111yyyyxxxxxx = 11110uuu 10uuzzzz 10yyyyyy 10xxxxxx
			if(pc >= last - 2) { outbuf.push(errc); break; }

			b = (b & 0x07) << 18;
			b |= (u32::from(utf[pc]) & 0x3f) << 12; pc += 1;
			b |= (u32::from(utf[pc]) & 0x3f) << 6; pc += 1;
			b |= (u32::from(utf[pc]) & 0x3f); pc += 1;

			// b shall contain now full 21-bit unicode code point.
			assert!((b & 0x1f_ffff) == b);
			if((b & 0x1f_ffff) != b) {
				outbuf.push(errc);
				num_errors += 1;
				continue;
			}

			outbuf.push( (0xd7c0 + (b >> 10)) as u16 );
			outbuf.push( (0xdc00 | (b & 0x3ff)) as u16 );

		} else {
			num_errors += 1;
			b = u32::from(errc);
		}

		outbuf.push(b as u16);
	}
	return num_errors == 0;
}


/// UTF-16 to UTF-8 converter.
#[allow(unused_parens)]
fn fromwcs(wcs: &[u16], outbuf: &mut Vec<u8>) -> bool
{
	let mut num_errors = 0;

	let last = wcs.len();
	let mut pc = 0;
	while (pc < last) {
		let c = u32::from(wcs[pc]);
		if (c < (1 << 7)) {
			outbuf.push(c as u8);

		} else if (c < (1 << 11)) {
			outbuf.push(((c >> 6) | 0xc0) as u8);
			outbuf.push(((c & 0x3f) | 0x80) as u8);

		} else if (c < (1 << 16)) {
			outbuf.push(((c >> 12) | 0xe0) as u8);
			outbuf.push((((c >> 6) & 0x3f) | 0x80) as u8);
			outbuf.push(((c & 0x3f) | 0x80) as u8);

		} else if (c < (1 << 21)) {
			outbuf.push(((c >> 18) | 0xf0) as u8);
			outbuf.push((((c >> 12) & 0x3f) | 0x80) as u8);
			outbuf.push((((c >> 6) & 0x3f) | 0x80) as u8);
			outbuf.push(((c & 0x3f) | 0x80) as u8);

		} else {
			num_errors += 1;
		}
		pc += 1;
	}
	return num_errors == 0;
}


/// UTF-16 string length like `libc::wcslen`.
fn wcslen(sz: LPCWSTR) -> usize
{
	if sz.is_null() {
		return 0;
	}
	let mut i: isize = 0;
	loop {
		let c = unsafe { *sz.offset(i) };
		if c == 0 {
			break;
		}
		i += 1;
	}
	return i as usize;
}

/// UTF8 to Rust string conversion. See also [`s2u!`](../macro.s2u.html).
pub fn u2s(sz: LPCSTR) -> String
{
	if sz.is_null() {
		return String::new();
	}
	let cs = unsafe { CStr::from_ptr(sz) };
	let cow = cs.to_string_lossy();
	return cow.into_owned();
}

/// UTF8 to Rust string conversion. See also [`s2u!`](../macro.s2u.html).
pub fn u2sn(sz: LPCSTR, len: usize) -> String
{
	let chars = unsafe { ::std::slice::from_raw_parts(sz as LPCBYTE, len) };
	let s = String::from_utf8_lossy(chars).into_owned();
	return s;
}

/// UTF-16 to Rust string conversion. See also [`s2w!`](../macro.s2w.html).
pub fn w2s(sz: LPCWSTR) -> String
{
	return w2sn(sz, wcslen(sz));
}

/// UTF-16 to Rust string conversion. See also [`s2w!`](../macro.s2w.html).
pub fn w2sn(sz: LPCWSTR, len: usize) -> String
{
	if sz.is_null() {
		return String::new();
	}
	let chars = unsafe { ::std::slice::from_raw_parts(sz, len) };
	let s = String::from_utf16_lossy(chars);
	return s;
}

/// Rust string to UTF-8 conversion.
pub fn s2un(s: &str) -> (CString, u32) {
	let cs = CString::new(s.trim_end_matches('\0')).unwrap_or(CString::new("").unwrap());
	let n = cs.as_bytes().len() as u32;
	return (cs, n);
}

/// Rust string to UTF-16 conversion.
pub fn s2vec(s: &str) -> Vec<u16> {
	s2vecn(s).0
}

/// Rust string to UTF-16 conversion.
pub fn s2vecn(s: &str) -> (Vec<u16>, u32) {
	let cs = CString::new(s.trim_end_matches('\0')).unwrap_or(CString::new("").unwrap());
	let mut out = Vec::with_capacity(s.len() * 2);
	towcs(cs.to_bytes(), &mut out);
	let n = out.len() as u32;
	if n > 0 {
		out.push(0);
	}
	return (out, n);
}

use capi::sctypes::{UINT, LPVOID};

/// Convert an incoming UTF-16 to `String`.
pub(crate) extern "system" fn store_wstr(szstr: LPCWSTR, str_length: UINT, param: LPVOID) {
	let s = self::w2sn(szstr, str_length as usize);
	let out = param as *mut String;
	unsafe { *out = s };
}

/// Convert an incoming UTF-8 to `String`.
pub(crate) extern "system" fn store_astr(szstr: LPCSTR,  str_length: UINT, param: LPVOID) {
	let s = self::u2sn(szstr, str_length as usize);
	let out = param as *mut String;
	unsafe { *out = s };
}

/// Convert an incoming html string (UTF-8 in fact) to `String`.
pub(crate) extern "system" fn store_bstr(szstr: LPCBYTE, str_length: UINT, param: LPVOID) {
	let s = unsafe { ::std::slice::from_raw_parts(szstr, str_length as usize) };
	let pout = param as *mut Vec<u8>;
	let out = unsafe {&mut *pout};
	out.extend_from_slice(s);
}


#[cfg(test)]
mod tests {
	#![allow(unused_imports)]

	use std::ffi::{CStr, CString};
	use capi::sctypes::{LPCWSTR, LPCSTR};
	use super::{wcslen, u2s, w2s, s2vec};

	#[test]
	fn test_wcslen() {
		let nullptr: LPCWSTR = ::std::ptr::null();
		assert_eq!(wcslen(nullptr), 0);

		let v = vec![0 as u16];
		assert_eq!(wcslen(v.as_ptr()), 0);

		let v = vec![32, 32, 0];
		assert_eq!(wcslen(v.as_ptr()), 2);
	}

	#[test]
	fn test_u2s() {
		let nullptr: LPCSTR = ::std::ptr::null();
		assert_eq!(u2s(nullptr), String::new());

		let s = "hi, there";
		assert_eq!(u2s(CString::new(s.trim_end_matches('\0')).unwrap_or(CString::new("").unwrap()).as_ptr()), s);
	}

	#[test]
	fn test_w2s() {
		let nullptr: LPCWSTR = ::std::ptr::null();
		assert_eq!(w2s(nullptr), String::new());

		let v = vec![32, 32, 0];	// SP
		assert_eq!(w2s(v.as_ptr()), "  ");
	}

	#[test]
	fn s2w_test() {
		let v = s2vec("");
		assert_eq!(v, []);

		assert_eq!(s2vec(""), []);

		assert_eq!(s2vec("A"), ['A' as u16, 0]);

		assert_eq!(s2vec("AB"), ['A' as u16, 'B' as u16, 0]);

		let (cs, n) = s2wn!("");
		assert_eq!(n, 0);
		assert_eq!(cs, []);
	}
}
