import 'dart:typed_data';
import 'dart:js' as js;
import 'package:flutter/cupertino.dart';
import 'dart:convert';

import 'common.dart';
import 'dart:html';
import 'dart:async';

final List<StreamSubscription<MouseEvent>> mouseListeners = [];
final List<StreamSubscription<KeyboardEvent>> keyListeners = [];
int lastMouseDownButtons = 0;
bool mouseIn = false;

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
    mouseIn = true;
    lastMouseDownButtons = 0;
    // document.body.getElementsByTagName('flt-glass-pane')[0].style.cursor = 'none';
    mouseListeners
        .add(window.document.onMouseEnter.listen((evt) => mouseIn = true));
    mouseListeners
        .add(window.document.onMouseLeave.listen((evt) => mouseIn = false));
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
    keyListeners
        .add(window.document.onKeyDown.listen((evt) => handleKey(evt, true)));
    keyListeners
        .add(window.document.onKeyUp.listen((evt) => handleKey(evt, false)));
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

  static Future<bool> invokeMethod(String method) async {
    return true;
  }
}

Map<String, dynamic> getEvent(MouseEvent evt) {
  // https://github.com/novnc/noVNC/blob/679b45fa3b453c7cf32f4b4455f4814818ecf161/core/rfb.js
  // https://developer.mozilla.org/zh-CN/docs/Web/API/Element/mousedown_event
  final Map<String, dynamic> out = {};
  out['type'] = evt.type;
  out['x'] = evt.client.x;
  out['y'] = evt.client.y;
  if (evt.altKey) out['alt'] = 'true';
  if (evt.shiftKey) out['shift'] = 'true';
  if (evt.ctrlKey) out['ctrl'] = 'true';
  if (evt.metaKey) out['command'] = 'true';
  out['buttons'] = evt
      .buttons; // left button: 1, right button: 2, middle button: 4, 1 | 2 = 3 (left + right)
  if (evt.buttons != 0) {
    lastMouseDownButtons = evt.buttons;
  } else {
    out['buttons'] = lastMouseDownButtons;
  }
  return out;
}

void handleKey(KeyboardEvent evt, bool down) {
  if (!mouseIn) return;
  evt.stopPropagation();
  evt.preventDefault();
  evt.stopImmediatePropagation();
  print('${evt.code} ${evt.key} ${evt.location}');
  final out = {};
  var name = ctrlKeyMap[evt.code];
  if (name == null) {
    if (evt.code == evt.key) {
      name = evt.code;
    } else {
      name = evt.key;
      if (name.toLowerCase() != name.toUpperCase() &&
          name == name.toUpperCase()) {
        if (!evt.shiftKey) out['shift'] = 'true';
      }
    }
  }
  out['name'] = name;
  if (evt.altKey) out['alt'] = 'true';
  if (evt.shiftKey) out['shift'] = 'true';
  if (evt.ctrlKey) out['ctrl'] = 'true';
  if (evt.metaKey) out['command'] = 'true';
  if (down) out['down'] = 'true';
  PlatformFFI.setByName('input_key', json.encode(out));
}

final localeName = window.navigator.language;

final ctrlKeyMap = {
  'AltLeft': 'Alt',
  'AltRight': 'RAlt',
  'ShiftLeft': 'Shift',
  'ShiftRight': 'RShift',
  'ControlLeft': 'Control',
  'ControlRight': 'RControl',
  'MetaLeft': 'Meta',
  'MetaRight': 'RWin',
  'ContextMenu': 'Apps',
  'ArrowUp': 'UpArrow',
  'ArrowDown': 'DownArrow',
  'ArrowLeft': 'LeftArrow',
  'ArrowRight': 'RightArrow',
  'NumpadDecimal': 'Decimal',
  'NumpadDivide': 'Divide',
  'NumpadMultiply': 'Multiply',
  'NumpadSubtract': 'Subtract',
  'NumpadAdd': 'Add',
  'NumpadEnter': 'NumpadEnter',
  'Enter': 'Return',
  'Space': 'Space',
  'NumpadClear': 'Clear',
  'NumpadBackspace': 'Backspace',
  'PrintScreen': 'Snapshot',
  'HangulMode': 'Hangul',
  'HanjaMode': 'Hanja',
  'KanaMode': 'Kana',
  'JunjaMode': 'Junja',
  'KanjiMode': 'Hanja',
};
