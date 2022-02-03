import 'dart:typed_data';
import 'dart:js' as js;
import 'common.dart';
import 'dart:html';
import 'dart:async';

final List<StreamSubscription<MouseEvent>> mouseListeners = [];
int lastMouseDownButtons = 0;

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
    isWeb = true;
    isDesktop = !js.context.callMethod('isMobile');
    js.context.callMethod('init');
  }

  // MouseRegion onHover not work for mouse move when right button down
  static void startDesktopWebListener(
      Function(Map<String, dynamic>) handleMouse) {
    lastMouseDownButtons = 0;
    // document.body.getElementsByTagName('flt-glass-pane')[0].style.cursor = 'none';
    mouseListeners.add(window.document.onMouseMove
        .listen((evt) => handleMouse(getEvent(evt))));
    mouseListeners.add(window.document.onMouseDown
        .listen((evt) => handleMouse(getEvent(evt))));
    mouseListeners.add(
        window.document.onMouseUp.listen((evt) => handleMouse(getEvent(evt))));
    mouseListeners.add(window.document.onMouseWheel.listen((evt) {
      var dx = evt.deltaX;
      var dy = evt.deltaY;
      if (dx > 0)
        dx = -1;
      else if (dx < 0) dx = 1;
      if (dy > 0)
        dy = -1;
      else if (dy < 0) dy = 1;
      setByName('send_mouse', '{"type": "wheel", "x": "$dx", "y": "$dy"}');
    }));
    mouseListeners.add(
        window.document.onContextMenu.listen((evt) => evt.preventDefault()));
  }

  static void stopDesktopWebListener() {
    mouseListeners.forEach((l) {
      l.cancel();
    });
    mouseListeners.clear();
  }
}

Map<String, dynamic> getEvent(MouseEvent evt) {
  // https://github.com/novnc/noVNC/blob/679b45fa3b453c7cf32f4b4455f4814818ecf161/core/rfb.js
  // https://developer.mozilla.org/zh-CN/docs/Web/API/Element/mousedown_event
  final Map<String, dynamic> out = {};
  out['type'] = evt.type;
  out['x'] = evt.client.x;
  out['y'] = evt.client.y;
  out['ctrl'] = evt.ctrlKey;
  out['shift'] = evt.shiftKey;
  out['alt'] = evt.altKey;
  out['command'] = evt.metaKey;
  out['buttons'] = evt
      .buttons; // left button: 1, right button: 2, middle button: 4, 1 | 2 = 3 (left + right)
  if (evt.buttons != 0) {
    lastMouseDownButtons = evt.buttons;
  } else {
    out['buttons'] = lastMouseDownButtons;
  }
  return out;
}

final localeName = window.navigator.language;
