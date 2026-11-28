import 'dart:async';

import 'package:flutter/foundation.dart' show defaultTargetPlatform;
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/mobile/widgets/mobile_text_input.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('typing is delivered without a local display or Send action',
      verifyDirectTyping);
  testWidgets('Android composing English is delivered before the word ends',
      verifyComposingEnglish,
      variant: TargetPlatformVariant.only(TargetPlatform.android));
  testWidgets('IME candidates commit automatically without a remote Enter',
      verifyComposition,
      variant:
          TargetPlatformVariant({TargetPlatform.android, TargetPlatform.iOS}));
  testWidgets('equal-length and Unicode replacements are delivered intact',
      verifyReplacement);
  testWidgets('Android Backspace deletes remotely once, including empty input',
      verifyBackspace,
      variant: TargetPlatformVariant.only(TargetPlatform.android));
  testWidgets(
      'Return respects native composition and action semantics', verifyReturn,
      variant:
          TargetPlatformVariant({TargetPlatform.android, TargetPlatform.iOS}));
  testWidgets('pending input keeps the keyboard active and accepts more typing',
      verifyPendingInput);
  testWidgets(
      'failed input reports the error and resets remote editing context',
      verifyFailure);
  testWidgets('a held modifier is released after focus changes or composition',
      verifyModifierRelease);
}

Future<void> verifyComposingEnglish(WidgetTester tester) async {
  final input = InputFixture();
  await input.mount(tester);
  await input.type(tester, 'h', composing: true);
  expect(input.sent, [('h', false)]);
  await input.type(tester, 'he', composing: true);
  expect(input.sent, [('h', false), ('e', false)]);
  await input.type(tester, 'he ');
  expect(input.sent, [('h', false), ('e', false), (' ', false)]);
  await tester.pumpWidget(const SizedBox.shrink());
}

class InputFixture {
  final controller = MobileTextInputController();
  final focus = FocusNode();
  final sent = <(String, bool)>[];
  final errors = <Object>[];
  var backspaces = 0;
  var remoteText = '';

  Future<void> mount(WidgetTester tester,
      {Future<void> Function(String, bool)? submit,
      KeyEventResult Function(KeyEvent)? onRemoteKey}) async {
    addTearDown(controller.dispose);
    addTearDown(focus.dispose);
    await tester.pumpWidget(MaterialApp(
      home: Scaffold(
        body: MobileTextInput(
          controller: controller,
          focusNode: focus,
          onSubmit: submit ??
              (text, enter) async {
                sent.add((text, enter));
                remoteText += text;
              },
          onRemoteBackspace: () async {
            backspaces++;
            remoteText = remoteText.characters.skipLast(1).toString();
          },
          onError: (error, _) => errors.add(error),
          onRemoteKey: onRemoteKey,
        ),
      ),
    ));
    focus.requestFocus();
    await tester.pump();
  }

  Future<void> type(WidgetTester tester, String text,
      {bool composing = false}) async {
    await tester.pump();
    final old = tester.testTextInput.editingState!['text'] as String;
    await deltas(tester, [
      {
        'oldText': old,
        'deltaStart': 1,
        'deltaEnd': old.length,
        'deltaText': text,
        'selectionBase': text.length + 1,
        'selectionExtent': text.length + 1,
        'composingBase': composing ? 1 : -1,
        'composingExtent': composing ? text.length + 1 : -1,
      }
    ]);
  }

  Future<void> deltas(
      WidgetTester tester, List<Map<String, dynamic>> edits) async {
    var value = TextEditingValue.fromJSON(tester.testTextInput.editingState!);
    for (final edit in edits) {
      value = TextEditingDelta.fromJSON(edit).apply(value);
    }
    tester.testTextInput.editingState = value.toJSON();
    await tester.binding.defaultBinaryMessenger.handlePlatformMessage(
      SystemChannels.textInput.name,
      SystemChannels.textInput.codec.encodeMethodCall(MethodCall(
        'TextInputClient.updateEditingStateWithDeltas',
        [
          -1,
          {'deltas': edits}
        ],
      )),
      (_) {},
    );
    await tester.pump();
  }
}

Future<void> verifyModifierRelease(WidgetTester tester) async {
  for (final focusAfterDown in [false, true]) {
    final input = InputFixture();
    final events = <Type>[];
    if (focusAfterDown) {
      await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
    }
    await input.mount(tester, onRemoteKey: (event) {
      if (event.logicalKey != LogicalKeyboardKey.shiftLeft) {
        return KeyEventResult.ignored;
      }
      events.add(event.runtimeType);
      return KeyEventResult.handled;
    });
    if (!focusAfterDown) {
      await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
    }
    await input.type(tester, 'あ', composing: true);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
    await tester.pump();
    expect(events, [if (!focusAfterDown) KeyDownEvent, KeyUpEvent]);
    await tester.pumpWidget(const SizedBox.shrink());
  }
}

Future<void> verifyDirectTyping(WidgetTester tester) async {
  final input = InputFixture();
  await input.mount(tester);
  await input.type(tester, 'a');
  expect(input.sent, [('a', false)]);
  await input.type(tester, 'ab');
  expect(input.sent, [('a', false), ('b', false)]);
  expect(find.byType(TextField), findsNothing);
  expect(find.byType(EditableText), findsNothing);
  expect(find.text('Send'), findsNothing);
  expect(tester.getSize(find.byType(MobileTextInput)), Size.zero);
  await tester.pumpWidget(const SizedBox.shrink());
}

Future<void> verifyComposition(WidgetTester tester) async {
  final input = InputFixture();
  final forward = defaultTargetPlatform == TargetPlatform.android;
  await input.mount(tester);
  const sequences = [
    ['に', 'にほ', '日本'],
    ['ㅎ', '하', '한', '한그', '한글'],
    ['ㄋ', 'ㄋㄧ', '你', '你好'],
    ['n', 'ni', '你好'],
    ['a', 'â', 'ấ'],
  ];
  for (final sequence in sequences) {
    input.controller.resetContext();
    input.sent.clear();
    input.remoteText = '';
    for (final text in sequence) {
      await input.type(tester, text, composing: true);
      expect(input.remoteText, forward ? text : '');
    }
    await input.type(tester, sequence.last);
    await input.type(tester, sequence.last);
    expect(input.remoteText, sequence.last);
    expect(input.sent.every((edit) => !edit.$2), isTrue);
    if (!forward) expect(input.sent, [(sequence.last, false)]);
  }
  await tester.pumpWidget(const SizedBox.shrink());
}

Future<void> verifyReplacement(WidgetTester tester) async {
  final input = InputFixture();
  await input.mount(tester);
  await input.type(tester, 'helo');
  await input.type(tester, 'help');
  expect(input.sent, [('helo', false), ('p', false)]);
  expect(input.backspaces, 1);
  input.controller.resetContext();
  input.sent.clear();
  await input.type(tester, 'a');
  await input.type(tester, 'â');
  expect(input.sent, [('a', false), ('â', false)]);
  expect(input.backspaces, 2);
  input.controller.resetContext();
  input.sent.clear();
  await input.type(tester, '👨‍👩‍👧‍👦');
  expect(input.sent, [('👨‍👩‍👧‍👦', false)]);
  await tester.pumpWidget(const SizedBox.shrink());
}

Future<void> verifyBackspace(WidgetTester tester) async {
  final input = InputFixture();
  await input.mount(tester);
  await tester.sendKeyEvent(LogicalKeyboardKey.backspace);
  await tester.pump();
  expect(input.backspaces, 1);
  await input.type(tester, 'a');
  await tester.sendKeyEvent(LogicalKeyboardKey.backspace);
  await tester.sendKeyEvent(LogicalKeyboardKey.backspace);
  await tester.pump();
  expect(input.backspaces, 3);
  await input.type(tester, '한글', composing: true);
  await tester.sendKeyEvent(LogicalKeyboardKey.backspace);
  await tester.pump();
  expect(input.backspaces, 4);
  await input.type(tester, '한');
  expect(input.remoteText, '한');
  await tester.pumpWidget(const SizedBox.shrink());
}

Future<void> verifyReturn(WidgetTester tester) async {
  final input = InputFixture();
  final android = defaultTargetPlatform == TargetPlatform.android;
  await input.mount(tester);
  await input.type(tester, '日本', composing: true);
  await tester.testTextInput.receiveAction(TextInputAction.done);
  await tester.pump();
  expect(input.sent, [('日本', false), if (android) ('', true)]);
  await tester.testTextInput.receiveAction(TextInputAction.done);
  await tester.pump();
  expect(input.sent, [
    ('日本', false),
    if (android) ('', true),
    ('', true),
  ]);
  expect(tester.testTextInput.hasAnyClients, isTrue);
  await input.type(tester, 'next');
  expect(input.sent.last, ('next', false));
  await tester.pumpWidget(const SizedBox.shrink());
}

Future<void> verifyPendingInput(WidgetTester tester) async {
  final input = InputFixture();
  final accepted = Completer<void>();
  await input.mount(tester, submit: (text, enter) async {
    input.sent.add((text, enter));
    await accepted.future;
  });
  await input.type(tester, 'a');
  await input.type(tester, 'ab');
  expect(input.sent, [('a', false), ('b', false)]);
  expect(tester.testTextInput.hasAnyClients, isTrue);
  accepted.complete();
  await tester.pump();
  await tester.pumpWidget(const SizedBox.shrink());
}

Future<void> verifyFailure(WidgetTester tester) async {
  final input = InputFixture();
  await input.mount(tester,
      submit: (_, __) => Future.error(StateError('Disconnected')));
  await input.type(tester, 'lost');
  expect(input.errors.single, isA<StateError>());
  expect(tester.testTextInput.editingState?['text'], isNot(contains('lost')));
  expect(tester.testTextInput.hasAnyClients, isTrue);
  await tester.pumpWidget(const SizedBox.shrink());
}
