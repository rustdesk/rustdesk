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
}
