/*! Handling all attributes of requests (GET/POST/PUT/DELETE) sent by
[`Element.request()`](https://sciter.com/docs/content/sciter/Element.htm) and
[`View.request()`](https://sciter.com/docs/content/sciter/View.htm)
functions and other load requests.

 */

#![allow(non_camel_case_types, non_snake_case)]

use capi::sctypes::{UINT, LPVOID, LPCBYTE, LPCSTR, LPCWSTR};
use capi::scdef::{LPCSTR_RECEIVER, LPCWSTR_RECEIVER, LPCBYTE_RECEIVER};
pub use capi::scdef::RESOURCE_TYPE;

MAKE_HANDLE!(#[doc = "Request native handle."] HREQUEST, _HREQUEST);

#[repr(C)]
#[derive(Debug, PartialEq)]
/// Type of the result value for Sciter Request functions.
pub enum REQUEST_RESULT {
	/// E.g. not enough memory.
	PANIC = -1,
	/// Success.
	OK = 0,
	/// Bad parameter.
	BAD_PARAM = 1,
	/// Operation failed, e.g. index out of bounds.
	FAILURE = 2,
	/// The platform does not support requested feature.
  NOTSUPPORTED = 3,
}

impl std::error::Error for REQUEST_RESULT {}

impl std::fmt::Display for REQUEST_RESULT {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		write!(f, "{:?}", self)
	}
}


#[repr(C)]
#[derive(Debug, PartialEq)]
/// Request methods.
pub enum REQUEST_METHOD {
	/// Sends a plain HTTP GET request.
	///
	/// Url-encoded params (if any) are appended to the url in order to form the request.
	GET = 1,
	/// Sends an HTTP POST request.
	///
	/// The params are serialized as `Content-Type: application/x-www-form-urlencoded;charset=utf-8`.
	POST = 2,
	/// Sends an HTTP PUT request.
	///
	/// The params are serialized as `Content-Type: multipart/form-data; boundary= ...`.
	PUT = 3,
	/// Sends an HTTP DELETE request.
	DELETE = 4,
}

#[repr(C)]
#[derive(Debug, PartialEq)]
/// HTTP methods for the [`Element::send_request`](../dom/struct.Element.html#method.send_request).
pub enum REQUEST_TYPE {
	/// Asynchronous GET.
	AsyncGet,
	/// Asynchronous POST.
	AsyncPost,
	/// Synchronous GET.
	Get,
	/// Synchronous POST.
	Post,
}


#[repr(C)]
#[derive(Debug, PartialEq)]
/// Completion state of a request.
pub enum REQUEST_STATE {
	/// The request is pending.
	PENDING = 0,
	/// Completed successfully.
	SUCCESS = 1,
	/// Completed with failure.
	FAILURE = 2,
}

#[repr(C)]
#[derive(Debug)]
pub struct REQUEST_PARAM {
	pub name: LPCWSTR,
	pub value: LPCWSTR,
}


#[repr(C)]
#[allow(missing_docs)]
pub struct SciterRequestAPI
{
  /// a.k.a AddRef()
  pub RequestUse: extern "system" fn (rq: HREQUEST) -> REQUEST_RESULT,

  /// a.k.a Release()
  pub RequestUnUse: extern "system" fn (rq: HREQUEST) -> REQUEST_RESULT,

  /// get requested URL
  pub RequestUrl: extern "system" fn (rq: HREQUEST, rcv: LPCSTR_RECEIVER, rcv_param: LPVOID) -> REQUEST_RESULT,

  /// get real, content URL (after possible redirection)
  pub RequestContentUrl: extern "system" fn (rq: HREQUEST, rcv: LPCSTR_RECEIVER, rcv_param: LPVOID) -> REQUEST_RESULT,

  /// get requested data type
  pub RequestGetRequestType: extern "system" fn (rq: HREQUEST, pType: &mut REQUEST_METHOD) -> REQUEST_RESULT,

  /// get requested data type
  pub RequestGetRequestedDataType: extern "system" fn (rq: HREQUEST, pData: &mut RESOURCE_TYPE) -> REQUEST_RESULT,

  /// get received data type, string, mime type
  pub RequestGetReceivedDataType: extern "system" fn (rq: HREQUEST, rcv: LPCSTR_RECEIVER, rcv_param: LPVOID) -> REQUEST_RESULT,


  /// get number of request parameters passed
  pub RequestGetNumberOfParameters: extern "system" fn (rq: HREQUEST, pNumber: &mut UINT) -> REQUEST_RESULT,

  /// get nth request parameter name
  pub RequestGetNthParameterName: extern "system" fn (rq: HREQUEST, n: UINT, rcv: LPCWSTR_RECEIVER, rcv_param: LPVOID) -> REQUEST_RESULT,

  /// get nth request parameter value
  pub RequestGetNthParameterValue: extern "system" fn (rq: HREQUEST, n: UINT, rcv: LPCWSTR_RECEIVER, rcv_param: LPVOID) -> REQUEST_RESULT,

  /// get request times , ended - started = milliseconds to get the requst
  pub RequestGetTimes: extern "system" fn (rq: HREQUEST, pStarted: &mut UINT, pEnded: &mut UINT) -> REQUEST_RESULT,

  /// get number of request headers
  pub RequestGetNumberOfRqHeaders: extern "system" fn (rq: HREQUEST, pNumber: &mut UINT) -> REQUEST_RESULT,

  /// get nth request header name
  pub RequestGetNthRqHeaderName: extern "system" fn (rq: HREQUEST, n: UINT, rcv: LPCWSTR_RECEIVER, rcv_param: LPVOID) -> REQUEST_RESULT,

  /// get nth request header value
  pub RequestGetNthRqHeaderValue: extern "system" fn (rq: HREQUEST, n: UINT, rcv: LPCWSTR_RECEIVER, rcv_param: LPVOID) -> REQUEST_RESULT,

  /// get number of response headers
  pub RequestGetNumberOfRspHeaders: extern "system" fn (rq: HREQUEST, pNumber: &mut UINT) -> REQUEST_RESULT,

  /// get nth response header name
  pub RequestGetNthRspHeaderName: extern "system" fn (rq: HREQUEST, n: UINT, rcv: LPCWSTR_RECEIVER, rcv_param: LPVOID) -> REQUEST_RESULT,

  /// get nth response header value
  pub RequestGetNthRspHeaderValue: extern "system" fn (rq: HREQUEST, n: UINT, rcv: LPCWSTR_RECEIVER, rcv_param: LPVOID) -> REQUEST_RESULT,

  /// get completion status (CompletionStatus - http response code : 200, 404, etc.)
  pub RequestGetCompletionStatus: extern "system" fn (rq: HREQUEST, pState: &mut REQUEST_STATE, pCompletionStatus: &mut UINT) -> REQUEST_RESULT,

  /// get proxy host
  pub RequestGetProxyHost: extern "system" fn (rq: HREQUEST, rcv: LPCSTR_RECEIVER, rcv_param: LPVOID) -> REQUEST_RESULT,

  /// get proxy port
  pub RequestGetProxyPort: extern "system" fn (rq: HREQUEST, pPort: &mut UINT) -> REQUEST_RESULT,

  /// mark reequest as complete with status and data
  pub RequestSetSucceeded: extern "system" fn (rq: HREQUEST, status: UINT, dataOrNull: LPCBYTE, dataLength: UINT) -> REQUEST_RESULT,

  /// mark reequest as complete with failure and optional data
  pub RequestSetFailed: extern "system" fn (rq: HREQUEST, status: UINT, dataOrNull: LPCBYTE, dataLength: UINT) -> REQUEST_RESULT,

  /// append received data chunk
  pub RequestAppendDataChunk: extern "system" fn (rq: HREQUEST, data: LPCBYTE, dataLength: UINT) -> REQUEST_RESULT,

  /// set request header (single item)
  pub RequestSetRqHeader: extern "system" fn (rq: HREQUEST, name: LPCWSTR, value: LPCWSTR) -> REQUEST_RESULT,

  /// set respone header (single item)
  pub RequestSetRspHeader: extern "system" fn (rq: HREQUEST, name: LPCWSTR, value: LPCWSTR) -> REQUEST_RESULT,

  /// set received data type, string, mime type
  pub RequestSetReceivedDataType: extern "system" fn (rq: HREQUEST, _type: LPCSTR) -> REQUEST_RESULT,

  /// set received data encoding, string
  pub RequestSetReceivedDataEncoding: extern "system" fn (rq: HREQUEST, encoding: LPCSTR) -> REQUEST_RESULT,

  /// get received (so far) data
  pub RequestGetData: extern "system" fn (rq: HREQUEST, rcv: LPCBYTE_RECEIVER, rcv_param: LPVOID) -> REQUEST_RESULT,
}
