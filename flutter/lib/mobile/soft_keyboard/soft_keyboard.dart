/// Interpretation of mobile soft keyboard input.
///
/// The remote page owns a hidden text field. Whatever the keyboard does to it
/// arrives as editing values, an interpreter turns those into actions, and the
/// dispatcher sends the actions in order. Nothing in here touches FFI or
/// widgets, so all of it runs in unit tests.
library;

import 'package:flutter/services.dart';

import 'legacy_soft_keyboard.dart';
import 'soft_keyboard_actions.dart';

export 'legacy_soft_keyboard.dart';
export 'soft_keyboard_actions.dart';
export 'soft_keyboard_dispatcher.dart';
export 'soft_keyboard_input.dart';

SoftKeyboardInterpreter createSoftKeyboardInterpreter({
  required bool isIOS,
  required TextEditingValue initial,
}) {
  return isIOS
      ? LegacyIosSoftKeyboard(initial)
      : LegacyAndroidSoftKeyboard(initial);
}
