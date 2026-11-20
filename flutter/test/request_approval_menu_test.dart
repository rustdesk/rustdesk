import 'dart:ui' show PointerDeviceKind;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/desktop/widgets/material_mod_popup_menu.dart'
    as mod_menu;
import 'package:flutter_hbb/desktop/widgets/popup_menu.dart';

const connectionTypes = [
  'Connect',
  'Transfer file',
  'View camera',
  'Terminal',
  'Terminal (Run as administrator)',
  'TCP tunneling',
  'RDP',
];

Finder get approvalRow => find.ancestor(
    of: find.text('Request approval'),
    matching: find.byWidgetPredicate(
        (widget) => widget is mod_menu.PopupMenuEntry<String>));

Future<void> openMenu(WidgetTester tester, List<String> actions,
    {ThemeData? theme,
    EdgeInsets? padding,
    bool includeRelay = false,
    bool includeRename = true,
    List<MenuEntryBase<String>> extraEntries = const [],
    RelativeRect position = const RelativeRect.fromLTRB(20, 20, 780, 580)}) async {
  MenuEntryButton<String> entry(String title, {bool requestApproval = false}) =>
      MenuEntryButton<String>(
        childBuilder: (style) => Text(title, style: style),
        proc: () =>
            actions.add(requestApproval ? '$title:request-approval' : title),
        padding: padding,
        dismissOnClicked: true,
      );

  await tester.pumpWidget(MaterialApp(
    theme: theme,
    home: Scaffold(
      body: Builder(builder: (context) {
        return TextButton(
          onPressed: () => mod_menu.showMenu<String>(
            context: context,
            position: position,
            items: [
              entry('Connect'),
              entry('Transfer file'),
              entry('View camera'),
              MenuEntryButton<String>(
                childBuilder: (style) => Row(children: [
                  Text('RDP', style: style),
                  const Spacer(),
                  IconButton(
                    icon: const Icon(Icons.edit),
                    onPressed: () {
                      Navigator.pop(context);
                      actions.add('Edit RDP');
                    },
                  ),
                ]),
                proc: () => actions.add('RDP'),
                padding: padding,
                dismissOnClicked: true,
              ),
              MenuEntrySubMenu<String>(
                text: 'Request approval',
                arrowScale: 1.0,
                dismissParentOnCanceled: true,
                padding: padding,
                entries: [
                  for (final title in connectionTypes)
                    entry(title, requestApproval: true),
                ],
              ),
              if (includeRelay)
                MenuEntrySwitchSync<String>(
                  switchType: SwitchType.scheckbox,
                  text: 'Always connect via relay',
                  currentValue: false,
                  setter: (_) async {},
                  padding: padding,
                  dismissOnClicked: true,
                ),
              if (includeRename) entry('Rename'),
              ...extraEntries,
            ]
                .expand((entry) => entry.build(context,
                    const MenuConfig(commonColor: Colors.black, height: 20)))
                .toList(),
          ),
          child: const Text('Open menu'),
        );
      }),
    ),
  ));
  await tester.pumpAndSettle();
  await tester.tap(find.text('Open menu'));
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('last approval item stays open on click and when exiting below it',
      (tester) async {
    tester.view.physicalSize = const Size(1200, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final actions = <String>[];
    await openMenu(tester, actions, includeRename: false);
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await mouse.addPointer(location: const Offset(2, 2));
    addTearDown(mouse.removePointer);
    final trigger = tester.getCenter(approvalRow);
    await mouse.moveTo(trigger);
    await tester.pump(const Duration(milliseconds: 100));
    expect(find.text('Terminal'), findsNothing);
    await mouse.moveTo(const Offset(2, 2));
    await tester.pump(const Duration(milliseconds: 300));
    expect(find.text('Terminal'), findsNothing);

    await mouse.moveTo(trigger);
    await tester.pump(const Duration(milliseconds: 200));
    await tester.pumpAndSettle();
    expect(find.text('Terminal'), findsOneWidget);
    for (final kind in [PointerDeviceKind.mouse, PointerDeviceKind.touch]) {
      if (kind == PointerDeviceKind.mouse) {
        await mouse.down(trigger);
        await mouse.up();
      } else {
        await tester.tapAt(trigger, kind: kind);
      }
      await tester.pumpAndSettle();
      expect(find.text('Terminal'), findsOneWidget, reason: '$kind click');
      expect(actions, isEmpty);
    }
    final bottom = tester.getRect(approvalRow).bottom;
    for (final distance in [1.0, 20.0]) {
      await mouse.moveTo(Offset(trigger.dx, bottom + distance));
      await tester.pumpAndSettle();
      expect(find.text('Terminal'), findsOneWidget);
    }
    final highlight = Theme.of(tester.element(approvalRow))
        .colorScheme
        .onSurface
        .withOpacity(0.08);
    expect(
        tester
            .widgetList<Material>(find.descendant(
                of: approvalRow, matching: find.byType(Material)))
            .where((material) => material.color == highlight),
        hasLength(1));
    await tester.pump(const Duration(seconds: 2));
    expect(find.text('Show menu'), findsNothing);
    await tester.tapAt(const Offset(2, 2));
    await tester.pumpAndSettle();
  });

  testWidgets('approval delays closing over parent items and cancels on leaving',
      (tester) async {
    tester.view.physicalSize = const Size(1200, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final actions = <String>[];
    await openMenu(tester, actions);
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await mouse.addPointer(location: const Offset(2, 2));
    addTearDown(mouse.removePointer);
    final trigger = tester.getCenter(approvalRow);
    await mouse.moveTo(trigger);
    await tester.pump(const Duration(milliseconds: 200));
    await tester.pumpAndSettle();
    await mouse.moveTo(tester.getCenter(find.text('Connect').last));
    await tester.pump(const Duration(milliseconds: 400));
    expect(find.text('Terminal'), findsOneWidget);
    await mouse.moveTo(trigger);
    await tester.pump(const Duration(milliseconds: 400));
    expect(find.text('Terminal'), findsOneWidget);

    await mouse.moveTo(tester.getCenter(find.text('Connect').last));
    await mouse.moveTo(const Offset(2, 2));
    await tester.pump(const Duration(seconds: 1));
    await tester.pumpAndSettle();
    expect(find.text('Terminal'), findsOneWidget);
    expect(find.text('Request approval'), findsOneWidget);
    expect(actions, isEmpty);
    for (final destination in [
      tester.getCenter(find.text('Terminal')),
      trigger,
      const Offset(2, 2),
    ]) {
      await mouse.moveTo(tester.getCenter(find.text('Rename')));
      await tester.pump(const Duration(milliseconds: 300));
      expect(find.text('Terminal'), findsOneWidget);
      await mouse.moveTo(destination);
      await tester.pump(const Duration(milliseconds: 500));
      expect(find.text('Terminal'), findsOneWidget);
    }
    await mouse.moveTo(trigger);
    await tester.pumpAndSettle();
    final connect = tester.getCenter(find.text('Connect').first);
    await mouse.moveTo(connect);
    await tester.pump(const Duration(milliseconds: 399));
    expect(find.text('Terminal'), findsOneWidget);
    await mouse.moveTo(connect + const Offset(1, 0));
    await tester.pump(const Duration(milliseconds: 1));
    await tester.pumpAndSettle();
    expect(find.text('Terminal'), findsNothing);
    expect(find.text('Request approval'), findsOneWidget);
    expect(actions, isEmpty);
    await tester.tap(find.text('Connect'));
    await tester.pumpAndSettle();
    expect(actions, ['Connect']);
    expect(find.text('Request approval'), findsNothing);
  });

  testWidgets('submenu prefers the left screen edge when neither side fits',
      (tester) async {
    tester.view.physicalSize = const Size(500, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    await openMenu(tester, [],
        position: const RelativeRect.fromLTRB(480, 20, 20, 780));
    await tester.tap(find.text('Request approval'));
    await tester.pumpAndSettle();
    final submenu = tester.getRect(find.ancestor(
        of: find.text('Connect').last,
        matching: find.byWidgetPredicate(
            (widget) => widget is mod_menu.PopupMenuItem<String>)));
    expect(submenu.left, closeTo(0, 0.01));
    expect(submenu.right, lessThanOrEqualTo(500));
    await tester.tapAt(const Offset(2, 2));
    await tester.pumpAndSettle();
  });

  testWidgets('approval submenu opens beside its parent on either screen edge',
      (tester) async {
    tester.view.physicalSize = const Size(1200, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    for (final atRight in [false, true]) {
      await openMenu(tester, [],
          position: RelativeRect.fromLTRB(atRight ? 1180 : 20, 20,
              atRight ? 20 : 1180, 780));
      final parent = tester.getRect(approvalRow);
      await tester.tap(find.text('Request approval'));
      await tester.pumpAndSettle();
      final submenu = tester.getRect(find.ancestor(
          of: find.text('Connect').last,
          matching: find.byWidgetPredicate(
              (widget) => widget is mod_menu.PopupMenuItem<String>)));
      if (atRight) {
        expect(submenu.right, lessThanOrEqualTo(parent.left));
      } else {
        expect(submenu.left, greaterThanOrEqualTo(parent.right));
      }
      await tester.tapAt(const Offset(2, 2));
      await tester.pumpAndSettle();
    }
  });

  testWidgets('approval trigger has one Tab stop and opens with Enter',
      (tester) async {
    final actions = <String>[];
    await openMenu(tester, actions);
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await mouse.addPointer(location: const Offset(2, 2));
    addTearDown(mouse.removePointer);
    bool approvalHasFocus() {
      var focused = false;
      FocusManager.instance.primaryFocus?.context?.visitAncestorElements((e) {
        if (e == tester.element(approvalRow)) {
          focused = true;
          return false;
        }
        return true;
      });
      return focused;
    }

    for (var i = 0; i < 30 && !approvalHasFocus(); i++) {
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pump();
    }
    expect(approvalHasFocus(), isTrue);
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pump();
    expect(approvalHasFocus(), isFalse,
        reason: 'Tab must leave the approval row after one stop');

    await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
    await tester.pump();
    expect(approvalHasFocus(), isTrue);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    await tester.pump(const Duration(milliseconds: 500));
    await tester.pumpAndSettle();
    expect(find.text('Terminal (Run as administrator)'), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(actions, ['Connect:request-approval']);
    expect(find.text('Request approval'), findsNothing);
    await tester.tap(find.text('Open menu'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Request approval'));
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(actions, ['Connect:request-approval']);
    expect(find.text('Request approval'), findsNothing);
  });

  testWidgets('tapping outside the approval submenu closes both menus',
      (tester) async {
    final actions = <String>[];
    await openMenu(tester, actions);
    await tester.tap(find.text('Request approval'));
    await tester.pumpAndSettle();
    await tester.tapAt(const Offset(2, 2));
    await tester.pumpAndSettle();
    expect(actions, isEmpty);
    expect(find.text('Request approval'), findsNothing);
    expect(find.text('Rename'), findsNothing);
    expect(find.text('Open menu'), findsOneWidget);
  });

  testWidgets('approval text and arrow align with normal menu items',
      (tester) async {
    for (final (theme, padding) in [
      (
        ThemeData(
            useMaterial3: false,
            platform: TargetPlatform.windows,
            visualDensity: VisualDensity.compact),
        const EdgeInsets.only(left: 12, right: 3)
      ),
      (
        ThemeData(useMaterial3: false),
        const EdgeInsets.symmetric(horizontal: 12)
      ),
      (
        ThemeData(
            useMaterial3: false,
            textButtonTheme: TextButtonThemeData(
              style: TextButton.styleFrom(
                  padding: const EdgeInsets.symmetric(horizontal: 20)),
            )),
        null
      ),
    ]) {
      await openMenu(tester, [],
          theme: theme, padding: padding, includeRelay: true);
      expect(tester.getTopLeft(find.text('Request approval')).dx,
          tester.getTopLeft(find.text('Connect')).dx);
      expect(tester.getCenter(find.byIcon(Icons.keyboard_arrow_right)).dx,
          closeTo(tester.getCenter(find.byType(Checkbox)).dx, 0.01));
      await tester.tap(find.text('Rename'));
      await tester.pumpAndSettle();
    }
  });

  testWidgets('normal connections and RDP editing remain direct actions',
      (tester) async {
    for (final title in [
      'Connect',
      'Transfer file',
      'View camera',
      'RDP',
      'Edit RDP'
    ]) {
      final actions = <String>[];
      await openMenu(tester, actions);
      expect(find.byIcon(Icons.keyboard_arrow_right), findsOneWidget);
      final left = tester.getTopLeft(find.text('Rename')).dx;
      for (final row in ['Connect', 'Transfer file', 'View camera', 'RDP']) {
        expect(tester.getTopLeft(find.text(row)).dx, left);
      }
      await tester.tap(
          title == 'Edit RDP' ? find.byIcon(Icons.edit) : find.text(title));
      await tester.pumpAndSettle();
      expect(actions, [title]);
      expect(find.text('Request approval'), findsNothing);
      expect(find.text('Rename'), findsNothing);
    }
  });

  testWidgets(
      'one approval menu offers all connection types and closes both menus',
      (tester) async {
    for (var i = 0; i < connectionTypes.length; i++) {
      final actions = <String>[];
      await openMenu(tester, actions);
      await tester.tap(i.isEven
          ? find.text('Request approval')
          : find.byIcon(Icons.keyboard_arrow_right));
      await tester.pumpAndSettle();
      expect(actions, isEmpty);
      for (final title in connectionTypes) {
        expect(find.text(title), findsWidgets);
      }
      await tester.tap(find.text(connectionTypes[i]).last);
      await tester.pumpAndSettle();
      expect(actions, ['${connectionTypes[i]}:request-approval']);
      expect(find.text('Request approval'), findsNothing);
      expect(find.text('Rename'), findsNothing);
    }
  },
      variant: TargetPlatformVariant({
        TargetPlatform.android,
        TargetPlatform.iOS,
        TargetPlatform.linux,
        TargetPlatform.macOS,
        TargetPlatform.windows,
      }));

  testWidgets('the whole approval row opens the submenu on a narrow screen',
      (tester) async {
    tester.view.physicalSize = const Size(360, 640);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    for (final corner in [
      Alignment.topLeft,
      Alignment.topRight,
      Alignment.bottomLeft,
      Alignment.bottomRight,
    ]) {
      final actions = <String>[];
      await openMenu(tester, actions);
      final row = tester.getRect(approvalRow);
      await tester.tapAt(corner.withinRect(row.deflate(2)));
      await tester.pumpAndSettle();
      expect(actions, isEmpty);
      expect(find.text('Terminal (Run as administrator)'), findsOneWidget);
      await tester.tap(find.text('Connect').last);
      await tester.pumpAndSettle();
      expect(actions, ['Connect:request-approval']);
      expect(find.text('Rename'), findsNothing);
    }
  },
      variant:
          TargetPlatformVariant({TargetPlatform.android, TargetPlatform.iOS}));
}
