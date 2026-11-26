import 'dart:convert';
import 'dart:ffi' hide Size;

import 'package:ffi/ffi.dart';

import 'package:win32/win32.dart' as win32;

String transformWindowsUserData(String value, {required bool protect}) {
  final bytes = protect ? utf8.encode(value) : base64Decode(value);
  return using((arena) {
    final input = arena<win32.CRYPT_INTEGER_BLOB>();
    final output = arena<win32.CRYPT_INTEGER_BLOB>();
    input.ref
      ..cbData = bytes.length
      ..pbData = arena<Uint8>(bytes.length);
    input.ref.pbData.asTypedList(bytes.length).setAll(0, bytes);

    const uiForbidden = 0x1; // CRYPTPROTECT_UI_FORBIDDEN
    try {
      final result = protect
          ? win32.CryptProtectData(
              input, nullptr, nullptr, nullptr, nullptr, uiForbidden, output)
          : win32.CryptUnprotectData(
              input, nullptr, nullptr, nullptr, nullptr, uiForbidden, output);
      if (result == 0) {
        throw StateError(
            'Windows data protection failed: ${win32.GetLastError()}');
      }
      final data = output.ref.pbData.asTypedList(output.ref.cbData);
      return protect ? base64Encode(data) : utf8.decode(data);
    } finally {
      if (output.ref.pbData != nullptr) {
        win32.LocalFree(output.ref.pbData);
      }
    }
  });
}

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
