import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/mobile/utils/ios_soft_keyboard_input.dart';
import 'package:flutter_hbb/mobile/widgets/hidden_keyboard_input.dart';
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
}

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
