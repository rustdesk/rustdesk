import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_hbb/common.dart' show AccessibilityListener;
import 'package:flutter_hbb/models/android_mouse_wheel.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('wheel signals survive the accessibility listener',
      (tester) async {
    const origin = Offset(100, 100);
    final signals = <Offset>[];
    final touches = <PointerDownEvent>[];
    tester.view.physicalSize = const Size(1600, 1200);
    addTearDown(tester.view.reset);
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
    await tester.sendEventToBinding(const PointerAddedEvent(
        kind: PointerDeviceKind.mouse, position: origin));
    await tester.sendEventToBinding(const PointerHoverEvent(
        kind: PointerDeviceKind.mouse, position: origin));
    for (final dpr in [1.0, 2.25, 3.5]) {
      tester.view.devicePixelRatio = dpr;
      await tester.pump();
      for (final direction in [1, -1]) {
        await tester.sendEventToBinding(PointerDownEvent(
            pointer: 1,
            kind: PointerDeviceKind.unknown,
            buttons: kPrimaryButton,
            size: 1,
            position: origin));
        for (final distance in [72 / dpr, 144 / dpr]) {
          await tester.sendEventToBinding(PointerMoveEvent(
              pointer: 1,
              kind: PointerDeviceKind.unknown,
              buttons: kPrimaryButton,
              size: 1,
              position: origin + Offset(0, distance * direction.toDouble()),
              delta: Offset(0, 72 / dpr * direction.toDouble())));
        }
        await tester.sendEventToBinding(PointerUpEvent(
            pointer: 1,
            kind: PointerDeviceKind.unknown,
            size: 1,
            position: origin + Offset(0, 144 / dpr * direction.toDouble())));
      }
    }
    expect(signals, [
      for (var i = 0; i < 3; i++) ...[
        const Offset(0, -64),
        const Offset(0, 64),
      ]
    ]);
    expect(touches, isEmpty);

    // Unmatched accessibility input must retain the existing touch workaround.
    await tester.sendEventToBinding(const PointerDownEvent(
        pointer: 2,
        kind: PointerDeviceKind.unknown,
        buttons: kPrimaryButton,
        size: 1,
        position: Offset(300, 100)));
    await tester.sendEventToBinding(const PointerUpEvent(
        pointer: 2,
        kind: PointerDeviceKind.unknown,
        size: 1,
        position: Offset(300, 100)));
    expect(touches, hasLength(1));
  });

  const start = Offset(400, 600);
  PointerHoverEvent hover() => const PointerHoverEvent(
      kind: PointerDeviceKind.mouse,
      position: start,
      timeStamp: Duration(milliseconds: 100));
  PointerDownEvent down(
          {PointerDeviceKind kind = PointerDeviceKind.unknown,
          Offset position = start,
          int milliseconds = 105}) =>
      PointerDownEvent(
          pointer: 1,
          kind: kind,
          buttons: kPrimaryButton,
          position: position,
          timeStamp: Duration(milliseconds: milliseconds));
  PointerMoveEvent move(double y,
          {PointerDeviceKind kind = PointerDeviceKind.unknown}) =>
      PointerMoveEvent(
          pointer: 1,
          kind: kind,
          buttons: kPrimaryButton,
          position: start + Offset(0, y));

  test(
      'synthetic half-steps produce one wheel tick per notch in either direction',
      () {
    for (final direction in [1, -1]) {
      final wheel = AndroidMouseWheel();
      expect(wheel.handle(hover()), isNull);
      expect(wheel.handle(down()), 0);
      expect(wheel.handle(move(32.0 * direction)), 0);
      expect(wheel.handle(move(64.0 * direction)), direction);
      expect(wheel.handle(move(64.0 * direction)), 0);
      expect(wheel.handle(move(256.0 * direction)), 3 * direction);
      expect(wheel.handle(const PointerUpEvent(pointer: 1)), 0);
      expect(wheel.handle(move(320.0 * direction)), isNull);
    }
  });

  test('finger swipes and mouse drags remain outside wheel conversion', () {
    for (final kind in [PointerDeviceKind.touch, PointerDeviceKind.mouse]) {
      final wheel = AndroidMouseWheel();
      wheel.handle(hover());
      expect(wheel.handle(down(kind: kind)), isNull);
      expect(wheel.handle(move(64, kind: kind)), isNull);
    }
  });

  test(
      'uses the last mouse position while stationary and stops on cancellation or another down',
      () {
    final wheel = AndroidMouseWheel();
    expect(wheel.handle(down()), isNull);
    wheel.handle(hover());
    expect(wheel.handle(down(position: start + const Offset(1, 0))), isNull);
    wheel.handle(hover());
    expect(wheel.handle(down(milliseconds: 1627)), 0);
    expect(wheel.handle(move(64)), 1);
    expect(wheel.handle(const PointerUpEvent(pointer: 1)), 0);
    wheel.handle(hover());
    expect(wheel.handle(down()), 0);
    expect(wheel.handle(const PointerCancelEvent(pointer: 1)), 0);
    expect(wheel.handle(move(64)), isNull);
    wheel.handle(
        const PointerUpEvent(kind: PointerDeviceKind.mouse, position: start));
    expect(wheel.handle(down()), 0);
    expect(wheel.handle(move(64)), 1);
    wheel.handle(hover());
    expect(wheel.handle(down()), 0);
    expect(wheel.handle(const PointerDownEvent(pointer: 2)), isNull);
    expect(wheel.handle(move(64)), isNull);
  });
}
