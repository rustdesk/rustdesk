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
import 'soft_keyboard_reducer.dart';

export 'legacy_soft_keyboard.dart';
export 'soft_keyboard_actions.dart';
export 'soft_keyboard_dispatcher.dart';
export 'soft_keyboard_input.dart';
export 'soft_keyboard_reducer.dart';
export 'soft_keyboard_trace.dart';

/// The interpreter for a platform: the old handler, or, when [useReducer] is
/// set, the one that knows about composing text.
///
/// The old handlers are the default on both platforms. The new ones have not
/// been run on a device.
SoftKeyboardInterpreter createSoftKeyboardInterpreter({
  required bool isIOS,
  required bool useReducer,
  required TextEditingValue initial,
}) {
  if (isIOS) {
    return useReducer
        ? SoftKeyboardReducer.ios(initial)
        : LegacyIosSoftKeyboard(initial);
  }
  return useReducer
      ? SoftKeyboardReducer.android(initial)
      : LegacyAndroidSoftKeyboard(initial);
}
