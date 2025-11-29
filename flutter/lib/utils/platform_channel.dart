import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/main.dart';
import 'package:flutter_hbb/common.dart';

enum SystemWindowTheme { light, dark }

/// The platform channel for RustDesk.
class RdPlatformChannel {
  RdPlatformChannel._();

  static final RdPlatformChannel _windowUtil = RdPlatformChannel._();

  static RdPlatformChannel get instance => _windowUtil;

  final MethodChannel _hostMethodChannel =
      MethodChannel("org.rustdesk.rustdesk/host");

  // Main Flutter method channel for Android communication
  final MethodChannel _mainChannel = MethodChannel("mChannel");

  /// Bump the position of the mouse cursor, if applicable
  Future<bool> bumpMouse({required int dx, required int dy}) async {
    // No debug output; this call is too chatty.

    bool? result = await _hostMethodChannel
      .invokeMethod("bumpMouse", {"dx": dx, "dy": dy});

    return result ?? false;
  }

  /// Change the theme of the system window
  Future<void> changeSystemWindowTheme(SystemWindowTheme theme) {
    assert(isMacOS);
    if (kDebugMode) {
      print(
          "[Window ${kWindowId ?? 'Main'}] change system window theme to ${theme.name}");
    }
    return _hostMethodChannel
        .invokeMethod("setWindowTheme", {"themeName": theme.name});
  }

  /// Terminate .app manually.
  Future<void> terminate() {
    assert(isMacOS);
    return _hostMethodChannel.invokeMethod("terminate");
  }

  /// Enable or disable Samsung DeX Meta (Windows/Command) key capture.
  /// When enabled, Meta key events will be sent to the app instead of
  /// being intercepted by the system.
  /// 
  /// Only works on Samsung devices with DeX mode.
  Future<void> setDexMetaCapture(bool enable) async {
    if (!isAndroid) return;
    try {
      await _mainChannel.invokeMethod('setDexMetaCapture', enable);
    } on PlatformException catch (e) {
      debugPrint("Failed to set DeX meta capture: '${e.message}'.");
    }
  }

  // NOTE: Pointer capture (togglePointerCapture) was removed because it breaks
  // normal mouse input. When pointer capture is enabled, Android delivers
  // relative movement deltas instead of absolute coordinates, which Flutter's
  // input system doesn't handle correctly. The DeX Meta key capture alone
  // provides the primary value for remote desktop use cases.
  
  /// Check if Samsung DeX mode is currently enabled.
  /// Returns true if DeX is active, false otherwise.
  Future<bool> isDexEnabled() async {
    if (!isAndroid) return false;
    try {
      final result = await _mainChannel.invokeMethod('isDexEnabled');
      return result as bool? ?? false;
    } catch (e) {
      debugPrint("Failed to check DeX status: '$e'.");
      return false;
    }
  }
}
