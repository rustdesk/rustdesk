import 'dart:typed_data';
import 'dart:js' as js;

import '../common.dart';
import 'dart:html';
import 'dart:async';

final List<StreamSubscription<MouseEvent>> mouseListeners = [];
final List<StreamSubscription<KeyboardEvent>> keyListeners = [];
int lastMouseDownButtons = 0;
bool mouseIn = false;

class PlatformFFI {
  static void clearRgbaFrame() {}

  static Uint8List? getRgba() {
    return js.context.callMethod('getRgba');
  }

  static String getByName(String name, [String arg = '']) {
    return js.context.callMethod('getByName', [name, arg]);
  }

  static void setByName(String name, [String value = '']) {
    js.context.callMethod('setByName', [name, value]);
  }

  static Future<Null> init() async {
    isWeb = true;
    isDesktop = !js.context.callMethod('isMobile');
    js.context.callMethod('init');
    version = getByName('version');
  }

  static void startDesktopWebListener() {
    mouseIn = true;
    mouseListeners.add(
        window.document.onContextMenu.listen((evt) => evt.preventDefault()));
  }

  static void stopDesktopWebListener() {
    mouseIn = true;
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
