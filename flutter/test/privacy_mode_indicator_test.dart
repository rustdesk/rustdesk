import 'package:flutter/material.dart';
import 'package:flutter_hbb/common/widgets/privacy_mode_indicator.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:get/get.dart';

void main() {
  testWidgets('privacy mode indicator follows state without blocking input',
      (tester) async {
    final state = ''.obs;
    addTearDown(state.close);

    await tester.pumpWidget(
      MaterialApp(
        home: PrivacyModeIndicator(
          state: state,
          color: Colors.blue,
          semanticsLabel: 'Privacy mode',
        ),
      ),
    );

    expect(find.byIcon(Icons.privacy_tip), findsNothing);

    state.value = 'privacy_mode_impl_mag';
    await tester.pump();

    expect(find.byIcon(Icons.privacy_tip), findsOneWidget);
    final ignorePointer = tester.widget<IgnorePointer>(
      find.descendant(
        of: find.byType(PrivacyModeIndicator),
        matching: find.byType(IgnorePointer),
      ),
    );
    expect(ignorePointer.ignoring, isTrue);

    state.value = '';
    await tester.pump();

    expect(find.byIcon(Icons.privacy_tip), findsNothing);
  });
}
