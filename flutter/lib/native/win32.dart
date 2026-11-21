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

/// Read only the menu modifier while the opted-in Windows game session has focus.
int gameMenuModifier_() {
  if ((win32.GetAsyncKeyState(win32.VK_LMENU) & 0x8000) == 0) return 0;
  return (win32.GetAsyncKeyState(win32.VK_CONTROL) & 0x8000) == 0 ? 1 : 2;
}

int gameCursorSizePixels_() {
  final window = win32.GetForegroundWindow();
  try {
    final dpi = win32.GetDpiForWindow(window);
    if (dpi > 0) {
      final width = win32.GetSystemMetricsForDpi(win32.SM_CXCURSOR, dpi);
      if (width > 0) return width;
    }
  } on ArgumentError {
    // Older versions of Windows expose only the system-wide metric.
  }
  return win32.GetSystemMetrics(win32.SM_CXCURSOR);
}
