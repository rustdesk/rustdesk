// Runs the soft keyboard input behind a real text field, set up like the
// hidden one on the remote page, with editing values injected the way the
// engine delivers them. This checks how the field reports changes. It does
// not show what a keyboard sends.

import 'package:flutter/material.dart';
import 'package:flutter_hbb/mobile/soft_keyboard/soft_keyboard.dart';
import 'package:flutter_test/flutter_test.dart';

import 'soft_keyboard_test_support.dart';

typedef _Create = SoftKeyboardInterpreter Function(TextEditingValue initial);

void main() {
  late TextEditingController controller;
  late FocusNode focusNode;
  late List<String> onChangedCalls;

  /// Everything that was sent: the text, and `<BS>` once per Backspace.
  late List<String> out;
  late bool allowed;
  late int reopened;

  Future<SoftKeyboardInput> pumpField(WidgetTester tester, _Create create,
      {bool attachBeforeField = true}) async {
    controller = TextEditingController(text: sentinel);
    addTearDown(controller.dispose);
    focusNode = FocusNode();
    addTearDown(focusNode.dispose);
    onChangedCalls = [];
    out = [];
    allowed = true;
    reopened = 0;

    final input = SoftKeyboardInput(
      controller: controller,
      focusNode: focusNode,
      sentinel: sentinel,
      createInterpreter: create,
      dispatcher: SoftKeyboardDispatcher(
        sendBackspace: () async => out.add('<BS>'),
        sendText: (text) async => out.add(text),
      ),
      isAllowed: () => allowed,
      reopenKeyboard: () => reopened++,
      trace: SoftKeyboardTrace(level: SoftKeyboardTraceLevel.off),
    );
    // On the remote page the input is attached long before the field exists.
    if (attachBeforeField) input.attach();
    addTearDown(input.detach);

    await tester.pumpWidget(MaterialApp(
      home: Material(
        child: TextFormField(
          textInputAction: TextInputAction.newline,
          autocorrect: false,
          autofocus: true,
          focusNode: focusNode,
          maxLines: null,
          controller: controller,
          keyboardType: TextInputType.multiline,
          onChanged: onChangedCalls.add,
        ),
      ),
    ));
    if (!attachBeforeField) input.attach();
    await tester.showKeyboard(find.byType(TextFormField));
    return input;
  }

  /// Delivers [value] as the keyboard would, and lets the sends finish.
  Future<void> type(WidgetTester tester, TextEditingValue value) async {
    tester.testTextInput.updateEditingValue(value);
    await tester.pump();
  }

  for (final (name, create) in <(String, _Create)>[
    ('iOS', LegacyIosSoftKeyboard.new),
    ('Android', LegacyAndroidSoftKeyboard.new),
  ]) {
    testWidgets(
        'the old $name handler sees through the controller what onChanged gave it',
        (tester) async {
      await pumpField(tester, create);

      // The same handler, driven by onChanged as it used to be. Its own
      // check for an unchanged text never triggers there.
      final direct = create(ev(''));
      final viaOnChanged = <String>[];
      var seen = 0;
      void drain() {
        for (; seen < onChangedCalls.length; seen++) {
          for (final action in direct.update(controller.value)) {
            switch (action) {
              case SoftKeyboardBackspace(:final count):
                viaOnChanged.addAll(List.filled(count, '<BS>'));
              case SoftKeyboardText(:final text):
                viaOnChanged.add(text);
              case SoftKeyboardResetField():
              case SoftKeyboardSetField():
                break;
            }
          }
        }
      }

      for (final spec in [
        '[n]', '[ni]', '你', '你|', '你a', '你ab', '你a', '你a[b]', '你ab', //
        '', '(|)', '(x|)',
      ]) {
        await type(tester, ev(spec));
        drain();
      }
      expect(out, viaOnChanged);
      expect(out, isNotEmpty);
    });
  }

  testWidgets('the field starts over when it loses focus', (tester) async {
    await pumpField(tester, LegacyIosSoftKeyboard.new);

    await type(tester, ev('ab'));
    focusNode.unfocus();
    await tester.pump();
    expect(controller.value, TextEditingValue(text: sentinel));
    expect(out, ['ab']);

    // What was typed before is gone from the field, so Backspace reaches the
    // sentinel. It still is one Backspace.
    await tester.showKeyboard(find.byType(TextFormField));
    await type(tester, ev('', lead: 1023));
    expect(out, ['ab', '<BS>']);
  });

  for (final (name, create) in <(String, _Create)>[
    ('old iOS handler', LegacyIosSoftKeyboard.new),
    ('old Android handler', LegacyAndroidSoftKeyboard.new),
  ]) {
    testWidgets('refilling the field is not taken for input by the $name',
        (tester) async {
      final input = await pumpField(tester, create);

      await type(tester, ev('abc'));
      expect(out, ['abc']);

      // What the page does when the keyboard is opened again.
      input.refill();
      await tester.pump();
      expect(controller.text, sentinel);
      expect(out, ['abc']);

      await type(tester, ev('d'));
      expect(out, ['abc', 'd']);
    });
  }

  testWidgets('a field that was just refilled does not start over again',
      (tester) async {
    var created = 0;
    final input = await pumpField(tester, (initial) {
      created++;
      return LegacyAndroidSoftKeyboard(initial);
    });
    await type(tester, ev('a'));

    // Opening the keyboard refills the field and then takes it away, which
    // also takes its focus.
    input.refill();
    final afterRefill = created;
    focusNode.unfocus();
    await tester.pump();
    expect(created, afterRefill);
  });

  testWidgets(
      'an interpreter can correct the field without anything being sent',
      (tester) async {
    late _Correcting interpreter;
    await pumpField(tester, (initial) => interpreter = _Correcting());
    // Getting focus already moved the caret, which the interpreter saw.
    interpreter.seen.clear();

    // Backspace takes one character of padding. The interpreter sends the
    // Backspace and asks for the padding back.
    await type(tester, ev('', lead: 1023));
    expect(out, ['<BS>']);
    expect(controller.value, ev(''));
    // It was given the keyboard's value and then its own correction.
    expect(interpreter.seen, [ev('', lead: 1023), ev('')]);

    await type(tester, ev('a'));
    expect(out, ['<BS>']);
    expect(interpreter.seen.last, ev('a'));
  });

  testWidgets('a correction answered with another correction is written once',
      (tester) async {
    await pumpField(tester, (initial) => _NeverSatisfied());

    await type(tester, ev('a'));
    expect(controller.text, '${sentinel}a!');
    expect(out, isEmpty);
  });

  testWidgets('nothing is sent while input is not allowed', (tester) async {
    await pumpField(tester, LegacyAndroidSoftKeyboard.new);

    allowed = false;
    await type(tester, ev('a'));
    await type(tester, ev('ab'));
    expect(out, isEmpty);
  });

  testWidgets('the old Android handler has the keyboard reopened after a pair',
      (tester) async {
    await pumpField(tester, LegacyAndroidSoftKeyboard.new);

    await type(tester, ev('a'));
    expect(reopened, 0);
    await type(tester, ev('a(|)'));
    expect(out, ['a', '()']);
    expect(reopened, 1);
  });

  testWidgets('nothing is sent after detaching', (tester) async {
    final input = await pumpField(tester, LegacyAndroidSoftKeyboard.new);

    await type(tester, ev('a'));
    input.detach();
    await type(tester, ev('ab'));
    expect(out, ['a']);
  });

  // With the interpreters that know about composing text.

  testWidgets(
      'a commit that leaves the text as it is reaches the controller but not onChanged',
      (tester) async {
    await pumpField(tester, SoftKeyboardReducer.ios);

    await type(tester, ev('[あり]'));
    expect(onChangedCalls, hasLength(1));
    expect(out, isEmpty);

    await type(tester, ev('あり'));
    // The text did not change, so onChanged stays silent ...
    expect(onChangedCalls, hasLength(1));
    // ... but the controller reports the change, and the text is sent.
    expect(controller.value.composing, TextRange.empty);
    expect(out, ['あり']);
  });

  testWidgets('a composition is sent once, when it is committed',
      (tester) async {
    await pumpField(tester, SoftKeyboardReducer.ios);

    for (final spec in ['[n]', '[ni]', '[nihao]', '你[hao]', '你好', '你好\n']) {
      await type(tester, ev(spec));
    }
    expect(out, ['你', '好', '\n']);
  });

  testWidgets('composing text is dropped when the field loses focus',
      (tester) async {
    await pumpField(tester, SoftKeyboardReducer.ios);

    await type(tester, ev('ab'));
    await type(tester, ev('ab[nihao]'));
    expect(out, ['ab']);

    // The framework clears the composing range of a field that loses focus
    // and keeps the text. That is not the keyboard committing the letters.
    focusNode.unfocus();
    await tester.pump();
    expect(out, ['ab']);
    // The field starts over, so the letters cannot come back later either.
    expect(controller.value, TextEditingValue(text: sentinel));

    await tester.showKeyboard(find.byType(TextFormField));
    await type(tester, ev('[ni]'));
    await type(tester, ev('你'));
    expect(out, ['ab', '你']);
    // What was typed before the field lost focus is gone from it, so
    // Backspace now reaches the sentinel. It still is one Backspace.
    await type(tester, ev(''));
    await type(tester, ev('', lead: 1023));
    expect(out, ['ab', '你', '<BS>', '<BS>']);
  });

  testWidgets(
      'composing text is dropped when the field hears of the focus loss first',
      (tester) async {
    // Listeners are told in the order they were added. Here the field clears
    // its composing range before the input has refilled it.
    await pumpField(tester, SoftKeyboardReducer.ios, attachBeforeField: false);

    await type(tester, ev('[nihao]'));
    focusNode.unfocus();
    await tester.pump();
    expect(out, isEmpty);
    expect(controller.value, TextEditingValue(text: sentinel));
  });

  for (final (name, create) in <(String, _Create)>[
    ('iOS interpreter', SoftKeyboardReducer.ios),
    ('Android interpreter', SoftKeyboardReducer.android),
  ]) {
    testWidgets('refilling the field is not taken for input by the $name',
        (tester) async {
      final input = await pumpField(tester, create);

      await type(tester, ev('abc'));
      expect(out, ['abc']);

      // What the page does when the keyboard is opened again.
      input.refill();
      await tester.pump();
      expect(controller.text, sentinel);
      expect(out, ['abc']);

      await type(tester, ev('d'));
      expect(out, ['abc', 'd']);
    });
  }
}

/// Sends a Backspace for every character of padding that goes missing and
/// asks for the padding back.
class _Correcting implements SoftKeyboardInterpreter {
  final seen = <TextEditingValue>[];

  @override
  String get name => 'correcting';

  @override
  String? get lastNote => null;

  @override
  List<SoftKeyboardAction> update(TextEditingValue value) {
    seen.add(value);
    if (value.text.length >= sentinel.length) return const [];
    return [
      SoftKeyboardBackspace(sentinel.length - value.text.length),
      SoftKeyboardSetField(ev('')),
    ];
  }
}

/// Answers every value by asking for a longer one.
class _NeverSatisfied implements SoftKeyboardInterpreter {
  @override
  String get name => 'never satisfied';

  @override
  String? get lastNote => null;

  @override
  List<SoftKeyboardAction> update(TextEditingValue value) => [
        SoftKeyboardSetField(TextEditingValue(
          text: '${value.text}!',
          selection: TextSelection.collapsed(offset: value.text.length + 1),
        )),
      ];
}
