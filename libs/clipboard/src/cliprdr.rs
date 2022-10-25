#![allow(dead_code)]
#![allow(non_camel_case_types)]
#![allow(unused_variables)]
#![allow(non_snake_case)]
#![allow(deref_nullptr)]

use std::{boxed::Box, result::Result};
use thiserror::Error;

pub type size_t = ::std::os::raw::c_ulonglong;
pub type __vcrt_bool = bool;
pub type wchar_t = ::std::os::raw::c_ushort;

pub type POINTER_64_INT = ::std::os::raw::c_ulonglong;
pub type INT8 = ::std::os::raw::c_schar;
pub type PINT8 = *mut ::std::os::raw::c_schar;
pub type INT16 = ::std::os::raw::c_short;
pub type PINT16 = *mut ::std::os::raw::c_short;
pub type INT32 = ::std::os::raw::c_int;
pub type PINT32 = *mut ::std::os::raw::c_int;
pub type INT64 = ::std::os::raw::c_longlong;
pub type PINT64 = *mut ::std::os::raw::c_longlong;
pub type UINT8 = ::std::os::raw::c_uchar;
pub type PUINT8 = *mut ::std::os::raw::c_uchar;
pub type UINT16 = ::std::os::raw::c_ushort;
pub type PUINT16 = *mut ::std::os::raw::c_ushort;
pub type UINT32 = ::std::os::raw::c_uint;
pub type PUINT32 = *mut ::std::os::raw::c_uint;
pub type UINT64 = ::std::os::raw::c_ulonglong;
pub type PUINT64 = *mut ::std::os::raw::c_ulonglong;
pub type LONG32 = ::std::os::raw::c_int;
pub type PLONG32 = *mut ::std::os::raw::c_int;
pub type ULONG32 = ::std::os::raw::c_uint;
pub type PULONG32 = *mut ::std::os::raw::c_uint;
pub type DWORD32 = ::std::os::raw::c_uint;
pub type PDWORD32 = *mut ::std::os::raw::c_uint;
pub type INT_PTR = ::std::os::raw::c_longlong;
pub type PINT_PTR = *mut ::std::os::raw::c_longlong;
pub type UINT_PTR = ::std::os::raw::c_ulonglong;
pub type PUINT_PTR = *mut ::std::os::raw::c_ulonglong;
pub type LONG_PTR = ::std::os::raw::c_longlong;
pub type PLONG_PTR = *mut ::std::os::raw::c_longlong;
pub type ULONG_PTR = ::std::os::raw::c_ulonglong;
pub type PULONG_PTR = *mut ::std::os::raw::c_ulonglong;
pub type SHANDLE_PTR = ::std::os::raw::c_longlong;
pub type HANDLE_PTR = ::std::os::raw::c_ulonglong;
pub type UHALF_PTR = ::std::os::raw::c_uint;
pub type PUHALF_PTR = *mut ::std::os::raw::c_uint;
pub type HALF_PTR = ::std::os::raw::c_int;
pub type PHALF_PTR = *mut ::std::os::raw::c_int;
pub type SIZE_T = ULONG_PTR;
pub type PSIZE_T = *mut ULONG_PTR;
pub type SSIZE_T = LONG_PTR;
pub type PSSIZE_T = *mut LONG_PTR;
pub type DWORD_PTR = ULONG_PTR;
pub type PDWORD_PTR = *mut ULONG_PTR;
pub type LONG64 = ::std::os::raw::c_longlong;
pub type PLONG64 = *mut ::std::os::raw::c_longlong;
pub type ULONG64 = ::std::os::raw::c_ulonglong;
pub type PULONG64 = *mut ::std::os::raw::c_ulonglong;
pub type DWORD64 = ::std::os::raw::c_ulonglong;
pub type PDWORD64 = *mut ::std::os::raw::c_ulonglong;
pub type KAFFINITY = ULONG_PTR;
pub type PKAFFINITY = *mut KAFFINITY;
pub type PVOID = *mut ::std::os::raw::c_void;
pub type CHAR = ::std::os::raw::c_char;
pub type SHORT = ::std::os::raw::c_short;
pub type LONG = ::std::os::raw::c_long;
pub type WCHAR = wchar_t;
pub type PWCHAR = *mut WCHAR;
pub type LPWCH = *mut WCHAR;
pub type PWCH = *mut WCHAR;
pub type LPCWCH = *const WCHAR;
pub type PCWCH = *const WCHAR;
pub type NWPSTR = *mut WCHAR;
pub type LPWSTR = *mut WCHAR;
pub type PWSTR = *mut WCHAR;
pub type PZPWSTR = *mut PWSTR;
pub type PCZPWSTR = *const PWSTR;
pub type LPUWSTR = *mut WCHAR;
pub type PUWSTR = *mut WCHAR;
pub type LPCWSTR = *const WCHAR;
pub type PCWSTR = *const WCHAR;
pub type PZPCWSTR = *mut PCWSTR;
pub type PCZPCWSTR = *const PCWSTR;
pub type LPCUWSTR = *const WCHAR;
pub type PCUWSTR = *const WCHAR;
pub type PZZWSTR = *mut WCHAR;
pub type PCZZWSTR = *const WCHAR;
pub type PUZZWSTR = *mut WCHAR;
pub type PCUZZWSTR = *const WCHAR;
pub type PNZWCH = *mut WCHAR;
pub type PCNZWCH = *const WCHAR;
pub type PUNZWCH = *mut WCHAR;
pub type PCUNZWCH = *const WCHAR;
pub type PCHAR = *mut CHAR;
pub type LPCH = *mut CHAR;
pub type PCH = *mut CHAR;
pub type LPCCH = *const CHAR;
pub type PCCH = *const CHAR;
pub type NPSTR = *mut CHAR;
pub type LPSTR = *mut CHAR;
pub type PSTR = *mut CHAR;
pub type PZPSTR = *mut PSTR;
pub type PCZPSTR = *const PSTR;
pub type LPCSTR = *const CHAR;
pub type PCSTR = *const CHAR;
pub type PZPCSTR = *mut PCSTR;
pub type PCZPCSTR = *const PCSTR;
pub type PZZSTR = *mut CHAR;
pub type PCZZSTR = *const CHAR;
pub type PNZCH = *mut CHAR;
pub type PCNZCH = *const CHAR;
pub type TCHAR = ::std::os::raw::c_char;
pub type PTCHAR = *mut ::std::os::raw::c_char;
pub type TBYTE = ::std::os::raw::c_uchar;
pub type PTBYTE = *mut ::std::os::raw::c_uchar;
pub type LPTCH = LPCH;
pub type PTCH = LPCH;
pub type LPCTCH = LPCCH;
pub type PCTCH = LPCCH;
pub type PTSTR = LPSTR;
pub type LPTSTR = LPSTR;
pub type PUTSTR = LPSTR;
pub type LPUTSTR = LPSTR;
pub type PCTSTR = LPCSTR;
pub type LPCTSTR = LPCSTR;
pub type PCUTSTR = LPCSTR;
pub type LPCUTSTR = LPCSTR;
pub type PZZTSTR = PZZSTR;
pub type PUZZTSTR = PZZSTR;
pub type PCZZTSTR = PCZZSTR;
pub type PCUZZTSTR = PCZZSTR;
pub type PZPTSTR = PZPSTR;
pub type PNZTCH = PNZCH;
pub type PUNZTCH = PNZCH;
pub type PCNZTCH = PCNZCH;
pub type PCUNZTCH = PCNZCH;
pub type PSHORT = *mut SHORT;
pub type PLONG = *mut LONG;
pub type ULONG = ::std::os::raw::c_ulong;
pub type PULONG = *mut ULONG;
pub type USHORT = ::std::os::raw::c_ushort;
pub type PUSHORT = *mut USHORT;
pub type UCHAR = ::std::os::raw::c_uchar;
pub type PUCHAR = *mut UCHAR;
pub type PSZ = *mut ::std::os::raw::c_char;
pub type DWORD = ::std::os::raw::c_ulong;
pub type BOOL = ::std::os::raw::c_int;
pub type BYTE = ::std::os::raw::c_uchar;
pub type WORD = ::std::os::raw::c_ushort;
pub type FLOAT = f32;
pub type PFLOAT = *mut FLOAT;
pub type PBOOL = *mut BOOL;
pub type LPBOOL = *mut BOOL;
pub type PBYTE = *mut BYTE;
pub type LPBYTE = *mut BYTE;
pub type PINT = *mut ::std::os::raw::c_int;
pub type LPINT = *mut ::std::os::raw::c_int;
pub type PWORD = *mut WORD;
pub type LPWORD = *mut WORD;
pub type LPLONG = *mut ::std::os::raw::c_long;
pub type PDWORD = *mut DWORD;
pub type LPDWORD = *mut DWORD;
pub type LPVOID = *mut ::std::os::raw::c_void;
pub type LPCVOID = *const ::std::os::raw::c_void;
pub type INT = ::std::os::raw::c_int;
pub type UINT = ::std::os::raw::c_uint;
pub type PUINT = *mut ::std::os::raw::c_uint;
pub type va_list = *mut ::std::os::raw::c_char;

pub const TRUE: ::std::os::raw::c_int = 1;
pub const FALSE: ::std::os::raw::c_int = 0;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct _CLIPRDR_HEADER {
    pub connID: UINT32,
    pub msgType: UINT16,
    pub msgFlags: UINT16,
    pub dataLen: UINT32,
}
pub type CLIPRDR_HEADER = _CLIPRDR_HEADER;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct _CLIPRDR_CAPABILITY_SET {
    pub capabilitySetType: UINT16,
    pub capabilitySetLength: UINT16,
}
pub type CLIPRDR_CAPABILITY_SET = _CLIPRDR_CAPABILITY_SET;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct _CLIPRDR_GENERAL_CAPABILITY_SET {
    pub capabilitySetType: UINT16,
    pub capabilitySetLength: UINT16,
    pub version: UINT32,
    pub generalFlags: UINT32,
}
pub type CLIPRDR_GENERAL_CAPABILITY_SET = _CLIPRDR_GENERAL_CAPABILITY_SET;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct _CLIPRDR_CAPABILITIES {
    pub connID: UINT32,
    pub msgType: UINT16,
    pub msgFlags: UINT16,
    pub dataLen: UINT32,
    pub cCapabilitiesSets: UINT32,
    pub capabilitySets: *mut CLIPRDR_CAPABILITY_SET,
}
pub type CLIPRDR_CAPABILITIES = _CLIPRDR_CAPABILITIES;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct _CLIPRDR_MONITOR_READY {
    pub connID: UINT32,
    pub msgType: UINT16,
    pub msgFlags: UINT16,
    pub dataLen: UINT32,
}
pub type CLIPRDR_MONITOR_READY = _CLIPRDR_MONITOR_READY;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct _CLIPRDR_TEMP_DIRECTORY {
    pub connID: UINT32,
    pub msgType: UINT16,
    pub msgFlags: UINT16,
    pub dataLen: UINT32,
    pub szTempDir: [::std::os::raw::c_char; 520usize],
}
pub type CLIPRDR_TEMP_DIRECTORY = _CLIPRDR_TEMP_DIRECTORY;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct _CLIPRDR_FORMAT {
    pub formatId: UINT32,
    pub formatName: *mut ::std::os::raw::c_char,
}
pub type CLIPRDR_FORMAT = _CLIPRDR_FORMAT;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct _CLIPRDR_FORMAT_LIST {
    pub connID: UINT32,
    pub msgType: UINT16,
    pub numFormats: UINT32,
    pub formats: *mut CLIPRDR_FORMAT,
}
pub type CLIPRDR_FORMAT_LIST = _CLIPRDR_FORMAT_LIST;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct _CLIPRDR_FORMAT_LIST_RESPONSE {
    pub connID: UINT32,
    pub msgType: UINT16,
    pub msgFlags: UINT16,
    pub dataLen: UINT32,
}
pub type CLIPRDR_FORMAT_LIST_RESPONSE = _CLIPRDR_FORMAT_LIST_RESPONSE;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct _CLIPRDR_LOCK_CLIPBOARD_DATA {
    pub connID: UINT32,
    pub msgType: UINT16,
    pub msgFlags: UINT16,
    pub dataLen: UINT32,
    pub clipDataId: UINT32,
}
pub type CLIPRDR_LOCK_CLIPBOARD_DATA = _CLIPRDR_LOCK_CLIPBOARD_DATA;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct _CLIPRDR_UNLOCK_CLIPBOARD_DATA {
    pub connID: UINT32,
    pub msgType: UINT16,
    pub msgFlags: UINT16,
    pub dataLen: UINT32,
    pub clipDataId: UINT32,
}
pub type CLIPRDR_UNLOCK_CLIPBOARD_DATA = _CLIPRDR_UNLOCK_CLIPBOARD_DATA;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct _CLIPRDR_FORMAT_DATA_REQUEST {
    pub connID: UINT32,
    pub msgType: UINT16,
    pub msgFlags: UINT16,
    pub dataLen: UINT32,
    pub requestedFormatId: UINT32,
}
pub type CLIPRDR_FORMAT_DATA_REQUEST = _CLIPRDR_FORMAT_DATA_REQUEST;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct _CLIPRDR_FORMAT_DATA_RESPONSE {
    pub connID: UINT32,
    pub msgType: UINT16,
    pub msgFlags: UINT16,
    pub dataLen: UINT32,
    pub requestedFormatData: *const BYTE,
}
pub type CLIPRDR_FORMAT_DATA_RESPONSE = _CLIPRDR_FORMAT_DATA_RESPONSE;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct _CLIPRDR_FILE_CONTENTS_REQUEST {
    pub connID: UINT32,
    pub msgType: UINT16,
    pub msgFlags: UINT16,
    pub dataLen: UINT32,
    pub streamId: UINT32,
    pub listIndex: UINT32,
    pub dwFlags: UINT32,
    pub nPositionLow: UINT32,
    pub nPositionHigh: UINT32,
    pub cbRequested: UINT32,
    pub haveClipDataId: BOOL,
    pub clipDataId: UINT32,
}
pub type CLIPRDR_FILE_CONTENTS_REQUEST = _CLIPRDR_FILE_CONTENTS_REQUEST;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct _CLIPRDR_FILE_CONTENTS_RESPONSE {
    pub connID: UINT32,
    pub msgType: UINT16,
    pub msgFlags: UINT16,
    pub dataLen: UINT32,
    pub streamId: UINT32,
    pub cbRequested: UINT32,
    pub requestedData: *const BYTE,
}
pub type CLIPRDR_FILE_CONTENTS_RESPONSE = _CLIPRDR_FILE_CONTENTS_RESPONSE;
pub type CliprdrClientContext = _cliprdr_client_context;
pub type pcCliprdrServerCapabilities = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        capabilities: *const CLIPRDR_CAPABILITIES,
    ) -> UINT,
>;
pub type pcCliprdrClientCapabilities = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        capabilities: *const CLIPRDR_CAPABILITIES,
    ) -> UINT,
>;
pub type pcCliprdrMonitorReady = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        monitorReady: *const CLIPRDR_MONITOR_READY,
    ) -> UINT,
>;
pub type pcCliprdrTempDirectory = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        tempDirectory: *const CLIPRDR_TEMP_DIRECTORY,
    ) -> UINT,
>;
pub type pcCliprdrClientFormatList = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        formatList: *const CLIPRDR_FORMAT_LIST,
    ) -> UINT,
>;
pub type pcCliprdrServerFormatList = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        formatList: *const CLIPRDR_FORMAT_LIST,
    ) -> UINT,
>;
pub type pcCliprdrClientFormatListResponse = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        formatListResponse: *const CLIPRDR_FORMAT_LIST_RESPONSE,
    ) -> UINT,
>;
pub type pcCliprdrServerFormatListResponse = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        formatListResponse: *const CLIPRDR_FORMAT_LIST_RESPONSE,
    ) -> UINT,
>;
pub type pcCliprdrClientLockClipboardData = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        lockClipboardData: *const CLIPRDR_LOCK_CLIPBOARD_DATA,
    ) -> UINT,
>;
pub type pcCliprdrServerLockClipboardData = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        lockClipboardData: *const CLIPRDR_LOCK_CLIPBOARD_DATA,
    ) -> UINT,
>;
pub type pcCliprdrClientUnlockClipboardData = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        unlockClipboardData: *const CLIPRDR_UNLOCK_CLIPBOARD_DATA,
    ) -> UINT,
>;
pub type pcCliprdrServerUnlockClipboardData = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        unlockClipboardData: *const CLIPRDR_UNLOCK_CLIPBOARD_DATA,
    ) -> UINT,
>;
pub type pcCliprdrClientFormatDataRequest = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        formatDataRequest: *const CLIPRDR_FORMAT_DATA_REQUEST,
    ) -> UINT,
>;
pub type pcCliprdrServerFormatDataRequest = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        formatDataRequest: *const CLIPRDR_FORMAT_DATA_REQUEST,
    ) -> UINT,
>;
pub type pcCliprdrClientFormatDataResponse = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        formatDataResponse: *const CLIPRDR_FORMAT_DATA_RESPONSE,
    ) -> UINT,
>;
pub type pcCliprdrServerFormatDataResponse = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        formatDataResponse: *const CLIPRDR_FORMAT_DATA_RESPONSE,
    ) -> UINT,
>;
pub type pcCliprdrClientFileContentsRequest = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        fileContentsRequest: *const CLIPRDR_FILE_CONTENTS_REQUEST,
    ) -> UINT,
>;
pub type pcCliprdrServerFileContentsRequest = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        fileContentsRequest: *const CLIPRDR_FILE_CONTENTS_REQUEST,
    ) -> UINT,
>;
pub type pcCliprdrClientFileContentsResponse = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        fileContentsResponse: *const CLIPRDR_FILE_CONTENTS_RESPONSE,
    ) -> UINT,
>;
pub type pcCliprdrServerFileContentsResponse = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        fileContentsResponse: *const CLIPRDR_FILE_CONTENTS_RESPONSE,
    ) -> UINT,
>;
pub type pcCheckEnabled = ::std::option::Option<
    unsafe extern "C" fn(connID: UINT32) -> BOOL,
>;

// TODO: hide more members of clipboard context
#[repr(C)]
#[derive(Debug, Clone)]
pub struct _cliprdr_client_context {
    pub custom: *mut ::std::os::raw::c_void,
    pub enableFiles: BOOL,
    pub enableOthers: BOOL,
    pub CheckEnabled: pcCheckEnabled,
    pub ServerCapabilities: pcCliprdrServerCapabilities,
    pub ClientCapabilities: pcCliprdrClientCapabilities,
    pub MonitorReady: pcCliprdrMonitorReady,
    pub TempDirectory: pcCliprdrTempDirectory,
    pub ClientFormatList: pcCliprdrClientFormatList,
    pub ServerFormatList: pcCliprdrServerFormatList,
    pub ClientFormatListResponse: pcCliprdrClientFormatListResponse,
    pub ServerFormatListResponse: pcCliprdrServerFormatListResponse,
    pub ClientLockClipboardData: pcCliprdrClientLockClipboardData,
    pub ServerLockClipboardData: pcCliprdrServerLockClipboardData,
    pub ClientUnlockClipboardData: pcCliprdrClientUnlockClipboardData,
    pub ServerUnlockClipboardData: pcCliprdrServerUnlockClipboardData,
    pub ClientFormatDataRequest: pcCliprdrClientFormatDataRequest,
    pub ServerFormatDataRequest: pcCliprdrServerFormatDataRequest,
    pub ClientFormatDataResponse: pcCliprdrClientFormatDataResponse,
    pub ServerFormatDataResponse: pcCliprdrServerFormatDataResponse,
    pub ClientFileContentsRequest: pcCliprdrClientFileContentsRequest,
    pub ServerFileContentsRequest: pcCliprdrServerFileContentsRequest,
    pub ClientFileContentsResponse: pcCliprdrClientFileContentsResponse,
    pub ServerFileContentsResponse: pcCliprdrServerFileContentsResponse,
    pub lastRequestedFormatId: UINT32,
}

// #[link(name = "user32")]
// #[link(name = "ole32")]
extern "C" {
    pub(crate) fn init_cliprdr(context: *mut CliprdrClientContext) -> BOOL;
    pub(crate) fn uninit_cliprdr(context: *mut CliprdrClientContext) -> BOOL;
    pub(crate) fn empty_cliprdr(
        context: *mut CliprdrClientContext,
        connID: UINT32,
    ) -> BOOL;
}

#[derive(Error, Debug)]
pub enum CliprdrError {
    #[error("invalid cliprdr name")]
    CliprdrName,
    #[error("failed to init cliprdr")]
    CliprdrInit,
    #[error("unknown cliprdr error")]
    Unknown,
}

impl CliprdrClientContext {
    pub fn create(
        enable_files: bool,
        enable_others: bool,
        check_enabled: pcCheckEnabled,
        client_format_list: pcCliprdrClientFormatList,
        client_format_list_response: pcCliprdrClientFormatListResponse,
        client_format_data_request: pcCliprdrClientFormatDataRequest,
        client_format_data_response: pcCliprdrClientFormatDataResponse,
        client_file_contents_request: pcCliprdrClientFileContentsRequest,
        client_file_contents_response: pcCliprdrClientFileContentsResponse,
    ) -> Result<Box<Self>, CliprdrError> {
        let context = CliprdrClientContext {
            custom: 0 as *mut _,
            enableFiles: if enable_files { TRUE } else { FALSE },
            enableOthers: if enable_others { TRUE } else { FALSE },
            CheckEnabled: check_enabled,
            ServerCapabilities: None,
            ClientCapabilities: None,
            MonitorReady: None,
            TempDirectory: None,
            ClientFormatList: client_format_list,
            ServerFormatList: None,
            ClientFormatListResponse: client_format_list_response,
            ServerFormatListResponse: None,
            ClientLockClipboardData: None,
            ServerLockClipboardData: None,
            ClientUnlockClipboardData: None,
            ServerUnlockClipboardData: None,
            ClientFormatDataRequest: client_format_data_request,
            ServerFormatDataRequest: None,
            ClientFormatDataResponse: client_format_data_response,
            ServerFormatDataResponse: None,
            ClientFileContentsRequest: client_file_contents_request,
            ServerFileContentsRequest: None,
            ClientFileContentsResponse: client_file_contents_response,
            ServerFileContentsResponse: None,
            lastRequestedFormatId: 0,
        };
        let mut context = Box::new(context);
        unsafe {
            if FALSE == init_cliprdr(&mut (*context)) {
                println!("Failed to init cliprdr");
                Err(CliprdrError::CliprdrInit)
            } else {
                Ok(context)
            }
        }
    }
}

impl Drop for CliprdrClientContext {
    fn drop(&mut self) {
        unsafe {
            if FALSE == uninit_cliprdr(&mut *self) {
                println!("Failed to uninit cliprdr");
            } else {
                println!("Succeeded to uninit cliprdr");
            }
        }
    }
}
