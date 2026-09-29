import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_hbb/common/widgets/gestures.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets(
      'two fingers to one finger starts exactly once while moving, '
      'and a fresh gesture restarts after release', (tester) async {
    var oneFingerStart = 0;
    var oneFingerEnd = 0;
    var twoFingerStart = 0;
    var twoFingerEnd = 0;
    final recognizer = CustomTouchGestureRecognizer();
    addTearDown(recognizer.dispose);
    recognizer
      ..onOneFingerPanStart = (d) {
        oneFingerStart++;
      }
      ..onOneFingerPanEnd = (d) {
        oneFingerEnd++;
      }
      ..onTwoFingerScaleStart = (d) {
        twoFingerStart++;
      }
      ..onTwoFingerScaleEnd = (d) {
        twoFingerEnd++;
      };

    ScaleUpdateDetails update(int pointerCount) => ScaleUpdateDetails(
          focalPoint: Offset.zero,
          localFocalPoint: Offset.zero,
          focalPointDelta: Offset.zero,
          pointerCount: pointerCount,
        );
    ScaleEndDetails end(int pointerCount) =>
        ScaleEndDetails(pointerCount: pointerCount);

    recognizer.onUpdate!.call(update(2));
    expect(twoFingerStart, 1);

    recognizer.onEnd!.call(end(1));
    expect(twoFingerEnd, 1);

    for (var i = 0; i < 5; i++) {
      recognizer.onUpdate!.call(update(1));
      await tester.pump(const Duration(milliseconds: 50));
    }
    expect(oneFingerStart, 1);
    expect(twoFingerEnd, 1);

    recognizer.onEnd!.call(end(0));
    expect(oneFingerEnd, 1);
    await tester.pump(const Duration(milliseconds: 200));
    recognizer.onUpdate!.call(update(1));
    expect(oneFingerStart, 2);

    await tester.pump(const Duration(milliseconds: 300));
    expect(oneFingerStart, 2);
    expect(twoFingerEnd, 1);
  });

  testWidgets('rapid pinch cycles keep callback lifecycles balanced',
      (tester) async {
    var oneActive = false;
    var twoActive = false;

    await tester.pumpWidget(
      RawGestureDetector(
        behavior: HitTestBehavior.opaque,
        gestures: <Type, GestureRecognizerFactory>{
          CustomTouchGestureRecognizer: GestureRecognizerFactoryWithHandlers<
                  CustomTouchGestureRecognizer>(
              CustomTouchGestureRecognizer.new,
              (recognizer) => recognizer
                ..onOneFingerPanStart = (_) {
                  expect(oneActive, isFalse);
                  oneActive = true;
                }
                ..onOneFingerPanUpdate = (_) {
                  expect(oneActive, isTrue);
                }
                ..onOneFingerPanEnd = (_) {
                  expect(oneActive, isTrue);
                  oneActive = false;
                }
                ..onTwoFingerScaleStart = (_) {
                  expect(twoActive, isFalse);
                  twoActive = true;
                }
                ..onTwoFingerScaleUpdate = (_) {
                  expect(twoActive, isTrue);
                }
                ..onTwoFingerScaleEnd = (_) {
                  expect(twoActive, isTrue);
                  twoActive = false;
                }),
        },
        child: const SizedBox.expand(),
      ),
    );

    final first = await tester.startGesture(const Offset(100, 100), pointer: 1);
    await first.moveTo(const Offset(130, 100));
    await tester.pump();

    for (var i = 0; i < 3; i++) {
      final second = await tester.startGesture(
        Offset(200 + i.toDouble(), 100),
        pointer: i + 2,
      );
      await second.moveBy(const Offset(30, 0));
      await first.moveBy(const Offset(-5, 0));
      await tester.pump();
      await second.up();
      await tester.pump();
      await first.moveBy(const Offset(5, 0));
      await tester.pump(const Duration(milliseconds: 50));
    }

    await first.up();
    await tester.pump(const Duration(milliseconds: 300));

    expect(oneActive, isFalse);
    expect(twoActive, isFalse);
  });

  testWidgets('a new pinch right after the previous one starts at once',
      (tester) async {
    var twoStarts = 0;
    var twoUpdates = 0;
    var twoEnds = 0;

    await tester.pumpWidget(
      RawGestureDetector(
        behavior: HitTestBehavior.opaque,
        gestures: <Type, GestureRecognizerFactory>{
          CustomTouchGestureRecognizer: GestureRecognizerFactoryWithHandlers<
                  CustomTouchGestureRecognizer>(
              CustomTouchGestureRecognizer.new,
              (recognizer) => recognizer
                ..onTwoFingerScaleStart = (_) {
                  twoStarts++;
                }
                ..onTwoFingerScaleUpdate = (_) {
                  twoUpdates++;
                }
                ..onTwoFingerScaleEnd = (_) {
                  twoEnds++;
                }),
        },
        child: const SizedBox.expand(),
      ),
    );

    Future<void> pinch(int firstPointer) async {
      final a = await tester.startGesture(const Offset(100, 100),
          pointer: firstPointer);
      final b = await tester.startGesture(const Offset(200, 100),
          pointer: firstPointer + 1);
      for (var i = 0; i < 3; i++) {
        await b.moveBy(const Offset(15, 0));
        await tester.pump(const Duration(milliseconds: 20));
      }
      await b.up();
      await a.up();
      await tester.pump(const Duration(milliseconds: 10));
    }

    await pinch(1);
    final updatesAfterFirst = twoUpdates;
    await pinch(3);
    await tester.pump(const Duration(milliseconds: 300));

    expect(twoStarts, 2);
    expect(twoEnds, 2);
    expect(twoUpdates, greaterThan(updatesAfterFirst));
  });
}
