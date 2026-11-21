import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/mobile/widgets/mobile_text_input.dart';

import 'mobile_text_input_test.dart' show InputFixture;

void main() {
  testWidgets('touch shortcuts leave composition and restore normal input',
      verifyCommandMode,
      variant:
          TargetPlatformVariant({TargetPlatform.android, TargetPlatform.iOS}));
}

Future<void> verifyCommandMode(WidgetTester tester) async {
  final input = InputFixture();
  final commandMode = ValueNotifier(false);
  addTearDown(input.controller.dispose);
  addTearDown(input.focus.dispose);
  addTearDown(commandMode.dispose);
  await tester.pumpWidget(MaterialApp(
    home: ValueListenableBuilder<bool>(
      valueListenable: commandMode,
      builder: (_, active, __) => MobileTextInput(
        controller: input.controller,
        focusNode: input.focus,
        commandMode: active,
        onSubmit: (text, enter) async => input.sent.add((text, enter)),
        onRemoteBackspace: () async {},
        onError: (error, _) => input.errors.add(error),
      ),
    ),
  ));
  input.focus.requestFocus();
  await tester.pump();
  await input.type(tester, 'ni', composing: true);
  input.sent.clear();
  commandMode.value = true;
  await tester.pump();
  expect(tester.testTextInput.setClientArgs?['inputType']['name'],
      TextInputType.visiblePassword.toJson()['name']);
  expect(tester.testTextInput.setClientArgs?['enableSuggestions'], isFalse);
  expect(tester.testTextInput.editingState?['text'], ' ');
  await input.type(tester, 'a');
  expect(input.sent, [('a', false)]);
  input.controller.resetContext();
  await tester.pump();
  expect(tester.testTextInput.setClientArgs?['inputType']['name'],
      TextInputType.visiblePassword.toJson()['name']);
  commandMode.value = false;
  await tester.pump();
  expect(tester.testTextInput.setClientArgs?['inputType']['name'],
      TextInputType.text.toJson()['name']);
  expect(tester.testTextInput.setClientArgs?['enableSuggestions'], isTrue);
  await input.type(tester, 'ni', composing: true);
  if (defaultTargetPlatform == TargetPlatform.iOS) {
    expect(input.sent, [('a', false)]);
  }
  await input.type(tester, '你好');
  expect(input.sent.last, ('你好', false));
  expect(input.errors, isEmpty);
  await tester.pumpWidget(const SizedBox.shrink());
}
