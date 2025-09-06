import 'dart:ffi' hide Size;

import 'package:ffi/ffi.dart';

import 'package:win32/win32.dart' as win32;

/// Get windows target build number.
///
/// [Note]
/// Please use this function wrapped with `Platform.isWindows`.
int getWindowsTargetBuildNumber_() {
  final rtlGetVersion = DynamicLibrary.open('ntdll.dll').lookupFunction<
      Void Function(Pointer<win32.OSVERSIONINFOEX>),
      void Function(Pointer<win32.OSVERSIONINFOEX>)>('RtlGetVersion');
  final osVersionInfo = _getOSVERSIONINFOEXPointer();
  rtlGetVersion(osVersionInfo);
  int buildNumber = osVersionInfo.ref.dwBuildNumber;
  calloc.free(osVersionInfo);
  return buildNumber;
}

/// Get Windows OS version pointer
///
/// [Note]
/// Please use this function wrapped with `Platform.isWindows`.
Pointer<win32.OSVERSIONINFOEX> _getOSVERSIONINFOEXPointer() {
  final pointer = calloc<win32.OSVERSIONINFOEX>();
  pointer.ref
    ..dwOSVersionInfoSize = sizeOf<win32.OSVERSIONINFOEX>()
    ..dwBuildNumber = 0
    ..dwMajorVersion = 0
    ..dwMinorVersion = 0
    ..dwPlatformId = 0
    ..szCSDVersion = ''
    ..wServicePackMajor = 0
    ..wServicePackMinor = 0
    ..wSuiteMask = 0
    ..wProductType = 0
    ..wReserved = 0;
  return pointer;
}
