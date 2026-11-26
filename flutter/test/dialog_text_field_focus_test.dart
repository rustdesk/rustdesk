import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/common/widgets/dialog.dart';

void main() {
  // Passes trivially on Flutter 3.24; the macOS arm64 CI job runs it against
  // the 3.44-patched sources, where desktop fields select all on focus (#17143).
  testWidgets('dialog text field keeps the caret when it regains focus',
      (tester) async {
    final controller = TextEditingController();
    final focusNode = FocusNode();
    addTearDown(controller.dispose);
    addTearDown(focusNode.dispose);

    await tester.pumpWidget(MaterialApp(
      home: Scaffold(
        body: DialogTextField(
          title: '2FA code',
          controller: controller,
          focusNode: focusNode,
        ),
      ),
    ));
    await tester.enterText(find.byType(TextField), '1');
    focusNode.unfocus();
    await tester.pump();
    focusNode.requestFocus();
    await tester.pump();

    expect(controller.selection, const TextSelection.collapsed(offset: 1));
  }, variant: TargetPlatformVariant.desktop());
}
