import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:math';
import 'dart:ui' as ui;

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_hbb/main.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:get/get.dart';

import '../../models/model.dart';
import '../../models/platform_model.dart';
import '../common.dart';
import '../consts.dart';

/// Mouse button enum.
enum MouseButtons { left, right, wheel }

const _kMouseEventDown = 'mousedown';
const _kMouseEventUp = 'mouseup';
const _kMouseEventMove = 'mousemove';

class CanvasCoords {
  double x = 0;
  double y = 0;
  double scale = 1.0;
  double scrollX = 0;
  double scrollY = 0;
  ScrollStyle scrollStyle = ScrollStyle.scrollauto;
  Size size = Size.zero;

  CanvasCoords();

  Map<String, dynamic> toJson() {
    return {
      'x': x,
      'y': y,
      'scale': scale,
      'scrollX': scrollX,
      'scrollY': scrollY,
      'scrollStyle':
          scrollStyle == ScrollStyle.scrollauto ? 'scrollauto' : 'scrollbar',
      'size': {
        'w': size.width,
        'h': size.height,
      }
    };
  }

  static CanvasCoords fromJson(Map<String, dynamic> json) {
    final model = CanvasCoords();
    model.x = json['x'];
    model.y = json['y'];
    model.scale = json['scale'];
    model.scrollX = json['scrollX'];
    model.scrollY = json['scrollY'];
    model.scrollStyle = json['scrollStyle'] == 'scrollauto'
        ? ScrollStyle.scrollauto
        : ScrollStyle.scrollbar;
    model.size = Size(json['size']['w'], json['size']['h']);
    return model;
  }

  static CanvasCoords fromCanvasModel(CanvasModel model) {
    final coords = CanvasCoords();
    coords.x = model.x;
    coords.y = model.y;
    coords.scale = model.scale;
    coords.scrollX = model.scrollX;
    coords.scrollY = model.scrollY;
    coords.scrollStyle = model.scrollStyle;
    coords.size = model.size;
    return coords;
  }
}

class CursorCoords {
  Offset offset = Offset.zero;

  CursorCoords();

  Map<String, dynamic> toJson() {
    return {
      'offset_x': offset.dx,
      'offset_y': offset.dy,
    };
  }

  static CursorCoords fromJson(Map<String, dynamic> json) {
    final model = CursorCoords();
    model.offset = Offset(json['offset_x'], json['offset_y']);
    return model;
  }

  static CursorCoords fromCursorModel(CursorModel model) {
    final coords = CursorCoords();
    coords.offset = model.offset;
    return coords;
  }
}

class RemoteWindowCoords {
  RemoteWindowCoords(
      this.windowRect, this.canvas, this.cursor, this.remoteRect);
  Rect windowRect;
  CanvasCoords canvas;
  CursorCoords cursor;
  Rect remoteRect;
  Offset relativeOffset = Offset.zero;

  Map<String, dynamic> toJson() {
    return {
      'canvas': canvas.toJson(),
      'cursor': cursor.toJson(),
      'windowRect': rectToJson(windowRect),
      'remoteRect': rectToJson(remoteRect),
    };
  }

  static Map<String, dynamic> rectToJson(Rect r) {
    return {
      'l': r.left,
      't': r.top,
      'w': r.width,
      'h': r.height,
    };
  }

  static Rect rectFromJson(Map<String, dynamic> json) {
    return Rect.fromLTWH(
      json['l'],
      json['t'],
      json['w'],
      json['h'],
    );
  }

  RemoteWindowCoords.fromJson(Map<String, dynamic> json)
      : windowRect = rectFromJson(json['windowRect']),
        canvas = CanvasCoords.fromJson(json['canvas']),
        cursor = CursorCoords.fromJson(json['cursor']),
        remoteRect = rectFromJson(json['remoteRect']);
}

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

class PointerEventToRust {
  final String kind;
  final String type;
  final dynamic value;

  PointerEventToRust(this.kind, this.type, this.value);

  Map<String, dynamic> toJson() {
    return {
      'k': kind,
      'v': {
        't': type,
        'v': value,
      }
    };
  }
}

class ToReleaseRawKeys {
  RawKeyEvent? lastLShiftKeyEvent;
  RawKeyEvent? lastRShiftKeyEvent;
  RawKeyEvent? lastLCtrlKeyEvent;
  RawKeyEvent? lastRCtrlKeyEvent;
  RawKeyEvent? lastLAltKeyEvent;
  RawKeyEvent? lastRAltKeyEvent;
  RawKeyEvent? lastLCommandKeyEvent;
  RawKeyEvent? lastRCommandKeyEvent;
  RawKeyEvent? lastSuperKeyEvent;

  reset() {
    lastLShiftKeyEvent = null;
    lastRShiftKeyEvent = null;
    lastLCtrlKeyEvent = null;
    lastRCtrlKeyEvent = null;
    lastLAltKeyEvent = null;
    lastRAltKeyEvent = null;
    lastLCommandKeyEvent = null;
    lastRCommandKeyEvent = null;
    lastSuperKeyEvent = null;
  }

  updateKeyDown(LogicalKeyboardKey logicKey, RawKeyDownEvent e) {
    if (e.isAltPressed) {
      if (logicKey == LogicalKeyboardKey.altLeft) {
        lastLAltKeyEvent = e;
      } else if (logicKey == LogicalKeyboardKey.altRight) {
        lastRAltKeyEvent = e;
      }
    } else if (e.isControlPressed) {
      if (logicKey == LogicalKeyboardKey.controlLeft) {
        lastLCtrlKeyEvent = e;
      } else if (logicKey == LogicalKeyboardKey.controlRight) {
        lastRCtrlKeyEvent = e;
      }
    } else if (e.isShiftPressed) {
      if (logicKey == LogicalKeyboardKey.shiftLeft) {
        lastLShiftKeyEvent = e;
      } else if (logicKey == LogicalKeyboardKey.shiftRight) {
        lastRShiftKeyEvent = e;
      }
    } else if (e.isMetaPressed) {
      if (logicKey == LogicalKeyboardKey.metaLeft) {
        lastLCommandKeyEvent = e;
      } else if (logicKey == LogicalKeyboardKey.metaRight) {
        lastRCommandKeyEvent = e;
      } else if (logicKey == LogicalKeyboardKey.superKey) {
        lastSuperKeyEvent = e;
      }
    }
  }

  updateKeyUp(LogicalKeyboardKey logicKey, RawKeyUpEvent e) {
    if (e.isAltPressed) {
      if (logicKey == LogicalKeyboardKey.altLeft) {
        lastLAltKeyEvent = null;
      } else if (logicKey == LogicalKeyboardKey.altRight) {
        lastRAltKeyEvent = null;
      }
    } else if (e.isControlPressed) {
      if (logicKey == LogicalKeyboardKey.controlLeft) {
        lastLCtrlKeyEvent = null;
      } else if (logicKey == LogicalKeyboardKey.controlRight) {
        lastRCtrlKeyEvent = null;
      }
    } else if (e.isShiftPressed) {
      if (logicKey == LogicalKeyboardKey.shiftLeft) {
        lastLShiftKeyEvent = null;
      } else if (logicKey == LogicalKeyboardKey.shiftRight) {
        lastRShiftKeyEvent = null;
      }
    } else if (e.isMetaPressed) {
      if (logicKey == LogicalKeyboardKey.metaLeft) {
        lastLCommandKeyEvent = null;
      } else if (logicKey == LogicalKeyboardKey.metaRight) {
        lastRCommandKeyEvent = null;
      } else if (logicKey == LogicalKeyboardKey.superKey) {
        lastSuperKeyEvent = null;
      }
    }
  }

  release(KeyEventResult Function(RawKeyEvent e) handleRawKeyEvent) {
    for (final key in [
      lastLShiftKeyEvent,
      lastRShiftKeyEvent,
      lastLCtrlKeyEvent,
      lastRCtrlKeyEvent,
      lastLAltKeyEvent,
      lastRAltKeyEvent,
      lastLCommandKeyEvent,
      lastRCommandKeyEvent,
      lastSuperKeyEvent,
    ]) {
      if (key != null) {
        handleRawKeyEvent(RawKeyUpEvent(
          data: key.data,
          character: key.character,
        ));
      }
    }
  }
}

class ToReleaseKeys {
  KeyEvent? lastLShiftKeyEvent;
  KeyEvent? lastRShiftKeyEvent;
  KeyEvent? lastLCtrlKeyEvent;
  KeyEvent? lastRCtrlKeyEvent;
  KeyEvent? lastLAltKeyEvent;
  KeyEvent? lastRAltKeyEvent;
  KeyEvent? lastLCommandKeyEvent;
  KeyEvent? lastRCommandKeyEvent;
  KeyEvent? lastSuperKeyEvent;

  reset() {
    lastLShiftKeyEvent = null;
    lastRShiftKeyEvent = null;
    lastLCtrlKeyEvent = null;
    lastRCtrlKeyEvent = null;
    lastLAltKeyEvent = null;
    lastRAltKeyEvent = null;
    lastLCommandKeyEvent = null;
    lastRCommandKeyEvent = null;
    lastSuperKeyEvent = null;
  }

  release(KeyEventResult Function(KeyEvent e) handleKeyEvent) {
    for (final key in [
      lastLShiftKeyEvent,
      lastRShiftKeyEvent,
      lastLCtrlKeyEvent,
      lastRCtrlKeyEvent,
      lastLAltKeyEvent,
      lastRAltKeyEvent,
      lastLCommandKeyEvent,
      lastRCommandKeyEvent,
      lastSuperKeyEvent,
    ]) {
      if (key != null) {
        handleKeyEvent(key);
      }
    }
  }
}

class InputModel {
  final WeakReference<FFI> parent;
  String keyboardMode = '';

  // keyboard
  var shift = false;
  var ctrl = false;
  var alt = false;
  var command = false;

  final ToReleaseRawKeys toReleaseRawKeys = ToReleaseRawKeys();
  final ToReleaseKeys toReleaseKeys = ToReleaseKeys();

  // trackpad
  var _trackpadLastDelta = Offset.zero;
  var _stopFling = true;
  var _fling = false;
  Timer? _flingTimer;
  final _flingBaseDelay = 30;
  // trackpad, peer linux
  final _trackpadSpeed = 0.06;
  var _trackpadScrollUnsent = Offset.zero;

  var _lastScale = 1.0;

  bool _pointerMovedAfterEnter = false;

  // mouse
  final isPhysicalMouse = false.obs;
  int _lastButtons = 0;
  Offset lastMousePos = Offset.zero;

  bool _queryOtherWindowCoords = false;
  Rect? _windowRect;
  List<RemoteWindowCoords> _remoteWindowCoords = [];

  late final SessionID sessionId;

  bool get keyboardPerm => parent.target!.ffiModel.keyboard;
  String get id => parent.target?.id ?? '';
  String? get peerPlatform => parent.target?.ffiModel.pi.platform;
  bool get isViewOnly => parent.target!.ffiModel.viewOnly;
  double get devicePixelRatio => parent.target!.canvasModel.devicePixelRatio;

  InputModel(this.parent) {
    sessionId = parent.target!.sessionId;
  }

  // This function must be called after the peer info is received.
  // Because `sessionGetKeyboardMode` relies on the peer version.
  updateKeyboardMode() async {
    // * Currently mobile does not enable map mode
    if (isDesktop || isWebDesktop) {
      keyboardMode = await bind.sessionGetKeyboardMode(sessionId: sessionId) ??
          kKeyLegacyMode;
    }
  }

  void handleKeyDownEventModifiers(KeyEvent e) {
    KeyUpEvent upEvent(e) => KeyUpEvent(
          physicalKey: e.physicalKey,
          logicalKey: e.logicalKey,
          timeStamp: e.timeStamp,
        );
    if (e.logicalKey == LogicalKeyboardKey.altLeft) {
      if (!alt) {
        alt = true;
      }
      toReleaseKeys.lastLAltKeyEvent = upEvent(e);
    } else if (e.logicalKey == LogicalKeyboardKey.altRight) {
      if (!alt) {
        alt = true;
      }
      toReleaseKeys.lastLAltKeyEvent = upEvent(e);
    } else if (e.logicalKey == LogicalKeyboardKey.controlLeft) {
      if (!ctrl) {
        ctrl = true;
      }
      toReleaseKeys.lastLCtrlKeyEvent = upEvent(e);
    } else if (e.logicalKey == LogicalKeyboardKey.controlRight) {
      if (!ctrl) {
        ctrl = true;
      }
      toReleaseKeys.lastRCtrlKeyEvent = upEvent(e);
    } else if (e.logicalKey == LogicalKeyboardKey.shiftLeft) {
      if (!shift) {
        shift = true;
      }
      toReleaseKeys.lastLShiftKeyEvent = upEvent(e);
    } else if (e.logicalKey == LogicalKeyboardKey.shiftRight) {
      if (!shift) {
        shift = true;
      }
      toReleaseKeys.lastRShiftKeyEvent = upEvent(e);
    } else if (e.logicalKey == LogicalKeyboardKey.metaLeft) {
      if (!command) {
        command = true;
      }
      toReleaseKeys.lastLCommandKeyEvent = upEvent(e);
    } else if (e.logicalKey == LogicalKeyboardKey.metaRight) {
      if (!command) {
        command = true;
      }
      toReleaseKeys.lastRCommandKeyEvent = upEvent(e);
    } else if (e.logicalKey == LogicalKeyboardKey.superKey) {
      if (!command) {
        command = true;
      }
      toReleaseKeys.lastSuperKeyEvent = upEvent(e);
    }
  }

  void handleKeyUpEventModifiers(KeyEvent e) {
    if (e.logicalKey == LogicalKeyboardKey.altLeft) {
      alt = false;
      toReleaseKeys.lastLAltKeyEvent = null;
    } else if (e.logicalKey == LogicalKeyboardKey.altRight) {
      alt = false;
      toReleaseKeys.lastRAltKeyEvent = null;
    } else if (e.logicalKey == LogicalKeyboardKey.controlLeft) {
      ctrl = false;
      toReleaseKeys.lastLCtrlKeyEvent = null;
    } else if (e.logicalKey == LogicalKeyboardKey.controlRight) {
      ctrl = false;
      toReleaseKeys.lastRCtrlKeyEvent = null;
    } else if (e.logicalKey == LogicalKeyboardKey.shiftLeft) {
      shift = false;
      toReleaseKeys.lastLShiftKeyEvent = null;
    } else if (e.logicalKey == LogicalKeyboardKey.shiftRight) {
      shift = false;
      toReleaseKeys.lastRShiftKeyEvent = null;
    } else if (e.logicalKey == LogicalKeyboardKey.metaLeft) {
      command = false;
      toReleaseKeys.lastLCommandKeyEvent = null;
    } else if (e.logicalKey == LogicalKeyboardKey.metaRight) {
      command = false;
      toReleaseKeys.lastRCommandKeyEvent = null;
    } else if (e.logicalKey == LogicalKeyboardKey.superKey) {
      command = false;
      toReleaseKeys.lastSuperKeyEvent = null;
    }
  }

  KeyEventResult handleRawKeyEvent(RawKeyEvent e) {
    if (isViewOnly) return KeyEventResult.handled;
    if (!isInputSourceFlutter) {
      if (isDesktop) {
        return KeyEventResult.handled;
      } else if (isWeb) {
        return KeyEventResult.ignored;
      }
    }

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
      toReleaseRawKeys.updateKeyDown(key, e);
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

      toReleaseRawKeys.updateKeyUp(key, e);
    }

    // * Currently mobile does not enable map mode
    if ((isDesktop || isWebDesktop) && keyboardMode == kKeyMapMode) {
      mapKeyboardModeRaw(e);
    } else {
      legacyKeyboardModeRaw(e);
    }

    return KeyEventResult.handled;
  }

  KeyEventResult handleKeyEvent(KeyEvent e) {
    if (isViewOnly) return KeyEventResult.handled;
    if (!isInputSourceFlutter) {
      if (isDesktop) {
        return KeyEventResult.handled;
      } else if (isWeb) {
        return KeyEventResult.ignored;
      }
    }
    if (isWindows || isLinux) {
      // Ignore meta keys. Because flutter window will loose focus if meta key is pressed.
      if (e.physicalKey == PhysicalKeyboardKey.metaLeft ||
          e.physicalKey == PhysicalKeyboardKey.metaRight) {
        return KeyEventResult.handled;
      }
    }

    if (e is KeyUpEvent) {
      handleKeyUpEventModifiers(e);
    } else if (e is KeyDownEvent) {
      handleKeyDownEventModifiers(e);
    }

    bool isMobileAndMapMode = false;
    if (isMobile) {
      // Do not use map mode if mobile -> Android. Android does not support map mode for now.
      // Because simulating the physical key events(uhid) which requires root permission is not supported.
      if (peerPlatform != kPeerPlatformAndroid) {
        if (isIOS) {
          isMobileAndMapMode = true;
        } else {
          // The physicalKey.usbHidUsage may be not correct for soft keyboard on Android.
          // iOS does not have this issue.
          // 1. Open the soft keyboard on Android
          // 2. Switch to input method like zh/ko/ja
          // 3. Click Backspace and Enter on the soft keyboard or physical keyboard
          // 4. The physicalKey.usbHidUsage is not correct.
          // PhysicalKeyboardKey#8ac83(usbHidUsage: "0x1100000042", debugName: "Key with ID 0x1100000042")
          // LogicalKeyboardKey#2604c(keyId: "0x10000000d", keyLabel: "Enter", debugName: "Enter")
          //
          // The correct PhysicalKeyboardKey should be
          // PhysicalKeyboardKey#e14a9(usbHidUsage: "0x00070028", debugName: "Enter")
          // https://github.com/flutter/flutter/issues/157771
          // We cannot use the debugName to determine the key is correct or not, because it's null in release mode.
          // The normal `usbHidUsage` for keyboard shoud be between [0x00000010, 0x000c029f]
          // https://github.com/flutter/flutter/blob/c051b69e2a2224300e20d93dbd15f4b91e8844d1/packages/flutter/lib/src/services/keyboard_key.g.dart#L5332 - 5600
          final isNormalHsbHidUsage = (e.physicalKey.usbHidUsage >> 20) == 0;
          isMobileAndMapMode = isNormalHsbHidUsage &&
              // No need to check `!['Backspace', 'Enter'].contains(e.logicalKey.keyLabel)`
              // But we still add it for more reliability.
              !['Backspace', 'Enter'].contains(e.logicalKey.keyLabel);
        }
      }
    }
    final isDesktopAndMapMode =
        isDesktop || (isWebDesktop && keyboardMode == kKeyMapMode);
    if (isMobileAndMapMode || isDesktopAndMapMode) {
      // FIXME: e.character is wrong for dead keys, eg: ^ in de
      newKeyboardMode(
          e.character ?? '',
          e.physicalKey.usbHidUsage & 0xFFFF,
          // Show repeat event be converted to "release+press" events?
          e is KeyDownEvent || e is KeyRepeatEvent);
    } else {
      legacyKeyboardMode(e);
    }

    return KeyEventResult.handled;
  }

  /// Send Key Event
  void newKeyboardMode(String character, int usbHid, bool down) {
    const capslock = 1;
    const numlock = 2;
    const scrolllock = 3;
    int lockModes = 0;
    if (HardwareKeyboard.instance.lockModesEnabled
        .contains(KeyboardLockMode.capsLock)) {
      lockModes |= (1 << capslock);
    }
    if (HardwareKeyboard.instance.lockModesEnabled
        .contains(KeyboardLockMode.numLock)) {
      lockModes |= (1 << numlock);
    }
    if (HardwareKeyboard.instance.lockModesEnabled
        .contains(KeyboardLockMode.scrollLock)) {
      lockModes |= (1 << scrolllock);
    }
    bind.sessionHandleFlutterKeyEvent(
        sessionId: sessionId,
        character: character,
        usbHid: usbHid,
        lockModes: lockModes,
        downOrUp: down);
  }

  void mapKeyboardModeRaw(RawKeyEvent e) {
    int positionCode = -1;
    int platformCode = -1;
    bool down;

    if (e.data is RawKeyEventDataMacOs) {
      RawKeyEventDataMacOs newData = e.data as RawKeyEventDataMacOs;
      positionCode = newData.keyCode;
      platformCode = newData.keyCode;
    } else if (e.data is RawKeyEventDataWindows) {
      RawKeyEventDataWindows newData = e.data as RawKeyEventDataWindows;
      positionCode = newData.scanCode;
      platformCode = newData.keyCode;
    } else if (e.data is RawKeyEventDataLinux) {
      RawKeyEventDataLinux newData = e.data as RawKeyEventDataLinux;
      // scanCode and keyCode of RawKeyEventDataLinux are incorrect.
      // 1. scanCode means keycode
      // 2. keyCode means keysym
      positionCode = newData.scanCode;
      platformCode = newData.keyCode;
    } else if (e.data is RawKeyEventDataAndroid) {
      RawKeyEventDataAndroid newData = e.data as RawKeyEventDataAndroid;
      positionCode = newData.scanCode + 8;
      platformCode = newData.keyCode;
    } else {}

    if (e is RawKeyDownEvent) {
      down = true;
    } else {
      down = false;
    }
    inputRawKey(e.character ?? '', platformCode, positionCode, down);
  }

  /// Send raw Key Event
  void inputRawKey(String name, int platformCode, int positionCode, bool down) {
    const capslock = 1;
    const numlock = 2;
    const scrolllock = 3;
    int lockModes = 0;
    if (HardwareKeyboard.instance.lockModesEnabled
        .contains(KeyboardLockMode.capsLock)) {
      lockModes |= (1 << capslock);
    }
    if (HardwareKeyboard.instance.lockModesEnabled
        .contains(KeyboardLockMode.numLock)) {
      lockModes |= (1 << numlock);
    }
    if (HardwareKeyboard.instance.lockModesEnabled
        .contains(KeyboardLockMode.scrollLock)) {
      lockModes |= (1 << scrolllock);
    }
    bind.sessionHandleFlutterRawKeyEvent(
        sessionId: sessionId,
        name: name,
        platformCode: platformCode,
        positionCode: positionCode,
        lockModes: lockModes,
        downOrUp: down);
  }

  void legacyKeyboardModeRaw(RawKeyEvent e) {
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

  void legacyKeyboardMode(KeyEvent e) {
    if (e is KeyDownEvent) {
      sendKey(e, down: true);
    } else if (e is KeyRepeatEvent) {
      sendKey(e, press: true);
    } else if (e is KeyUpEvent) {
      sendKey(e);
    }
  }

  void sendKey(KeyEvent e, {bool? down, bool? press}) {
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
    if (!keyboardPerm) return;
    bind.sessionInputKey(
        sessionId: sessionId,
        name: name,
        down: down ?? false,
        press: press ?? true,
        alt: alt,
        ctrl: ctrl,
        shift: shift,
        command: command);
  }

  Map<String, dynamic> _getMouseEvent(PointerEvent evt, String type) {
    final Map<String, dynamic> out = {};

    // Check update event type and set buttons to be sent.
    int buttons = _lastButtons;
    if (type == _kMouseEventMove) {
      // flutter may emit move event if one button is pressed and another button
      // is pressing or releasing.
      if (evt.buttons != _lastButtons) {
        // For simplicity
        // Just consider 3 - 1 ((Left + Right buttons) - Left button)
        // Do not consider 2 - 1 (Right button - Left button)
        // or 6 - 5 ((Right + Mid buttons) - (Left + Mid buttons))
        // and so on
        buttons = evt.buttons - _lastButtons;
        if (buttons > 0) {
          type = _kMouseEventDown;
        } else {
          type = _kMouseEventUp;
          buttons = -buttons;
        }
      }
    } else {
      if (evt.buttons != 0) {
        buttons = evt.buttons;
      }
    }
    _lastButtons = evt.buttons;

    out['buttons'] = buttons;
    out['type'] = type;
    return out;
  }

  /// Send a mouse tap event(down and up).
  Future<void> tap(MouseButtons button) async {
    await sendMouse('down', button);
    await sendMouse('up', button);
  }

  Future<void> tapDown(MouseButtons button) async {
    await sendMouse('down', button);
  }

  Future<void> tapUp(MouseButtons button) async {
    await sendMouse('up', button);
  }

  /// Send scroll event with scroll distance [y].
  Future<void> scroll(int y) async {
    await bind.sessionSendMouse(
        sessionId: sessionId,
        msg: json
            .encode(modify({'id': id, 'type': 'wheel', 'y': y.toString()})));
  }

  /// Reset key modifiers to false, including [shift], [ctrl], [alt] and [command].
  void resetModifiers() {
    shift = ctrl = alt = command = false;
  }

  /// Modify the given modifier map [evt] based on current modifier key status.
  Map<String, dynamic> modify(Map<String, dynamic> evt) {
    if (ctrl) evt['ctrl'] = 'true';
    if (shift) evt['shift'] = 'true';
    if (alt) evt['alt'] = 'true';
    if (command) evt['command'] = 'true';
    return evt;
  }

  /// Send mouse press event.
  Future<void> sendMouse(String type, MouseButtons button) async {
    if (!keyboardPerm) return;
    await bind.sessionSendMouse(
        sessionId: sessionId,
        msg: json.encode(modify({'type': type, 'buttons': button.value})));
  }

  void enterOrLeave(bool enter) {
    toReleaseKeys.release(handleKeyEvent);
    toReleaseRawKeys.release(handleRawKeyEvent);
    _pointerMovedAfterEnter = false;

    // Fix status
    if (!enter) {
      resetModifiers();
    }
    _flingTimer?.cancel();
    if (!isInputSourceFlutter) {
      bind.sessionEnterOrLeave(sessionId: sessionId, enter: enter);
    }
    if (!isWeb && enter) {
      bind.setCurSessionId(sessionId: sessionId);
    }
  }

  /// Send mouse movement event with distance in [x] and [y].
  Future<void> moveMouse(double x, double y) async {
    if (!keyboardPerm) return;
    var x2 = x.toInt();
    var y2 = y.toInt();
    await bind.sessionSendMouse(
        sessionId: sessionId,
        msg: json.encode(modify({'x': '$x2', 'y': '$y2'})));
  }

  void onPointHoverImage(PointerHoverEvent e) {
    _stopFling = true;
    if (isViewOnly) return;
    if (e.kind != ui.PointerDeviceKind.mouse) return;
    if (!isPhysicalMouse.value) {
      isPhysicalMouse.value = true;
    }
    if (isPhysicalMouse.value) {
      handleMouse(_getMouseEvent(e, _kMouseEventMove), e.position);
    }
  }

  void onPointerPanZoomStart(PointerPanZoomStartEvent e) {
    _lastScale = 1.0;
    _stopFling = true;
    if (isViewOnly) return;
    if (peerPlatform == kPeerPlatformAndroid) {
      handlePointerEvent('touch', kMouseEventTypePanStart, e.position);
    }
  }

  // https://docs.flutter.dev/release/breaking-changes/trackpad-gestures
  void onPointerPanZoomUpdate(PointerPanZoomUpdateEvent e) {
    if (isViewOnly) return;
    if (peerPlatform != kPeerPlatformAndroid) {
      final scale = ((e.scale - _lastScale) * 1000).toInt();
      _lastScale = e.scale;

      if (scale != 0) {
        bind.sessionSendPointer(
            sessionId: sessionId,
            msg: json.encode(
                PointerEventToRust(kPointerEventKindTouch, 'scale', scale)
                    .toJson()));
        return;
      }
    }

    final delta = e.panDelta;
    _trackpadLastDelta = delta;

    var x = delta.dx.toInt();
    var y = delta.dy.toInt();
    if (peerPlatform == kPeerPlatformLinux) {
      _trackpadScrollUnsent += (delta * _trackpadSpeed);
      x = _trackpadScrollUnsent.dx.truncate();
      y = _trackpadScrollUnsent.dy.truncate();
      _trackpadScrollUnsent -= Offset(x.toDouble(), y.toDouble());
    } else {
      if (x == 0 && y == 0) {
        final thr = 0.1;
        if (delta.dx.abs() > delta.dy.abs()) {
          x = delta.dx > thr ? 1 : (delta.dx < -thr ? -1 : 0);
        } else {
          y = delta.dy > thr ? 1 : (delta.dy < -thr ? -1 : 0);
        }
      }
    }
    if (x != 0 || y != 0) {
      if (peerPlatform == kPeerPlatformAndroid) {
        handlePointerEvent('touch', kMouseEventTypePanUpdate,
            Offset(x.toDouble(), y.toDouble()));
      } else {
        bind.sessionSendMouse(
            sessionId: sessionId,
            msg: '{"type": "trackpad", "x": "$x", "y": "$y"}');
      }
    }
  }

  void _scheduleFling(double x, double y, int delay) {
    if ((x == 0 && y == 0) || _stopFling) {
      _fling = false;
      return;
    }

    _flingTimer = Timer(Duration(milliseconds: delay), () {
      if (_stopFling) {
        _fling = false;
        return;
      }

      final d = 0.97;
      x *= d;
      y *= d;

      // Try set delta (x,y) and delay.
      var dx = x.toInt();
      var dy = y.toInt();
      if (parent.target?.ffiModel.pi.platform == kPeerPlatformLinux) {
        dx = (x * _trackpadSpeed).toInt();
        dy = (y * _trackpadSpeed).toInt();
      }

      var delay = _flingBaseDelay;

      if (dx == 0 && dy == 0) {
        _fling = false;
        return;
      }

      bind.sessionSendMouse(
          sessionId: sessionId,
          msg: '{"type": "trackpad", "x": "$dx", "y": "$dy"}');
      _scheduleFling(x, y, delay);
    });
  }

  void waitLastFlingDone() {
    if (_fling) {
      _stopFling = true;
    }
    for (var i = 0; i < 5; i++) {
      if (!_fling) {
        break;
      }
      sleep(Duration(milliseconds: 10));
    }
    _flingTimer?.cancel();
  }

  void onPointerPanZoomEnd(PointerPanZoomEndEvent e) {
    if (peerPlatform == kPeerPlatformAndroid) {
      handlePointerEvent('touch', kMouseEventTypePanEnd, e.position);
      return;
    }

    bind.sessionSendPointer(
        sessionId: sessionId,
        msg: json.encode(
            PointerEventToRust(kPointerEventKindTouch, 'scale', 0).toJson()));

    waitLastFlingDone();
    _stopFling = false;

    // 2.0 is an experience value
    double minFlingValue = 2.0;
    if (_trackpadLastDelta.dx.abs() > minFlingValue ||
        _trackpadLastDelta.dy.abs() > minFlingValue) {
      _fling = true;
      _scheduleFling(
          _trackpadLastDelta.dx, _trackpadLastDelta.dy, _flingBaseDelay);
    }
    _trackpadLastDelta = Offset.zero;
  }

  void onPointDownImage(PointerDownEvent e) {
    debugPrint("onPointDownImage ${e.kind}");
    _stopFling = true;
    if (isDesktop) _queryOtherWindowCoords = true;
    _remoteWindowCoords = [];
    _windowRect = null;
    if (isViewOnly) return;
    if (e.kind != ui.PointerDeviceKind.mouse) {
      if (isPhysicalMouse.value) {
        isPhysicalMouse.value = false;
      }
    }
    if (isPhysicalMouse.value) {
      handleMouse(_getMouseEvent(e, _kMouseEventDown), e.position);
    }
  }

  void onPointUpImage(PointerUpEvent e) {
    if (isDesktop) _queryOtherWindowCoords = false;
    if (isViewOnly) return;
    if (e.kind != ui.PointerDeviceKind.mouse) return;
    if (isPhysicalMouse.value) {
      handleMouse(_getMouseEvent(e, _kMouseEventUp), e.position);
    }
  }

  void onPointMoveImage(PointerMoveEvent e) {
    if (isViewOnly) return;
    if (e.kind != ui.PointerDeviceKind.mouse) return;
    if (_queryOtherWindowCoords) {
      Future.delayed(Duration.zero, () async {
        _windowRect = await fillRemoteCoordsAndGetCurFrame(_remoteWindowCoords);
      });
      _queryOtherWindowCoords = false;
    }
    if (isPhysicalMouse.value) {
      handleMouse(_getMouseEvent(e, _kMouseEventMove), e.position);
    }
  }

  static Future<Rect?> fillRemoteCoordsAndGetCurFrame(
      List<RemoteWindowCoords> remoteWindowCoords) async {
    final coords =
        await rustDeskWinManager.getOtherRemoteWindowCoordsFromMain();
    final wc = WindowController.fromWindowId(kWindowId!);
    try {
      final frame = await wc.getFrame();
      for (final c in coords) {
        c.relativeOffset = Offset(
            c.windowRect.left - frame.left, c.windowRect.top - frame.top);
        remoteWindowCoords.add(c);
      }
      return frame;
    } catch (e) {
      // Unreachable code
      debugPrint("Failed to get frame of window $kWindowId, it may be hidden");
    }
    return null;
  }

  void onPointerSignalImage(PointerSignalEvent e) {
    if (isViewOnly) return;
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
          sessionId: sessionId,
          msg: '{"type": "wheel", "x": "$dx", "y": "$dy"}');
    }
  }

  void refreshMousePos() => handleMouse({
        'buttons': 0,
        'type': _kMouseEventMove,
      }, lastMousePos);

  void tryMoveEdgeOnExit(Offset pos) => handleMouse(
        {
          'buttons': 0,
          'type': _kMouseEventMove,
        },
        pos,
        onExit: true,
      );

  static int tryGetNearestRange(int v, int min, int max, int n) {
    if (v < min && v >= min - n) {
      v = min;
    }
    if (v > max && v <= max + n) {
      v = max;
    }
    return v;
  }

  Offset setNearestEdge(double x, double y, Rect rect) {
    double left = x - rect.left;
    double right = rect.right - 1 - x;
    double top = y - rect.top;
    double bottom = rect.bottom - 1 - y;
    if (left < right && left < top && left < bottom) {
      x = rect.left;
    }
    if (right < left && right < top && right < bottom) {
      x = rect.right - 1;
    }
    if (top < left && top < right && top < bottom) {
      y = rect.top;
    }
    if (bottom < left && bottom < right && bottom < top) {
      y = rect.bottom - 1;
    }
    return Offset(x, y);
  }

  void handlePointerEvent(String kind, String type, Offset offset) {
    double x = offset.dx;
    double y = offset.dy;
    if (_checkPeerControlProtected(x, y)) {
      return;
    }
    // Only touch events are handled for now. So we can just ignore buttons.
    // to-do: handle mouse events

    late final dynamic evtValue;
    if (type == kMouseEventTypePanUpdate) {
      evtValue = {
        'x': x.toInt(),
        'y': y.toInt(),
      };
    } else {
      final isMoveTypes = [kMouseEventTypePanStart, kMouseEventTypePanEnd];
      final pos = handlePointerDevicePos(
        kPointerEventKindTouch,
        x,
        y,
        isMoveTypes.contains(type),
        type,
      );
      if (pos == null) {
        return;
      }
      evtValue = {
        'x': pos.x,
        'y': pos.y,
      };
    }

    final evt = PointerEventToRust(kind, type, evtValue).toJson();
    bind.sessionSendPointer(
        sessionId: sessionId, msg: json.encode(modify(evt)));
  }

  bool _checkPeerControlProtected(double x, double y) {
    final cursorModel = parent.target!.cursorModel;
    if (cursorModel.isPeerControlProtected) {
      lastMousePos = ui.Offset(x, y);
      return true;
    }

    if (!cursorModel.gotMouseControl) {
      bool selfGetControl =
          (x - lastMousePos.dx).abs() > kMouseControlDistance ||
              (y - lastMousePos.dy).abs() > kMouseControlDistance;
      if (selfGetControl) {
        cursorModel.gotMouseControl = true;
      } else {
        lastMousePos = ui.Offset(x, y);
        return true;
      }
    }
    lastMousePos = ui.Offset(x, y);
    return false;
  }

  void handleMouse(
    Map<String, dynamic> evt,
    Offset offset, {
    bool onExit = false,
  }) {
    double x = offset.dx;
    double y = max(0.0, offset.dy);
    if (_checkPeerControlProtected(x, y)) {
      return;
    }

    var type = kMouseEventTypeDefault;
    var isMove = false;
    switch (evt['type']) {
      case _kMouseEventDown:
        type = kMouseEventTypeDown;
        break;
      case _kMouseEventUp:
        type = kMouseEventTypeUp;
        break;
      case _kMouseEventMove:
        _pointerMovedAfterEnter = true;
        isMove = true;
        break;
      default:
        return;
    }
    evt['type'] = type;

    if (type == kMouseEventTypeDown && !_pointerMovedAfterEnter) {
      // Move mouse to the position of the down event first.
      lastMousePos = ui.Offset(x, y);
      refreshMousePos();
    }

    final pos = handlePointerDevicePos(
      kPointerEventKindMouse,
      x,
      y,
      isMove,
      type,
      onExit: onExit,
      buttons: evt['buttons'],
    );
    if (pos == null) {
      return;
    }
    if (type != '') {
      evt['x'] = '0';
      evt['y'] = '0';
    } else {
      evt['x'] = '${pos.x}';
      evt['y'] = '${pos.y}';
    }

    Map<int, String> mapButtons = {
      kPrimaryMouseButton: 'left',
      kSecondaryMouseButton: 'right',
      kMiddleMouseButton: 'wheel',
      kBackMouseButton: 'back',
      kForwardMouseButton: 'forward'
    };
    evt['buttons'] = mapButtons[evt['buttons']] ?? '';
    bind.sessionSendMouse(sessionId: sessionId, msg: json.encode(modify(evt)));
  }

  Point? handlePointerDevicePos(
    String kind,
    double x,
    double y,
    bool isMove,
    String evtType, {
    bool onExit = false,
    int buttons = kPrimaryMouseButton,
  }) {
    final ffiModel = parent.target!.ffiModel;
    CanvasCoords canvas =
        CanvasCoords.fromCanvasModel(parent.target!.canvasModel);
    Rect? rect = ffiModel.rect;

    if (isMove) {
      if (_remoteWindowCoords.isNotEmpty &&
          _windowRect != null &&
          !_isInCurrentWindow(x, y)) {
        final coords =
            findRemoteCoords(x, y, _remoteWindowCoords, devicePixelRatio);
        if (coords != null) {
          isMove = false;
          canvas = coords.canvas;
          rect = coords.remoteRect;
          x -= coords.relativeOffset.dx / devicePixelRatio;
          y -= coords.relativeOffset.dy / devicePixelRatio;
        }
      }
    }

    y -= CanvasModel.topToEdge;
    x -= CanvasModel.leftToEdge;
    if (isMove) {
      parent.target!.canvasModel.moveDesktopMouse(x, y);
    }

    return _handlePointerDevicePos(
      kind,
      x,
      y,
      isMove,
      canvas,
      rect,
      evtType,
      onExit: onExit,
      buttons: buttons,
    );
  }

  bool _isInCurrentWindow(double x, double y) {
    final w = _windowRect!.width / devicePixelRatio;
    final h = _windowRect!.width / devicePixelRatio;
    return x >= 0 && y >= 0 && x <= w && y <= h;
  }

  static RemoteWindowCoords? findRemoteCoords(double x, double y,
      List<RemoteWindowCoords> remoteWindowCoords, double devicePixelRatio) {
    x *= devicePixelRatio;
    y *= devicePixelRatio;
    for (final c in remoteWindowCoords) {
      if (x >= c.relativeOffset.dx &&
          y >= c.relativeOffset.dy &&
          x <= c.relativeOffset.dx + c.windowRect.width &&
          y <= c.relativeOffset.dy + c.windowRect.height) {
        return c;
      }
    }
    return null;
  }

  Point? _handlePointerDevicePos(
    String kind,
    double x,
    double y,
    bool moveInCanvas,
    CanvasCoords canvas,
    Rect? rect,
    String evtType, {
    bool onExit = false,
    int buttons = kPrimaryMouseButton,
  }) {
    if (rect == null) {
      return null;
    }

    final nearThr = 3;
    var nearRight = (canvas.size.width - x) < nearThr;
    var nearBottom = (canvas.size.height - y) < nearThr;
    final imageWidth = rect.width * canvas.scale;
    final imageHeight = rect.height * canvas.scale;
    if (canvas.scrollStyle == ScrollStyle.scrollbar) {
      x += imageWidth * canvas.scrollX;
      y += imageHeight * canvas.scrollY;

      // boxed size is a center widget
      if (canvas.size.width > imageWidth) {
        x -= ((canvas.size.width - imageWidth) / 2);
      }
      if (canvas.size.height > imageHeight) {
        y -= ((canvas.size.height - imageHeight) / 2);
      }
    } else {
      x -= canvas.x;
      y -= canvas.y;
    }

    x /= canvas.scale;
    y /= canvas.scale;
    if (canvas.scale > 0 && canvas.scale < 1) {
      final step = 1.0 / canvas.scale - 1;
      if (nearRight) {
        x += step;
      }
      if (nearBottom) {
        y += step;
      }
    }
    x += rect.left;
    y += rect.top;

    if (onExit) {
      final pos = setNearestEdge(x, y, rect);
      x = pos.dx;
      y = pos.dy;
    }

    var evtX = 0;
    var evtY = 0;
    try {
      evtX = x.round();
      evtY = y.round();
    } catch (e) {
      debugPrintStack(label: 'canvas.scale value ${canvas.scale}, $e');
      return null;
    }

    return InputModel.getPointInRemoteRect(
        true, peerPlatform, kind, evtType, evtX, evtY, rect,
        buttons: buttons);
  }

  static Point? getPointInRemoteRect(bool isLocalDesktop, String? peerPlatform,
      String kind, String evtType, int evtX, int evtY, Rect rect,
      {int buttons = kPrimaryMouseButton}) {
    int minX = rect.left.toInt();
    // https://github.com/rustdesk/rustdesk/issues/6678
    // For Windows, [0,maxX], [0,maxY] should be set to enable window snapping.
    int maxX = (rect.left + rect.width).toInt() -
        (peerPlatform == kPeerPlatformWindows ? 0 : 1);
    int minY = rect.top.toInt();
    int maxY = (rect.top + rect.height).toInt() -
        (peerPlatform == kPeerPlatformWindows ? 0 : 1);
    evtX = InputModel.tryGetNearestRange(evtX, minX, maxX, 5);
    evtY = InputModel.tryGetNearestRange(evtY, minY, maxY, 5);
    if (isLocalDesktop) {
      if (kind == kPointerEventKindMouse) {
        if (evtX < minX || evtY < minY || evtX > maxX || evtY > maxY) {
          // If left mouse up, no early return.
          if (!(buttons == kPrimaryMouseButton &&
              evtType == kMouseEventTypeUp)) {
            return null;
          }
        }
      }
    } else {
      bool evtXInRange = evtX >= minX && evtX <= maxX;
      bool evtYInRange = evtY >= minY && evtY <= maxY;
      if (!(evtXInRange || evtYInRange)) {
        return null;
      }
      if (evtX < minX) {
        evtX = minX;
      } else if (evtX > maxX) {
        evtX = maxX;
      }
      if (evtY < minY) {
        evtY = minY;
      } else if (evtY > maxY) {
        evtY = maxY;
      }
    }

    return Point(evtX, evtY);
  }

  /// Web only
  void listenToMouse(bool yesOrNo) {
    if (yesOrNo) {
      platformFFI.startDesktopWebListener();
    } else {
      platformFFI.stopDesktopWebListener();
    }
  }

  void onMobileBack() => tap(MouseButtons.right);
  void onMobileHome() => tap(MouseButtons.wheel);
  Future<void> onMobileApps() async {
    sendMouse('down', MouseButtons.wheel);
    await Future.delayed(const Duration(milliseconds: 500));
    sendMouse('up', MouseButtons.wheel);
  }

  // Simulate a key press event.
  // `usbHidUsage` is the USB HID usage code of the key.
  Future<void> tapHidKey(int usbHidUsage) async {
    newKeyboardMode(kKeyFlutterKey, usbHidUsage, true);
    await Future.delayed(Duration(milliseconds: 100));
    newKeyboardMode(kKeyFlutterKey, usbHidUsage, false);
  }

  Future<void> onMobileVolumeUp() async =>
      await tapHidKey(PhysicalKeyboardKey.audioVolumeUp.usbHidUsage & 0xFFFF);
  Future<void> onMobileVolumeDown() async =>
      await tapHidKey(PhysicalKeyboardKey.audioVolumeDown.usbHidUsage & 0xFFFF);
  Future<void> onMobilePower() async =>
      await tapHidKey(PhysicalKeyboardKey.power.usbHidUsage & 0xFFFF);
}
