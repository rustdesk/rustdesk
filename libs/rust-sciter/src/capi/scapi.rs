//! Sciter C API interface.

#![allow(non_snake_case, non_camel_case_types)]

use capi::sctypes::*;
use capi::scdef::*;
use capi::scdom::*;
use capi::scvalue::*;
use capi::sctiscript::{HVM, tiscript_value, tiscript_native_interface};
use capi::scbehavior::*;
use capi::scgraphics::SciterGraphicsAPI;
use capi::screquest::{SciterRequestAPI, HREQUEST, REQUEST_PARAM};
use capi::scmsg::{SCITER_X_MSG};
use capi::scom::som_asset_t;


/// Sciter API functions.
#[repr(C)]
#[allow(missing_docs)]
#[doc(hidden)]
pub struct ISciterAPI
{
	pub version: UINT,

	pub SciterClassName: extern "system" fn () -> LPCWSTR,
	pub SciterVersion: extern "system" fn (major: BOOL) -> UINT,

	pub SciterDataReady: extern "system" fn (hwnd: HWINDOW, uri: LPCWSTR, data: LPCBYTE, dataLength: UINT) -> BOOL,
	pub SciterDataReadyAsync: extern "system" fn (hwnd: HWINDOW, uri: LPCWSTR, data: LPCBYTE, dataLength: UINT, requestId: HREQUEST) -> BOOL,

  // #ifdef WINDOWS
  #[cfg(all(windows, not(feature = "windowless")))]
	pub SciterProc: extern "system" fn (hwnd: HWINDOW, msg: UINT, wParam: WPARAM, lParam: LPARAM) -> LRESULT,
	#[cfg(all(windows, not(feature = "windowless")))]
	pub SciterProcND: extern "system" fn (hwnd: HWINDOW, msg: UINT, wParam: WPARAM, lParam: LPARAM, pbHandled: * mut BOOL) -> LRESULT,
  // #endif

	pub SciterLoadFile: extern "system" fn (hWndSciter: HWINDOW, filename: LPCWSTR) -> BOOL,

	pub SciterLoadHtml: extern "system" fn (hWndSciter: HWINDOW, html: LPCBYTE, htmlSize: UINT, baseUrl: LPCWSTR) -> BOOL,
	pub SciterSetCallback: extern "system" fn (hWndSciter: HWINDOW, cb: SciterHostCallback, cbParam: LPVOID) -> VOID,
	pub SciterSetMasterCSS: extern "system" fn (utf8: LPCBYTE, numBytes: UINT) -> BOOL,
	pub SciterAppendMasterCSS: extern "system" fn (utf8: LPCBYTE, numBytes: UINT) -> BOOL,
	pub SciterSetCSS: extern "system" fn (hWndSciter: HWINDOW, utf8: LPCBYTE, numBytes: UINT, baseUrl: LPCWSTR, mediaType: LPCWSTR) -> BOOL,
	pub SciterSetMediaType: extern "system" fn (hWndSciter: HWINDOW, mediaType: LPCWSTR) -> BOOL,
	pub SciterSetMediaVars: extern "system" fn (hWndSciter: HWINDOW, mediaVars: * const VALUE) -> BOOL,
	pub SciterGetMinWidth: extern "system" fn (hWndSciter: HWINDOW) -> UINT,
	pub SciterGetMinHeight: extern "system" fn (hWndSciter: HWINDOW, width: UINT) -> UINT,
	pub SciterCall: extern "system" fn (hWnd: HWINDOW, functionName: LPCSTR, argc: UINT, argv: * const VALUE, retval: * mut VALUE) -> BOOL,
	pub SciterEval: extern "system" fn (hwnd: HWINDOW, script: LPCWSTR, scriptLength: UINT, pretval: * mut VALUE) -> BOOL,
	pub SciterUpdateWindow: extern "system" fn (hwnd: HWINDOW) -> VOID,

  // #ifdef WINDOWS
  #[cfg(all(windows, not(feature = "windowless")))]
	pub SciterTranslateMessage: extern "system" fn (lpMsg: * mut MSG) -> BOOL,
  // #endif

	pub SciterSetOption: extern "system" fn (hWnd: HWINDOW, option: SCITER_RT_OPTIONS, value: UINT_PTR) -> BOOL,
	pub SciterGetPPI: extern "system" fn (hWndSciter: HWINDOW, px: * mut UINT, py: * mut UINT) -> VOID,
	pub SciterGetViewExpando: extern "system" fn (hwnd: HWINDOW, pval: * mut VALUE) -> BOOL,

  // #ifdef WINDOWS
  #[cfg(all(windows, not(feature = "windowless")))]
	pub SciterRenderD2D: extern "system" fn (hWndSciter: HWINDOW, prt: * mut ID2D1RenderTarget) -> BOOL,
	#[cfg(all(windows, not(feature = "windowless")))]
	pub SciterD2DFactory: extern "system" fn (ppf: * mut* mut ID2D1Factory) -> BOOL,
	#[cfg(all(windows, not(feature = "windowless")))]
	pub SciterDWFactory: extern "system" fn (ppf: * mut* mut IDWriteFactory) -> BOOL,
  // #endif

	pub SciterGraphicsCaps: extern "system" fn (pcaps: LPUINT) -> BOOL,
	pub SciterSetHomeURL: extern "system" fn (hWndSciter: HWINDOW, baseUrl: LPCWSTR) -> BOOL,

  // #if defined(OSX)
	#[cfg(all(target_os="macos", not(feature = "windowless")))]
	pub SciterCreateNSView: extern "system" fn (frame: LPRECT) -> HWINDOW, // returns NSView*
  // #endif

  // #if defined(LINUX)
	#[cfg(all(target_os="linux", not(feature = "windowless")))]
	pub SciterCreateWidget: extern "system" fn (frame: LPRECT) -> HWINDOW, // returns GtkWidget
  // #endif

	#[cfg(not(feature = "windowless"))]
	pub SciterCreateWindow: extern "system" fn (creationFlags: UINT, frame: LPCRECT, delegate: * const SciterWindowDelegate, delegateParam: LPVOID, parent: HWINDOW) -> HWINDOW,

	pub SciterSetupDebugOutput: extern "system" fn (hwndOrNull: HWINDOW, param: LPVOID, pfOutput: DEBUG_OUTPUT_PROC),

	//|
	//| DOM Element API
	//|
	pub Sciter_UseElement: extern "system" fn (he: HELEMENT) -> SCDOM_RESULT,
	pub Sciter_UnuseElement: extern "system" fn (he: HELEMENT) -> SCDOM_RESULT,
	pub SciterGetRootElement: extern "system" fn (hwnd: HWINDOW, phe: * mut HELEMENT) -> SCDOM_RESULT,
	pub SciterGetFocusElement: extern "system" fn (hwnd: HWINDOW, phe: * mut HELEMENT) -> SCDOM_RESULT,
	pub SciterFindElement: extern "system" fn (hwnd: HWINDOW, pt: POINT, phe: * mut HELEMENT) -> SCDOM_RESULT,
	pub SciterGetChildrenCount: extern "system" fn (he: HELEMENT, count: * mut UINT) -> SCDOM_RESULT,
	pub SciterGetNthChild: extern "system" fn (he: HELEMENT, n: UINT, phe: * mut HELEMENT) -> SCDOM_RESULT,
	pub SciterGetParentElement: extern "system" fn (he: HELEMENT, p_parent_he: * mut HELEMENT) -> SCDOM_RESULT,
	pub SciterGetElementHtmlCB: extern "system" fn (he: HELEMENT, outer: BOOL, rcv: LPCBYTE_RECEIVER, rcv_param: LPVOID) -> SCDOM_RESULT,
	pub SciterGetElementTextCB: extern "system" fn (he: HELEMENT, rcv: LPCWSTR_RECEIVER, rcv_param: LPVOID) -> SCDOM_RESULT,
	pub SciterSetElementText: extern "system" fn (he: HELEMENT, utf16: LPCWSTR, length: UINT) -> SCDOM_RESULT,
	pub SciterGetAttributeCount: extern "system" fn (he: HELEMENT, p_count: LPUINT) -> SCDOM_RESULT,
	pub SciterGetNthAttributeNameCB: extern "system" fn (he: HELEMENT, n: UINT, rcv: LPCSTR_RECEIVER, rcv_param: LPVOID) -> SCDOM_RESULT,
	pub SciterGetNthAttributeValueCB: extern "system" fn (he: HELEMENT, n: UINT, rcv: LPCWSTR_RECEIVER, rcv_param: LPVOID) -> SCDOM_RESULT,
	pub SciterGetAttributeByNameCB: extern "system" fn (he: HELEMENT, name: LPCSTR, rcv: LPCWSTR_RECEIVER, rcv_param: LPVOID) -> SCDOM_RESULT,
	pub SciterSetAttributeByName: extern "system" fn (he: HELEMENT, name: LPCSTR, value: LPCWSTR) -> SCDOM_RESULT,
	pub SciterClearAttributes: extern "system" fn (he: HELEMENT) -> SCDOM_RESULT,
	pub SciterGetElementIndex: extern "system" fn (he: HELEMENT, p_index: LPUINT) -> SCDOM_RESULT,
	pub SciterGetElementType: extern "system" fn (he: HELEMENT, p_type: * mut LPCSTR) -> SCDOM_RESULT,
	pub SciterGetElementTypeCB: extern "system" fn (he: HELEMENT, rcv: LPCSTR_RECEIVER, rcv_param: LPVOID) -> SCDOM_RESULT,
	pub SciterGetStyleAttributeCB: extern "system" fn (he: HELEMENT, name: LPCSTR, rcv: LPCWSTR_RECEIVER, rcv_param: LPVOID) -> SCDOM_RESULT,
	pub SciterSetStyleAttribute: extern "system" fn (he: HELEMENT, name: LPCSTR, value: LPCWSTR) -> SCDOM_RESULT,
	pub SciterGetElementLocation: extern "system" fn (he: HELEMENT, p_location: LPRECT, areas: UINT /*ELEMENT_AREAS*/) -> SCDOM_RESULT,
	pub SciterScrollToView: extern "system" fn (he: HELEMENT, SciterScrollFlags: UINT) -> SCDOM_RESULT,
	pub SciterUpdateElement: extern "system" fn (he: HELEMENT, andForceRender: BOOL) -> SCDOM_RESULT,
	pub SciterRefreshElementArea: extern "system" fn (he: HELEMENT, rc: RECT) -> SCDOM_RESULT,
	pub SciterSetCapture: extern "system" fn (he: HELEMENT) -> SCDOM_RESULT,
	pub SciterReleaseCapture: extern "system" fn (he: HELEMENT) -> SCDOM_RESULT,
	pub SciterGetElementHwnd: extern "system" fn (he: HELEMENT, p_hwnd: * mut HWINDOW, rootWindow: BOOL) -> SCDOM_RESULT,
	pub SciterCombineURL: extern "system" fn (he: HELEMENT, szUrlBuffer: LPWSTR, UrlBufferSize: UINT) -> SCDOM_RESULT,
	pub SciterSelectElements: extern "system" fn (he: HELEMENT, CSS_selectors: LPCSTR, callback: SciterElementCallback, param: LPVOID) -> SCDOM_RESULT,
	pub SciterSelectElementsW: extern "system" fn (he: HELEMENT, CSS_selectors: LPCWSTR, callback: SciterElementCallback, param: LPVOID) -> SCDOM_RESULT,
	pub SciterSelectParent: extern "system" fn (he: HELEMENT, selector: LPCSTR, depth: UINT, heFound: * mut HELEMENT) -> SCDOM_RESULT,
	pub SciterSelectParentW: extern "system" fn (he: HELEMENT, selector: LPCWSTR, depth: UINT, heFound: * mut HELEMENT) -> SCDOM_RESULT,
	pub SciterSetElementHtml: extern "system" fn (he: HELEMENT, html: * const BYTE, htmlLength: UINT, how: UINT) -> SCDOM_RESULT,
	pub SciterGetElementUID: extern "system" fn (he: HELEMENT, puid: * mut UINT) -> SCDOM_RESULT,
	pub SciterGetElementByUID: extern "system" fn (hwnd: HWINDOW, uid: UINT, phe: * mut HELEMENT) -> SCDOM_RESULT,
	pub SciterShowPopup: extern "system" fn (hePopup: HELEMENT, heAnchor: HELEMENT, placement: UINT) -> SCDOM_RESULT,
	pub SciterShowPopupAt: extern "system" fn (hePopup: HELEMENT, pos: POINT, placement: UINT) -> SCDOM_RESULT,
	pub SciterHidePopup: extern "system" fn (he: HELEMENT) -> SCDOM_RESULT,
	pub SciterGetElementState: extern "system" fn (he: HELEMENT, pstateBits: * mut UINT) -> SCDOM_RESULT,
	pub SciterSetElementState: extern "system" fn (he: HELEMENT, stateBitsToSet: UINT, stateBitsToClear: UINT, updateView: BOOL) -> SCDOM_RESULT,
	pub SciterCreateElement: extern "system" fn (tagname: LPCSTR, textOrNull: LPCWSTR, /*out*/ phe: * mut HELEMENT) -> SCDOM_RESULT,
	pub SciterCloneElement: extern "system" fn (he: HELEMENT, /*out*/ phe: * mut HELEMENT) -> SCDOM_RESULT,
	pub SciterInsertElement: extern "system" fn (he: HELEMENT, hparent: HELEMENT, index: UINT) -> SCDOM_RESULT,
	pub SciterDetachElement: extern "system" fn (he: HELEMENT) -> SCDOM_RESULT,
	pub SciterDeleteElement: extern "system" fn (he: HELEMENT) -> SCDOM_RESULT,
	pub SciterSetTimer: extern "system" fn (he: HELEMENT, milliseconds: UINT, timer_id: UINT_PTR) -> SCDOM_RESULT,
	pub SciterDetachEventHandler: extern "system" fn (he: HELEMENT, pep: ElementEventProc, tag: LPVOID) -> SCDOM_RESULT,
	pub SciterAttachEventHandler: extern "system" fn (he: HELEMENT, pep: ElementEventProc, tag: LPVOID) -> SCDOM_RESULT,
	pub SciterWindowAttachEventHandler: extern "system" fn (hwndLayout: HWINDOW, pep: ElementEventProc, tag: LPVOID, subscription: UINT) -> SCDOM_RESULT,
	pub SciterWindowDetachEventHandler: extern "system" fn (hwndLayout: HWINDOW, pep: ElementEventProc, tag: LPVOID) -> SCDOM_RESULT,
	pub SciterSendEvent: extern "system" fn (he: HELEMENT, appEventCode: UINT, heSource: HELEMENT, reason: UINT_PTR, /*out*/ handled: * mut BOOL) -> SCDOM_RESULT,
	pub SciterPostEvent: extern "system" fn (he: HELEMENT, appEventCode: UINT, heSource: HELEMENT, reason: UINT_PTR) -> SCDOM_RESULT,
	pub SciterCallBehaviorMethod: extern "system" fn (he: HELEMENT, params: * const METHOD_PARAMS) -> SCDOM_RESULT,
	pub SciterRequestElementData: extern "system" fn (he: HELEMENT, url: LPCWSTR, dataType: UINT, initiator: HELEMENT) -> SCDOM_RESULT,
	pub SciterHttpRequest: extern "system" fn (he: HELEMENT, url: LPCWSTR, dataType: UINT, requestType: UINT, requestParams: * const REQUEST_PARAM, nParams: UINT) -> SCDOM_RESULT,
	pub SciterGetScrollInfo: extern "system" fn (he: HELEMENT, scrollPos: LPPOINT, viewRect: LPRECT, contentSize: LPSIZE) -> SCDOM_RESULT,
	pub SciterSetScrollPos: extern "system" fn (he: HELEMENT, scrollPos: POINT, smooth: BOOL) -> SCDOM_RESULT,
	pub SciterGetElementIntrinsicWidths: extern "system" fn (he: HELEMENT, pMinWidth: * mut INT, pMaxWidth: * mut INT) -> SCDOM_RESULT,
	pub SciterGetElementIntrinsicHeight: extern "system" fn (he: HELEMENT, forWidth: INT, pHeight: * mut INT) -> SCDOM_RESULT,
	pub SciterIsElementVisible: extern "system" fn (he: HELEMENT, pVisible: * mut BOOL) -> SCDOM_RESULT,
	pub SciterIsElementEnabled: extern "system" fn (he: HELEMENT, pEnabled: * mut BOOL) -> SCDOM_RESULT,
	pub SciterSortElements: extern "system" fn (he: HELEMENT, firstIndex: UINT, lastIndex: UINT, cmpFunc: * mut ELEMENT_COMPARATOR, cmpFuncParam: LPVOID) -> SCDOM_RESULT,
	pub SciterSwapElements: extern "system" fn (he1: HELEMENT, he2: HELEMENT) -> SCDOM_RESULT,
	pub SciterTraverseUIEvent: extern "system" fn (evt: UINT, eventCtlStruct: LPVOID, bOutProcessed: * mut BOOL) -> SCDOM_RESULT,
	pub SciterCallScriptingMethod: extern "system" fn (he: HELEMENT, name: LPCSTR, argv: * const VALUE, argc: UINT, retval: * mut VALUE) -> SCDOM_RESULT,
	pub SciterCallScriptingFunction: extern "system" fn (he: HELEMENT, name: LPCSTR, argv: * const VALUE, argc: UINT, retval: * mut VALUE) -> SCDOM_RESULT,
	pub SciterEvalElementScript: extern "system" fn (he: HELEMENT, script: LPCWSTR, scriptLength: UINT, retval: * mut VALUE) -> SCDOM_RESULT,
	pub SciterAttachHwndToElement: extern "system" fn (he: HELEMENT, hwnd: HWINDOW) -> SCDOM_RESULT,
	pub SciterControlGetType: extern "system" fn (he: HELEMENT, /*CTL_TYPE*/ pType: * mut UINT) -> SCDOM_RESULT,
	pub SciterGetValue: extern "system" fn (he: HELEMENT, pval: * mut VALUE) -> SCDOM_RESULT,
	pub SciterSetValue: extern "system" fn (he: HELEMENT, pval: * const VALUE) -> SCDOM_RESULT,
	pub SciterGetExpando: extern "system" fn (he: HELEMENT, pval: * mut VALUE, forceCreation: BOOL) -> SCDOM_RESULT,

	#[deprecated(since="Sciter 4.4.3.24", note="TIScript native API is gone, use SOM instead.")]
	pub SciterGetObject: extern "system" fn (he: HELEMENT, pval: * mut tiscript_value, forceCreation: BOOL) -> SCDOM_RESULT,

	#[deprecated(since="Sciter 4.4.3.24", note="TIScript native API is gone, use SOM instead.")]
	pub SciterGetElementNamespace: extern "system" fn (he: HELEMENT, pval: * mut tiscript_value) -> SCDOM_RESULT,

	pub SciterGetHighlightedElement: extern "system" fn (hwnd: HWINDOW, phe: * mut HELEMENT) -> SCDOM_RESULT,
	pub SciterSetHighlightedElement: extern "system" fn (hwnd: HWINDOW, he: HELEMENT) -> SCDOM_RESULT,
	//|
	//| DOM Node API
	//|
	pub SciterNodeAddRef: extern "system" fn (hn: HNODE) -> SCDOM_RESULT,
	pub SciterNodeRelease: extern "system" fn (hn: HNODE) -> SCDOM_RESULT,
	pub SciterNodeCastFromElement: extern "system" fn (he: HELEMENT, phn: * mut HNODE) -> SCDOM_RESULT,
	pub SciterNodeCastToElement: extern "system" fn (hn: HNODE, he: * mut HELEMENT) -> SCDOM_RESULT,
	pub SciterNodeFirstChild: extern "system" fn (hn: HNODE, phn: * mut HNODE) -> SCDOM_RESULT,
	pub SciterNodeLastChild: extern "system" fn (hn: HNODE, phn: * mut HNODE) -> SCDOM_RESULT,
	pub SciterNodeNextSibling: extern "system" fn (hn: HNODE, phn: * mut HNODE) -> SCDOM_RESULT,
	pub SciterNodePrevSibling: extern "system" fn (hn: HNODE, phn: * mut HNODE) -> SCDOM_RESULT,
	pub SciterNodeParent: extern "system" fn (hnode: HNODE, pheParent: * mut HELEMENT) -> SCDOM_RESULT,
	pub SciterNodeNthChild: extern "system" fn (hnode: HNODE, n: UINT, phn: * mut HNODE) -> SCDOM_RESULT,
	pub SciterNodeChildrenCount: extern "system" fn (hnode: HNODE, pn: * mut UINT) -> SCDOM_RESULT,
	pub SciterNodeType: extern "system" fn (hnode: HNODE, pNodeType: * mut UINT /*NODE_TYPE*/) -> SCDOM_RESULT,
	pub SciterNodeGetText: extern "system" fn (hnode: HNODE, rcv: * mut LPCWSTR_RECEIVER, rcv_param: LPVOID) -> SCDOM_RESULT,
	pub SciterNodeSetText: extern "system" fn (hnode: HNODE, text: LPCWSTR, textLength: UINT) -> SCDOM_RESULT,
	pub SciterNodeInsert: extern "system" fn (hnode: HNODE, how: UINT /*NODE_INS_TARGET*/, what: HNODE) -> SCDOM_RESULT,
	pub SciterNodeRemove: extern "system" fn (hnode: HNODE, finalize: BOOL) -> SCDOM_RESULT,
	pub SciterCreateTextNode: extern "system" fn (text: LPCWSTR, textLength: UINT, phnode: * mut HNODE) -> SCDOM_RESULT,
	pub SciterCreateCommentNode: extern "system" fn (text: LPCWSTR, textLength: UINT, phnode: * mut HNODE) -> SCDOM_RESULT,
	//|
	//| Value API
	//|
	pub ValueInit: extern "system" fn (pval: * mut VALUE) -> VALUE_RESULT,
	pub ValueClear: extern "system" fn (pval: * mut VALUE) -> VALUE_RESULT,
	pub ValueCompare: extern "system" fn (pval1: * const VALUE, pval2: * const VALUE) -> VALUE_RESULT,
	pub ValueCopy: extern "system" fn (pdst: * mut VALUE, psrc: * const VALUE) -> VALUE_RESULT,
	pub ValueIsolate: extern "system" fn (pdst: * mut VALUE) -> VALUE_RESULT,
	pub ValueType: extern "system" fn (pval: * const VALUE, pType: * mut UINT, pUnits: * mut UINT) -> VALUE_RESULT,
	pub ValueStringData: extern "system" fn (pval: * const VALUE, pChars: * mut LPCWSTR, pNumChars: * mut UINT) -> VALUE_RESULT,
	pub ValueStringDataSet: extern "system" fn (pval: * mut VALUE, chars: LPCWSTR, numChars: UINT, units: UINT) -> VALUE_RESULT,
	pub ValueIntData: extern "system" fn (pval: * const VALUE, pData: * mut INT) -> VALUE_RESULT,
	pub ValueIntDataSet: extern "system" fn (pval: * mut VALUE, data: INT, vtype: UINT, units: UINT) -> VALUE_RESULT,
	pub ValueInt64Data: extern "system" fn (pval: * const VALUE, pData: * mut INT64) -> VALUE_RESULT,
	pub ValueInt64DataSet: extern "system" fn (pval: * mut VALUE, data: INT64, vtype: UINT, units: UINT) -> VALUE_RESULT,
	pub ValueFloatData: extern "system" fn (pval: * const VALUE, pData: * mut FLOAT_VALUE) -> VALUE_RESULT,
	pub ValueFloatDataSet: extern "system" fn (pval: * mut VALUE, data: FLOAT_VALUE, vtype: UINT, units: UINT) -> VALUE_RESULT,
	pub ValueBinaryData: extern "system" fn (pval: * const VALUE, pBytes: * mut LPCBYTE, pnBytes: * mut UINT) -> VALUE_RESULT,
	pub ValueBinaryDataSet: extern "system" fn (pval: * mut VALUE, pBytes: LPCBYTE, nBytes: UINT, vtype: UINT, units: UINT) -> VALUE_RESULT,
	pub ValueElementsCount: extern "system" fn (pval: * const VALUE, pn: * mut INT) -> VALUE_RESULT,
	pub ValueNthElementValue: extern "system" fn (pval: * const VALUE, n: INT, pretval: * mut VALUE) -> VALUE_RESULT,
	pub ValueNthElementValueSet: extern "system" fn (pval: * mut VALUE, n: INT, pval_to_set: * const VALUE) -> VALUE_RESULT,
	pub ValueNthElementKey: extern "system" fn (pval: * const VALUE, n: INT, pretval: * mut VALUE) -> VALUE_RESULT,
	pub ValueEnumElements: extern "system" fn (pval: * const VALUE, penum: KeyValueCallback, param: LPVOID) -> VALUE_RESULT,
	pub ValueSetValueToKey: extern "system" fn (pval: * mut VALUE, pkey: * const VALUE, pval_to_set: * const VALUE) -> VALUE_RESULT,
	pub ValueGetValueOfKey: extern "system" fn (pval: * const VALUE, pkey: * const VALUE, pretval: * mut VALUE) -> VALUE_RESULT,
	pub ValueToString: extern "system" fn (pval: * mut VALUE, how: VALUE_STRING_CVT_TYPE) -> VALUE_RESULT,
	pub ValueFromString: extern "system" fn (pval: * mut VALUE, str: LPCWSTR, strLength: UINT, how: VALUE_STRING_CVT_TYPE) -> UINT,
	pub ValueInvoke: extern "system" fn (pval: * const VALUE, pthis: * mut VALUE, argc: UINT, argv: * const VALUE, pretval: * mut VALUE, url: LPCWSTR) -> VALUE_RESULT,
	pub ValueNativeFunctorSet: extern "system" fn (pval: * mut VALUE, pinvoke: NATIVE_FUNCTOR_INVOKE, prelease: NATIVE_FUNCTOR_RELEASE, tag: LPVOID) -> VALUE_RESULT,
	pub ValueIsNativeFunctor: extern "system" fn (pval: * const VALUE) -> BOOL,

	// tiscript VM API

	#[deprecated(since="Sciter 4.4.3.24", note="TIScript native API is gone, use SOM instead.")]
	pub TIScriptAPI: extern "system" fn () -> * mut tiscript_native_interface,

	#[deprecated(since="Sciter 4.4.3.24", note="TIScript native API is gone, use SOM instead.")]
	pub SciterGetVM: extern "system" fn (hwnd: HWINDOW) -> HVM,

	// since 3.1.0.12
	#[deprecated(since="Sciter 4.4.3.24", note="TIScript native API is gone, use SOM instead.")]
	pub Sciter_v2V: extern "system" fn (vm: HVM, script_value: tiscript_value, value: * mut VALUE, isolate: BOOL) -> BOOL,

	#[deprecated(since="Sciter 4.4.3.24", note="TIScript native API is gone, use SOM instead.")]
	pub Sciter_V2v: extern "system" fn (vm: HVM, valuev: * const VALUE, script_value: * mut tiscript_value) -> BOOL,

	// since 3.1.0.18
	pub SciterOpenArchive: extern "system" fn (archiveData: LPCBYTE, archiveDataLength: UINT) -> HSARCHIVE,
	pub SciterGetArchiveItem: extern "system" fn (harc: HSARCHIVE, path: LPCWSTR, pdata: * mut LPCBYTE, pdataLength: * mut UINT) -> BOOL,
	pub SciterCloseArchive: extern "system" fn (harc: HSARCHIVE) -> BOOL,

	// since 3.2.0.0
	pub SciterFireEvent: extern "system" fn (evt: * const BEHAVIOR_EVENT_PARAMS, post: BOOL, handled: * mut BOOL) -> SCDOM_RESULT,

	pub SciterGetCallbackParam: extern "system" fn (hwnd: HWINDOW) -> LPVOID,
	pub SciterPostCallback: extern "system" fn (hwnd: HWINDOW, wparam: UINT_PTR, lparam: UINT_PTR, timeoutms: UINT) -> UINT_PTR,

	// since 3.3.1.0
	pub GetSciterGraphicsAPI: extern "system" fn () -> * const SciterGraphicsAPI,

	// since 3.3.1.6
	pub GetSciterRequestAPI: extern "system" fn () -> * const SciterRequestAPI,

  // #ifdef WINDOWS
  // since 3.3.1.4
  #[cfg(all(windows, not(feature = "windowless")))]
	pub SciterCreateOnDirectXWindow: extern "system" fn (hwnd: HWINDOW, pSwapChain: * mut IDXGISwapChain) -> BOOL,
	#[cfg(all(windows, not(feature = "windowless")))]
	pub SciterRenderOnDirectXWindow: extern "system" fn (hwnd: HWINDOW, elementToRenderOrNull: HELEMENT, frontLayer: BOOL) -> BOOL,
	#[cfg(all(windows, not(feature = "windowless")))]
	pub SciterRenderOnDirectXTexture: extern "system" fn (hwnd: HWINDOW, elementToRenderOrNull: HELEMENT, surface: * mut IDXGISurface) -> BOOL,
  // #endif

  // since 4.0.0.0
	pub SciterProcX: extern "system" fn(hwnd: HWINDOW, msg: * const SCITER_X_MSG) -> BOOL,

	// since 4.4.2.14
	pub SciterAtomValue: extern "system" fn(name: LPCSTR) -> UINT64,
	pub SciterAtomNameCB: extern "system" fn(atomv: UINT64, rcv: LPCSTR_RECEIVER, rcv_param: LPVOID) -> BOOL,

	// since 4.4.2.16
	pub SciterSetGlobalAsset: extern "system" fn(pass: *mut som_asset_t) -> BOOL,
}
