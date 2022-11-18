import 'dart:convert';
import 'dart:math';
import 'dart:ui' as ui;

import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:get/get.dart';

import '../../models/model.dart';
import '../../models/platform_model.dart';
import '../common.dart';
import '../consts.dart';
import './state_model.dart';

/// Mouse button enum.
enum MouseButtons { left, right, wheel }

extension ToString on MouseButtons {
  String get value {
    switch (this) {
      case MouseButtons.left:
        return 'left';
      case MouseButtons.right:
        return 'right';
      case MouseButtons.wheel:
        return 'wheel';
    }
  }
}

class InputModel {
  final WeakReference<FFI> parent;
  String keyboardMode = "legacy";

  // keyboard
  var shift = false;
  var ctrl = false;
  var alt = false;
  var command = false;

  // mouse
  final isPhysicalMouse = false.obs;
  int _lastMouseDownButtons = 0;
  Offset lastMousePos = Offset.zero;

  get id => parent.target?.id ?? "";

  InputModel(this.parent);

  KeyEventResult handleRawKeyEvent(FocusNode data, RawKeyEvent e) {
    bind.sessionGetKeyboardName(id: id).then((result) {
      keyboardMode = result.toString();
    });

    final key = e.logicalKey;
    if (e is RawKeyDownEvent) {
      if (!e.repeat) {
        if (e.isAltPressed && !alt) {
          alt = true;
        } else if (e.isControlPressed && !ctrl) {
          ctrl = true;
        } else if (e.isShiftPressed && !shift) {
          shift = true;
        } else if (e.isMetaPressed && !command) {
          command = true;
        }
      }
    }
    if (e is RawKeyUpEvent) {
      if (key == LogicalKeyboardKey.altLeft ||
          key == LogicalKeyboardKey.altRight) {
        alt = false;
      } else if (key == LogicalKeyboardKey.controlLeft ||
          key == LogicalKeyboardKey.controlRight) {
        ctrl = false;
      } else if (key == LogicalKeyboardKey.shiftRight ||
          key == LogicalKeyboardKey.shiftLeft) {
        shift = false;
      } else if (key == LogicalKeyboardKey.metaLeft ||
          key == LogicalKeyboardKey.metaRight ||
          key == LogicalKeyboardKey.superKey) {
        command = false;
      }
    }

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
    } else if (e.data is RawKeyEventDataAndroid) {
      RawKeyEventDataAndroid newData = e.data as RawKeyEventDataAndroid;
      scanCode = newData.scanCode + 8;
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
    inputRawKey(e.character ?? "", keyCode, scanCode, down);
  }

  /// Send raw Key Event
  void inputRawKey(String name, int keyCode, int scanCode, bool down) {
    bind.sessionHandleFlutterKeyEvent(
        id: id,
        name: name,
        keycode: keyCode,
        scancode: scanCode,
        downOrUp: down);
  }

  void legacyKeyboardMode(RawKeyEvent e) {
    if (e is RawKeyDownEvent) {
      if (e.repeat) {
        sendRawKey(e, press: true);
      } else {
        sendRawKey(e, down: true);
      }
    }
    if (e is RawKeyUpEvent) {
      sendRawKey(e);
    }
  }

  void sendRawKey(RawKeyEvent e, {bool? down, bool? press}) {
    // for maximum compatibility
    final label = physicalKeyMap[e.physicalKey.usbHidUsage] ??
        logicalKeyMap[e.logicalKey.keyId] ??
        e.logicalKey.keyLabel;
    inputKey(label, down: down, press: press ?? false);
  }

  /// Send key stroke event.
  /// [down] indicates the key's state(down or up).
  /// [press] indicates a click event(down and up).
  void inputKey(String name, {bool? down, bool? press}) {
    if (!parent.target!.ffiModel.keyboard()) return;
    bind.sessionInputKey(
        id: id,
        name: name,
        down: down ?? false,
        press: press ?? true,
        alt: alt,
        ctrl: ctrl,
        shift: shift,
        command: command);
  }

  Map<String, dynamic> getEvent(PointerEvent evt, String type) {
    final Map<String, dynamic> out = {};
    out['type'] = type;
    out['x'] = evt.position.dx;
    out['y'] = evt.position.dy;
    if (alt) out['alt'] = 'true';
    if (shift) out['shift'] = 'true';
    if (ctrl) out['ctrl'] = 'true';
    if (command) out['command'] = 'true';
    out['buttons'] = evt
        .buttons; // left button: 1, right button: 2, middle button: 4, 1 | 2 = 3 (left + right)
    if (evt.buttons != 0) {
      _lastMouseDownButtons = evt.buttons;
    } else {
      out['buttons'] = _lastMouseDownButtons;
    }
    return out;
  }

  /// Send a mouse tap event(down and up).
  void tap(MouseButtons button) {
    sendMouse('down', button);
    sendMouse('up', button);
  }

  /// Send scroll event with scroll distance [y].
  void scroll(int y) {
    bind.sessionSendMouse(
        id: id,
        msg: json
            .encode(modify({'id': id, 'type': 'wheel', 'y': y.toString()})));
  }

  /// Reset key modifiers to false, including [shift], [ctrl], [alt] and [command].
  void resetModifiers() {
    shift = ctrl = alt = command = false;
  }

  /// Modify the given modifier map [evt] based on current modifier key status.
  Map<String, String> modify(Map<String, String> evt) {
    if (ctrl) evt['ctrl'] = 'true';
    if (shift) evt['shift'] = 'true';
    if (alt) evt['alt'] = 'true';
    if (command) evt['command'] = 'true';
    return evt;
  }

  /// Send mouse press event.
  void sendMouse(String type, MouseButtons button) {
    if (!parent.target!.ffiModel.keyboard()) return;
    bind.sessionSendMouse(
        id: id,
        msg: json.encode(modify({'type': type, 'buttons': button.value})));
  }

  void enterOrLeave(bool enter) {
    // Fix status
    if (!enter) {
      resetModifiers();
    }
    bind.sessionEnterOrLeave(id: id, enter: enter);
  }

  /// Send mouse movement event with distance in [x] and [y].
  void moveMouse(double x, double y) {
    if (!parent.target!.ffiModel.keyboard()) return;
    var x2 = x.toInt();
    var y2 = y.toInt();
    bind.sessionSendMouse(
        id: id, msg: json.encode(modify({'x': '$x2', 'y': '$y2'})));
  }

  void onPointHoverImage(PointerHoverEvent e) {
    if (e.kind != ui.PointerDeviceKind.mouse) return;
    if (!isPhysicalMouse.value) {
      isPhysicalMouse.value = true;
    }
    if (isPhysicalMouse.value) {
      handleMouse(getEvent(e, 'mousemove'));
    }
  }

  void onPointDownImage(PointerDownEvent e) {
    debugPrint("onPointDownImage");
    if (e.kind != ui.PointerDeviceKind.mouse) {
      if (isPhysicalMouse.value) {
        isPhysicalMouse.value = false;
      }
    }
    if (isPhysicalMouse.value) {
      handleMouse(getEvent(e, 'mousedown'));
    }
  }

  void onPointUpImage(PointerUpEvent e) {
    if (e.kind != ui.PointerDeviceKind.mouse) return;
    if (isPhysicalMouse.value) {
      handleMouse(getEvent(e, 'mouseup'));
    }
  }

  void onPointMoveImage(PointerMoveEvent e) {
    if (e.kind != ui.PointerDeviceKind.mouse) return;
    if (isPhysicalMouse.value) {
      handleMouse(getEvent(e, 'mousemove'));
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
          id: id, msg: '{"type": "wheel", "x": "$dx", "y": "$dy"}');
    }
  }

  void handleMouse(Map<String, dynamic> evt) {
    double x = evt['x'];
    double y = max(0.0, evt['y']);
    final cursorModel = parent.target!.cursorModel;

    if (cursorModel.isPeerControlProtected) {
      lastMousePos = ui.Offset(x, y);
      return;
    }

    if (!cursorModel.gotMouseControl) {
      bool selfGetControl =
          (x - lastMousePos.dx).abs() > kMouseControlDistance ||
              (y - lastMousePos.dy).abs() > kMouseControlDistance;
      if (selfGetControl) {
        cursorModel.gotMouseControl = true;
      } else {
        lastMousePos = ui.Offset(x, y);
        return;
      }
    }
    lastMousePos = ui.Offset(x, y);

    var type = '';
    var isMove = false;
    switch (evt['type']) {
      case 'mousedown':
        type = 'down';
        break;
      case 'mouseup':
        type = 'up';
        break;
      case 'mousemove':
        isMove = true;
        break;
      default:
        return;
    }
    evt['type'] = type;
    if (isDesktop) {
      y = y - stateGlobal.tabBarHeight;
    }
    final canvasModel = parent.target!.canvasModel;
    final ffiModel = parent.target!.ffiModel;
    if (isMove) {
      canvasModel.moveDesktopMouse(x, y);
    }
    final d = ffiModel.display;
    if (canvasModel.scrollStyle == ScrollStyle.scrollbar) {
      final imageWidth = d.width * canvasModel.scale;
      final imageHeight = d.height * canvasModel.scale;
      x += imageWidth * canvasModel.scrollX;
      y += imageHeight * canvasModel.scrollY;

      // boxed size is a center widget
      if (canvasModel.size.width > imageWidth) {
        x -= ((canvasModel.size.width - imageWidth) / 2);
      }
      if (canvasModel.size.height > imageHeight) {
        y -= ((canvasModel.size.height - imageHeight) / 2);
      }
    } else {
      x -= canvasModel.x;
      y -= canvasModel.y;
    }

    x /= canvasModel.scale;
    y /= canvasModel.scale;
    x += d.x;
    y += d.y;
    if (type != '') {
      x = 0;
      y = 0;
    }
    evt['x'] = '${x.round()}';
    evt['y'] = '${y.round()}';
    var buttons = '';
    switch (evt['buttons']) {
      case 1:
        buttons = 'left';
        break;
      case 2:
        buttons = 'right';
        break;
      case 4:
        buttons = 'wheel';
        break;
    }
    evt['buttons'] = buttons;
    bind.sessionSendMouse(id: id, msg: json.encode(evt));
  }

  /// Web only
  void listenToMouse(bool yesOrNo) {
    if (yesOrNo) {
      platformFFI.startDesktopWebListener();
    } else {
      platformFFI.stopDesktopWebListener();
    }
  }
}
