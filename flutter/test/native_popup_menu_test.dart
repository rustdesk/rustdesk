import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/common/widgets/native_popup_menu.dart';
import 'package:flutter_hbb/desktop/widgets/popup_menu.dart';
import 'package:get/get.dart';

Future<void> openMenu<T>(WidgetTester tester, List<MenuEntryBase<T>> items,
    {double width = 1200}) async {
  tester.view.physicalSize = Size(width, 800);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  await tester.pumpWidget(MaterialApp(
    home: Scaffold(body: Builder(builder: (context) {
      return TextButton(
        onPressed: () => showNativePopupMenu<T>(
          context: context,
          position: RelativeRect.fromLTRB(width - 20, 20, 20, 780),
          items: items,
        ),
        child: const Text('Open menu'),
      );
    })),
  ));
  await tester.tap(find.text('Open menu'));
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('multiple submenus position independently on a narrow screen',
      (tester) async {
    final actions = <String>[];
    const entries = {
      'Connection actions': 'Open a separate terminal',
      'Device settings': 'Move files to device',
    };
    await openMenu<String>(
        tester,
        [
          for (final entry in entries.entries)
            MenuEntrySubMenu<String>(text: entry.key, entries: [
              MenuEntryButton<String>(
                childBuilder: (_) => Text(entry.value),
                proc: () => actions.add(entry.key),
                dismissOnClicked: true,
              ),
            ]),
        ],
        width: 500);
    for (final name in [
      entries.keys.first,
      entries.keys.last,
      entries.keys.first
    ]) {
      final parent = tester.getRect(find.ancestor(
          of: find.text(name), matching: find.byType(TextButton)));
      await tester.tapAt(parent.centerRight - const Offset(12, 0));
      await tester.pumpAndSettle();
      final submenu = tester.getRect(find.ancestor(
          of: find.text(entries[name]!),
          matching: find.byType(MenuItemButton)));
      expect(submenu.left, closeTo(0, 0.01));
      expect(submenu.right, lessThan(parent.right));
      expect(find.text(entries.entries.firstWhere((e) => e.key != name).value),
          findsNothing);
      expect(actions, isEmpty);
    }
    await tester.tap(find.text(entries.values.first));
    await tester.pumpAndSettle();
    expect(actions, [entries.keys.first]);
    expect(find.text(entries.keys.first), findsNothing);
  });

  testWidgets('typed nested menus preserve enabled state and item actions',
      (tester) async {
    final enabled = false.obs;
    final actions = <String>[];
    await openMenu<int>(tester, [
      MenuEntrySubMenu<int>(text: 'Options', enabled: enabled, entries: [
        MenuEntryDivider<int>(),
        MenuEntrySwitchSync<int>(
          switchType: SwitchType.scheckbox,
          text: 'Keep session',
          currentValue: false,
          setter: (value) async => actions.add('keep:$value'),
        ),
        MenuEntrySubMenu<int>(text: 'More', entries: [
          MenuEntryButton<int>(
            childBuilder: (_) => const Text('Inspect'),
            proc: () => actions.add('inspect'),
          ),
          MenuEntryButton<int>(
            childBuilder: (_) => const Text('Apply'),
            proc: () => actions.add('apply'),
            dismissOnClicked: true,
            dismissCallback: () => actions.add('dismissed'),
          ),
        ]),
      ]),
    ]);
    await tester.tap(find.text('Options'));
    await tester.pumpAndSettle();
    expect(find.text('Keep session'), findsNothing);
    enabled.value = true;
    await tester.pumpAndSettle();
    await tester.tap(find.text('Options'));
    await tester.pumpAndSettle();
    await tester.tapAt(tester.getCenter(find.byType(Checkbox)));
    await tester.pumpAndSettle();
    expect(tester.widget<Checkbox>(find.byType(Checkbox)).value, isTrue);
    await tester.tap(find.text('More'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Inspect'));
    await tester.pumpAndSettle();
    expect(find.text('Apply'), findsOneWidget);
    await tester.tap(find.text('Apply'));
    await tester.pumpAndSettle();
    expect(actions, ['keep:true', 'inspect', 'dismissed', 'apply']);
    expect(find.text('Options'), findsNothing);
    expect(find.text('More'), findsNothing);
  });
}
