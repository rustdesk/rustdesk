import 'dart:ui' show PointerDeviceKind;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/desktop/widgets/popup_menu.dart';
import 'package:get/get.dart';

import 'request_approval_menu_test.dart' as menu;

void main() {
  testWidgets('submenu preserves mixed entries and their close policies',
      (tester) async {
    final actions = <String>[];
    await menu.openMenu(tester, actions, extraEntries: [
      MenuEntrySubMenu<String>(text: 'Options', entries: [
        MenuEntryButton<String>(
          childBuilder: (_) => const Text('Inspect'),
          proc: () => actions.add('inspect'),
        ),
        MenuEntryDivider<String>(),
        MenuEntrySwitchSync<String>(
          switchType: SwitchType.scheckbox,
          text: 'Keep session',
          currentValue: false,
          setter: (value) async => actions.add('keep:$value'),
        ),
        MenuEntryButton<String>(
          childBuilder: (_) => const Text('Apply'),
          proc: () => actions.add('apply'),
          dismissOnClicked: true,
          dismissCallback: () => actions.add('dismissed'),
        ),
      ]),
    ]);
    await tester.tap(find.text('Options'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Inspect'));
    await tester.pumpAndSettle();
    expect(find.text('Keep session'), findsOneWidget);
    await tester.tap(find.byType(Checkbox));
    await tester.pumpAndSettle();
    expect(tester.widget<Checkbox>(find.byType(Checkbox)).value, isTrue);
    await tester.tap(find.text('Apply'));
    await tester.pumpAndSettle();
    expect(actions, ['inspect', 'keep:true', 'dismissed', 'apply']);
    expect(find.text('Apply'), findsNothing);
    expect(find.text('Options'), findsOneWidget);
    await tester.tap(find.text('Options'));
    await tester.pumpAndSettle();
    await tester.tapAt(const Offset(2, 2));
    await tester.pumpAndSettle();
    expect(find.text('Apply'), findsNothing);
    expect(find.text('Options'), findsOneWidget);
    await tester.tap(find.text('Rename'));
    await tester.pumpAndSettle();
  });

  testWidgets('disabled submenu cannot open and can close its parent on cancel',
      (tester) async {
    final enabled = false.obs;
    await menu.openMenu(tester, [], extraEntries: [
      MenuEntrySubMenu<String>(
        text: 'Options',
        enabled: enabled,
        dismissParentOnCanceled: true,
        entries: [
          MenuEntryButton<String>(
            childBuilder: (_) => const Text('Inspect'),
            proc: () {},
          ),
        ],
      ),
    ]);
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await mouse.addPointer(location: const Offset(2, 2));
    addTearDown(mouse.removePointer);
    await mouse.moveTo(tester.getCenter(find.text('Options')));
    await tester.pump(const Duration(milliseconds: 300));
    await tester.tap(find.text('Options'));
    await tester.pumpAndSettle();
    expect(find.text('Inspect'), findsNothing);
    enabled.value = true;
    await tester.pumpAndSettle();
    await tester.tap(find.text('Options'));
    await tester.pumpAndSettle();
    expect(find.text('Inspect'), findsOneWidget);
    await tester.tapAt(const Offset(2, 2));
    await tester.pumpAndSettle();
    expect(find.text('Options'), findsNothing);
    expect(find.text('Inspect'), findsNothing);
  });
}
