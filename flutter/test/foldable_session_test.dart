import 'package:flutter/material.dart';
import 'package:flutter_hbb/mobile/services/foldable_layout.dart';
import 'package:flutter_hbb/mobile/widgets/session_control_panel.dart';
import 'package:flutter_hbb/mobile/widgets/session_toolbar.dart';
import 'package:flutter_hbb/mobile/widgets/session_key_menu.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('fold, flat and resized windows use current safe canvas bounds', () {
    const size = Size(626, 890);
    const fold = Rect.fromLTWH(0, 430, 626, 30);
    const layout =
        FoldableLayout(supported: true, size: size, divisions: [fold]);
    final seated = FoldableSessionGeometry.resolve(
        size: size,
        padding: const EdgeInsets.only(top: 24, bottom: 20),
        layout: layout,
        showControls: true);
    expect(seated.canvas, const Rect.fromLTRB(0, 24, 626, 430));
    expect(seated.controls, const Rect.fromLTRB(0, 460, 626, 870));
    final withKeyboard = FoldableSessionGeometry.resolve(
        size: size,
        padding: EdgeInsets.zero,
        layout: layout,
        viewInsets: const EdgeInsets.only(bottom: 350),
        showControls: true);
    expect(withKeyboard.canvas.bottom, 430);
    expect(withKeyboard.canvas.right, 578);
    final book = FoldableSessionGeometry.resolve(
        size: const Size(890, 626),
        padding: EdgeInsets.zero,
        layout: const FoldableLayout(
            supported: true,
            size: Size(890, 626),
            divisions: [Rect.fromLTWH(430, 0, 30, 626)]),
        showControls: true);
    expect(book.canvas.right, 430);
    expect(book.controls!.left, 460);
    final resized = FoldableSessionGeometry.resolve(
        size: const Size(400, 626),
        padding: const EdgeInsets.only(right: 20),
        layout: layout,
        showControls: true);
    expect(resized.divided, isFalse);
    expect(resized.canvas, const Rect.fromLTWH(0, 0, 380, 578));
    final hidden = FoldableSessionGeometry.resolve(
        size: const Size(890, 626),
        padding: EdgeInsets.zero,
        layout: const FoldableLayout(),
        showControls: false);
    expect(hidden.canvas.size, const Size(890, 626));
    expect(hidden.controls, isNull);
  });

  testWidgets('narrow horizontal and vertical toolbars reach the final action',
      (tester) async {
    var presses = 0;
    for (final axis in Axis.values) {
      await tester.pumpWidget(MaterialApp(
          home: Scaffold(
              body: SizedBox(
        width: axis == Axis.horizontal ? 260 : 48,
        height: axis == Axis.horizontal ? 48 : 260,
        child: SessionToolbar(color: Colors.blue, axis: axis, actions: [
          for (var i = 0; i < 8; ++i)
            IconButton(
                tooltip: 'Action $i',
                icon: const Icon(Icons.settings),
                onPressed: () => presses++),
        ]),
      ))));
      expect(tester.takeException(), isNull);
      await tester.drag(
          find.byType(SingleChildScrollView),
          axis == Axis.horizontal
              ? const Offset(-400, 0)
              : const Offset(0, -400));
      await tester.pumpAndSettle();
      await tester.tap(find.byTooltip('Action 7'));
    }
    expect(presses, 2);
  });

  testWidgets(
      'seated keyboard and trackpad dispatch input and respect read-only mode',
      (tester) async {
    final keys = <String>[];
    final clicks = <bool>[];
    final moves = <Offset>[];
    Future<void> render(bool enabled, Size size,
        {bool keyboardEnabled = true}) async {
      await tester.binding.setSurfaceSize(size);
      await tester.pumpWidget(MaterialApp(
          home: SessionControlPanel(
        toolbar: const SizedBox(),
        enabled: enabled,
        keyboardEnabled: keyboardEnabled,
        onKey: keys.add,
        onMove: moves.add,
        onClick: clicks.add,
        keyboardLabel: 'Keyboard',
        mouseLabel: 'Mouse mode',
        leftClickLabel: 'Left click',
        rightClickLabel: 'Right click',
      )));
      expect(tester.takeException(), isNull);
    }

    addTearDown(() => tester.binding.setSurfaceSize(null));
    await render(true, const Size(626, 410));
    await tester.tap(find.text('q'));
    await tester.tap(find.byTooltip('Shift'));
    await tester.pump();
    await tester.tap(find.text('Q'));
    await tester.tap(find.text('Left click'));
    await tester.drag(
        find.byIcon(Icons.touch_app_outlined), const Offset(80, 0));
    expect(keys, ['q', 'Q']);
    expect(clicks, [true]);
    expect(moves, isNotEmpty);
    await tester.pump();
    expect(find.text('q'), findsOneWidget);
    await render(true, const Size(626, 410), keyboardEnabled: false);
    await tester.tap(find.text('q'));
    await tester.tap(find.text('Right click'));
    expect(keys, ['q', 'Q']);
    expect(clicks, [true, false]);
    await render(false, const Size(626, 410));
    await tester.tap(find.text('q'));
    await tester.tap(find.text('Right click'));
    expect(keys, ['q', 'Q']);
    expect(clicks, [true, false]);
    await render(false, const Size(280, 190));
    expect(find.text('q'), findsNothing);
    expect(find.byIcon(Icons.touch_app_outlined), findsOneWidget);
    expect(tester.takeException(), isNull);
  });
  testWidgets(
      'folding keyboard types punctuation and releases Shift after a character',
      (tester) async {
    await tester.binding.setSurfaceSize(const Size(669, 420));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    for (final isMac in [true, false]) {
      final modifiers = <SessionModifier>{};
      final sent = <String>[];
      await tester.pumpWidget(MaterialApp(
          key: ValueKey(isMac),
          home: StatefulBuilder(
              builder: (context, setState) => SessionControlPanel(
                    toolbar: const SizedBox(),
                    enabled: true,
                    isMac: isMac,
                    modifiers: modifiers,
                    onModifier: (modifier) => setState(() {
                      if (!modifiers.add(modifier)) modifiers.remove(modifier);
                    }),
                    onKey: sent.add,
                    onMove: (_) {},
                    onClick: (_) {},
                    keyboardLabel: 'Keyboard',
                    mouseLabel: 'Mouse mode',
                    leftClickLabel: 'Left click',
                    rightClickLabel: 'Right click',
                  ))));
      await tester.tap(find.byTooltip('Shift'));
      await tester.pump();
      expect(find.text('!'), findsOneWidget);
      await tester.tap(find.text('!'));
      await tester.pump();
      await tester.tap(find.text('a'));
      await tester.pump();
      expect(sent, ['!', 'a']);
      expect(modifiers, isEmpty);
      await tester.tap(find.byTooltip('Shift'));
      await tester.pump();
      await tester.tap(find.text('A'));
      await tester.pump();
      await tester.tap(find.text('b'));
      expect(sent, ['!', 'a', 'A', 'b']);
      await tester.tap(find.text('?123'));
      await tester.pump();
      for (final symbol in ['?', '@', '.', '/', '+', '_']) {
        await tester.tap(find.text(symbol));
        await tester.pump();
        expect(sent.last, symbol);
      }
      await tester.tap(find.text('ABC'));
      await tester.pump();
      expect(find.text('a'), findsOneWidget);
      expect(tester.takeException(), isNull);
    }
  });

  testWidgets('Mac modifier keys select the existing input flags',
      (tester) async {
    final modifiers = <SessionModifier>{};
    Set<SessionModifier>? sentModifiers;
    String? sentKey;
    await tester.binding.setSurfaceSize(const Size(669, 420));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(MaterialApp(
        home: StatefulBuilder(
      builder: (context, setState) => SessionControlPanel(
        toolbar: const SizedBox(),
        enabled: true,
        isMac: true,
        modifiers: modifiers,
        onModifier: (modifier) => setState(() {
          if (!modifiers.add(modifier)) modifiers.remove(modifier);
        }),
        onKey: (key) {
          sentKey = key;
          sentModifiers = Set.of(modifiers);
        },
        onMove: (_) {},
        onClick: (_) {},
        keyboardLabel: 'Keyboard',
        mouseLabel: 'Mouse mode',
        leftClickLabel: 'Left click',
        rightClickLabel: 'Right click',
      ),
    )));
    await tester.tap(find.byIcon(Icons.keyboard_command_key));
    await tester.pump();
    await tester.tap(find.text('c'));
    expect(sentKey, 'c');
    expect(sentModifiers, {SessionModifier.command});
    await tester.tap(find.byIcon(Icons.keyboard_option_key));
    await tester.pump();
    await tester.tap(find.byIcon(Icons.keyboard_arrow_left));
    expect(sentKey, 'VK_LEFT');
    expect(sentModifiers, {SessionModifier.command, SessionModifier.option});
    await tester.tap(find.byTooltip('Fn'));
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.text('Home'));
    await tester.tap(find.text('Home'));
    await tester.pumpAndSettle();
    expect(sentKey, 'VK_HOME');
    expect(find.byTooltip('Fn'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });
  testWidgets('flat Fn controls retain escape, tab, enter and arrow keys',
      (tester) async {
    final sent = <String>[];
    await tester.pumpWidget(MaterialApp(
        home: Scaffold(
            body: SessionKeyMenu(
      enabled: true,
      isMac: true,
      modifiers: const {},
      onModifier: (_) {},
      onKey: sent.add,
      onShortcut: (_) {},
    ))));
    final keys = {
      'Esc': 'VK_ESCAPE',
      'Tab': 'VK_TAB',
      'Enter': 'VK_RETURN',
      'Left': 'VK_LEFT',
      'Up': 'VK_UP',
      'Down': 'VK_DOWN',
      'Right': 'VK_RIGHT'
    };
    for (final entry in keys.entries) {
      await tester.tap(find.byTooltip('Fn'));
      await tester.pumpAndSettle();
      expect(find.text(entry.key), findsOneWidget);
      await tester.ensureVisible(find.text(entry.key));
      await tester.tap(find.text(entry.key));
      await tester.pumpAndSettle();
      expect(sent.last, entry.value);
    }
    expect(sent.length, keys.length);
  });

  testWidgets('hidden toolbar can be revealed above the system keyboard',
      (tester) async {
    const size = Size(626, 890);
    const insets = EdgeInsets.only(bottom: 350);
    final geometry = FoldableSessionGeometry.resolve(
        size: size,
        padding: EdgeInsets.zero,
        viewInsets: insets,
        layout: const FoldableLayout(),
        showControls: false);
    var revealed = false;
    await tester.binding.setSurfaceSize(size);
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(MaterialApp(
        home: MediaQuery(
      data: const MediaQueryData(size: size, viewInsets: insets),
      child: Scaffold(
          resizeToAvoidBottomInset: false,
          body: Stack(children: [
            SessionToolbarReveal(
                canvas: geometry.canvas,
                tooltip: 'Show Toolbar',
                onPressed: () => revealed = true),
          ])),
    )));
    expect(tester.getRect(find.byType(FloatingActionButton)).bottom,
        lessThanOrEqualTo(size.height - insets.bottom));
    await tester.tap(find.byTooltip('Show Toolbar'));
    expect(revealed, isTrue);
  });
}
