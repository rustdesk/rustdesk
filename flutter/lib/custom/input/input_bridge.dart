import 'package:flutter_hbb/models/platform_model.dart';
import 'package:uuid/uuid.dart';

class InputBridge {
  final UuidValue sessionId;

  const InputBridge(this.sessionId);

  factory InputBridge.poc() =>
      InputBridge(UuidValue('00000000-0000-0000-0000-000000000000'));

  Future<void> tapKey(String name) async {
    await _key(name, down: true);
    await Future<void>.delayed(const Duration(milliseconds: 8));
    await _key(name, down: false);
  }

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
