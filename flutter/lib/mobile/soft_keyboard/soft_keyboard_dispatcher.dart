import 'dart:async';
import 'dart:collection';

import 'package:flutter/foundation.dart';

import 'soft_keyboard_actions.dart';

/// Sends the remote operations among soft keyboard actions one at a time, in
/// order.
///
/// Each send is an asynchronous call that the bridge runs on a pool of worker
/// threads, so two calls made back to back are not guaranteed to reach the
/// connection in the order they were made. Text that follows a Backspace has
/// to arrive after it. Waiting for one call to return before making the next
/// is what guarantees that.
class SoftKeyboardDispatcher {
  SoftKeyboardDispatcher({
    required this.sendBackspace,
    required this.sendText,
  });

  /// Presses Backspace once.
  final Future<void> Function() sendBackspace;

  /// Sends committed text.
  final Future<void> Function(String text) sendText;

  final Queue<SoftKeyboardAction> _pending = Queue();
  bool _draining = false;

  /// Queues [actions]. If nothing is in flight the first one is sent before
  /// this returns.
  ///
  /// Actions that are not remote operations are skipped. The owner of the
  /// field acts on those.
  void dispatch(List<SoftKeyboardAction> actions) {
    _pending.addAll(actions.where((action) => action.isRemote));
    if (_pending.isEmpty) return;
    if (!_draining) unawaited(_drain());
  }

  /// Drops everything that has not been sent yet.
  ///
  /// For when the session goes away. Input typed for one session must not be
  /// delivered to whatever comes next.
  void cancel() {
    _pending.clear();
  }

  Future<void> _drain() async {
    _draining = true;
    try {
      while (_pending.isNotEmpty) {
        final action = _pending.removeFirst();
        try {
          await _send(action);
        } catch (e) {
          // Keep going. One failed send must not hold back later input. The
          // action is not logged: it may hold what was typed.
          debugPrint('soft keyboard: a send failed (${e.runtimeType})');
        }
      }
    } finally {
      _draining = false;
    }
  }

  Future<void> _send(SoftKeyboardAction action) async {
    switch (action) {
      case SoftKeyboardBackspace(:final count):
        // Backspaces cannot overtake each other in any way that matters, so
        // they go out together. Only what follows has to wait for them.
        await Future.wait([for (var i = 0; i < count; i++) sendBackspace()]);
      case SoftKeyboardText(:final text):
        await sendText(text);
      case SoftKeyboardResetField():
      case SoftKeyboardSetField():
        break;
    }
  }
}
