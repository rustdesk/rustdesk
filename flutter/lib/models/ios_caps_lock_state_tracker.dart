import 'package:flutter/services.dart';

class IosCapsLockStateTracker {
  bool _capsLock = false;

  bool get value => _capsLock;

  bool update({
    required String? character,
    required bool shiftPressed,
    required LogicalKeyboardKey logicalKey,
    required bool isKeyDown,
  }) {
    if (isKeyDown && logicalKey == LogicalKeyboardKey.capsLock) {
      _capsLock = !_capsLock;
      return _capsLock;
    }
    final inferred = inferFromCharacter(character, shiftPressed);
    if (inferred != null) {
      _capsLock = inferred;
    }
    return _capsLock;
  }

  static bool? inferFromCharacter(String? character, bool shiftPressed) {
    if (shiftPressed) return null;
    if (character == null || character.length != 1) return null;
    final upper = character.toUpperCase();
    final lower = character.toLowerCase();
    final isUpper = upper == character && lower != character;
    final isLower = lower == character && upper != character;
    if (!isUpper && !isLower) return null;
    return isUpper;
  }
}
