import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/main.dart';

enum SystemWindowTheme { light, dark }

/// The platform channel for RustDesk
class RdPlatformChannel {
  RdPlatformChannel._();

  static final RdPlatformChannel _windowUtil = RdPlatformChannel._();

  static RdPlatformChannel get instance => _windowUtil;

  final MethodChannel _osxMethodChannel =
      MethodChannel("org.rustdesk.rustdesk/macos");
  final MethodChannel _winMethodChannel =
      MethodChannel("org.rustdesk.rustdesk/windows");
  final MethodChannel _linuxMethodChannel =
      MethodChannel("org.rustdesk.rustdesk/linux");

  /// Change the theme of the system window
  Future<void> changeSystemWindowTheme(SystemWindowTheme theme) {
    assert(Platform.isMacOS);
    if (kDebugMode) {
      print(
          "[Window ${kWindowId ?? 'Main'}] change system window theme to ${theme.name}");
    }
    return _osxMethodChannel
        .invokeMethod("setWindowTheme", {"themeName": theme.name});
  }
}
