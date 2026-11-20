import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/mobile/widgets/mobile_text_input.dart';
import 'package:flutter_test/flutter_test.dart';

import 'mobile_text_input_test.dart' show InputFixture;
import 'mobile_text_input_context_test.dart' show currentClient;

void main() {
  testWidgets('floating cursor moves remotely and retires its old text context',
      verifyFloatingCursor,
      variant: TargetPlatformVariant.only(TargetPlatform.iOS));
  testWidgets('native cursor movement edits the remote insertion point',
      verifyNativeCursor,
      variant: TargetPlatformVariant.only(TargetPlatform.android));
}

Future<void> verifyFloatingCursor(WidgetTester tester) async {
  final input = InputFixture();
  final editor = RemoteEditor();
  await mount(tester, input, editor);
  await input.type(tester, 'hello');
  await floating(tester, 'Start', Offset.zero);
  await floating(tester, 'Update', const Offset(-24, 0));
  await floating(tester, 'End', Offset.zero);
  expect(editor.cursor, 3);
  expect(editor.text, 'hello');
  await input.type(tester, 'X');
  expect(editor.text, 'helXlo');
  expect(input.errors, isEmpty);
  await tester.pumpWidget(const SizedBox.shrink());
}

Future<void> verifyNativeCursor(WidgetTester tester) async {
  final input = InputFixture();
  final editor = RemoteEditor();
  await mount(tester, input, editor);
  await input.type(tester, 'hello');
  await input.deltas(tester, [
    {
      'oldText': ' hello',
      'deltaStart': -1,
      'deltaEnd': -1,
      'deltaText': '',
      'selectionBase': 3,
      'selectionExtent': 3,
      'composingBase': -1,
      'composingExtent': -1,
    },
  ]);
  expect(editor.cursor, 2);
  expect(editor.text, 'hello');
  await input.deltas(tester, [
    {
      'oldText': ' hello',
      'deltaStart': 3,
      'deltaEnd': 3,
      'deltaText': 'X',
      'selectionBase': 4,
      'selectionExtent': 4,
      'composingBase': 3,
      'composingExtent': 4,
    },
  ]);
  expect(editor.text, 'heXllo');
  expect(editor.cursor, 3);
  expect(input.errors, isEmpty);
  await tester.pumpWidget(const SizedBox.shrink());
}

Future<void> floating(WidgetTester tester, String state, Offset offset) async {
  await tester.binding.defaultBinaryMessenger.handlePlatformMessage(
    SystemChannels.textInput.name,
    SystemChannels.textInput.codec.encodeMethodCall(MethodCall(
      'TextInputClient.updateFloatingCursor',
      [
        currentClient(tester),
        'FloatingCursorDragState.${state.toLowerCase()}',
        {'X': offset.dx, 'Y': offset.dy}
      ],
    )),
    (_) {},
  );
  await tester.pump();
}

class RemoteEditor {
  String text = '';
  int cursor = 0;

  Future<void> insert(String value, bool enter) async {
    text = text.replaceRange(cursor, cursor, value);
    cursor += value.length;
  }

  Future<void> arrow(String key) async {
    cursor += key == 'VK_LEFT' ? -1 : 1;
  }

  Future<void> backspace() async {
    text = text.replaceRange(cursor - 1, cursor, '');
    cursor--;
  }
}

Future<void> mount(
    WidgetTester tester, InputFixture input, RemoteEditor editor) async {
  addTearDown(input.controller.dispose);
  addTearDown(input.focus.dispose);
  await tester.pumpWidget(MaterialApp(
    home: MobileTextInput(
      controller: input.controller,
      focusNode: input.focus,
      onSubmit: editor.insert,
      onRemoteBackspace: editor.backspace,
      onRemoteArrow: editor.arrow,
      onError: (error, _) => input.errors.add(error),
    ),
  ));
  input.focus.requestFocus();
  await tester.pump();
}
