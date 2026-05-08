import 'dart:convert';

import 'package:flutter_hbb/models/platform_model.dart';
import 'package:uuid/uuid.dart';

// Friendly names used by the PowerStrip layout → names recognised by
// RustDesk's KEY_MAP (src/client.rs). Multi-char names that are not in
// KEY_MAP are silently dropped by ui_session_interface.rs::input_key, so
// every non-single-char key in our layout must be translated here.
const _kKeyAliases = <String, String>{
  'escape': 'VK_ESCAPE',
  'tab': 'VK_TAB',
  'backspace': 'VK_BACK',
  'return': 'VK_RETURN',
  'enter': 'VK_RETURN',
  'space': 'VK_SPACE',
  'delete': 'VK_DELETE',
  'left': 'VK_LEFT',
  'right': 'VK_RIGHT',
  'up': 'VK_UP',
  'down': 'VK_DOWN',
  'home': 'VK_HOME',
  'end': 'VK_END',
  'pageup': 'VK_PRIOR',
  'pagedown': 'VK_NEXT',
  'control': 'VK_CONTROL',
  'ctrl': 'VK_CONTROL',
  'alt': 'VK_MENU',
  'shift': 'VK_SHIFT',
  'meta': 'Meta',
  'cmd': 'Meta',
  'command': 'Meta',
  'win': 'Meta',
  '[': 'VK_LBRACKET',
  'f1': 'VK_F1',
  'f2': 'VK_F2',
  'f3': 'VK_F3',
  'f4': 'VK_F4',
  'f5': 'VK_F5',
  'f6': 'VK_F6',
  'f7': 'VK_F7',
  'f8': 'VK_F8',
  'f9': 'VK_F9',
  'f10': 'VK_F10',
  'f11': 'VK_F11',
  'f12': 'VK_F12',
};

String _resolveKeyName(String name) =>
    _kKeyAliases[name.toLowerCase()] ?? name;

class _ModifierFlags {
  final bool alt;
  final bool ctrl;
  final bool shift;
  final bool command;
  const _ModifierFlags({
    this.alt = false,
    this.ctrl = false,
    this.shift = false,
    this.command = false,
  });

  static const empty = _ModifierFlags();

  factory _ModifierFlags.from(Set<String> modifiers) {
    var alt = false, ctrl = false, shift = false, command = false;
    for (final m in modifiers) {
      switch (m.toLowerCase()) {
        case 'alt':
        case 'option':
        case 'menu':
          alt = true;
        case 'ctrl':
        case 'control':
          ctrl = true;
        case 'shift':
          shift = true;
        case 'meta':
        case 'cmd':
        case 'command':
        case 'win':
          command = true;
      }
    }
    return _ModifierFlags(alt: alt, ctrl: ctrl, shift: shift, command: command);
  }
}

class InputBridge {
  final UuidValue sessionId;

  const InputBridge(this.sessionId);

  factory InputBridge.poc() =>
      InputBridge(UuidValue('00000000-0000-0000-0000-000000000000'));

  Future<void> tapKey(String name, {Set<String> modifiers = const {}}) async {
    final flags = _ModifierFlags.from(modifiers);
    await _key(name, down: true, flags: flags);
    await Future<void>.delayed(const Duration(milliseconds: 8));
    await _key(name, down: false, flags: flags);
  }

  Future<void> typeString(String s) =>
      bind.sessionInputString(sessionId: sessionId, value: s);

  // RustDesk's flutter_ffi.rs deserialises mouse events as
  // HashMap<String, String>, so all values must be strings — int x/y are
  // silently dropped.
  Future<void> scroll(int dx, int dy) => bind.sessionSendMouse(
        sessionId: sessionId,
        msg: jsonEncode({
          'type': 'wheel',
          'x': dx.toString(),
          'y': dy.toString(),
        }),
      );

  Future<void> _key(
    String name, {
    required bool down,
    _ModifierFlags flags = _ModifierFlags.empty,
  }) {
    return bind.sessionInputKey(
      sessionId: sessionId,
      name: _resolveKeyName(name),
      down: down,
      press: false,
      alt: flags.alt,
      ctrl: flags.ctrl,
      shift: flags.shift,
      command: flags.command,
    );
  }
}
