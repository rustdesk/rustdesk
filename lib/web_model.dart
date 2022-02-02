import 'dart:typed_data';
import 'dart:js' as js;
import 'common.dart';
import 'dart:html';
import 'dart:async';

final List<StreamSubscription<MouseEvent>> mouselisteners = [];

class PlatformFFI {
  static void clearRgbaFrame() {}

  static Uint8List getRgba() {
    return js.context.callMethod('getRgba');
  }

  static Future<String> getVersion() async {
    return getByName('version');
  }

  static String getByName(String name, [String arg = '']) {
    return js.context.callMethod('getByName', [name, arg]);
  }

  static void setByName(String name, [String value = '']) {
    js.context.callMethod('setByName', [name, value]);
  }

  static Future<Null> init() async {
    window.document.onContextMenu.listen((evt) => evt.preventDefault());
    isWeb = true;
    isDesktop = !js.context.callMethod('isMobile');
    js.context.callMethod('init');
  }

  // MouseRegion onHover not work for mouse move when right button down
  static void startDesktopWebListener(
      Function(Map<String, dynamic>) handleMouse) {
    // document.body.getElementsByTagName('flt-glass-pane')[0].style.cursor = 'none';
    mouselisteners.add(window.document.onMouseMove
        .listen((evt) => handleMouse(getEvent(evt))));
    mouselisteners.add(window.document.onMouseDown
        .listen((evt) => handleMouse(getEvent(evt))));
    mouselisteners.add(
        window.document.onMouseUp.listen((evt) => handleMouse(getEvent(evt))));
    mouselisteners.add(window.document.onMouseWheel.listen((evt) => {}));
  }

  static void stopDesktopWebListener() {
    mouselisteners.forEach((l) {
      l.cancel();
    });
    mouselisteners.clear();
  }
}

Map<String, dynamic> getEvent(MouseEvent evt) {
  // https://github.com/novnc/noVNC/blob/679b45fa3b453c7cf32f4b4455f4814818ecf161/core/rfb.js
  // https://developer.mozilla.org/zh-CN/docs/Web/API/Element/mousedown_event
  final out = {};
  out['type'] = evt.type;
  out['x'] = evt.client.x;
  out['y'] = evt.client.y;
  out['ctrl'] = evt.ctrlKey;
  out['shift'] = evt.shiftKey;
  out['alt'] = evt.altKey;
  out['meta'] = evt.metaKey;
  out['buttons'] = evt
      .buttons; // left button: 1, right button: 2, middle button: 4, 1 | 2 = 3 (left + right)
  return out;
}

final localeName = window.navigator.language;
