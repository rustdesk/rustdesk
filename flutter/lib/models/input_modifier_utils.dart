import 'package:flutter/services.dart';

bool shouldReleaseStaleMobileShift({
  required bool isMobile,
  required bool cachedShiftPressed,
  required bool actualShiftPressed,
  required LogicalKeyboardKey logicalKey,
  required String? character,
  required bool hasTrackedShiftKeyDown,
}) {
  if (!isMobile || !cachedShiftPressed || actualShiftPressed) {
    return false;
  }
  if (!hasTrackedShiftKeyDown) {
    return false;
  }
  if (logicalKey == LogicalKeyboardKey.shiftLeft ||
      logicalKey == LogicalKeyboardKey.shiftRight) {
    return false;
  }
  return true;
}
