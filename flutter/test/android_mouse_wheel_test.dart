import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_hbb/common.dart' show AccessibilityListener;
import 'package:flutter_hbb/models/android_mouse_wheel.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  const origin = Offset(100, 100);

  Future<({List<Offset> signals, List<PointerDownEvent> touches})> pumpAndCapture(
      WidgetTester tester) async {
    tester.view.devicePixelRatio = 2.25; // MRO-W00; AGS5-W00 to be measured
    tester.view.physicalSize = const Size(2000, 1200);
    addTearDown(tester.view.reset);
    final signals = <Offset>[];
    final touches = <PointerDownEvent>[];
    await tester.pumpWidget(AccessibilityListener(
      child: Listener(
        behavior: HitTestBehavior.opaque,
        onPointerSignal: (event) {
          if (event is PointerScrollEvent) signals.add(event.scrollDelta);
        },
        onPointerDown: (event) {
          if (event.kind == PointerDeviceKind.touch) touches.add(event);
        },
        child: const SizedBox.expand(),
      ),
    ));
    return (signals: signals, touches: touches);
  }

  testWidgets('AGS5-style wheel sequence becomes a scroll signal',
      (tester) async {
    final captured = await pumpAndCapture(tester);
    final signals = captured.signals;
    final touches = captured.touches;

    await tester
        .sendEventToBinding(const PointerAddedEvent(kind: PointerDeviceKind.mouse));
    await tester.sendEventToBinding(
        const PointerHoverEvent(kind: PointerDeviceKind.mouse, position: origin));

    for (final direction in [1, -1]) {
      await tester.sendEventToBinding(PointerDownEvent(
          pointer: 1,
          kind: PointerDeviceKind.unknown,
          buttons: 0, // firmware reports no buttons at all
          size: 1,
          position: origin));
      for (final distance in [32, 64]) {
        await tester.sendEventToBinding(PointerMoveEvent(
            pointer: 1,
            kind: PointerDeviceKind.unknown,
            buttons: 0,
            size: 1,
            position: origin + Offset(0, distance * direction.toDouble()),
            delta: Offset(0, 32 * direction.toDouble())));
      }
      await tester.sendEventToBinding(PointerUpEvent(
          pointer: 1,
          kind: PointerDeviceKind.unknown,
          size: 1,
          position: origin + Offset(0, 64 * direction.toDouble())));
    }

    // One notch per 64 logical px at DPR 2.25 (= 144 physical px).
    expect(signals, [const Offset(0, -64), const Offset(0, 64)]);
    // The synthetic sequence must not be re-dispatched as a touch.
    expect(touches, isEmpty);
  });

  testWidgets('wheel works without a preceding hover', (tester) async {
    final captured = await pumpAndCapture(tester);
    await tester.sendEventToBinding(PointerDownEvent(
        pointer: 3,
        kind: PointerDeviceKind.unknown,
        buttons: 0,
        size: 1,
        position: const Offset(300, 400)));
    await tester.sendEventToBinding(PointerMoveEvent(
        pointer: 3,
        kind: PointerDeviceKind.unknown,
        buttons: 0,
        size: 1,
        delta: const Offset(0, 64),
        position: const Offset(300, 464)));
    expect(captured.signals, [const Offset(0, -64)]);
  });

  testWidgets('finger touches keep the accessibility workaround',
      (tester) async {
    final captured = await pumpAndCapture(tester);
    await tester.sendEventToBinding(PointerDownEvent(
        pointer: 4,
        kind: PointerDeviceKind.touch,
        size: 12,
        position: const Offset(200, 200)));
    await tester.sendEventToBinding(PointerUpEvent(
        pointer: 4,
        kind: PointerDeviceKind.touch,
        size: 12,
        position: const Offset(200, 200)));
    expect(captured.signals, isEmpty);
    // The original finger touch reaches the child listener (expected); what must
    // not happen is an extra, re-synthesized pointer (AccessibilityListener adds
    // its 100 offset to synthesized pointers).
    expect(captured.touches.map((e) => e.pointer), [4]);
  });

  test('recognizer keeps the remainder across notches', () {
    final wheel = AndroidMouseWheel(logicalPerNotch: 64);
    expect(
        wheel.handle(const PointerHoverEvent(
            kind: PointerDeviceKind.mouse, position: origin)),
        isNull);
    expect(
        wheel.handle(const PointerDownEvent(
            pointer: 1,
            kind: PointerDeviceKind.unknown,
            size: 1,
            position: origin)),
        0);
    expect(
        wheel.handle(const PointerMoveEvent(
            pointer: 1,
            kind: PointerDeviceKind.unknown,
            size: 1,
            delta: Offset(0, 32),
            position: Offset(100, 132))),
        0);
    expect(
        wheel.handle(const PointerMoveEvent(
            pointer: 1,
            kind: PointerDeviceKind.unknown,
            size: 1,
            delta: Offset(0, 32),
            position: Offset(100, 164))),
        1);
    expect(
        wheel.handle(const PointerMoveEvent(
            pointer: 1,
            kind: PointerDeviceKind.unknown,
            size: 1,
            delta: Offset(0, 64),
            position: Offset(100, 228))),
        1);
  });

  test('sequence shorter than the constant still yields one notch', () {
    // Firmware on another density may report only 32 px per notch; the estimate
    // (144) must not swallow the notch entirely.
    final wheel = AndroidMouseWheel(logicalPerNotch: 144);
    wheel.handle(const PointerHoverEvent(
        kind: PointerDeviceKind.mouse, position: origin));
    expect(
        wheel.handle(const PointerDownEvent(
            pointer: 7,
            kind: PointerDeviceKind.unknown,
            size: 1,
            position: origin)),
        0);
    expect(
        wheel.handle(const PointerMoveEvent(
            pointer: 7,
            kind: PointerDeviceKind.unknown,
            size: 1,
            delta: Offset(0, 32),
            position: Offset(100, 132))),
        0);
    expect(
        wheel.handle(const PointerUpEvent(
            pointer: 7,
            kind: PointerDeviceKind.unknown,
            size: 1,
            position: Offset(100, 132))),
        1);
  });
}
