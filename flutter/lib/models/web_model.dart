import 'dart:convert';
import 'dart:typed_data';
import 'dart:js';

import '../common.dart';
import 'dart:html';
import 'dart:async';

final List<StreamSubscription<MouseEvent>> mouseListeners = [];
final List<StreamSubscription<KeyboardEvent>> keyListeners = [];

class PlatformFFI {
  static void clearRgbaFrame() {}

  static Uint8List? getRgba() {
    return context.callMethod('getRgba');
  }

  static String getByName(String name, [String arg = '']) {
    return context.callMethod('getByName', [name, arg]);
  }

  static void setByName(String name, [String value = '']) {
    context.callMethod('setByName', [name, value]);
  }

  static Future<Null> init() async {
    isWeb = true;
    isDesktop = !context.callMethod('isMobile');
    context.callMethod('init');
    version = getByName('version');
  }

  static void setEventCallback(void Function(Map<String, dynamic>) fun) async {
    context["onGlobalEvent"] = (String message) {
      try {
        Map<String, dynamic> event = json.decode(message);
        fun(event);
      } catch (e) {
        print('json.decode fail(): $e');
      }
    };
  }

  static void startDesktopWebListener() {
    mouseListeners.add(
        window.document.onContextMenu.listen((evt) => evt.preventDefault()));
  }

  static void stopDesktopWebListener() {
    mouseListeners.forEach((l) {
      l.cancel();
    });
    mouseListeners.clear();
    keyListeners.forEach((l) {
      l.cancel();
    });
    keyListeners.clear();
  }

  static void setMethodCallHandler(FMethod callback) {}

  static Future<bool> invokeMethod(String method, [dynamic arguments]) async {
    return true;
  }
}

final localeName = window.navigator.language;
