import 'package:flutter/gestures.dart' show PointerDeviceKind;
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/common.dart' show MyTheme;
import 'package:flutter_hbb/desktop/widgets/popup_menu.dart';

import 'request_approval_menu_test.dart' as menu;

void main() {
  testWidgets('submenu highlight stays steady while opening', (tester) async {
    for (final theme in [MyTheme.lightTheme, MyTheme.darkTheme]) {
      await menu.openMenu(tester, [], theme: theme);
      final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
      await mouse.addPointer(location: Offset.zero);
      await mouse.moveTo(tester.getCenter(menu.approvalRow));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 199));
      for (final milliseconds in [1, 16, 50]) {
        await tester.pump(Duration(milliseconds: milliseconds));
        final background = tester.widget<PhysicalShape>(find.descendant(
            of: menu.approvalRow, matching: find.byType(PhysicalShape)).first);
        expect(background.color, theme.colorScheme.onSurface.withOpacity(0.08));
        final innerButton = find.ancestor(
            of: find.text('Request approval'),
            matching: find.byType(TextButton)).first;
        final ink = Material.of(tester.element(innerButton)) as RenderObject;
        expect(ink, isNot(paints..rect(color: Colors.black.withOpacity(0.08))));
      }
      expect(find.text('Terminal'), findsOneWidget);
      await tester.tapAt(const Offset(2, 2));
      await tester.pumpAndSettle();
      await mouse.removePointer();
    }
  });

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

  testWidgets('menu actions have one Tab stop and RDP edit stays independent',
      (tester) async {
    for (final theme in [MyTheme.lightTheme, MyTheme.darkTheme]) {
      for (final action in ['Rename', 'RDP', 'Edit RDP']) {
        final actions = <String>[];
        await menu.openMenu(tester, actions, theme: theme);
        final text = find.text(action == 'Edit RDP' ? 'RDP' : action);
        final button =
            find.ancestor(of: text, matching: find.byType(TextButton)).first;
        final outerFocus = Focus.of(tester.element(button));
        for (var i = 0; i < 30 && !outerFocus.hasFocus; i++) {
          await tester.sendKeyEvent(LogicalKeyboardKey.tab);
          await tester.pump();
        }
        expect(Focus.of(tester.element(text)).hasPrimaryFocus, isTrue);
        await tester.pumpAndSettle();
        final ink = Material.of(tester.element(button)) as RenderObject;
        expect(ink,
            paints..rect(color: theme.colorScheme.onSurface.withOpacity(0.1)));
        await tester.sendKeyEvent(LogicalKeyboardKey.tab);
        await tester.pump();
        if (action == 'Rename') {
          expect(outerFocus.hasFocus, isFalse);
        } else {
          expect(Focus.of(tester.element(find.byIcon(Icons.edit))).hasPrimaryFocus,
              isTrue);
        }
        if (action != 'Edit RDP') {
          await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
          await tester.sendKeyEvent(LogicalKeyboardKey.tab);
          await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
          await tester.pump();
        }
        await tester.sendKeyEvent(LogicalKeyboardKey.enter);
        await tester.pumpAndSettle();
        expect(actions, [action]);
        expect(find.text('Request approval'), findsNothing);
      }
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
