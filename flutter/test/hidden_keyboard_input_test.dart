import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/mobile/utils/ios_keyboard_edit_queue.dart';
import 'package:flutter_hbb/mobile/utils/ios_soft_keyboard_input.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('hidden iOS input preserves composition across a Space drag',
      (tester) async {
    const paddingLength = 1024;
    final preedit = '${' ' * paddingLength}ni\u2006hao';
    final composing = TextRange(start: paddingLength, end: preedit.length);
    final controller = IosSoftKeyboardController(text: preedit);
    final focus = FocusNode();
    addTearDown(controller.dispose);
    addTearDown(focus.dispose);
    await tester.pumpWidget(MaterialApp(
      home: Scaffold(
        body: Stack(children: [
          HiddenKeyboardInput(
            isIOS: true,
            child: TextFormField(
              controller: controller,
              focusNode: focus,
              maxLines: null,
              keyboardType: TextInputType.multiline,
            ),
          ),
        ]),
      ),
    ));
    focus.requestFocus();
    final markedValue = TextEditingValue(
      text: preedit,
      selection: TextSelection.collapsed(offset: preedit.length),
      composing: composing,
    );
    controller.value = markedValue;
    await tester.pump();
    await dragUp(tester, controller);
    expect(controller.value, markedValue);
    tester.testTextInput.updateEditingValue(
        controller.value.copyWith(composing: TextRange.empty));
    await tester.pump();
    expect(controller.text, preedit);
    expect(controller.value.composing, TextRange.empty);
    tester.testTextInput.enterText('abcd');
    await tester.pump();
    expect(controller.text, 'abcd');
    expect(focus.hasFocus, isTrue);
    expect(find.byType(EditableText), findsNothing);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('blur cancels preedit without replaying it on refocus',
      verifyBlurAndRefocus);
}

Future<void> verifyBlurAndRefocus(WidgetTester tester) async {
  final padding = ' ' * 1024;
  final controller = IosSoftKeyboardController(text: padding);
  final input = IosSoftKeyboardInput(padding);
  final focus = FocusNode();
  final otherFocus = FocusNode();
  final sent = <String>[];
  final queue = IosKeyboardEditQueue();
  Future<void> send(String text) async => sent.add(text);
  controller.addListener(() {
    final edit = input.update(controller.value, hasFocus: focus.hasFocus);
    unawaited(queue.send(edit, sendKey: send, sendText: send));
    if (edit.localValue != null) controller.value = edit.localValue!;
  });
  addTearDown(controller.dispose);
  addTearDown(focus.dispose);
  addTearDown(otherFocus.dispose);
  await tester.pumpWidget(focusTestFields(controller, focus, otherFocus));
  focus.requestFocus();
  await tester.pump();
  tester.testTextInput.enterText('${padding}abc');
  await tester.pump();
  final marked = '${padding}abcni\u2006hao';
  tester.testTextInput.updateEditingValue(TextEditingValue(
    text: marked,
    selection: TextSelection.collapsed(offset: marked.length),
    composing: TextRange(start: padding.length + 3, end: marked.length),
  ));
  await tester.pump();
  expect(sent, ['abc']);
  otherFocus.requestFocus();
  await tester.pump();
  expect(sent, ['abc'], reason: 'Framework clearComposing is not a commit');
  focus.requestFocus();
  await tester.pump();
  tester.testTextInput.enterText('${controller.text}x');
  await tester.pump();
  expect(sent, ['abc', 'x'], reason: 'Cancelled preedit must not reappear');
  await tester.pumpWidget(const SizedBox.shrink());
}

Widget focusTestFields(TextEditingController controller, FocusNode focus,
        FocusNode otherFocus) =>
    MaterialApp(
      theme: ThemeData(platform: TargetPlatform.iOS),
      home: Scaffold(
        body: Stack(children: [
          HiddenKeyboardInput(
            isIOS: true,
            child: TextFormField(
              controller: controller,
              focusNode: focus,
              maxLines: null,
            ),
          ),
          TextField(focusNode: otherFocus),
        ]),
      ),
    );

Future<void> dragUp(
    WidgetTester tester, TextEditingController controller) async {
  final editable = tester
      .state<EditableTextState>(find.byType(EditableText, skipOffstage: false));
  const dragOffset = Offset(0, -40);
  for (final state in FloatingCursorDragState.values) {
    editable.updateFloatingCursor(RawFloatingCursorPoint(
      state: state,
      offset:
          state == FloatingCursorDragState.Update ? dragOffset : Offset.zero,
    ));
  }
  await tester.pumpAndSettle();
  // iOS can follow the ignored drag with a selection at the marked range start.
  tester.testTextInput.updateEditingValue(controller.value.copyWith(
    selection:
        TextSelection.collapsed(offset: controller.value.composing.start),
  ));
}
