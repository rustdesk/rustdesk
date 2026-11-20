import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';

import 'mobile_text_input_context_test.dart' show currentClient, lateEdit;
import 'mobile_text_input_test.dart' show InputFixture;

void main() {
  final platforms =
      TargetPlatformVariant({TargetPlatform.android, TargetPlatform.iOS});
  for (final (name, initial, remaining) in [
    ('family emoji', 'he👨‍👩‍👧‍👦', 'he'),
    ('combining accent', 'hee\u0301', 'he'),
    ('partial emoji', 'he👨‍👩‍👧‍👦', 'he👨‍👩‍👧'),
  ]) {
    testWidgets(
        'deleting $name retires old text without retyping its remainder',
        (tester) async {
      final input = InputFixture();
      await input.mount(tester);
      await input.type(tester, initial);
      final client = currentClient(tester);
      await input.type(tester, remaining);
      expect(input.sent, [(initial, false)]);
      await lateEdit(tester, client, insertion: true);
      expect(input.errors, isEmpty);
      await input.type(tester, 'x');
      expect(input.sent, [(initial, false), ('x', false)]);
      expect(input.backspaces, 1);
      expect(tester.testTextInput.hasAnyClients, isTrue);
      await tester.pumpWidget(const SizedBox.shrink());
    }, variant: platforms);
  }
}
