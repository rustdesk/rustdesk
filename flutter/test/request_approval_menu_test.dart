import 'package:flutter/gestures.dart' show PointerDeviceKind;
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/common.dart' show MyTheme;
import 'package:flutter_hbb/common/widgets/native_popup_menu.dart';
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

Future<void> openMenu(WidgetTester tester, List<String> actions,
    {ThemeData? theme,
    EdgeInsets? padding,
    bool includeRelay = false,
    bool includeIcons = false,
    bool legacy = false,
    TextDirection? textDirection,
    List<MenuEntryBase<String>> extraEntries = const [],
    RelativeRect position =
        const RelativeRect.fromLTRB(20, 20, 780, 580)}) async {
  Future<void> showItems({
    required BuildContext context,
    required RelativeRect position,
    required List<MenuEntryBase<String>> items,
  }) async {
    if (legacy) {
      await mod_menu.showMenu<String>(
        context: context,
        position: position,
        items: items
            .expand((entry) => entry.build(
                context,
                const MenuConfig(
                    commonColor: Colors.black,
                    height: 20,
                    dividerHeight: CustomPopupMenuTheme.dividerHeight)))
            .toList(),
        elevation: 8,
      );
    } else {
      await showNativePopupMenu<String>(
          context: context, position: position, items: items);
    }
  }

  MenuEntryButton<String> entry(String title, {bool requestApproval = false}) =>
      MenuEntryButton<String>(
        childBuilder: (style) => Text(title, style: style),
        proc: () =>
            actions.add(requestApproval ? '$title:request-approval' : title),
        padding: padding,
        dismissOnClicked: true,
      );

  EdgeInsets legacySubmenuPadding(BuildContext context) {
    final theme = Theme.of(context);
    final checkboxTheme = CheckboxTheme.of(context);
    final checkboxWidth =
        ((checkboxTheme.materialTapTargetSize ?? theme.materialTapTargetSize) ==
                    MaterialTapTargetSize.padded
                ? kMinInteractiveDimension
                : kMinInteractiveDimension - 8) +
            (checkboxTheme.visualDensity ?? theme.visualDensity)
                .baseSizeAdjustment.dx;
    return (padding ?? EdgeInsets.zero).copyWith(
        right: (padding?.right ?? 0) +
            (checkboxWidth - (IconTheme.of(context).size ?? 24)) / 2);
  }

  await tester.pumpWidget(MaterialApp(
    theme: theme,
    builder: textDirection == null
        ? null
        : (_, child) => Directionality(
            textDirection: textDirection, child: child!),
    home: Scaffold(
      body: Builder(builder: (context) {
        return TextButton(
          onPressed: () => showItems(
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
                  Transform.scale(
                    scale: 0.8,
                    child: IconButton(
                      icon: const Icon(Icons.edit),
                      padding: EdgeInsets.zero,
                      constraints: const BoxConstraints(
                        minWidth: kMinInteractiveDimension - 8,
                        minHeight: kMinInteractiveDimension,
                      ),
                      onPressed: () {
                        Navigator.pop(context);
                        actions.add('Edit RDP');
                      },
                    ),
                  ),
                ]),
                proc: () => actions.add('RDP'),
                padding: padding,
                dismissOnClicked: true,
              ),
              MenuEntrySubMenu<String>(
                text: 'Request approval',
                dismissParentOnCanceled: true,
                arrowScale: 1.0,
                padding: legacy ? legacySubmenuPadding(context) : padding,
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
                  setter: (value) async => actions.add('relay:$value'),
                  padding: padding,
                  dismissOnClicked: true,
                ),
              entry('Rename'),
              ...extraEntries,
              if (includeIcons)
                for (final (title, icon) in [
                  ('Add to Favorites', Icons.star_outline),
                  ('Remove from Favorites', Icons.star),
                  ('Delete', Icons.delete_forever),
                ])
                  MenuEntryButton<String>(
                    childBuilder: (style) => Row(children: [
                      Text(title, style: style),
                      Expanded(
                        child: Padding(
                          padding: const EdgeInsets.only(right: 4),
                          child: Align(
                            alignment: Alignment.centerRight,
                            child: Transform.scale(
                              scale: 0.8,
                              child: Icon(icon),
                            ),
                          ),
                        ),
                      ),
                    ]),
                    proc: () => actions.add(title),
                    padding: padding,
                    dismissOnClicked: true,
                  ),
            ],
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
  testWidgets('empty submenus do not prevent normal menu actions',
      (tester) async {
    final actions = <String>[];
    await openMenu(tester, actions, extraEntries: [
      MenuEntrySubMenu<String>(text: 'Empty', entries: []),
    ]);
    expect(tester.takeException(), isNull);
    await tester.tap(find.text('Connect'));
    await tester.pumpAndSettle();
    expect(actions, ['Connect']);
    expect(find.text('Request approval'), findsNothing);
  });

  testWidgets('native menu preserves previous row sizing and content insets',
      (tester) async {
    tester.view.physicalSize = const Size(1400, 1000);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    for (final (theme, padding, relay) in [
      (
        MyTheme.lightTheme.copyWith(
            platform: TargetPlatform.windows,
            visualDensity: VisualDensity.compact,
            materialTapTargetSize: MaterialTapTargetSize.shrinkWrap),
        const EdgeInsets.only(left: 12, right: 3),
        true
      ),
      (
        MyTheme.darkTheme.copyWith(
            platform: TargetPlatform.macOS,
            visualDensity: VisualDensity.compact,
            materialTapTargetSize: MaterialTapTargetSize.shrinkWrap),
        const EdgeInsets.symmetric(horizontal: 12),
        false
      ),
      (
        MyTheme.lightTheme.copyWith(
            platform: TargetPlatform.android,
            visualDensity: VisualDensity.standard,
            materialTapTargetSize: MaterialTapTargetSize.padded,
            textButtonTheme: MyTheme.mobileTextButtonTheme),
        null,
        true
      ),
    ]) {
      final previous = <(Size, double, Offset)>[];
      for (final legacy in [true, false]) {
        await openMenu(tester, [],
            theme: theme,
            padding: padding,
            includeRelay: relay,
            legacy: legacy);
        for (final submenu in [false, true]) {
          if (submenu) {
            await tester.tap(find.text('Request approval'));
            await tester.pumpAndSettle();
          }
          final text = find.text('Connect').last;
          final panel = tester.getRect(find
              .ancestor(
                  of: text,
                  matching: find.byWidgetPredicate(
                      (widget) => widget is Material && widget.elevation == 8))
              .first);
          final row = tester.getRect(find.ancestor(
              of: text,
              matching: legacy
                  ? find.byType(mod_menu.PopupMenuItem<String>)
                  : find.byType(MenuItemButton)));
          final inset = tester.getTopLeft(text) - panel.topLeft;
          if (legacy) {
            previous.add((panel.size, row.height, inset));
          } else {
            final expected = previous[submenu ? 1 : 0];
            expect(panel.height, expected.$1.height);
            // The ID menu needs 8px more for the native arrow's aligned inset.
            expect(panel.width,
                expected.$1.width + (!relay && !submenu ? 8 : 0));
            expect(row.height, expected.$2);
            expect(inset, expected.$3);
          }
        }
        await tester.tapAt(const Offset(2, 2));
        await tester.pumpAndSettle();
      }
    }
  });

  testWidgets('approval submenu opens on hover without a tooltip',
      (tester) async {
    final actions = <String>[];
    await openMenu(tester, actions);
    expect(find.byTooltip('Show menu'), findsNothing);
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await mouse.addPointer(location: Offset.zero);
    await mouse.moveTo(tester.getCenter(find.text('Request approval')));
    await tester.pumpAndSettle();
    expect(find.text('Terminal (Run as administrator)'), findsOneWidget);
    final triggerContext = tester.element(find.text('Request approval'));
    expect(
        Material.of(triggerContext) as RenderObject,
        paints
          ..rect(
              color: Theme.of(triggerContext)
                  .colorScheme
                  .onSurface
                  .withOpacity(0.08)));
    expect(actions, isEmpty);
    await mouse.moveTo(tester.getCenter(find.text('Connect').last));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Connect').last);
    await tester.pumpAndSettle();
    expect(actions, ['Connect:request-approval']);
    expect(find.text('Request approval'), findsNothing);
    await mouse.removePointer();
  });

  testWidgets('hovering away closes only the approval submenu on either side',
      (tester) async {
    tester.view.physicalSize = const Size(1200, 800);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await mouse.addPointer(location: Offset.zero);
    for (final atRight in [false, true]) {
      final actions = <String>[];
      await openMenu(tester, actions,
          position: RelativeRect.fromLTRB(atRight ? 1180 : 20, 20,
              atRight ? 20 : 1180, 780));
      final trigger = tester.getCenter(find.text('Request approval'));
      for (final destination in [
        tester.getCenter(find.text('Rename')),
        tester.getCenter(find.text('Transfer file')),
      ]) {
        await mouse.moveTo(trigger);
        await tester.pumpAndSettle();
        expect(find.text('Terminal (Run as administrator)'), findsOneWidget);
        await mouse.moveTo(tester.getCenter(find.text('Connect').last));
        await tester.pumpAndSettle();
        await mouse.moveTo(trigger);
        await tester.pumpAndSettle();
        expect(find.text('Terminal (Run as administrator)'), findsOneWidget);
        await mouse.moveTo(destination);
        await tester.pumpAndSettle();
        expect(find.text('Terminal (Run as administrator)'), findsNothing);
        expect(find.text('Request approval'), findsOneWidget);
        expect(actions, isEmpty);
      }
      await tester.tap(find.text('Rename'));
      await tester.pumpAndSettle();
      expect(actions, ['Rename']);
    }
    await mouse.removePointer();
  });

  testWidgets('submenu keeps part of its parent visible on narrow screens',
      (tester) async {
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    for (final width in [360.0, 500.0]) {
      tester.view.physicalSize = Size(width, 800);
      for (final direction in TextDirection.values) {
        final rtl = direction == TextDirection.rtl;
        final actions = <String>[];
        await openMenu(tester, actions,
            theme: MyTheme.lightTheme.copyWith(
              visualDensity: VisualDensity.standard,
              materialTapTargetSize: MaterialTapTargetSize.padded,
              textButtonTheme: MyTheme.mobileTextButtonTheme,
            ),
            textDirection: direction,
            position: RelativeRect.fromLTRB(
                rtl ? 20 : width - 20, 20, rtl ? width - 20 : 20, 780));
        final parent = tester.getRect(find.ancestor(
            of: find.text('Request approval'),
            matching: find.byType(TapRegion)).first);
        await tester.tap(find.text('Request approval'));
        await tester.pumpAndSettle();
        final submenu = tester.getRect(find.ancestor(
            of: find.text('Connect').last,
            matching: find.byType(TapRegion)).first);
        expect(submenu.left, greaterThanOrEqualTo(0));
        expect(submenu.right, lessThanOrEqualTo(width));
        if (rtl) {
          expect(submenu.left, greaterThan(parent.left));
          expect(submenu.right, closeTo(width, 0.01));
        } else {
          expect(submenu.left, closeTo(0, 0.01));
          expect(submenu.right, lessThan(parent.right));
        }
        await tester.tap(find.text('Connect').last);
        await tester.pumpAndSettle();
        expect(actions, ['Connect:request-approval']);
        expect(find.text('Request approval'), findsNothing);
      }
    }
  },
      variant:
          TargetPlatformVariant({TargetPlatform.android, TargetPlatform.iOS}));

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
      final parent = tester.getRect(find.byType(SubmenuButton));
      await tester.tap(find.text('Request approval'));
      await tester.pumpAndSettle();
      final submenu = tester.getRect(find.ancestor(
          of: find.text('Connect').last,
          matching: find.byType(MenuItemButton)));
      if (atRight) {
        expect(submenu.right, lessThanOrEqualTo(parent.left));
      } else {
        expect(submenu.left, greaterThanOrEqualTo(parent.right));
      }
      await tester.tapAt(const Offset(2, 2));
      await tester.pumpAndSettle();
    }
  });

  testWidgets('approval submenu supports arrow keys and Enter', (tester) async {
    final actions = <String>[];
    await openMenu(tester, actions);
    bool approvalHasFocus() =>
        FocusManager.instance.primaryFocus?.context
            ?.findAncestorWidgetOfExactType<SubmenuButton>() !=
        null;

    for (var i = 0; i < 30 && !approvalHasFocus(); i++) {
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
      await tester.pump();
    }
    expect(approvalHasFocus(), isTrue);
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowRight);
    await tester.pumpAndSettle();
    expect(find.text('Terminal (Run as administrator)'), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
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
          theme: theme, padding: padding, includeRelay: true, includeIcons: true);
      expect(tester.getTopLeft(find.text('Request approval')).dx,
          tester.getTopLeft(find.text('Connect')).dx);
      expect(tester.getCenter(find.byIcon(Icons.arrow_right)).dx,
          closeTo(tester.getCenter(find.byType(Checkbox)).dx, 0.01));
      for (final icon in [Icons.star_outline, Icons.star, Icons.delete_forever]) {
        expect(tester.getCenter(find.byIcon(icon)).dx,
            closeTo(tester.getCenter(find.byType(Checkbox)).dx, 0.01));
      }
      if (theme.platform == TargetPlatform.windows) {
        expect(tester.getCenter(find.byIcon(Icons.edit)).dx,
            closeTo(tester.getCenter(find.byType(Checkbox)).dx, 0.01));
      }
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
      expect(find.byIcon(Icons.arrow_right), findsOneWidget);
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

  testWidgets('relay row and checkbox toggle once and close the menu',
      (tester) async {
    for (final tapCheckbox in [false, true]) {
      final actions = <String>[];
      await openMenu(tester, actions, includeRelay: true);
      await tester.tapAt(tester.getCenter(tapCheckbox
          ? find.byType(Checkbox)
          : find.text('Always connect via relay')));
      await tester.pumpAndSettle();
      expect(actions, ['relay:true']);
      expect(find.text('Request approval'), findsNothing);
      expect(find.text('Open menu'), findsOneWidget);
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
          : find.byIcon(Icons.arrow_right));
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
      final row = tester.getRect(find.byType(SubmenuButton));
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
