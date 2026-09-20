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

  testWidgets('reject closes an active gesture so no end is lost',
      (tester) async {
    var oneStart = 0;
    var oneEnd = 0;
    var oneCancel = 0;
    var twoStart = 0;
    var twoEnd = 0;
    final recognizer = CustomTouchGestureRecognizer();
    addTearDown(recognizer.dispose);
    recognizer
      ..onOneFingerPanStart = (d) {
        oneStart++;
      }
      ..onOneFingerPanEnd = (d) {
        oneEnd++;
      }
      ..onOneFingerPanCancel = () {
        oneCancel++;
      }
      ..onTwoFingerScaleStart = (d) {
        twoStart++;
      }
      ..onTwoFingerScaleEnd = (d) {
        twoEnd++;
      };

    ScaleUpdateDetails update(int pointerCount) => ScaleUpdateDetails(
          focalPoint: Offset.zero,
          localFocalPoint: Offset.zero,
          focalPointDelta: Offset.zero,
          pointerCount: pointerCount,
        );

    recognizer.onUpdate!.call(update(1));
    expect(oneStart, 1);
    recognizer.rejectGesture(1);
    expect(oneCancel, 1);
    expect(oneEnd, 1);

    recognizer.rejectGesture(2);
    expect(oneStart, 1);
    expect(oneEnd, 1);
    expect(oneCancel, 1);
    expect(twoStart, 0);
    expect(twoEnd, 0);

    recognizer.onUpdate!.call(update(2));
    expect(twoStart, 1);
    recognizer.rejectGesture(3);
    expect(twoEnd, 1);
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

  testWidgets('small pinch in and out movements start scaling', (tester) async {
    var scaleStarts = 0;
    final scales = <double>[];

    await tester.pumpWidget(
      RawGestureDetector(
        behavior: HitTestBehavior.opaque,
        gestures: <Type, GestureRecognizerFactory>{
          CustomTouchGestureRecognizer: GestureRecognizerFactoryWithHandlers<
                  CustomTouchGestureRecognizer>(
              CustomTouchGestureRecognizer.new,
              (recognizer) => recognizer
                ..onTwoFingerScaleStart = (_) {
                  scaleStarts++;
                }
                ..onTwoFingerScaleUpdate = (details) {
                  scales.add(details.scale);
                }),
          DoubleFinerTapGestureRecognizer: GestureRecognizerFactoryWithHandlers<
                  DoubleFinerTapGestureRecognizer>(
              DoubleFinerTapGestureRecognizer.new,
              (recognizer) => recognizer.onDoubleFinerTap = (_) {}),
        },
        child: const SizedBox.expand(),
      ),
    );

    Future<void> pinch(
      Offset firstDelta,
      Offset secondDelta,
      int expectedStarts,
      bool Function(double) scaleMatches,
    ) async {
      final first = await tester.startGesture(const Offset(100, 100));
      final second = await tester.startGesture(const Offset(200, 100));
      await first.moveBy(firstDelta);
      await second.moveBy(secondDelta);
      await tester.pump();
      expect(scaleStarts, expectedStarts);
      expect(scales.where(scaleMatches), isNotEmpty);
      await first.up();
      await second.up();
      await tester.pump(const Duration(milliseconds: 300));
    }

    await pinch(
        const Offset(-6, 0), const Offset(6, 0), 1, (scale) => scale > 1);
    await pinch(
        const Offset(6, 0), const Offset(-6, 0), 2, (scale) => scale < 1);

    expect(scaleStarts, 2);
  });

  testWidgets('two finger tap jitter does not start scaling', (tester) async {
    var scaleStarts = 0;
    var twoFingerTaps = 0;

    await tester.pumpWidget(
      RawGestureDetector(
        behavior: HitTestBehavior.opaque,
        gestures: <Type, GestureRecognizerFactory>{
          CustomTouchGestureRecognizer: GestureRecognizerFactoryWithHandlers<
                  CustomTouchGestureRecognizer>(
              CustomTouchGestureRecognizer.new,
              (recognizer) => recognizer.onTwoFingerScaleStart = (_) {
                    scaleStarts++;
                  }),
          DoubleFinerTapGestureRecognizer: GestureRecognizerFactoryWithHandlers<
                  DoubleFinerTapGestureRecognizer>(
              DoubleFinerTapGestureRecognizer.new,
              (recognizer) => recognizer.onDoubleFinerTap = (_) {
                    twoFingerTaps++;
                  }),
        },
        child: const SizedBox.expand(),
      ),
    );

    final first = await tester.startGesture(const Offset(100, 100));
    final second = await tester.startGesture(const Offset(200, 100));
    await first.moveBy(const Offset(-5, 0));
    await second.moveBy(const Offset(5, 0));
    await tester.pump();
    expect(scaleStarts, 0);

    await first.up();
    await second.up();
    await tester.pump(kDoubleTapTimeout);

    expect(scaleStarts, 0);
    expect(twoFingerTaps, 1);
  });
}
