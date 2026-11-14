import 'package:flutter/widgets.dart';

import 'soft_keyboard_actions.dart';
import 'soft_keyboard_dispatcher.dart';

/// Connects the hidden text field to an interpreter and a dispatcher.
///
/// This is the one place that decides whether a change of the field is input
/// at all. It is not input when the app refills the field, and it is not
/// input when the field has no focus.
class SoftKeyboardInput {
  SoftKeyboardInput({
    required this.controller,
    required this.focusNode,
    required this.sentinel,
    required this.createInterpreter,
    required this.dispatcher,
    required this.isAllowed,
    required this.reopenKeyboard,
  });

  final TextEditingController controller;
  final FocusNode focusNode;

  /// What the field holds when nothing has been typed.
  final String sentinel;

  /// Creates the interpreter to use from now on. Called on every [refill].
  final SoftKeyboardInterpreter Function(TextEditingValue initial)
      createInterpreter;

  final SoftKeyboardDispatcher dispatcher;

  /// Whether soft keyboard input may be sent at the moment.
  final bool Function() isAllowed;

  /// Closes and opens the keyboard again. Only the legacy Android handler
  /// asks for it.
  final VoidCallback reopenKeyboard;

  late SoftKeyboardInterpreter _interpreter;

  /// Whether the field still is as the last [refill] left it.
  bool _untouched = false;

  /// Set while a correction asked for by the interpreter is being written.
  bool _correcting = false;

  void attach() {
    refill();
    controller.addListener(_handleValue);
    focusNode.addListener(_handleFocus);
  }

  /// Stops listening and drops what has not been sent yet. Input typed for
  /// one session must not be delivered to whatever comes next.
  void detach() {
    controller.removeListener(_handleValue);
    focusNode.removeListener(_handleFocus);
    dispatcher.cancel();
  }

  /// Fills the field with the sentinel again and starts interpreting from
  /// scratch. Text that was still being composed is dropped.
  void refill() {
    // The interpreter starts from the refilled field, so that the refill is
    // not taken for input.
    final initial = TextEditingValue(text: sentinel);
    _interpreter = createInterpreter(initial);
    controller.value = initial;
    _untouched = true;
  }

  // This listens to the controller instead of `onChanged`, because `onChanged`
  // is only called when the text changes. An input method can finish a
  // composition without changing the text, and that has to be seen too.
  void _handleValue() {
    // Without focus the field is not connected to a keyboard. What changes it
    // then is the framework, which clears the composing range of a field
    // that loses focus. That is not the keyboard committing the text.
    if (!focusNode.hasFocus) return;
    _untouched = false;
    if (!isAllowed()) return;
    final actions = _interpreter.update(controller.value);
    dispatcher.dispatch(actions);
    for (final action in actions) {
      switch (action) {
        case SoftKeyboardSetField(:final value):
          _correct(value);
        case SoftKeyboardResetField():
          reopenKeyboard();
        case SoftKeyboardBackspace():
        case SoftKeyboardText():
          break;
      }
    }
  }

  // Writing the field comes straight back here, so that the interpreter sees
  // the value it asked for. A correction asked for in answer to a correction
  // is not written. That would never end.
  void _correct(TextEditingValue value) {
    if (_correcting) return;
    _correcting = true;
    try {
      controller.value = value;
    } finally {
      _correcting = false;
    }
  }

  // Leaving the field ends the input. However the keyboard is closed, a
  // composition that was not committed is cancelled, and what was typed
  // before can no longer be edited from here.
  void _handleFocus() {
    if (!focusNode.hasFocus && !_untouched) refill();
  }
}
