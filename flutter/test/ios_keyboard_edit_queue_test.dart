import 'dart:async';

import 'package:flutter_hbb/mobile/utils/ios_keyboard_edit_queue.dart';
import 'package:flutter_hbb/mobile/utils/ios_soft_keyboard_input.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('Backspaces, replacement, toolbar keys and later typing stay ordered',
      verifyOrderedReplacement);
  test('failed deletion is reported and prevents dependent text edits',
      verifyFailedDeletion);
}

Future<void> verifyOrderedReplacement() async {
  final queue = IosKeyboardEditQueue();
  final calls = <String>[];
  final completed = <Completer<void>>[];
  Future<void> record(String call) {
    calls.add(call);
    final done = Completer<void>();
    completed.add(done);
    return done.future;
  }

  Future<void> send(IosKeyboardEdit edit) => queue.send(edit,
      sendKey: (key) => record('key:$key'),
      sendText: (text) => record('text:$text'));
  final first = send(const IosKeyboardEdit(backspaces: 2, text: 'hello'));
  final enter = queue.enqueue(() => record('key:VK_ENTER'));
  final left = queue.enqueue(() => record('key:VK_LEFT'));
  final second = send(const IosKeyboardEdit(backspaces: 1, text: 'é'));
  const expected = [
    'key:VK_BACK',
    'key:VK_BACK',
    'text:hello',
    'key:VK_ENTER',
    'key:VK_LEFT',
    'key:VK_BACK',
    'key:é',
  ];
  await Future<void>.delayed(Duration.zero);
  expect(calls, ['key:VK_BACK', 'key:VK_BACK']);
  // Finish the second Backspace first; replacement must still wait for both.
  completed[1].complete();
  await Future<void>.delayed(Duration.zero);
  expect(calls, ['key:VK_BACK', 'key:VK_BACK']);
  completed[0].complete();
  for (var i = 2; i < expected.length; i++) {
    await Future<void>.delayed(Duration.zero);
    expect(calls, expected.take(i + 1).toList());
    completed[i].complete();
  }
  await Future.wait([first, enter, left, second]);
  expect(calls, expected);
}

Future<void> verifyFailedDeletion() async {
  final queue = IosKeyboardEditQueue();
  final sent = <String>[];
  Future<void> send(IosKeyboardEdit edit) => queue.send(edit,
      sendKey: (_) => Future.error(StateError('send failed')),
      sendText: (text) async => sent.add(text));
  final failed = send(const IosKeyboardEdit(backspaces: 1, text: 'abc'));
  final following = send(const IosKeyboardEdit(text: 'def'));
  await Future.wait([
    expectLater(failed, throwsStateError),
    expectLater(following, throwsStateError),
  ]);
  expect(sent, isEmpty);
}
