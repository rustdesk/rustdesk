/*! DOM access methods via the [`dom::Element`](struct.Element.html).


## Introduction.

Let’s assume you have already integrated Sciter in your application and so you have a Sciter window with the loaded content.

From Sciter's point of view the loaded document is a tree of DOM elements (elements of Document Object Model).
Sciter builds this tree while loading/parsing of input HTML.
As a rule, each tag in the source HTML is matching with a DOM element (there are exceptions, see below).

You can change the text, attributes, state flags of DOM elements;
add new or remove existing DOM elements.
You can also attach your own DOM event handlers to DOM elements in order to receive events and notifications.

Therefore your UI in Sciter is a collection of uniform DOM elements
that can be styled by CSS and manipulated by native or script code.


## Basic operations

To access the DOM tree we need to get a reference of its root element
(the root element is the element representing the `<html>` tag in HTML source).

```rust,no_run
# use sciter::dom::Element;
# let hwnd = ::std::ptr::null_mut();
let root = Element::from_window(hwnd).unwrap();
assert_eq!(root.get_tag(), "html");
```

*TBD:* Other ways to access DOM tree.

By having a root element reference we are able to access any other element in the tree
using various access and search functions like `SciterGetNthChild`, `SciterSelectElements`, etc.
All of them are wrapped into methods of [`dom::Element`](struct.Element.html).

Here is how you would get a reference to the first `<div>` element with class "sidebar" using CSS selectors:

```rust,no_run
# let root = sciter::dom::Element::from(::std::ptr::null_mut());
let sidebar = root.find_first("div.sidebar").unwrap();
```

The same in script:

```tiscript
var sidebar = self.select("div.sidebar"); // or
var sidebar = self.$(div.sidebar); // using the stringizer variant of select()
```

*TBD:* Other select methods.

## DOM element operations

You can change the **text** or **html** of a DOM element:

```rust,no_run
# let root = sciter::dom::Element::from(::std::ptr::null_mut());
if let Some(mut el) = root.find_first("#cancel").unwrap() {
  el.set_text("Abort!");
  el.set_html(br##"<img src="http://lorempixel.com/32/32/cats/" alt="some cat"/>"##, None);
}
```

The same but in script:

```tiscript
var el = ...;
el.text = "Hello world"; // text
el.html = "Hello <b>wrold</b>!"; // inner html
```

You can also get or set DOM **attributes** of any DOM element:

```rust,no_run
# let mut el = sciter::dom::Element::from(::std::ptr::null_mut());
let val = el.get_attribute("class").unwrap();
el.set_attribute("class", "new-class");
```

To **remove** an existing DOM element (to detach it from the DOM) you will do this:

```rust,no_run
# let mut el = sciter::dom::Element::from(::std::ptr::null_mut());
el.detach();
```

and when the code leaves the scope where the `el` variable is defined, the DOM element will be destroyed.

Creation and population of DOM elements looks like this:

```rust,no_run
# use sciter::dom::Element;
# let mut el = sciter::dom::Element::from(::std::ptr::null_mut());
let p = Element::with_text("p", "Hello").unwrap(); // create <p> element
el.append(&p); // append it to the existing element, or use insert() ...
```

And in script:

```tiscript
var p = new Element("p", "Hello");
el.append(p);
```

To change runtime state flags of a DOM element we do something like this:

```rust,ignore
# let mut el = sciter::dom::Element::from(::std::ptr::null_mut());
el.set_state(ELEMENT_STATE_BITS::STATE_VISITED);
```

And in script:

```tiscript
el.state.visited = true;
```

(after such call the element will match the `:visited` CSS selector)


## Getting and setting values of DOM elements.

By default the value of a DOM element is its text but some DOM elements may have
so called behaviors attached to them (see below).
`<input>` elements, for example, are plain DOM elements
but each input type has its own behavior assigned to the element.
The behavior, among other things, is responsible for providing and setting the value of the element.

For example, the value of an `<input type=checkbox>` is boolean – _true_ or _false_,
and the value of a `<form>` element is a collection (name/value map) of all named inputs on the form.

In native code values are represented by [`sciter::Value`](../value/index.html) objects.
[`sciter::Value`](../value/index.html) is a structure that can hold different types of values:
numbers, strings, arrays, objects, etc
(see [documentation](https://sciter.com/docs/content/script/language/Types.htm)).

Here is how to set a numeric value of a DOM element in native code:

```rust,no_run
# use sciter::Value;
# let root = sciter::dom::Element::from(::std::ptr::null_mut());
if let Some(mut num) = root.find_first("input[type=number]").unwrap() {
  num.set_value( Value::from(12) );  // sciter::Value with T_INT type (i32 in Rust)
  num.set_value(12);  // equivalent but with implicit conversion
}
```

In script the same will look like:

```tiscript
if (var num = self.select("input[type=number]")) {
  num.value = 12;
}
```

.
*/

use ::{_API};
use capi::sctypes::*;
use value::Value;

use capi::screquest::{REQUEST_PARAM, REQUEST_TYPE};
use capi::scdef::RESOURCE_TYPE;
use capi::scbehavior::{CLICK_REASON, BEHAVIOR_EVENTS, BEHAVIOR_EVENT_PARAMS};
use utf::{store_astr, store_wstr, store_bstr};

pub use capi::scdom::{SCDOM_RESULT, HELEMENT, SET_ELEMENT_HTML, ELEMENT_AREAS, ELEMENT_STATE_BITS};
pub use dom::event::{EventHandler, EventReason};


/// A specialized `Result` type for DOM operations.
pub type Result<T> = ::std::result::Result<T, SCDOM_RESULT>;


/// Initialize HELEMENT by nullptr.
macro_rules! HELEMENT {
	() => { ::std::ptr::null_mut() }
}


macro_rules! ok_or {
	($rv:expr, $ok:ident) => {
		if $ok == SCDOM_RESULT::OK {
			Ok($rv)
		} else {
			Err($ok)
		}
	};

	// for DOM access not_handled is ok
	// for calling function operation_failed is also ok
	($rv:expr, $ok:ident, $skip_not_handled:expr) => {
		if $ok == SCDOM_RESULT::OK || ($ok == $skip_not_handled) {
			Ok($rv)
		} else {
			Err($ok)
		}
	};
}


trait ElementVisitor {
	fn on_element(&mut self, el: Element) -> bool;
	fn result(&self) -> Vec<Element>;
}

#[derive(Default)]
struct FindFirstElement {
	all: Vec<Element>,
}

impl ElementVisitor for FindFirstElement {
	fn on_element(&mut self, el: Element) -> bool {
		self.all.push(el);
		return true;	// stop enumeration
	}
	fn result(&self) -> Vec<Element> {
		self.all.clone()
	}
}

#[derive(Default)]
struct FindAllElements {
	all: Vec<Element>,
}

impl ElementVisitor for FindAllElements {
	fn on_element(&mut self, el: Element) -> bool {
		self.all.push(el);
		return false;	// continue enumeration
	}
	fn result(&self) -> Vec<Element> {
		self.all.clone()
	}
}


/// DOM element wrapper. See the module-level documentation also.
#[derive(PartialEq)]
pub struct Element {
	he: HELEMENT,
}

/// `sciter::Element` can be transferred across thread boundaries.
unsafe impl Send for Element {}

/// It is safe to share `sciter::Element` between threads - underlaying API is thread-safe.
unsafe impl Sync for Element {}

impl From<HELEMENT> for Element {
	/// Construct an Element object from an `HELEMENT` handle.
	fn from(he: HELEMENT) -> Self {
		Element { he: Element::use_or(he) }
	}
}

/// Store the DOM element as a `Value`.
///
/// Since 4.4.3.26, perhaps.
impl std::convert::TryFrom<Element> for Value {
	type Error = SCDOM_RESULT;
	fn try_from(e: Element) -> Result<Value> {
		let mut v = Value::new();
		let ok = (_API.SciterGetExpando)(e.as_ptr(), v.as_ptr(), true as BOOL);
		ok_or!(v, ok)
	}
}

/// Get an `Element` object contained in the `Value`.
impl crate::value::FromValue for Element {
	fn from_value(v: &Value) -> Option<Element> {
		let mut pv: LPCBYTE = std::ptr::null();
		let mut cb: UINT = 0;
		let ok = (_API.ValueBinaryData)(v.as_cptr(), &mut pv, &mut cb);
		if ok == crate::value::VALUE_RESULT::OK {
			Some(Element::from(pv as HELEMENT))
		} else {
			None
		}
	}
}

impl Element {

	//\name Creation

	/// Create a new element, it is disconnected initially from the DOM.
	pub fn create(tag: &str) -> Result<Element> {
		let mut e = Element { he: HELEMENT!() };
		let tag = s2u!(tag);
		let text = 0 as LPCWSTR;
		let ok = (_API.SciterCreateElement)(tag.as_ptr(), text, &mut e.he);
		ok_or!(e, ok)
	}

	/// Create new element as child of `parent`.
	pub fn with_parent(tag: &str, parent: &mut Element) -> Result<Element> {
		let mut e = Element { he: HELEMENT!() };
		let tag = s2u!(tag);
		let text = 0 as LPCWSTR;
		(_API.SciterCreateElement)(tag.as_ptr(), text, &mut e.he);
		let ok = parent.append(&e);
		ok.map(|_| e)
	}

	/// Create new element as child of `parent`. Deprecated.
	#[deprecated(since="0.5.0", note="please use `Element::with_parent()` instead.")]
	pub fn create_at(tag: &str, parent: &mut Element) -> Result<Element> {
		Element::with_parent(tag, parent)
	}

	/// Create new element with specified `text`, it is disconnected initially from the DOM.
	pub fn with_text(tag: &str, text: &str) -> Result<Element> {
		let mut e = Element { he: HELEMENT!() };
		let tag = s2u!(tag);
		let text = s2w!(text);
		let ok = (_API.SciterCreateElement)(tag.as_ptr(), text.as_ptr(), &mut e.he);
		ok_or!(e, ok)
	}

	/// Create new element with specified `type`, which is useful for controls and widgets (initially disconnected).
	pub fn with_type(tag: &str, el_type: &str) -> Result<Element> {
		let mut e = Element { he: HELEMENT!() };
		let tag = s2u!(tag);
		let text = 0 as LPCWSTR;
		let ok = (_API.SciterCreateElement)(tag.as_ptr(), text, &mut e.he);
		if ok == SCDOM_RESULT::OK {
			let r = e.set_attribute("type", el_type);
			r.map(|_| e)
		} else {
			Err(ok)
		}
	}
	/// Get root DOM element of the Sciter document.
	pub fn from_window(hwnd: HWINDOW) -> Result<Element> {
		let mut p = HELEMENT!();
		let ok = (_API.SciterGetRootElement)(hwnd, &mut p);
		ok_or!(Element::from(p), ok)
	}

	/// Get focus DOM element of the Sciter document.
	pub fn from_focus(hwnd: HWINDOW) -> Result<Element> {
		let mut p = HELEMENT!();
		let ok = (_API.SciterGetFocusElement)(hwnd, &mut p);
		ok_or!(Element::from(p), ok)
	}

	/// Get highlighted element.
	pub fn from_highlighted(hwnd: HWINDOW) -> Result<Element> {
		let mut p = HELEMENT!();
		let ok = (_API.SciterGetHighlightedElement)(hwnd, &mut p);
		ok_or!(Element::from(p), ok)
	}

	/// Find DOM element of the Sciter document by coordinates.
	pub fn from_point(hwnd: HWINDOW, pt: POINT) -> Result<Element> {
		let mut p = HELEMENT!();
		let ok = (_API.SciterFindElement)(hwnd, pt, &mut p);
		ok_or!(Element::from(p), ok)
	}

	/// Get element handle by its UID.
	pub fn from_uid(hwnd: HWINDOW, uid: u32) -> Result<Element> {
		let mut p = HELEMENT!();
		let ok = (_API.SciterGetElementByUID)(hwnd, uid, &mut p);
		ok_or!(Element::from(p), ok)
	}

	#[doc(hidden)]
	fn use_or(he: HELEMENT) -> HELEMENT {
		let ok = (_API.Sciter_UseElement)(he);
		if ok == SCDOM_RESULT::OK {
			he
		} else {
			HELEMENT!()
		}
	}


	//\name Common methods

	/// Access element pointer.
	pub fn as_ptr(&self) -> HELEMENT {
		self.he
	}

	/// Get element UID - identifier suitable for storage.
	pub fn get_uid(&self) -> u32 {
		let mut n = 0;
		(_API.SciterGetElementUID)(self.he, &mut n);
		return n;
	}

	/// Return element tag as string (e.g. 'div', 'body').
	pub fn get_tag(&self) -> String {
		let mut s = String::new();
		(_API.SciterGetElementTypeCB)(self.he, store_astr, &mut s as *mut String as LPVOID);
		return s;
	}

	/// Get inner text of the element as string.
	pub fn get_text(&self) -> String {
		let mut s = String::new();
		(_API.SciterGetElementTextCB)(self.he, store_wstr, &mut s as *mut String as LPVOID);
		return s;
	}

	/// Set inner text of the element.
	pub fn set_text(&mut self, text: &str) -> Result<()> {
		let (s,n) = s2wn!(text);
		let ok = (_API.SciterSetElementText)(self.he, s.as_ptr(), n);
		ok_or!((), ok)
	}

	/// Get html representation of the element as utf-8 bytes.
	pub fn get_html(&self, with_outer_html: bool) -> Vec<u8> {
		let mut s = Vec::new();
		(_API.SciterGetElementHtmlCB)(self.he, with_outer_html as BOOL, store_bstr, &mut s as *mut Vec<u8> as LPVOID);
		return s;
	}

	/// Set inner or outer html of the element.
	pub fn set_html(&mut self, html: &[u8], how: Option<SET_ELEMENT_HTML>) -> Result<()> {
		if html.is_empty() {
			return self.clear();
		}
		let ok = (_API.SciterSetElementHtml)(self.he, html.as_ptr(), html.len() as UINT, how.unwrap_or(SET_ELEMENT_HTML::SIH_REPLACE_CONTENT) as UINT);
		ok_or!((), ok)
	}

	/// Get value of the element.
	pub fn get_value(&self) -> Value {
		let mut rv = Value::new();
		(_API.SciterGetValue)(self.he, rv.as_ptr());
		return rv;
	}

	/// Set value of the element.
	pub fn set_value<T: Into<Value>>(&mut self, val: T) -> Result<()> {
		let ok = (_API.SciterSetValue)(self.he, val.into().as_cptr());
		ok_or!((), ok)
	}

	/// Checks if particular UI state bits are set in the element.
	pub fn get_state(&self) -> ELEMENT_STATE_BITS {
		let mut rv = 0u32;
		(_API.SciterGetElementState)(self.he, &mut rv as *mut _);
		let state = unsafe { ::std::mem::transmute(rv) };
		return state;
	}

	/// Set UI state of the element with optional view update.
	pub fn set_state(&mut self, set: ELEMENT_STATE_BITS, clear: Option<ELEMENT_STATE_BITS>, update: bool) -> Result<()> {
		let clear = clear.unwrap_or(ELEMENT_STATE_BITS::STATE_NONE);
		let ok = (_API.SciterSetElementState)(self.he, set as UINT, clear as UINT, update as BOOL);
		ok_or!((), ok)
	}

	/// Get `HWINDOW` of containing window.
	pub fn get_hwnd(&self, for_root: bool) -> HWINDOW {
		let mut hwnd: HWINDOW = ::std::ptr::null_mut();
		(_API.SciterGetElementHwnd)(self.he, &mut hwnd as *mut HWINDOW, for_root as BOOL);
		return hwnd;
	}

	/// Attach a native window to the element as a child.
	pub fn attach_hwnd(&mut self, child: HWINDOW) -> Result<()> {
		let ok = (_API.SciterAttachHwndToElement)(self.he, child);
		ok_or!((), ok)
	}

	/// Detach a child native window (if any) from the element.
	pub fn detach_hwnd(&mut self) -> Result<()> {
		let ok = (_API.SciterAttachHwndToElement)(self.he, 0 as HWINDOW);
		ok_or!((), ok)
	}

	/// Get bounding rectangle of the element. See the [`ELEMENT_AREAS`](enum.ELEMENT_AREAS.html) enum for `kind` flags.
	pub fn get_location(&self, kind: u32) -> Result<RECT> {
		let mut rc = RECT::default();
		let ok = (_API.SciterGetElementLocation)(self.he, &mut rc as *mut _, kind as u32);
		ok_or!(rc, ok)
	}

	/// Request data download for this element.
	pub fn request_data(&self, url: &str, data_type: RESOURCE_TYPE, initiator: Option<HELEMENT>) -> Result<()> {
		let url = s2w!(url);
		let ok = (_API.SciterRequestElementData)(self.he, url.as_ptr(), data_type as u32, initiator.unwrap_or(HELEMENT!()));
		ok_or!((), ok)
	}

	/// Request HTML data download for this element.
	pub fn request_html(&self, url: &str, initiator: Option<HELEMENT>) -> Result<()> {
		self.request_data(url, RESOURCE_TYPE::HTML, initiator)
	}

	/// Send an asynchronous HTTP GET request for the element.
	///
	/// The contents of this element is replaced with the HTTP response (in text or html form).
	pub fn send_get_request(&self, url: &str) -> Result<()> {
		let url = s2w!(url);
		let no_params = ::std::ptr::null();
		let ok = (_API.SciterHttpRequest)(self.he, url.as_ptr(), RESOURCE_TYPE::HTML as u32, REQUEST_TYPE::AsyncGet as u32, no_params, 0);
		ok_or!((), ok)
	}

	/// Send an HTTP GET or POST request for the element.
	///
	/// GET params (if any) are appended to the url to form the request.<br/>
	/// HTTP POST params are serialized as `Content-Type: application/x-www-form-urlencoded;charset=utf-8;`.
	pub fn send_request(&self, url: &str, params: Option<&[(&str, &str)]>, method: Option<REQUEST_TYPE>, data_type: Option<RESOURCE_TYPE>) -> Result<()> {

		let url = s2w!(url);
		let method = method.unwrap_or(REQUEST_TYPE::AsyncGet) as u32;
		let data_type = data_type.unwrap_or(RESOURCE_TYPE::HTML) as u32;

		type WSTR = Vec<u16>;

		let mut wide_params: Vec<(WSTR, WSTR)> = Vec::new();
		let mut call_params: Vec<REQUEST_PARAM> = Vec::new();

		if let Some(params) = params {
			let count = params.len();

			wide_params.reserve_exact(count);
			call_params.reserve_exact(count);

			for (k,v) in params {
				let (kw, vw) = (s2w!(k), s2w!(v));
				call_params.push (REQUEST_PARAM {
					name: kw.as_ptr(),
					value: vw.as_ptr(),
				});
				wide_params.push((kw, vw));
			}
		}

		let ok = (_API.SciterHttpRequest)(self.he, url.as_ptr(), data_type, method, call_params.as_ptr(), call_params.len() as u32);
		ok_or!((), ok)
	}

	/// Sends sinking/bubbling event to the child/parent chain of the element.
	pub fn send_event(&self, code: BEHAVIOR_EVENTS, reason: Option<CLICK_REASON>, source: Option<HELEMENT>) -> Result<bool> {
		let mut handled = false as BOOL;
		let r = reason.unwrap_or(CLICK_REASON::SYNTHESIZED);
		let s = source.unwrap_or(self.he);
		let ok = (_API.SciterSendEvent)(self.he, code as u32, s, r as UINT_PTR, &mut handled);
		ok_or!(handled != 0, ok)
	}

	/// Post asynchronously a sinking/bubbling event to the child/parent chain of the element.
	pub fn post_event(&self, code: BEHAVIOR_EVENTS, reason: Option<CLICK_REASON>, source: Option<HELEMENT>) -> Result<()> {
		let r = reason.unwrap_or(CLICK_REASON::SYNTHESIZED);
		let s = source.unwrap_or(self.he);
		let ok = (_API.SciterPostEvent)(self.he, code as u32, s, r as UINT_PTR);
		ok_or!((), ok)
	}

	/// Send or posts event to the child/parent chain of the element.
	pub fn fire_event(&self, code: BEHAVIOR_EVENTS, reason: Option<CLICK_REASON>, source: Option<HELEMENT>, post: bool, data: Option<Value>) -> Result<bool> {
		let mut handled = false as BOOL;
		let mut params = BEHAVIOR_EVENT_PARAMS {
			cmd: code as UINT,
			reason: reason.unwrap_or(CLICK_REASON::SYNTHESIZED) as UINT_PTR,
			he: source.unwrap_or(self.he),
			heTarget: self.he,
			data: Default::default(),
			name: 0 as LPCWSTR,
		};
		if let Some(data) = data {
			data.pack_to(&mut params.data);
		}
		let ok = (_API.SciterFireEvent)(&params, post as BOOL, &mut handled);
		ok_or!(handled != 0, ok)
	}

	/// Send or posts event with specified params to the child/parent chain of the element.
	pub fn fire_event_params(evt: &BEHAVIOR_EVENT_PARAMS, post: bool) -> Result<bool> {
		let mut handled = false as BOOL;
		let ok = (_API.SciterFireEvent)(evt, post as BOOL, &mut handled);
		ok_or!(handled != 0, ok)
	}


	/// Evaluate the given script in context of the element.
	pub fn eval_script(&self, script: &str) -> Result<Value> {
		let mut rv = Value::new();
		let (s,n) = s2wn!(script);
		let ok = (_API.SciterEvalElementScript)(self.he, s.as_ptr(), n, rv.as_ptr());
		return ok_or!(rv, ok, SCDOM_RESULT::OPERATION_FAILED);
	}

	/// Call scripting function defined in the namespace of the element (a.k.a. global function).
	///
	/// You can use the [`make_args!(args...)`](../macro.make_args.html) macro which helps you
	/// to construct script arguments from Rust types.
	pub fn call_function(&self, name: &str, args: &[Value]) -> Result<Value> {
		let mut rv = Value::new();
		let name = s2u!(name);
		let argv = Value::pack_args(args);
		let ok = (_API.SciterCallScriptingFunction)(self.he, name.as_ptr(), argv.as_ptr(), argv.len() as UINT, rv.as_ptr());
		return ok_or!(rv, ok, SCDOM_RESULT::OPERATION_FAILED);
	}

	/// Call scripting method defined for the element.
	///
	/// You can use the [`make_args!(args...)`](../macro.make_args.html) macro which helps you
	/// to construct script arguments from Rust types.
	pub fn call_method(&self, name: &str, args: &[Value]) -> Result<Value> {
		let mut rv = Value::new();
		let name = s2u!(name);
		let argv = Value::pack_args(args);
		let ok = (_API.SciterCallScriptingMethod)(self.he, name.as_ptr(), argv.as_ptr(), argv.len() as UINT, rv.as_ptr());
		return ok_or!(rv, ok, SCDOM_RESULT::OPERATION_FAILED);
	}

  /// Call behavior specific method.
  pub fn call_behavior_method(&self, params: event::MethodParams) -> Result<()> {
    let call = |p| {
      (_API.SciterCallBehaviorMethod)(self.he, p)
    };
    use capi::scbehavior::{METHOD_PARAMS, VALUE_PARAMS, IS_EMPTY_PARAMS};
    use capi::scbehavior::BEHAVIOR_METHOD_IDENTIFIERS::*;
    let ok = match params {
      event::MethodParams::Click => {
        let mut p = METHOD_PARAMS {
          method: DO_CLICK as u32,
        };
        call(&mut p as *mut _)
      },
      event::MethodParams::SetValue(v) => {
        let mut p = VALUE_PARAMS {
          method: SET_VALUE as u32,
          value: Default::default(),
        };
        v.pack_to(&mut p.value);
        call(&mut p as *mut _ as *mut METHOD_PARAMS)
      },
      event::MethodParams::GetValue(retv) => {
        let mut p = VALUE_PARAMS {
          method: SET_VALUE as u32,
          value: Default::default(),
        };
        let ok = call(&mut p as *mut _ as *mut METHOD_PARAMS);
        if ok != SCDOM_RESULT::OK {
          return Err(ok);
        }
        *retv = Value::from(&p.value);
        ok
      },
      event::MethodParams::IsEmpty(retv) => {
        let mut p = IS_EMPTY_PARAMS {
          method: IS_EMPTY as u32,
          is_empty: Default::default(),
        };
        let ok = call(&mut p as *mut _ as *mut METHOD_PARAMS);
        if ok != SCDOM_RESULT::OK {
          return Err(ok);
        }
        *retv = p.is_empty != 0;
        ok
      },

      _ => {
        // Can't handle `MethodParams::Custom` yet.
        SCDOM_RESULT::INVALID_PARAMETER
      },
    };
    ok_or!((), ok)
  }


	//\name Attributes
	/// Get number of the attributes.
	pub fn attribute_count(&self) -> usize {
		let mut n = 0u32;
		(_API.SciterGetAttributeCount)(self.he, &mut n);
		return n as usize;
	}

	/// Get attribute name by its index.
	pub fn attribute_name(&self, index: usize) -> String {
		let mut s = String::new();
		(_API.SciterGetNthAttributeNameCB)(self.he, index as UINT, store_astr, &mut s as *mut String as LPVOID);
		return s;
	}

	/// Get attribute value by its index.
	pub fn attribute(&self, index: usize) -> String {
		let mut s = String::new();
		(_API.SciterGetNthAttributeValueCB)(self.he, index as UINT, store_wstr, &mut s as *mut String as LPVOID);
		return s;
	}

	/// Get attribute value by its name.
	pub fn get_attribute(&self, name: &str) -> Option<String> {
		let mut s = String::new();
		let name = s2u!(name);
		let ok = (_API.SciterGetAttributeByNameCB)(self.he, name.as_ptr(), store_wstr, &mut s as *mut String as LPVOID);
		match ok {
			SCDOM_RESULT::OK => Some(s),
			// SCDOM_RESULT::OK_NOT_HANDLED => None,
			_ => None,
		}
	}

	/// Add or replace attribute.
	pub fn set_attribute(&mut self, name: &str, value: &str) -> Result<()> {
		let name = s2u!(name);
		let value = s2w!(value);
		let ok = (_API.SciterSetAttributeByName)(self.he, name.as_ptr(), value.as_ptr());
		ok_or!((), ok)
	}

	/// Remove attribute.
	pub fn remove_attribute(&mut self, name: &str) -> Result<()> {
		let name = s2u!(name);
		let value = ::std::ptr::null();
		let ok = (_API.SciterSetAttributeByName)(self.he, name.as_ptr(), value);
		ok_or!((), ok)
	}

	/// Toggle attribute.
	pub fn toggle_attribute(&mut self, name: &str, isset: bool, value: Option<&str>) -> Result<()> {
		if isset {
			self.set_attribute(name, value.unwrap())
		} else {
			self.remove_attribute(name)
		}
	}

	/// Remove all attributes from the element.
	pub fn clear_attributes(&mut self) -> Result<()> {
		let ok = (_API.SciterClearAttributes)(self.he);
		ok_or!((), ok)
	}


	//\name Style Attributes

	/// Get [style attribute](https://sciter.com/docs/content/sciter/Style.htm) of the element by its name.
	pub fn get_style_attribute(&self, name: &str) -> String {
		let mut s = String::new();
		let name = s2u!(name);
		(_API.SciterGetStyleAttributeCB)(self.he, name.as_ptr(), store_wstr, &mut s as *mut String as LPVOID);
		return s;
	}

	/// Set [style attribute](https://sciter.com/docs/content/sciter/Style.htm).
	pub fn set_style_attribute(&mut self, name: &str, value: &str) -> Result<()> {
		let name = s2u!(name);
		let value = s2w!(value);
		let ok = (_API.SciterSetStyleAttribute)(self.he, name.as_ptr(), value.as_ptr());
		ok_or!((), ok)
	}

	//\name State methods


	//\name DOM tree access

	/// Get index of this element in its parent collection.
	pub fn index(&self) -> usize {
		let mut n = 0u32;
		(_API.SciterGetElementIndex)(self.he, &mut n as *mut UINT);
		return n as usize;
	}

	/// Get root of the element.
	pub fn root(&self) -> Element {
		if let Some(dad) = self.parent() {
			dad.root()
		} else {
			self.clone()
		}
	}

	/// Get parent element.
	pub fn parent(&self) -> Option<Element> {
		let mut p = HELEMENT!();
		(_API.SciterGetParentElement)(self.he, &mut p);
		if p.is_null() {
			None
		} else {
			Some(Element::from(p))
		}
	}

	/// Get first sibling element.
	pub fn first_sibling(&self) -> Option<Element> {
		if let Some(dad) = self.parent() {
			let count = dad.len();
			if count > 0 {
				return dad.child(0);
			}
		}
		None
	}

	/// Get last sibling element.
	pub fn last_sibling(&self) -> Option<Element> {
		if let Some(dad) = self.parent() {
			let count = dad.len();
			if count > 0 {
				return dad.child(count - 1);
			}
		}
		None
	}

	/// Get next sibling element.
	pub fn next_sibling(&self) -> Option<Element> {
		let idx = self.index() + 1;
		if let Some(dad) = self.parent() {
			let count = dad.len();
			if idx < count {
				return dad.child(idx);
			}
		}
		None
	}

	/// Get previous sibling element.
	pub fn prev_sibling(&self) -> Option<Element> {
		let idx = self.index();
		if let Some(dad) = self.parent() {
			let count = dad.len();
			if idx > 0 && (idx - 1) < count {
				return dad.child(idx - 1);
			}
		}
		None
	}

	/// Get first child element.
	pub fn first_child(&self) -> Option<Element> {
		return self.child(0);
	}

	/// Get last child element.
	pub fn last_child(&self) -> Option<Element> {
		let count = self.len();
		if count > 0 {
			return self.child(count - 1);
		}
		None
	}

	/// Get element's child at specified index.
	pub fn get(&self, index: usize) -> Option<Element> {
		return self.child(index);
	}

	/// An iterator over the direct children of a DOM element.
	pub fn children(&self) -> Children {
		Children {
			base: self,
			index: 0,
			count: self.children_count(),
		}
	}

	/// Get element's child at specified index.
	pub fn child(&self, index: usize) -> Option<Element> {
		let mut p = HELEMENT!();
		let ok = (_API.SciterGetNthChild)(self.he, index as UINT, &mut p);
		match ok {
			SCDOM_RESULT::OK => Some(Element::from(p)),
			_ => None
		}
	}

	/// Get number of child elements.
	pub fn children_count(&self) -> usize {
		let mut n = 0u32;
		(_API.SciterGetChildrenCount)(self.he, &mut n as *mut UINT);
		return n as usize;
	}

	/// Get number of child elements.
	pub fn len(&self) -> usize {
		return self.children_count();
	}

	/// Returns `true` is `self` has zero elements.
	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}

	/// Clear content of the element.
	pub fn clear(&mut self) -> Result<()> {
		let ok = (_API.SciterSetElementText)(self.he, ::std::ptr::null(), 0);
		ok_or!((), ok)
	}

	/// Create new element as copy of existing element.
	///
	/// The new element is a full (deep) copy of the element and is initially disconnected from the DOM.
	/// Note that [`Element.clone()`](struct.Element.html#impl-Clone) does not clone the DOM element,
	/// just increments its reference count.
	pub fn clone_element(&self) -> Element {
		let mut e = Element { he: HELEMENT!() };
		(_API.SciterCloneElement)(self.he, &mut e.he);
		return e;
	}

	/// Insert element at `index` position of this element.
	///
	/// Note that we cannot follow Rust semantic here
	/// because the newly created `Element` is unusable before it will be inserted at DOM.
	pub fn insert(&mut self, index: usize, child: &Element) -> Result<()> {
		let ok = (_API.SciterInsertElement)(child.he, self.he, index as UINT);
		ok_or!((), ok)
	}

	/// Append element as last child of this element.
	pub fn append(&mut self, child: &Element) -> Result<()> {
		self.insert(0x7FFF_FFFF, child)
	}

	/// Append element as last child of this element.
	#[allow(clippy::needless_pass_by_value)]
	pub fn push(&mut self, element: Element) {
		self.append(&element).expect("Could not append element.");
	}

	/// Remove the last child from this element and returns it, or `None` if this element is empty.
	pub fn pop(&mut self) -> Option<Element> {
		let count = self.len();
		if count > 0 {
			if let Some(mut child) = self.get(count - 1) {
				child.detach().expect("Could not detach element.");
				return Some(child);
			}
		}
		return None;
	}

	/// Take element out of its container (and DOM tree).
	pub fn detach(&mut self) -> Result<()> {
		let ok = (_API.SciterDetachElement)(self.he);
		ok_or!((), ok)
	}

	/// Take element out of its container (and DOM tree) and force destruction of all behaviors.
	pub fn destroy(&mut self) -> Result<()> {
		let mut p = HELEMENT!();
		::std::mem::swap(&mut self.he, &mut p);
		let ok = (_API.SciterDeleteElement)(p);
		ok_or!((), ok)
	}

	/// Swap element positions.
	pub fn swap(&mut self, other: &mut Element) -> Result<()> {
		let ok = (_API.SciterSwapElements)(self.he, other.he);
		ok_or!((), ok)
	}

	//\name Selectors

	/// Test this element against CSS selector(s).
	pub fn test(&self, selector: &str) -> bool {
		let mut p = HELEMENT!();
		let s = s2u!(selector);
		(_API.SciterSelectParent)(self.he, s.as_ptr(), 1, &mut p);
		return !p.is_null();
	}

	/// Call specified function for every element in a DOM that meets specified CSS selectors.
	fn select_elements<T: ElementVisitor>(&self, selector: &str, callback: T) -> Result<Vec<Element>> {
		extern "system" fn inner<T: ElementVisitor>(he: HELEMENT, param: LPVOID) -> BOOL {
      let p = param as *mut T;
			let obj = unsafe { &mut *p };
			let e = Element::from(he);
			let stop = obj.on_element(e);
			return stop as BOOL;
		}
		let s = s2u!(selector);
		let handler = Box::new(callback);
    let param = Box::into_raw(handler);
		let ok = (_API.SciterSelectElements)(self.he, s.as_ptr(), inner::<T>, param as LPVOID);
    let handler = unsafe { Box::from_raw(param) };
		if ok != SCDOM_RESULT::OK {
			return Err(ok);
		}
		return Ok(handler.result());
	}

	/// Will find first parent element starting from this satisfying given css selector(s).
	pub fn find_nearest_parent(&self, selector: &str) -> Result<Option<Element>> {
		let mut p = HELEMENT!();
		let s = s2u!(selector);
		let ok = (_API.SciterSelectParent)(self.he, s.as_ptr(), 0, &mut p);
		if ok != SCDOM_RESULT::OK {
			return Err(ok);
		}
		if p.is_null() { Ok(None) } else { Ok(Some(Element::from(p))) }
	}

	/// Will find first element starting from this satisfying given css selector(s).
	pub fn find_first(&self, selector: &str) -> Result<Option<Element>> {
		let cb = FindFirstElement::default();
		let all = self.select_elements(selector, cb);
		all.map(|mut x| { x.pop() })
	}

	/// Will find all elements starting from this satisfying given css selector(s).
	pub fn find_all(&self, selector: &str) -> Result<Option<Vec<Element>>> {
		let cb = FindAllElements::default();
		let all = self.select_elements(selector, cb);
		all.map(Some)
	}

	//\name Scroll methods:

	//\name Other methods:

	/// Apply changes and refresh element area in its window.
	pub fn update(&self, render_now: bool) -> Result<()> {
		let ok = (_API.SciterUpdateElement)(self.he, render_now as BOOL);
		ok_or!((), ok)
	}

	/// Refresh element area in its window.
	///
	/// If the element has drawing behavior attached it will receive [`on_draw`](event/trait.EventHandler.html#method.on_draw) call after that.
	pub fn refresh(&self) -> Result<()> {
		let rect = self.get_location(ELEMENT_AREAS::self_content())?;
		let ok = (_API.SciterRefreshElementArea)(self.he, rect);
		ok_or!((), ok)
	}

	/// Start Timer for the element.
	///
	/// Element will receive [`on_timer`](event/trait.EventHandler.html#method.on_timer) events.
	///
	/// Note that timer events are not bubbling, so you need attach handler to the target element directly.
	pub fn start_timer(&self, period_ms: u32, timer_id: u64) -> Result<()> {
		let ok = (_API.SciterSetTimer)(self.he, period_ms as UINT, timer_id as ::capi::sctypes::UINT_PTR);
		ok_or!((), ok)
	}

	/// Stop Timer for the element.
	pub fn stop_timer(&self, timer_id: u64) -> Result<()> {
		if !self.he.is_null() {
			let ok = (_API.SciterSetTimer)(self.he, 0 as UINT, timer_id as ::capi::sctypes::UINT_PTR);
			ok_or!((), ok)
		} else {
			Ok(())
		}
	}

	/// Attach the native event handler to this element.
	pub fn attach_handler<Handler: EventHandler>(&mut self, handler: Handler) -> Result<u64> {
		// make native handler
		let boxed = Box::new(handler);
		let ptr = Box::into_raw(boxed);	// dropped in `_event_handler_proc`
		let token = ptr as usize as u64;
		let ok = (_API.SciterAttachEventHandler)(self.he, ::eventhandler::_event_handler_proc::<Handler>, ptr as LPVOID);
		ok_or!(token, ok)
	}

	/// Detach your handler from the element. Handlers identified by `token` from `attach_handler()` result.
	pub fn detach_handler<Handler: EventHandler>(&mut self, token: u64) -> Result<()> {
		let ptr = token as usize as *mut Handler;
		let ok = (_API.SciterDetachEventHandler)(self.he, ::eventhandler::_event_handler_proc::<Handler>, ptr as LPVOID);
		ok_or!((), ok)
	}
}

/// Release element pointer.
impl Drop for Element {
	fn drop(&mut self) {
		(_API.Sciter_UnuseElement)(self.he);
		self.he = HELEMENT!();
	}
}

/// Increment reference count of the dom element.
impl Clone for Element {
	fn clone(&self) -> Self {
		Element::from(self.he)
	}
}

/// Human element representation.
impl ::std::fmt::Display for Element {
	fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
		if self.he.is_null() {
			return f.write_str("None");
		}
		// "tag#id.class|type(name)"
		// "tag#id.class"

    let tag = self.get_tag();
		f.write_str(&tag)?;

		if let Some(i) = self.get_attribute("id") {
			write!(f, "#{}", i)?;
		}
    if let Some(c) = self.get_attribute("class") {
			write!(f, ".{}", c)?;
		}
    if let Some(t) = self.get_attribute("type") {
			write!(f, "|{}", t)?;
		}
    if let Some(n) = self.get_attribute("name") {
			write!(f, "({})", n)?;
		}
		return Ok(());
	}
}

/// Machine-like element visualization (`{:?}` and `{:#?}`).
impl ::std::fmt::Debug for Element {
	fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
		if f.alternate() {
			use ::std::mem;

			fn state_name(value: &ELEMENT_STATE_BITS) -> &'static str {
				match *value {
					ELEMENT_STATE_BITS::STATE_LINK => "link",
					ELEMENT_STATE_BITS::STATE_HOVER => "hover",
					ELEMENT_STATE_BITS::STATE_ACTIVE => "active",
					ELEMENT_STATE_BITS::STATE_VISITED => "visited",
					ELEMENT_STATE_BITS::STATE_FOCUS => "focus",
					ELEMENT_STATE_BITS::STATE_POPUP => "popup",
					ELEMENT_STATE_BITS::STATE_CURRENT => "current",
					ELEMENT_STATE_BITS::STATE_CHECKED => "checked",
					ELEMENT_STATE_BITS::STATE_EXPANDED => "expanded",
					ELEMENT_STATE_BITS::STATE_COLLAPSED => "collapsed",
					ELEMENT_STATE_BITS::STATE_DISABLED => "disabled",
					ELEMENT_STATE_BITS::STATE_INCOMPLETE => "incomplete",
					ELEMENT_STATE_BITS::STATE_BUSY => "busy",
					ELEMENT_STATE_BITS::STATE_ANIMATING => "animating",
					ELEMENT_STATE_BITS::STATE_FOCUSABLE => "",
					ELEMENT_STATE_BITS::STATE_READONLY => "readonly",
					ELEMENT_STATE_BITS::STATE_EMPTY => "empty",
					ELEMENT_STATE_BITS::STATE_ANCHOR => "anchor",
					ELEMENT_STATE_BITS::STATE_SYNTHETIC => "synthetic",
					ELEMENT_STATE_BITS::STATE_OWNS_POPUP => "owns_popup",
					ELEMENT_STATE_BITS::STATE_TABFOCUS => "tabfocus",
					ELEMENT_STATE_BITS::STATE_IS_RTL => "is_rtl",
					ELEMENT_STATE_BITS::STATE_IS_LTR => "is_ltr",
					ELEMENT_STATE_BITS::STATE_DRAG_OVER => "drag_over",
					ELEMENT_STATE_BITS::STATE_DROP_TARGET => "drop_target",
					ELEMENT_STATE_BITS::STATE_MOVING => "moving",
					ELEMENT_STATE_BITS::STATE_COPYING => "copying",
					ELEMENT_STATE_BITS::STATE_DRAG_SOURCE => "drag_source",
					ELEMENT_STATE_BITS::STATE_DROP_MARKER => "drop_marker",

					ELEMENT_STATE_BITS::STATE_READY => "",
					ELEMENT_STATE_BITS::STATE_PRESSED => "pressed",

					ELEMENT_STATE_BITS::STATE_NONE => "",
				}
			}

			// "tag#id.class:state1:state2..."
			let state = self.get_state() as u32;

			write!(f, "{{{}", self)?;
			for i in 0..31 {
				let bit = state & (1 << i);
				if bit != 0 {
					let state_bit: ELEMENT_STATE_BITS = unsafe { mem::transmute(bit) };
					let name = state_name(&state_bit);
					if !name.is_empty() {
						write!(f, ":{}", name)?;
					}
				}
			}
			write!(f, "}}")

		} else {
			// "tag#id.class(name):0xdfdfdf"
			write!(f, "{{{}:{:?}}}", self, self.he)
		}
	}
}


/// An iterator over the direct children of a DOM element.
pub struct Children<'a> {
	base: &'a Element,
	index: usize,
	count: usize,
}

/// Allows `for child in el.children() {}` enumeration.
impl<'a> ::std::iter::Iterator for Children<'a> {
	type Item = Element;

	fn next(&mut self) -> Option<Element> {
		if self.index < self.count {
			let pos = self.index;
			self.index += 1;
			self.base.child(pos)
		} else {
			None
		}
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		let remain = self.count - self.index;
		(remain, Some(remain))
	}

	fn count(self) -> usize {
		self.count
	}
}

impl<'a> ::std::iter::DoubleEndedIterator for Children<'a> {
	fn next_back(&mut self) -> Option<Element> {
		if self.index == self.count || self.count == 0 {
			None
		} else {
			self.count -= 1;
			self.base.child(self.count)
		}
	}
}

/// Allows `for child in &el {}` enumeration.
impl<'a> ::std::iter::IntoIterator for &'a Element {
	type Item = Element;
	type IntoIter = Children<'a>;

	fn into_iter(self) -> Children<'a> {
		self.children()
	}
}


/* Not implemented yet or not used APIs:

SciterCallBehaviorMethod
SciterCombineURL
SciterControlGetType
SciterGetElementIntrinsicHeight
SciterGetElementIntrinsicWidths
SciterGetElementLocation
SciterGetElementNamespace
SciterGetElementType
SciterGetExpando
SciterGetObject
SciterGetScrollInfo
SciterHidePopup
SciterHttpRequest
SciterIsElementEnabled
SciterIsElementVisible
SciterReleaseCapture
SciterRequestElementData
SciterScrollToView
SciterSetCapture
SciterSetHighlightedElement
SciterSetScrollPos
SciterShowPopup
SciterShowPopupAt
SciterSortElements
SciterTraverseUIEvent

SciterCreateCommentNode
SciterCreateTextNode
SciterNodeAddRef
SciterNodeCastFromElement
SciterNodeCastToElement
SciterNodeChildrenCount
SciterNodeFirstChild
SciterNodeGetText
SciterNodeInsert
SciterNodeLastChild
SciterNodeNextSibling
SciterNodeNthChild
SciterNodeParent
SciterNodePrevSibling
SciterNodeRelease
SciterNodeRemove
SciterNodeSetText
SciterNodeType

*/

pub mod event {
	//!
	//! Behaviors support (a.k.a windowless controls).
	//!
	/*!

# Behaviors and event handling.

The primary goal of the User Interface (UI) as a subsystem is to present some information to the user
and generate some events according to user’s actions.
Your application handles UI events and acts accordingly executing its functions.

To be able to handle events in native code you will need to attach an instance of
[`sciter::EventHandler`](trait.EventHandler.html)
to an existing DOM element or to the window itself.
In `EventHandler`'s implementation you will receive all events
dispatched to the element and its children as before children (in [`PHASE_MASK::SINKING`](enum.PHASE_MASK.html) phase)
as after them ([`PHASE_MASK::BUBBLING`](enum.PHASE_MASK.html) event phase),
see [Events Propagation](https://sciter.com/developers/for-native-gui-programmers/#events-propagation).

`EventHandler` attached to the window will receive all DOM events no matter which element they are targeted to.

`EventHandler` contains [various methods](trait.EventHandler.html#provided-methods) –
receivers of events of various types.
You can override any of these methods in order to receive events you are interested in
in your implementation of `sciter::EventHandler`.


To attach a native event handler to a DOM element or to the window you can do one of these:

* "Manually", to a Sciter window: `sciter::Window.event_handler(your_event_handler)`
* "Manually", to an arbitrary DOM element: `sciter::dom::Element.attach_handler(your_event_handler)`
* To a group of DOM elements by declaration in CSS: `selector { behavior:your-behavior-name }`

You also can assign events handlers defined in script code:

* "Manually", individual events: if you have a reference `el` of some element then
to handle mouse events you can do this, for example:

```tiscript
el.onMouse = function(evt) { ... }
```

* "Manually", by assigning a behavior class to the [Element](https://sciter.com/docs/content/sciter/Element.htm):

```tiscript
class MyEventsHandler: Element { ... }  // your behavior class which inherits sciter's Element class
el.prototype = MyEventsHandler; // "sub-class" the element.
```

* By declaration in CSS - to all elements satisfying some CSS selector:

```css
selector { prototype: MyEventsHandler; }
```

In this case `MyEventsHandler` class should be defined in one of script files loaded by your HTML.

See the **Behavior attributes** section of [Sciter CSS property map](https://sciter.com/docs/content/css/cssmap.html)
and [this blog article](http://www.terrainformatica.com/2014/07/sciter-declarative-behavior-assignment-by-css-prototype-and-aspect-properties/) which covers
Behaviors, Prototypes and Aspects of Sciter CSS behavior assignment.





# Script and native code interaction


In Sciter you may want to define native functions that can be called by script.
At the same time you may need to call script functions from native code.
Sciter supports such interaction providing set of simple API functions:

## Evaluating scripts and invoking script functions from native code

You can use one of these methods to call scripts from code of your application:

* To evaluate arbitrary script in context of current document loaded into the window:

```rust,no_run
# use sciter::dom::Element;
# use sciter::Value;
# let hwnd = ::std::ptr::null_mut();
let root = Element::from_window(hwnd).unwrap();
let result: Value = root.eval_script("... script ...").unwrap();
```

* To call a global function defined in script using its full name (may include the name of namespaces where it resides):

```ignore
# #[macro_use] extern crate sciter;
# use sciter::Value;
# let root = sciter::dom::Element::from(::std::ptr::null_mut());
let result: Value = root.call_function("namespace.name", &make_args!(1, "2", 3.0)).unwrap();
```
The parameters are passed as a `&[Value]` slice.

* To call a method (function) defined in script for particular DOM element:

```ignore
# #[macro_use] extern crate sciter;
# use sciter::Value;
# let root = sciter::dom::Element::from(::std::ptr::null_mut());
if let Some(el) = root.find_first("input").unwrap() {
  let result: Value = el.call_method("canUndo", &make_args!()).unwrap();
}
```


## Calling native code from script

If needed, your application may expose some [native] functions to be called by script code.
Usually this is made by implementing your own `EventHandler` and overriding its `on_script_call` method.
If you do this, then you can invoke this callback from script as:

* "global" native functions: `var r = view.funcName( p0, p1, ... );` – calling
`on_script_call` of an `EventHandler` instance attached to the **window**.
* As element’s methods: `var r = el.funcName( p0, p1, ... );` – calling
`on_script_call` of an `EventHandler` instance (native behavior) attached to the **element**.

This way you can establish interaction between scipt and native code inside your application.

*/

	pub use capi::scbehavior::{EVENT_GROUPS, BEHAVIOR_EVENTS, PHASE_MASK};
  pub use capi::scbehavior::{CLICK_REASON, EDIT_CHANGED_REASON, DRAW_EVENTS};

	use capi::sctypes::*;
	use capi::scdom::HELEMENT;
	use capi::scgraphics::HGFX;
	use value::Value;

	/// Default subscription events
	///
	/// Default is `HANDLE_BEHAVIOR_EVENT | HANDLE_SCRIPTING_METHOD_CALL` which covers behavior events
	/// (like `document_complete` or `button_click`) and script calls to native window.
	pub fn default_events() -> EVENT_GROUPS {
		return EVENT_GROUPS::HANDLE_BEHAVIOR_EVENT | EVENT_GROUPS::HANDLE_SCRIPTING_METHOD_CALL;
	}

	/// UI action causing change.
	#[derive(Debug)]
	pub enum EventReason {
		/// General event source triggers (by mouse, key or synthesized).
		General(CLICK_REASON),
		/// Edit control change trigger.
		EditChanged(EDIT_CHANGED_REASON),
		/// `<video>` request for frame source binding.
    ///
    /// See the [`sciter::video`](../../video/index.html) module for more reference.
		VideoBind(LPVOID),
	}

  /// Behavior method params.
  ///
  /// Sciter sends these events to native behaviors.
  #[derive(Debug)]
  pub enum MethodParams<'a> {
    /// Click event (either from mouse or code).
    Click,

    /// Get current [`:empty`](https://sciter.com/docs/content/sciter/States.htm) state,
    /// i.e. if behavior has no children and no text.
    IsEmpty(&'a mut bool),

    /// Get the current value of the behavior.
    GetValue(&'a mut Value),

    /// Set the current value of the behavior.
    SetValue(Value),

    /// Custom methods, unknown for engine. Sciter will not intrepret it and will do just dispatching.
    Custom(u32, LPVOID),
  }


	/// DOM event handler which can be attached to any DOM element.
	///
	/// In notifications:
	///
	/// * `root` means the DOM element to which we are attached (`<html>` for `Window` event handlers).
	/// * `target` contains a reference to the notification target DOM element.
	/// * `source` element e.g. in `SELECTION_CHANGED` it is the new selected `<option>`,
	/// in `MENU_ITEM_CLICK` it is a menu item (`<li>`) element.
	///
	/// For example, if we are attached to the `<body>` element,
	/// we will receive `document_complete` with `target` set to `<html>`.
	///
	#[allow(unused_variables)]
	pub trait EventHandler {

		/// Return a list of event groups this event handler is subscribed to.
		///
		/// Default is `HANDLE_BEHAVIOR_EVENT | HANDLE_SCRIPTING_METHOD_CALL`.
		/// See also [`default_events()`](fn.default_events.html).
		fn get_subscription(&mut self) -> Option<EVENT_GROUPS> {
			return Some(default_events());
		}

		/// Called when handler was attached to element or window.
		/// `root` is `NULL` if attaching to window without loaded document.
    ///
    /// **Subscription**: always.
		fn attached(&mut self, root: HELEMENT) {}

		/// Called when handler was detached from element or window.
    ///
    /// **Subscription**: always.
		fn detached(&mut self, root: HELEMENT) {}

		/// Notification that document finishes its loading - all requests for external resources are finished.
    ///
    /// **Subscription**: requires [`HANDLE_BEHAVIOR_EVENT`](enum.EVENT_GROUPS.html),
    /// but will be sent only for the root element (`<html>`).
		fn document_complete(&mut self, root: HELEMENT, target: HELEMENT) {}

		/// The last notification before document removal from the DOM.
    ///
    /// **Subscription**: requires [`HANDLE_BEHAVIOR_EVENT`](enum.EVENT_GROUPS.html),
    /// but will be sent only for the root element (`<html>`).
		fn document_close(&mut self, root: HELEMENT, target: HELEMENT) {}

    /// Behavior method calls from script or other behaviors.
    ///
    /// Return `false` to skip this event.
    ///
    /// **Subscription**: requires [`HANDLE_METHOD_CALL`](enum.EVENT_GROUPS.html).
    fn on_method_call(&mut self, root: HELEMENT, params: MethodParams) -> bool { return false }

		/// Script calls from CSSS! script and TIScript.
    ///
    /// Return `None` to skip this event.
    ///
    /// **Subscription**: requires [`HANDLE_SCRIPTING_METHOD_CALL`](enum.EVENT_GROUPS.html).
		fn on_script_call(&mut self, root: HELEMENT, name: &str, args: &[Value]) -> Option<Value> {
			return self.dispatch_script_call(root, name, args);
		}

		/// Autogenerated dispatcher for script calls.
    #[doc(hidden)]
		fn dispatch_script_call(&mut self, root: HELEMENT, name: &str, args: &[Value]) -> Option<Value> {
			return None;
		}

		/// Return the reference to a native asset assotiated with behavior.
		#[doc(hidden)]
		fn get_asset(&mut self) -> Option<&crate::capi::scom::som_asset_t> {
			// TODO: is this good?
			return None;
		}

		/// Notification event from builtin behaviors.
    ///
    /// Return `false` to skip this event.
    ///
    /// **Subscription**: requires [`HANDLE_BEHAVIOR_EVENT`](enum.EVENT_GROUPS.html).
		fn on_event(&mut self, root: HELEMENT, source: HELEMENT, target: HELEMENT, code: BEHAVIOR_EVENTS, phase: PHASE_MASK, reason: EventReason) -> bool {
			return false;
		}

		/// Timer event from attached element.
    ///
    /// Return `false` to skip this event.
    ///
    /// **Subscription**: requires [`HANDLE_TIMER`](enum.EVENT_GROUPS.html).
		fn on_timer(&mut self, root: HELEMENT, timer_id: u64) -> bool { return false; }

		/// Drawing request event.
		///
		/// It allows to intercept drawing events of an `Element` and to manually draw its content, background and foreground layers.
    ///
    /// Return `false` to skip this event.
    ///
    /// **Subscription**: requires [`HANDLE_DRAW`](enum.EVENT_GROUPS.html).
		fn on_draw(&mut self, root: HELEMENT, gfx: HGFX, area: &RECT, layer: DRAW_EVENTS) -> bool { return false; }

		/// Size changed event.
    ///
    /// **Subscription**: requires [`HANDLE_SIZE`](enum.EVENT_GROUPS.html).
		fn on_size(&mut self, root: HELEMENT) {}
	}

}
