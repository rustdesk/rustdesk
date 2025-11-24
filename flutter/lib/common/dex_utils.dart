import 'dart:io';
import 'package:flutter/services.dart';

/// Utilities for Samsung DeX and pointer capture features on Android.
/// 
/// Based on termux-x11 implementation:
/// https://github.com/termux/termux-x11
class DexUtils {
  static const platform = MethodChannel('mChannel');

  /// Enable or disable Samsung DeX Meta (Windows/Command) key capture.
  /// When enabled, Meta key events will be sent to the app instead of
  /// being intercepted by the system.
  /// 
  /// Only works on Samsung devices with DeX mode.
  static Future<void> setDexMetaCapture(bool enable) async {
    if (!Platform.isAndroid) return;
    try {
      await platform.invokeMethod('setDexMetaCapture', {'enable': enable});
    } on PlatformException catch (e) {
      print("Failed to set DeX meta capture: '${e.message}'.");
    }
  }

  /// Toggle pointer capture for immersive mouse control.
  /// When enabled, the app receives raw relative mouse movements
  /// instead of absolute coordinates.
  /// 
  /// Useful for games and applications that need precise mouse control.
  static Future<void> togglePointerCapture(bool enable) async {
    if (!Platform.isAndroid) return;
    try {
      await platform.invokeMethod('togglePointerCapture', {'enable': enable});
    } on PlatformException catch (e) {
      print("Failed to toggle pointer capture: '${e.message}'.");
    }
  }
  
  /// Check if Samsung DeX mode is currently enabled.
  /// Returns true if DeX is active, false otherwise.
  static Future<bool> isDexEnabled() async {
    if (!Platform.isAndroid) return false;
    try {
      final result = await platform.invokeMethod('isDexEnabled');
      return result as bool? ?? false;
    } catch (e) {
      print("Failed to check DeX status: '$e'.");
      return false;
    }
  }
}
