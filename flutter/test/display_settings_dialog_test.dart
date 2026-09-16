import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart' as app;
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/common/widgets/display_settings_dialog.dart';
import 'package:flutter_hbb/utils/virtual_display.dart';

void main() {
  Finder textButton(String label) => find.ancestor(
      of: find.text(label),
      matching: find.byWidgetPredicate((widget) => widget is TextButton));

  Future<void> selectRatio(WidgetTester tester, String ratio) async {
    await tester
        .ensureVisible(find.byKey(const ValueKey('resolution-aspect-ratio')));
    await tester.tap(find.byKey(const ValueKey('resolution-aspect-ratio')));
    await tester.pumpAndSettle();
    final item = find.widgetWithText(MenuItemButton, ratio);
    await tester.ensureVisible(item);
    await tester.tap(item);
    await tester.pumpAndSettle();
  }

  testWidgets('preset menu is clickable above the session overlay dialog',
      (tester) async {
    final overlay = app.OverlayKeyState();
    final dialogs = app.OverlayDialogManager()..setOverlayState(overlay);
    addTearDown(dialogs.dismissAll);
    await tester.pumpWidget(MaterialApp(
        theme: app.MyTheme.lightTheme,
        home: Overlay(key: overlay.key, initialEntries: [
          OverlayEntry(builder: (_) => const Scaffold()),
        ])));
    List<int>? applied;
    dialogs.show(
        (_, close, context) => app.CustomAlertDialog(
              title: const Text('Resolution'),
              content: DisplaySettings(
                  translate: (s) => s,
                  minDimension: 320,
                  maxDimension: 4096,
                  width: 2560,
                  height: 1600,
                  usesLogicalSize: true,
                  scales: const [1, 2],
                  initialScale: 2,
                  supportedResolutions: const [(1920, 1080), (2560, 1600)],
                  onCancel: close,
                  onApply: (w, h, scale) {
                    applied = [w, h, scale];
                    return null;
                  }),
            ),
        clickMaskDismiss: true,
        backDismiss: true);
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('resolution-aspect-ratio')));
    await tester.pumpAndSettle();
    final preset = find.widgetWithText(MenuItemButton, '16:9');
    expect(preset.hitTestable(), findsOneWidget);
    await tester.tap(preset);
    await tester.pumpAndSettle();
    expect(applied, isNull);
    expect(find.byType(DisplaySettings), findsOneWidget);
    expect(
        tester.widget<TextField>(find.byType(TextField).first).controller!.text,
        '1280');
    await tester.tap(find.byKey(const ValueKey('resolution-aspect-ratio')));
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(find.byType(MenuItemButton), findsNothing);
    expect(find.byType(DisplaySettings), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('resolution-aspect-ratio')));
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(find.byType(MenuItemButton), findsNothing);
    expect(
        tester.widget<TextField>(find.byType(TextField).first).controller!.text,
        '1280');
    await tester.ensureVisible(find.text('Standard'));
    await tester.tap(find.text('Standard'));
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.text('Apply'));
    await tester.tap(find.text('Apply'));
    expect(applied, [1280, 720, 1]);
    await tester
        .ensureVisible(find.byKey(const ValueKey('resolution-aspect-ratio')));
    await tester.tap(find.byKey(const ValueKey('resolution-aspect-ratio')));
    await tester.pumpAndSettle();
    dialogs.dismissAll();
    await tester.pumpAndSettle();
    expect(find.byType(MenuItemButton), findsNothing);
    expect(tester.takeException(), isNull);
  });

  testWidgets('logical local fit converts output pixels before fitting bounds',
      (tester) async {
    List<int>? applied;
    await tester.pumpWidget(MaterialApp(
        home: Scaffold(
            body: AlertDialog(
      content: DisplaySettings(
          translate: (s) => s,
          minDimension: 160,
          maxDimension: 2048,
          width: 160,
          height: 160,
          outputPixelRatio: 2,
          localResolution: (3200, 1800),
          onCancel: () {},
          onApply: (w, h, scale) {
            applied = [w, h, scale];
            return null;
          }),
    ))));
    await tester.ensureVisible(find.text('resolution_fit_local_tip'));
    await tester.tap(find.text('resolution_fit_local_tip'));
    await tester.pump();
    await tester.ensureVisible(find.text('Apply'));
    await tester.tap(find.text('Apply'));
    expect(applied, [1600, 900, 1]);
  });

  testWidgets(
      'logical editor applies HiDPI minimum and rejects oversized output',
      (tester) async {
    final dimensions = virtualDisplayResolutionDimensions((320, 320, 2, 42),
        outputPixels: false);
    List<int>? applied;
    await tester.pumpWidget(MaterialApp(
        home: Scaffold(
            body: Center(
                child: SizedBox(
      width: 400,
      child: DisplaySettings(
        translate: (s) => s,
        minDimension: dimensions.minDimension,
        maxDimension: dimensions.maxDimension,
        width: dimensions.width,
        height: dimensions.height,
        onCancel: () {},
        onApply: (w, h, scale) {
          applied = [w, h, scale];
          return null;
        },
      ),
    )))));
    await tester.ensureVisible(find.text('Apply'));
    await tester.tap(find.text('Apply'));
    expect(applied, isNull);
    await selectRatio(tester, 'Custom');
    await tester.enterText(find.byType(TextField).first, '162');
    await tester.pump();
    await tester.ensureVisible(find.text('Apply'));
    await tester.tap(find.text('Apply'));
    expect(applied, [162, 160, 1]);
    await tester.enterText(find.byType(TextField).first, '3000');
    await tester.enterText(find.byType(TextField).last, '1800');
    await tester.pump();
    expect(
        tester
            .widget<ElevatedButton>(
                find.widgetWithText(ElevatedButton, 'Apply'))
            .onPressed,
        isNull);
    await tester.tap(find.text('Reset changes'));
    await tester.pump();
    expect(
        tester.widget<TextField>(find.byType(TextField).first).controller!.text,
        '160');
    expect(
        tester
            .widget<ElevatedButton>(
                find.widgetWithText(ElevatedButton, 'Apply'))
            .onPressed,
        isNull);
  });

  Future<void> openEditor(
          WidgetTester tester, void Function(int, int, int) apply,
          {List<(int, int)>? supportedResolutions,
          bool allowArbitrarySize = true,
          bool excludeInputSemantics = false,
          (int, int)? localResolution}) =>
      tester.pumpWidget(MaterialApp(
          home: Scaffold(
              body: AlertDialog(
        content: SizedBox(
            width: 400,
            child: DisplaySettings(
              key: UniqueKey(),
              translate: (value) => value,
              supportedResolutions:
                  supportedResolutions ?? const [(1920, 1080), (2560, 1600)],
              allowArbitrarySize: allowArbitrarySize,
              excludeInputSemantics: excludeInputSemantics,
              localResolution: localResolution,
              minDimension: 320,
              maxDimension: 4096,
              width: 2560,
              height: 1600,
              usesLogicalSize: allowArbitrarySize,
              scales: allowArbitrarySize ? const [1, 2] : const [1],
              initialScale: allowArbitrarySize ? 2 : 1,
              onApply: (w, h, scale) {
                apply(w, h, scale);
                return null;
              },
              onCancel: () {},
            )),
      ))));

  testWidgets('ratio linking, custom mode, rotation and reset stay coordinated',
      (tester) async {
    await openEditor(tester, (_, __, ___) {});
    final width = find.byType(TextField).first;
    final height = find.byType(TextField).last;
    String value(Finder field) =>
        tester.widget<TextField>(field).controller!.text;
    final ratioButton = find.byKey(const ValueKey('resolution-aspect-ratio'));
    await selectRatio(tester, 'Custom');
    expect(tester.widget<TextButton>(textButton('Reset changes')).onPressed,
        isNotNull);
    expect(tester.widget<ElevatedButton>(find.byType(ElevatedButton)).onPressed,
        isNull);
    await tester.ensureVisible(find.text('Reset changes'));
    await tester.tap(find.text('Reset changes'));
    await tester.pump();
    expect(find.descendant(of: ratioButton, matching: find.text('16:10')),
        findsOneWidget);

    await selectRatio(tester, '16:9');
    await tester.enterText(width, '1920');
    await tester.pump();
    expect(value(height), '1080');
    await tester.enterText(height, '900');
    await tester.pump();
    expect(value(width), '1600');
    await tester.tap(find.byTooltip('Swap width and height'));
    await tester.pump();
    expect((value(width), value(height)), ('900', '1600'));
    expect(find.descendant(of: ratioButton, matching: find.text('9:16')),
        findsOneWidget);
    await tester.enterText(width, '720');
    await tester.pump();
    expect(value(height), '1280');

    await selectRatio(tester, 'Custom');
    await tester.enterText(height, '1111');
    await tester.pump();
    expect(value(width), '720');
    await tester.tap(find.byTooltip('Swap width and height'));
    await tester.pump();
    expect((value(width), value(height)), ('1111', '720'));
    expect(find.descendant(of: ratioButton, matching: find.text('Custom')),
        findsOneWidget);
    await tester.ensureVisible(find.text('Reset changes'));
    await tester.tap(find.text('Reset changes'));
    await tester.pump();
    expect((value(width), value(height)), ('1280', '800'));
    expect(find.descendant(of: ratioButton, matching: find.text('16:10')),
        findsOneWidget);
  });

  testWidgets('linked input keeps overflow visible and handles partial entry',
      (tester) async {
    await openEditor(tester, (_, __, ___) {});
    await selectRatio(tester, '16:9');
    final width = find.byType(TextField).first;
    final height = find.byType(TextField).last;
    await tester.enterText(width, '3000');
    await tester.pump();
    expect(tester.widget<TextField>(width).controller!.text, '3000');
    expect(tester.widget<TextField>(height).controller!.text, '1688');
    expect(tester.widget<ElevatedButton>(find.byType(ElevatedButton)).onPressed,
        isNull);
    await tester.enterText(height, '');
    await tester.pump();
    expect(tester.widget<TextField>(width).controller!.text, '3000');
    await selectRatio(tester, '1:1');
    expect(tester.widget<TextField>(width).controller!.text, '2048');
    expect(tester.widget<TextField>(height).controller!.text, '2048');
    expect(find.text('Output resolution: 4096 × 4096 px'), findsOneWidget);
    await tester.enterText(width, '1');
    await tester.pump();
    await selectRatio(tester, '32:9');
    expect(tester.widget<TextField>(width).controller!.text, '569');
    expect(tester.widget<TextField>(height).controller!.text, '160');
  });

  testWidgets('physical ratio linking applies only advertised dimensions',
      (tester) async {
    List<int>? applied;
    await openEditor(tester, (w, h, scale) => applied = [w, h, scale],
        allowArbitrarySize: false,
        supportedResolutions: [(1280, 720), (1920, 1080), (2560, 1600)]);
    await selectRatio(tester, '16:9');
    final width = find.byType(TextField).first;
    final height = find.byType(TextField).last;
    expect(tester.widget<TextField>(width).controller!.text, '1920');
    await tester.enterText(width, '1600');
    await tester.pump();
    expect(tester.widget<TextField>(height).controller!.text, '900');
    expect(find.text('Select a resolution supported by the display'),
        findsOneWidget);
    expect(tester.widget<ElevatedButton>(find.byType(ElevatedButton)).onPressed,
        isNull);
    expect(applied, isNull);
    await tester.enterText(width, '1280');
    await tester.pump();
    expect(tester.widget<TextField>(height).controller!.text, '720');
    await tester.ensureVisible(find.text('Apply'));
    await tester.tap(find.text('Apply'));
    expect(applied, [1280, 720, 1]);
  });

  testWidgets('quality changes preserve desktop size and enforce output bounds',
      (tester) async {
    List<int>? applied;
    await openEditor(tester, (w, h, scale) => applied = [w, h, scale]);
    final width = find.byType(TextField).first;
    final height = find.byType(TextField).last;
    ElevatedButton applyButton() => tester
        .widget<ElevatedButton>(find.widgetWithText(ElevatedButton, 'Apply'));
    await selectRatio(tester, 'Custom');
    await tester.enterText(width, '1441');
    await tester.enterText(height, '901');
    await tester.ensureVisible(find.text('Standard'));
    await tester.tap(find.text('Standard'));
    await tester.pump();
    expect(tester.widget<TextField>(width).controller!.text, '1441');
    expect(tester.widget<TextField>(height).controller!.text, '901');
    expect(find.text('Output resolution: 1441 × 901 px'), findsOneWidget);
    await tester.enterText(width, '3000');
    await tester.pump();
    expect(applyButton().onPressed, isNotNull);
    await tester.ensureVisible(find.text('HiDPI'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('HiDPI'));
    await tester.pumpAndSettle();
    expect(tester.widget<TextField>(width).controller!.text, '3000');
    expect(find.text('Enter dimensions within the range (160–2048 px)'),
        findsOneWidget);
    expect(applyButton().onPressed, isNull);
    for (final size in [159, 160, 2048, 2049]) {
      await tester.enterText(width, '$size');
      await tester.enterText(height, '$size');
      await tester.pump();
      expect(applyButton().onPressed,
          size == 160 || size == 2048 ? isNotNull : isNull);
    }
    await tester.enterText(width, '1441');
    await tester.enterText(height, '901');
    await tester.pump();
    await tester.ensureVisible(find.text('Apply'));
    await tester.tap(find.text('Apply'));
    expect(applied, [2882, 1802, 2]);

    await tester.pumpWidget(MaterialApp(
        home: Scaffold(
            body: DisplaySettings(
                translate: (s) => s,
                minDimension: 320,
                maxDimension: 4096,
                width: applied![0],
                height: applied![1],
                initialScale: applied![2],
                usesLogicalSize: true,
                scales: const [1, 2],
                onCancel: () {},
                onApply: (_, __, ___) => null))));
    expect(tester.widget<TextField>(width).controller!.text, '1441');
    expect(tester.widget<TextField>(height).controller!.text, '901');
    expect(tester.widget<TextButton>(textButton('Reset changes')).onPressed,
        isNull);
    expect(applyButton().onPressed, isNull);
  });

  testWidgets('local fit matches output pixels and preserves rendering choice',
      (tester) async {
    addTearDown(tester.view.display.reset);
    for (final (density, pixels, scale) in [
      (3.0, (1440, 3120), 2),
      (1.25, (1920, 1080), 2),
      (3.0, (1920, 1080), 1),
    ]) {
      tester.view.display.devicePixelRatio = density;
      List<int>? applied;
      await openEditor(tester, (w, h, scale) => applied = [w, h, scale],
          localResolution: pixels);
      if (scale == 1) {
        await tester.ensureVisible(find.text('Standard'));
        await tester.tap(find.text('Standard'));
        await tester.pump();
      }
      await tester.ensureVisible(find.text('resolution_fit_local_tip'));
      await tester.tap(find.text('resolution_fit_local_tip'));
      await tester.pump();
      await tester.ensureVisible(find.text('Apply'));
      await tester.tap(find.text('Apply'));
      expect(applied, [pixels.$1, pixels.$2, scale]);
    }
  });

  testWidgets('fit uses full display output pixels rather than window size',
      (tester) async {
    tester.view.display.size = const Size(3048, 2032);
    tester.view.display.devicePixelRatio = 2;
    addTearDown(tester.view.display.reset);
    List<int>? applied;
    await openEditor(tester, (w, h, scale) => applied = [w, h, scale]);
    await tester.ensureVisible(find.text('resolution_fit_local_tip'));
    await tester.tap(find.text('resolution_fit_local_tip'));
    await tester.pump();
    await tester.ensureVisible(find.text('Apply'));
    await tester.tap(find.text('Apply'));
    expect(applied, [3048, 2032, 2]);
  });

  testWidgets('physical ratios deduplicate and retain available resolutions',
      (tester) async {
    List<int>? applied;
    await openEditor(tester, (w, h, scale) => applied = [w, h, scale],
        allowArbitrarySize: false,
        supportedResolutions: const [
          (1920, 1080),
          (1280, 720),
          (1920, 1080),
          (5120, 2880),
          (200, 100),
          (2560, 1600),
        ]);
    await tester.tap(find.byKey(const ValueKey('resolution-aspect-ratio')));
    await tester.pumpAndSettle();
    expect(find.widgetWithText(MenuItemButton, '16:9'), findsOneWidget);
    expect(find.widgetWithText(MenuItemButton, '16:10'), findsOneWidget);
    final modes = find.widgetWithText(SubmenuButton, 'Available resolutions');
    await tester.ensureVisible(modes);
    await tester.tap(modes);
    await tester.pumpAndSettle();
    expect(find.widgetWithText(MenuItemButton, '1920 × 1080'), findsOneWidget);
    expect(find.widgetWithText(MenuItemButton, '5120 × 2880'), findsNothing);
    await tester.tap(find.widgetWithText(MenuItemButton, '1280 × 720'));
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.text('Apply'));
    await tester.tap(find.text('Apply'));
    expect(applied, [1280, 720, 1]);
  });

  testWidgets('physical local matching offers the closest advertised mode',
      (tester) async {
    await openEditor(tester, (_, __, ___) {},
        allowArbitrarySize: false, localResolution: (9999, 8888));
    expect(find.text('Closest supported resolution'), findsNothing);
    expect(find.byTooltip('Closest supported resolution\n2560 × 1600'),
        findsOneWidget);
    await tester.ensureVisible(find.text('resolution_fit_local_tip'));
    await tester.tap(find.text('resolution_fit_local_tip'));
    await tester.pump();
    expect(
        tester.widget<TextField>(find.byType(TextField).first).controller!.text,
        '2560');
    await openEditor(tester, (_, __, ___) {},
        allowArbitrarySize: false, localResolution: (1920, 1080));
    await tester.ensureVisible(find.text('resolution_fit_local_tip'));
    await tester.tap(find.text('resolution_fit_local_tip'));
    await tester.pump();
    expect(find.text('1920'), findsOneWidget);
    expect(find.text('1080'), findsOneWidget);
  });

  testWidgets('unlisted current mode stays visible without becoming selectable',
      (tester) async {
    await openEditor(tester, (_, __, ___) {},
        allowArbitrarySize: false, supportedResolutions: [(1920, 1080)]);
    expect(
        tester.widget<TextField>(find.byType(TextField).first).controller!.text,
        '2560');
    expect(
        tester.widget<TextField>(find.byType(TextField).last).controller!.text,
        '1600');
    expect(
        tester
            .widget<ElevatedButton>(
                find.widgetWithText(ElevatedButton, 'Apply'))
            .onPressed,
        isNull);
    expect(find.text('16:10'), findsOneWidget);
  });

  testWidgets('Linux input workaround excludes only the dimension fields',
      (tester) async {
    await openEditor(tester, (_, __, ___) {}, excludeInputSemantics: true);
    for (final field in find.byType(TextField).evaluate()) {
      final wrapper = tester.widget<ExcludeSemantics>(find
          .ancestor(
              of: find.byWidget(field.widget),
              matching: find.byType(ExcludeSemantics))
          .first);
      expect(wrapper.excluding, isTrue);
    }
    expect(
        find.ancestor(
            of: find.text('Apply'),
            matching: find.byWidgetPredicate(
                (w) => w is ExcludeSemantics && w.excluding)),
        findsNothing);
  });

  testWidgets('quick settings adapt to narrow and wide screens with large text',
      (tester) async {
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetDevicePixelRatio);
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetViewInsets);
    for (final width in [360.0, 900.0]) {
      tester.view.physicalSize = Size(width, 720);
      tester.view.viewInsets = const FakeViewPadding();
      List<int>? applied;
      await tester.pumpWidget(MaterialApp(
          theme: app.MyTheme.lightTheme,
          builder: (context, child) => MediaQuery(
              data: MediaQuery.of(context)
                  .copyWith(textScaler: const TextScaler.linear(1.6)),
              child: child!),
          home: Scaffold(
              body: AlertDialog(
            content: SizedBox(
                width: 500,
                child: DisplaySettings(
                    key: UniqueKey(),
                    translate: (s) => s == 'resolution_fit_local_tip'
                        ? 'Fit local resolution'
                        : s,
                    minDimension: 320,
                    maxDimension: 4096,
                    width: 2560,
                    height: 1600,
                    usesLogicalSize: true,
                    scales: const [1, 2],
                    initialScale: 2,
                    defaultResolution: (1920, 1080, 1),
                    localResolution: (1440, 3120),
                    onCancel: () {},
                    onApply: (w, h, scale) {
                      applied = [w, h, scale];
                      return null;
                    })),
          ))));
      expect(tester.takeException(), isNull);
      final local = textButton('Fit local resolution');
      final defaults = textButton('Default');
      final localRect = tester.getRect(local);
      final defaultRect = tester.getRect(defaults);
      expect(localRect.overlaps(defaultRect), isFalse);
      expect(defaultRect.top, greaterThanOrEqualTo(localRect.top));
      if (defaultRect.top > localRect.top) {
        expect(defaultRect.top, greaterThanOrEqualTo(localRect.bottom));
      }
      await tester.ensureVisible(defaults);
      await tester.tap(defaults);
      await tester.pump();
      expect(applied, isNull);
      expect(find.text('Output resolution: 1920 × 1080 px'), findsOneWidget);
      tester.view.viewInsets = const FakeViewPadding(bottom: 260);
      await tester.pumpAndSettle();
      await tester.ensureVisible(find.text('Apply'));
      await tester.pumpAndSettle();
      expect(find.text('Apply').hitTestable(), findsOneWidget);
      await tester.tap(find.text('Apply'));
      expect(applied, [1920, 1080, 1]);
      expect(tester.takeException(), isNull);
    }
  });
}
