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
};

String _resolveKeyName(String name) =>
    _kKeyAliases[name.toLowerCase()] ?? name;

class InputBridge {
  final UuidValue sessionId;

  const InputBridge(this.sessionId);

  factory InputBridge.poc() =>
      InputBridge(UuidValue('00000000-0000-0000-0000-000000000000'));

  Future<void> tapKey(String name) async {
    await keyDown(name);
    await Future<void>.delayed(const Duration(milliseconds: 8));
    await keyUp(name);
  }

  Future<void> keyDown(String name) => _key(name, down: true);
  Future<void> keyUp(String name) => _key(name, down: false);

  Future<void> typeString(String s) =>
      bind.sessionInputString(sessionId: sessionId, value: s);

  Future<void> tapKeyWithModifiers(String key, Set<String> modifiers) async {
    for (final m in modifiers) {
      await keyDown(m);
      await Future<void>.delayed(const Duration(milliseconds: 12));
    }
    await tapKey(key);
    for (final m in modifiers) {
      await keyUp(m);
      await Future<void>.delayed(const Duration(milliseconds: 8));
    }
  }

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

  Future<void> _key(String name, {required bool down}) {
    return bind.sessionInputKey(
      sessionId: sessionId,
      name: _resolveKeyName(name),
      down: down,
      press: false,
      alt: false,
      ctrl: false,
      shift: false,
      command: false,
    );
  }
}
