import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter_hbb/mobile/soft_keyboard/soft_keyboard.dart';
import 'package:flutter_test/flutter_test.dart';

/// Stands in for the bridge: every send stays open until the test completes
/// it, the way a call stays open until a worker thread has run it.
class _Bridge {
  final log = <String>[];
  final _open = <Completer<void>>[];

  int get inFlight => _open.length;

  Future<void> backspace() => _send('<BS>');

  Future<void> text(String text) => _send(text);

  Future<void> _send(String what) {
    log.add(what);
    final completer = Completer<void>();
    _open.add(completer);
    return completer.future;
  }

  /// Lets the oldest open send return.
  Future<void> finishOne({Object? error}) async {
    final completer = _open.removeAt(0);
    if (error != null) {
      completer.completeError(error);
    } else {
      completer.complete();
    }
    await pumpEventQueue();
  }

  Future<void> finishAll() async {
    while (_open.isNotEmpty) {
      await finishOne();
    }
  }
}

void main() {
  late _Bridge bridge;
  late SoftKeyboardDispatcher dispatcher;

  setUp(() {
    bridge = _Bridge();
    dispatcher = SoftKeyboardDispatcher(
      sendBackspace: bridge.backspace,
      sendText: bridge.text,
    );
  });

  test('sends the first action before returning', () {
    dispatcher.dispatch(const [SoftKeyboardText('a')]);
    expect(bridge.log, ['a']);
  });

  test('does not send text before the Backspaces in front of it have returned',
      () async {
    dispatcher
        .dispatch(const [SoftKeyboardBackspace(2), SoftKeyboardText('he ')]);
    // The Backspaces go out together.
    expect(bridge.log, ['<BS>', '<BS>']);
    expect(bridge.inFlight, 2);

    await bridge.finishOne();
    expect(bridge.log, ['<BS>', '<BS>']);
    await bridge.finishOne();
    expect(bridge.log, ['<BS>', '<BS>', 'he ']);
    await bridge.finishAll();
  });

  test('does not send an action before the text in front of it has returned',
      () async {
    dispatcher.dispatch(const [
      SoftKeyboardText('a'),
      SoftKeyboardBackspace(1),
      SoftKeyboardText('b'),
    ]);
    expect(bridge.log, ['a']);
    await bridge.finishOne();
    expect(bridge.log, ['a', '<BS>']);
    await bridge.finishOne();
    expect(bridge.log, ['a', '<BS>', 'b']);
    await bridge.finishAll();
  });

  test('keeps the order across dispatches', () async {
    dispatcher
        .dispatch(const [SoftKeyboardBackspace(1), SoftKeyboardText('마')]);
    dispatcher
        .dispatch(const [SoftKeyboardBackspace(1), SoftKeyboardText('만')]);
    expect(bridge.inFlight, 1);
    await bridge.finishAll();
    expect(bridge.log, ['<BS>', '마', '<BS>', '만']);
  });

  test('leaves what concerns the field to the owner of the field', () async {
    dispatcher
        .dispatch(const [SoftKeyboardText('()'), SoftKeyboardResetField()]);
    dispatcher.dispatch(const [SoftKeyboardResetField()]);
    dispatcher.dispatch(const [
      SoftKeyboardSetField(TextEditingValue(text: 'padding')),
      SoftKeyboardText('a'),
    ]);
    await bridge.finishAll();
    expect(bridge.log, ['()', 'a']);
  });

  test('carries on after a failed send, without logging what was typed',
      () async {
    final printed = <String>[];
    final previous = debugPrint;
    debugPrint = (String? message, {int? wrapWidth}) => printed.add('$message');
    addTearDown(() => debugPrint = previous);

    dispatcher
        .dispatch(const [SoftKeyboardText('hunter2'), SoftKeyboardText('b')]);
    await bridge.finishOne(error: StateError('session closed'));
    expect(bridge.log, ['hunter2', 'b']);
    await bridge.finishAll();

    expect(printed, hasLength(1));
    expect(printed.single.contains('hunter2'), isFalse);

    dispatcher.dispatch(const [SoftKeyboardText('c')]);
    expect(bridge.log, ['hunter2', 'b', 'c']);
  });

  test('cancel drops what has not been sent', () async {
    dispatcher
        .dispatch(const [SoftKeyboardBackspace(3), SoftKeyboardText('x')]);
    expect(bridge.log, ['<BS>', '<BS>', '<BS>']);
    dispatcher.cancel();
    await bridge.finishAll();
    expect(bridge.log, ['<BS>', '<BS>', '<BS>']);

    // And it still works afterwards.
    dispatcher.dispatch(const [SoftKeyboardText('y')]);
    expect(bridge.log, ['<BS>', '<BS>', '<BS>', 'y']);
  });

  test('ignores an empty list', () {
    dispatcher.dispatch(const []);
    expect(bridge.log, isEmpty);
  });
}
