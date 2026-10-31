import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/common/widgets/login_method_menu.dart';
import 'package:flutter_hbb/desktop/widgets/material_mod_popup_menu.dart'
    as mod_menu;
import 'package:flutter_hbb/desktop/widgets/popup_menu.dart';

Future<void> openMenu(WidgetTester tester, List<String> actions) async {
  MenuEntryButton<String> entry(String title) => MenuEntryButton<String>(
        childBuilder: (style) => Text(title, style: style),
        proc: () => actions.add(title),
        dismissOnClicked: true,
      );

  await tester.pumpWidget(MaterialApp(
    home: Scaffold(
      body: Builder(builder: (context) {
        return TextButton(
          onPressed: () => mod_menu.showMenu<String>(
            context: context,
            position: const RelativeRect.fromLTRB(20, 20, 780, 580),
            items: [
              LoginMethodMenu(
                defaultEntry: entry('Connect'),
                methods: [entry('Without password')],
              ),
              LoginMethodMenu(
                defaultEntry: entry('View camera'),
                methods: [entry('Without password')],
              ),
              LoginMethodMenu(
                defaultEntry: MenuEntryButton<String>(
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
                ),
                methods: [entry('Without password')],
              ),
              entry('Rename'),
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
  await tester.tap(find.text('Open menu'));
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('main rows align and clicking them connects by default',
      (tester) async {
    for (final title in ['Connect', 'View camera', 'RDP']) {
      final actions = <String>[];
      await openMenu(tester, actions);
      final left = tester.getTopLeft(find.text('Rename')).dx;
      for (final row in ['Connect', 'View camera', 'RDP']) {
        expect(tester.getTopLeft(find.text(row)).dx, left);
      }
      await tester.tap(find.text(title));
      await tester.pumpAndSettle();
      expect(actions, [title]);
      expect(find.text('Without password'), findsNothing);
      expect(find.text('Rename'), findsNothing);
    }
  });

  testWidgets(
      'arrow offers connection without password and dismisses both menus',
      (tester) async {
    final actions = <String>[];
    await openMenu(tester, actions);
    await tester.tap(find.byIcon(Icons.keyboard_arrow_right).first);
    await tester.pumpAndSettle();
    expect(actions, isEmpty);
    expect(find.text('Without password'), findsOneWidget);
    expect(find.text('Enter password manually'), findsNothing);
    await tester.tap(find.text('Without password'));
    await tester.pumpAndSettle();
    expect(actions, ['Without password']);
    expect(find.text('Without password'), findsNothing);
    expect(find.text('Rename'), findsNothing);
  },
      variant: TargetPlatformVariant({
        TargetPlatform.android,
        TargetPlatform.iOS,
        TargetPlatform.linux,
        TargetPlatform.macOS,
        TargetPlatform.windows,
      }));

  testWidgets('RDP edit remains independent of the connection action',
      (tester) async {
    final actions = <String>[];
    await openMenu(tester, actions);
    await tester.tap(find.byIcon(Icons.edit));
    await tester.pumpAndSettle();
    expect(actions, ['Edit RDP']);
    expect(find.text('Rename'), findsNothing);
  });

  testWidgets('touches around the arrow open the submenu without connecting',
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
      final arrow = find.byIcon(Icons.keyboard_arrow_right).first;
      final row = tester
          .getRect(find.ancestor(of: arrow, matching: find.byType(Row)).first);
      final target = Rect.fromCenter(
          center: tester.getCenter(arrow), width: 56, height: row.height);
      await tester.tapAt(corner.withinRect(target.deflate(2)));
      await tester.pumpAndSettle();
      expect(actions, isEmpty);
      expect(find.text('Without password'), findsOneWidget);
      await tester.tap(find.text('Without password'));
      await tester.pumpAndSettle();
      expect(actions, ['Without password']);
      expect(find.text('Rename'), findsNothing);
    }
  },
      variant:
          TargetPlatformVariant({TargetPlatform.android, TargetPlatform.iOS}));
}
