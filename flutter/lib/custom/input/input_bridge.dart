import 'dart:convert';

import 'package:flutter_hbb/models/platform_model.dart';
import 'package:uuid/uuid.dart';

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

  Future<void> scroll(int dx, int dy) => bind.sessionSendMouse(
        sessionId: sessionId,
        msg: jsonEncode({'type': 'wheel', 'x': dx, 'y': dy}),
      );

  Future<void> _key(String name, {required bool down}) {
    return bind.sessionInputKey(
      sessionId: sessionId,
      name: name,
      down: down,
      press: false,
      alt: false,
      ctrl: false,
      shift: false,
      command: false,
    );
  }
}
