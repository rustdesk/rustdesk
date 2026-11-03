import 'package:flutter/gestures.dart' show PointerDeviceKind;
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/common.dart' show MyTheme;
import 'package:flutter_hbb/desktop/widgets/popup_menu.dart';

import 'request_approval_menu_test.dart' as menu;

void main() {
  testWidgets('menu hover paints one full row and keeps its action',
      (tester) async {
    for (final theme in [MyTheme.lightTheme, MyTheme.darkTheme]) {
      final actions = <String>[];
      await menu.openMenu(tester, actions, theme: theme);
      final text = find.text('Connect');
      final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
      await mouse.addPointer(location: Offset.zero);
      await mouse.moveTo(tester.getCenter(text));
      await tester.pumpAndSettle();
      final innerButton =
          find.ancestor(of: text, matching: find.byType(TextButton)).first;
      final rowInk = Material.of(tester.element(innerButton)) as RenderObject;
      expect(rowInk,
          paints..rect(color: theme.colorScheme.onSurface.withOpacity(0.08)));
      final innerInk = Material.of(tester.element(text)) as RenderObject;
      expect(innerInk, paints..rect(color: Colors.transparent));
      await tester.tap(text);
      await tester.pumpAndSettle();
      expect(actions, ['Connect']);
      expect(find.text('Request approval'), findsNothing);
      await mouse.removePointer();
    }
  });

  testWidgets('checkbox and row toggle once without closing the menu',
      (tester) async {
    for (final checkbox in [false, true]) {
      final actions = <String>[];
      await menu.openMenu(tester, actions, extraEntries: [
        MenuEntrySwitchSync<String>(
          switchType: SwitchType.scheckbox,
          text: 'Keep session',
          currentValue: false,
          setter: (value) async => actions.add('keep:$value'),
        ),
      ]);
      await tester
          .tap(checkbox ? find.byType(Checkbox) : find.text('Keep session'));
      await tester.pumpAndSettle();
      expect(actions, ['keep:true']);
      expect(find.text('Keep session'), findsOneWidget);
      expect(tester.widget<Checkbox>(find.byType(Checkbox)).value, isTrue);
      await tester.tap(find.text('Rename'));
      await tester.pumpAndSettle();
    }
  });
}
