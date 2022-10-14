#![allow(dead_code)]
#![allow(non_camel_case_types)]
#![allow(unused_variables)]
#![allow(non_snake_case)]
#![allow(deref_nullptr)]

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
pub struct _MonitorMode {
    pub width: DWORD,
    pub height: DWORD,
    pub sync: DWORD,
}
pub type MonitorMode = _MonitorMode;
pub type PMonitorMode = *mut MonitorMode;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct HSWDEVICE__ {
    pub unused: ::std::os::raw::c_int,
}
pub type HSWDEVICE = *mut HSWDEVICE__;
pub type PHSWDEVICE = *mut HSWDEVICE;

#[link(name = "Newdev")]
extern "C" {
    pub fn InstallUpdate(fullInfPath: LPCTSTR, rebootRequired: PBOOL) -> BOOL;
}

#[link(name = "Setupapi")]
extern "C" {
    pub fn IsDeviceCreated(created: PBOOL) -> BOOL;
}

#[link(name = "Swdevice")]
#[link(name = "OneCoreUAP")]
extern "C" {
    pub fn DeviceCreate(hSwDevice: PHSWDEVICE) -> BOOL;
    pub fn DeviceClose(hSwDevice: HSWDEVICE);
}

extern "C" {
    pub fn Uninstall(fullInfPath: LPCTSTR, rebootRequired: PBOOL) -> BOOL;
    pub fn MonitorPlugIn(index: UINT, edid: UINT, retries: INT) -> BOOL;
    pub fn MonitorPlugOut(index: UINT) -> BOOL;
    pub fn MonitorModesUpdate(index: UINT, modeCount: UINT, modes: PMonitorMode) -> BOOL;

    pub fn GetLastMsg() -> PCHAR;
    pub fn SetPrintErrMsg(b: BOOL);
}
