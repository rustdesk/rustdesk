import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

import '../../models/model.dart';
import '../../models/platform_model.dart';
import '../consts.dart';
import 'dart:ui' as ui;

class Keyboard {
  late FFI _ffi;
  late String _id;
  String keyboardMode = "legacy";

  Keyboard(FFI ffi, String id) {
    _ffi = ffi;
    _id = id;
  }

  KeyEventResult handleRawKeyEvent(FocusNode data, RawKeyEvent e) {
    bind.sessionGetKeyboardName(id: _id).then((result) {
      keyboardMode = result.toString();
    });

    if (keyboardMode == 'map') {
      mapKeyboardMode(e);
    } else if (keyboardMode == 'translate') {
      legacyKeyboardMode(e);
    } else {
      legacyKeyboardMode(e);
    }

    return KeyEventResult.handled;
  }

  void mapKeyboardMode(RawKeyEvent e) {
    int scanCode;
    int keyCode;
    bool down;

    if (e.data is RawKeyEventDataMacOs) {
      RawKeyEventDataMacOs newData = e.data as RawKeyEventDataMacOs;
      scanCode = newData.keyCode;
      keyCode = newData.keyCode;
    } else if (e.data is RawKeyEventDataWindows) {
      RawKeyEventDataWindows newData = e.data as RawKeyEventDataWindows;
      scanCode = newData.scanCode;
      keyCode = newData.keyCode;
    } else if (e.data is RawKeyEventDataLinux) {
      RawKeyEventDataLinux newData = e.data as RawKeyEventDataLinux;
      scanCode = newData.scanCode;
      keyCode = newData.keyCode;
    } else {
      scanCode = -1;
      keyCode = -1;
    }

    if (e is RawKeyDownEvent) {
      down = true;
    } else {
      down = false;
    }

    _ffi.inputRawKey(e.character ?? "", keyCode, scanCode, down);
  }

  void legacyKeyboardMode(RawKeyEvent e) {
    final key = e.logicalKey;
    if (e is RawKeyDownEvent) {
      if (e.repeat) {
        sendRawKey(e, press: true);
      } else {
        if (e.isAltPressed && !_ffi.alt) {
          _ffi.alt = true;
        } else if (e.isControlPressed && !_ffi.ctrl) {
          _ffi.ctrl = true;
        } else if (e.isShiftPressed && !_ffi.shift) {
          _ffi.shift = true;
        } else if (e.isMetaPressed && !_ffi.command) {
          _ffi.command = true;
        }
        sendRawKey(e, down: true);
      }
    }
    if (e is RawKeyUpEvent) {
      if (key == LogicalKeyboardKey.altLeft ||
          key == LogicalKeyboardKey.altRight) {
        _ffi.alt = false;
      } else if (key == LogicalKeyboardKey.controlLeft ||
          key == LogicalKeyboardKey.controlRight) {
        _ffi.ctrl = false;
      } else if (key == LogicalKeyboardKey.shiftRight ||
          key == LogicalKeyboardKey.shiftLeft) {
        _ffi.shift = false;
      } else if (key == LogicalKeyboardKey.metaLeft ||
          key == LogicalKeyboardKey.metaRight ||
          key == LogicalKeyboardKey.superKey) {
        _ffi.command = false;
      }
      sendRawKey(e);
    }
  }

  void sendRawKey(RawKeyEvent e, {bool? down, bool? press}) {
    // for maximum compatibility
    final label = physicalKeyMap[e.physicalKey.usbHidUsage] ??
        logicalKeyMap[e.logicalKey.keyId] ??
        e.logicalKey.keyLabel;
    _ffi.inputKey(label, down: down, press: press ?? false);
  }
}

class Mouse {
  var _isPhysicalMouse = false;
  int _lastMouseDownButtons = 0;

  late FFI _ffi;
  late String _id;
  late double tabBarHeight;

  Mouse(FFI ffi, String id, double tabBarHeight_) {
    _ffi = ffi;
    _id = id;
    tabBarHeight = tabBarHeight_;
  }

  Map<String, dynamic> getEvent(PointerEvent evt, String type) {
    final Map<String, dynamic> out = {};
    out['type'] = type;
    out['x'] = evt.position.dx;
    out['y'] = evt.position.dy;
    if (_ffi.alt) out['alt'] = 'true';
    if (_ffi.shift) out['shift'] = 'true';
    if (_ffi.ctrl) out['ctrl'] = 'true';
    if (_ffi.command) out['command'] = 'true';
    out['buttons'] = evt
        .buttons; // left button: 1, right button: 2, middle button: 4, 1 | 2 = 3 (left + right)
    if (evt.buttons != 0) {
      _lastMouseDownButtons = evt.buttons;
    } else {
      out['buttons'] = _lastMouseDownButtons;
    }
    return out;
  }

  void onPointHoverImage(PointerHoverEvent e) {
    if (e.kind != ui.PointerDeviceKind.mouse) return;
    if (!_isPhysicalMouse) {
      _isPhysicalMouse = true;
    }
    if (_isPhysicalMouse) {
      _ffi.handleMouse(getEvent(e, 'mousemove'), tabBarHeight: tabBarHeight);
    }
  }

  void onPointDownImage(PointerDownEvent e) {
    if (e.kind != ui.PointerDeviceKind.mouse) {
      if (_isPhysicalMouse) {
        _isPhysicalMouse = false;
      }
    }
    if (_isPhysicalMouse) {
      _ffi.handleMouse(getEvent(e, 'mousedown'), tabBarHeight: tabBarHeight);
    }
  }

  void onPointUpImage(PointerUpEvent e) {
    if (e.kind != ui.PointerDeviceKind.mouse) return;
    if (_isPhysicalMouse) {
      _ffi.handleMouse(getEvent(e, 'mouseup'), tabBarHeight: tabBarHeight);
    }
  }

  void onPointMoveImage(PointerMoveEvent e) {
    if (e.kind != ui.PointerDeviceKind.mouse) return;
    if (_isPhysicalMouse) {
      _ffi.handleMouse(getEvent(e, 'mousemove'), tabBarHeight: tabBarHeight);
    }
  }

  void onPointerSignalImage(PointerSignalEvent e) {
    if (e is PointerScrollEvent) {
      var dx = e.scrollDelta.dx.toInt();
      var dy = e.scrollDelta.dy.toInt();
      if (dx > 0) {
        dx = -1;
      } else if (dx < 0) {
        dx = 1;
      }
      if (dy > 0) {
        dy = -1;
      } else if (dy < 0) {
        dy = 1;
      }
      bind.sessionSendMouse(
          id: _id, msg: '{"type": "wheel", "x": "$dx", "y": "$dy"}');
    }
  }
}
