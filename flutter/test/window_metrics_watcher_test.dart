import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:flutter_hbb/mobile/widgets/window_metrics_watcher.dart';

void main() {
  testWidgets(
      'toolbar updates geometry without refitting a user-positioned image',
      (tester) async {
    final toolbarHeight = ValueNotifier(60.0);
    addTearDown(toolbarHeight.dispose);
    final viewports = <Size>[];
    var changes = 0;
    var refit = true;
    await tester.pumpWidget(MediaQuery(
      data: const MediaQueryData(size: Size(800, 600)),
      child: Center(
        child: SizedBox(
          width: 800,
          height: 600,
          child: ValueListenableBuilder<double>(
            valueListenable: toolbarHeight,
            builder: (_, height, __) => Column(children: [
              Expanded(
                child: WindowMetricsWatcher(
                  refitOnViewportChange: refit,
                  onChanged: () => changes++,
                  onViewportChanged: viewports.add,
                  child: const SizedBox.expand(),
                ),
              ),
              SizedBox(height: height),
            ]),
          ),
        ),
      ),
    ));
    expect(viewports, [const Size(800, 540)]);
    toolbarHeight.value = 120;
    await tester.pump();
    expect(viewports.last, const Size(800, 480));
    expect(changes, 2);
    refit = false;
    toolbarHeight.value = 0;
    await tester.pump();
    expect(viewports.last, const Size(800, 600));
    expect(changes, 2);
  });

  Future<void> watch(WidgetTester tester, ValueNotifier<MediaQueryData> metrics,
      VoidCallback onChanged) async {
    await tester.pumpWidget(ValueListenableBuilder<MediaQueryData>(
      valueListenable: metrics,
      builder: (_, value, __) => MediaQuery(
        data: value,
        child: WindowMetricsWatcher(
          refitOnViewportChange: false,
          onChanged: onChanged,
          child: const SizedBox.expand(),
        ),
      ),
    ));
  }

  testWidgets('same-orientation window resize refreshes without a timer',
      (tester) async {
    final metrics = ValueNotifier(const MediaQueryData(size: Size(420, 900)));
    addTearDown(metrics.dispose);
    var changes = 0;
    await watch(tester, metrics, () => changes++);
    expect(changes, 1);
    metrics.value = metrics.value.copyWith(size: const Size(800, 1000));
    metrics.value = metrics.value.copyWith(size: const Size(820, 1100));
    await tester.pump();
    expect(changes, 2);
    await tester.pump();
    expect(changes, 2);
  });

  testWidgets('Scaffold keyboard resize preserves scale unless refit is requested',
      (tester) async {
    tester.view.physicalSize = const Size(820, 1100);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    addTearDown(tester.view.resetViewInsets);
    final refitOnKeyboard = ValueNotifier(false);
    addTearDown(refitOnKeyboard.dispose);
    final viewports = <Size>[];
    var changes = 0;
    await tester.pumpWidget(MaterialApp(
      home: Scaffold(
        bottomNavigationBar: const SizedBox(height: 60),
        body: ValueListenableBuilder<bool>(
          valueListenable: refitOnKeyboard,
          builder: (_, refit, __) => WindowMetricsWatcher(
            refitOnKeyboardChange: refit,
            onChanged: () => changes++,
            onViewportChanged: viewports.add,
            child: const SizedBox.expand(),
          ),
        ),
      ),
    ));
    expect(changes, 1);
    expect(viewports.last, const Size(820, 1040));
    tester.view.viewInsets = const FakeViewPadding(bottom: 400);
    await tester.pump();
    expect(viewports.last, const Size(820, 700));
    expect(changes, 1);
    tester.view.viewInsets = const FakeViewPadding();
    await tester.pump();
    expect(viewports.last, const Size(820, 1040));
    expect(changes, 1);
    refitOnKeyboard.value = true;
    await tester.pump();
    tester.view.viewInsets = const FakeViewPadding(bottom: 400);
    await tester.pump();
    expect(viewports.last, const Size(820, 700));
    expect(changes, 2);
  });

  testWidgets('initial empty viewport defers refit until keyboard dismissal',
      (tester) async {
    tester.view.physicalSize = const Size(669, 869);
    tester.view.devicePixelRatio = 1;
    tester.view.viewInsets = const FakeViewPadding(bottom: 869);
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    addTearDown(tester.view.resetViewInsets);
    final viewports = <Size>[];
    final refits = <Size>[];
    await tester.pumpWidget(MaterialApp(
      home: Scaffold(
        body: WindowMetricsWatcher(
          refitOnViewportChange: false,
          refitOnKeyboardChange: false,
          onViewportChanged: viewports.add,
          onChanged: () => refits.add(viewports.last),
          child: const SizedBox.expand(),
        ),
      ),
    ));
    expect(viewports, isEmpty);
    expect(refits, isEmpty);
    tester.view.viewInsets = const FakeViewPadding();
    await tester.pump();
    expect(viewports, [const Size(669, 869)]);
    expect(refits, [const Size(669, 869)]);
  });

  testWidgets('display scale changes refresh without changing window size',
      (tester) async {
    final metrics = ValueNotifier(const MediaQueryData(size: Size(820, 1100)));
    addTearDown(metrics.dispose);
    var changes = 0;
    await watch(tester, metrics, () => changes++);
    metrics.value = metrics.value.copyWith(devicePixelRatio: 3);
    await tester.pump();
    expect(changes, 2);
  });
}
