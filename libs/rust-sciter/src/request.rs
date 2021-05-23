/*! Sciter Request API.

Handling all attributes of requests (GET/POST/PUT/DELETE) sent by
[`Element.request()`](https://sciter.com/docs/content/sciter/Element.htm) and
[`View.request()`](https://sciter.com/docs/content/sciter/View.htm)
functions and other load requests.

*/
pub use capi::screquest::{HREQUEST, REQUEST_RESULT};
pub use capi::screquest::{REQUEST_METHOD, REQUEST_STATE, REQUEST_TYPE, RESOURCE_TYPE};

use capi::sctypes::{LPVOID, UINT};
use capi::scdef::{LPCWSTR_RECEIVER};

use utf::{store_astr, store_wstr, store_bstr};

use _RAPI;



macro_rules! ok_or {
	($ok:ident) => {
		ok_or!((), $ok)
	};

  ($rv:expr, $ok:ident) => {
    if $ok == REQUEST_RESULT::OK {
      Ok($rv)
    } else {
      Err($ok)
    }
  };
}

/// A specialized `Result` type for request operations.
pub type Result<T> = ::std::result::Result<T, REQUEST_RESULT>;

type GetCountFn = extern "system" fn (rq: HREQUEST, pNumber: &mut UINT) -> REQUEST_RESULT;
type GetNameFn = extern "system" fn (rq: HREQUEST, n: UINT, rcv: LPCWSTR_RECEIVER, rcv_param: LPVOID) -> REQUEST_RESULT;
type GetValueFn = extern "system" fn (rq: HREQUEST, n: UINT, rcv: LPCWSTR_RECEIVER, rcv_param: LPVOID) -> REQUEST_RESULT;

/// Request object.
///
/// Can be constructed from a `HREQUEST` handle only:
///
/// ```rust,no_run
/// # use sciter::request::Request;
/// # use sciter::host::{SCN_LOAD_DATA, LOAD_RESULT};
/// fn on_data_load(pnm: &mut SCN_LOAD_DATA) -> Option<LOAD_RESULT> {
///   let mut rq = Request::from(pnm.request_id);
///   // ...
///   # None
/// }
/// ```
pub struct Request(HREQUEST);

/// Destroy the object.
impl Drop for Request {
	fn drop(&mut self) {
		(_RAPI.RequestUnUse)(self.0);
	}
}

/// Copies the object.
///
/// All allocated objects are reference counted so copying is just a matter of increasing reference counts.
impl Clone for Request {
	fn clone(&self) -> Self {
		let dst = Request(self.0);
		(_RAPI.RequestUse)(dst.0);
		dst
	}
}

/// Construct a Request object from `HREQUEST` handle.
impl From<HREQUEST> for Request {
	fn from(hrq: HREQUEST) -> Request {
		assert!(!hrq.is_null());
  	(_RAPI.RequestUse)(hrq);
		Request(hrq)
	}
}

impl Request {
	/// Mark the request as complete with status and optional data.
	pub fn succeeded(&mut self, status: u32, data: Option<&[u8]>) -> Result<()> {
		let (ptr, size) = if let Some(data) = data {
			(data.as_ptr(), data.len() as u32)
		} else {
			(std::ptr::null(), 0_u32)
		};
		let ok = (_RAPI.RequestSetSucceeded)(self.0, status, ptr, size);
		ok_or!(ok)
	}

	/// Mark the request as complete with failure.
	pub fn failed(&mut self, status: u32, data: Option<&[u8]>) -> Result<()> {
		let (ptr, size) = if let Some(data) = data {
			(data.as_ptr(), data.len() as u32)
		} else {
			(std::ptr::null(), 0_u32)
		};
		let ok = (_RAPI.RequestSetSucceeded)(self.0, status, ptr, size);
		ok_or!(ok)
	}

	/// Append a data chunk to the received data.
	pub fn append_received_data(&mut self, data: &[u8]) -> Result<()> {
		let (ptr, size) = (data.as_ptr(), data.len() as u32);
		let ok = (_RAPI.RequestAppendDataChunk)(self.0, ptr, size);
		ok_or!(ok)
	}

	/// Get received (so far) data.
	pub fn get_received_data(&self) -> Result<Vec<u8>> {
		let mut data = Vec::new();
		let ok = (_RAPI.RequestGetData)(self.0, store_bstr, &mut data as *mut _ as LPVOID);
		ok_or!(data, ok)
	}

	/// Get the URL of the request.
	pub fn url(&self) -> Result<String> {
		let mut s = String::new();
		let ok = (_RAPI.RequestUrl)(self.0, store_astr, &mut s as *mut _ as LPVOID);
		ok_or!(s, ok)
	}

	/// Get a real URL of the content (e.g., after possible redirection).
	pub fn content_url(&self) -> Result<String> {
		let mut s = String::new();
		let ok = (_RAPI.RequestContentUrl)(self.0, store_astr, &mut s as *mut _ as LPVOID);
		ok_or!(s, ok)
	}

	/// Get the data type of the request.
	pub fn method(&self) -> Result<REQUEST_METHOD> {
		let mut t = REQUEST_METHOD::GET;
		let ok = (_RAPI.RequestGetRequestType)(self.0, &mut t);
		ok_or!(t, ok)
	}

	/// Get the resource data type of the request.
	pub fn request_type(&self) -> Result<RESOURCE_TYPE> {
		let mut t = RESOURCE_TYPE::RAW;
		let ok = (_RAPI.RequestGetRequestedDataType)(self.0, &mut t);
		ok_or!(t, ok)
	}

	/// Get the MIME type of the received data.
	pub fn response_type(&self) -> Result<String> {
		let mut s = String::new();
		let ok = (_RAPI.RequestGetReceivedDataType)(self.0, store_astr, &mut s as *mut _ as LPVOID);
		ok_or!(s, ok)
	}

	/// Set the MIME type of the received data.
	pub fn set_response_type(&mut self, mime_type: &str) -> Result<()> {
		let text = s2u!(mime_type);
		let ok = (_RAPI.RequestSetReceivedDataType)(self.0, text.as_ptr());
		ok_or!(ok)
	}

	/// Set the data encoding for the received data.
	pub fn set_response_encoding(&mut self, encoding_type: &str) -> Result<()> {
		let text = s2u!(encoding_type);
		let ok = (_RAPI.RequestSetReceivedDataEncoding)(self.0, text.as_ptr());
		ok_or!(ok)
	}

	fn get_collection_impl(&self, get_count: GetCountFn, get_name: GetNameFn, get_value: GetValueFn) -> Result<std::collections::HashMap<String, String>>	{
		let mut count = 0;
		let ok = get_count(self.0, &mut count);
		if ok != REQUEST_RESULT::OK {
			return Err(ok);
		}

		let mut args = std::collections::HashMap::with_capacity(count as usize);
		for i in 0..count {
			let mut name = String::new();
			let mut ok = get_name(self.0, i, store_wstr, &mut name as *mut _ as LPVOID);
			if ok == REQUEST_RESULT::OK {
				let mut value = String::new();
				ok = get_value(self.0, i, store_wstr, &mut value as *mut _ as LPVOID);
				if ok == REQUEST_RESULT::OK {
					args.insert(name, value);
				}
			}
			if ok != REQUEST_RESULT::OK {
				return Err(ok);
			}
		}

		Ok(args)
	}

	/// Get the parameters of the request.
	pub fn parameters(&self) -> Result<std::collections::HashMap<String, String>> {
		self.get_collection_impl(_RAPI.RequestGetNumberOfParameters, _RAPI.RequestGetNthParameterName, _RAPI.RequestGetNthParameterValue)
	}

	/// Get the headers of the request.
	pub fn request_headers(&self) -> Result<std::collections::HashMap<String, String>> {
		self.get_collection_impl(_RAPI.RequestGetNumberOfRqHeaders, _RAPI.RequestGetNthRqHeaderName, _RAPI.RequestGetNthRqHeaderValue)
	}

	/// Set request header (a single item).
	pub fn set_request_header(&mut self, name: &str, value: &str) -> Result<()> {
		let wname = s2w!(name);
		let wtext = s2w!(value);
		let ok = (_RAPI.RequestSetRqHeader)(self.0, wname.as_ptr(), wtext.as_ptr());
		ok_or!(ok)
	}

	/// Get the headers of the response.
	pub fn response_headers(&self) -> Result<std::collections::HashMap<String, String>> {
		self.get_collection_impl(_RAPI.RequestGetNumberOfRspHeaders, _RAPI.RequestGetNthRspHeaderName, _RAPI.RequestGetNthRspHeaderValue)
	}

	/// Set respone header (a single item).
	pub fn set_response_header(&mut self, name: &str, value: &str) -> Result<()> {
		let wname = s2w!(name);
		let wtext = s2w!(value);
		let ok = (_RAPI.RequestSetRspHeader)(self.0, wname.as_ptr(), wtext.as_ptr());
		ok_or!(ok)
	}

	/// Get proxy host and port (if any).
	pub fn proxy(&self) -> Result<(String, u16)> {
		let mut s = String::new();
		let mut ok = (_RAPI.RequestGetProxyHost)(self.0, store_astr, &mut s as *mut _ as LPVOID);
		if ok == REQUEST_RESULT::OK {
			let mut n = 0_u32;
			ok = (_RAPI.RequestGetProxyPort)(self.0, &mut n);
			if ok == REQUEST_RESULT::OK {
				return Ok((s, n as u16));
			}
		}
		Err(ok)
	}

	/// Get the current completion status of the request.
	///
	/// Returns current state and HTTP response code.
	pub fn completion_status(&self) -> Result<(REQUEST_STATE, u32)> {
		let mut state = REQUEST_STATE::SUCCESS;
		let mut code = 0_u32;
		let ok = (_RAPI.RequestGetCompletionStatus)(self.0, &mut state, &mut code);
		ok_or!((state, code), ok)
	}

	/// Get the execution duratiom of the request.
	pub fn request_duration(&self) -> Result<std::time::Duration> {
		let mut started = 0;
		let mut ended = 0;
		let ok = (_RAPI.RequestGetTimes)(self.0, &mut started, &mut ended);
		if ok == REQUEST_RESULT::OK && ended > started {
			let d = std::time::Duration::from_millis(ended as u64 - started as u64);
			Ok(d)
		} else {
			Err(ok)
		}
	}

	/// Get the execution `started` and `ended` time of the request, in milliseconds.
	pub fn request_time(&self) -> Result<(std::time::Duration, std::time::Duration)> {
		let mut started = 0;
		let mut ended = 0;
		let ok = (_RAPI.RequestGetTimes)(self.0, &mut started, &mut ended);
		if ok == REQUEST_RESULT::OK {
			use std::time::Duration;
			let s = Duration::from_millis(started as u64);
			let e = Duration::from_millis(ended as u64);
			Ok((s, e))
		} else {
			Err(ok)
		}
	}

}
