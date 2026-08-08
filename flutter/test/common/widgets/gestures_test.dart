import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/common/widgets/gestures.dart';

void main() {
  testWidgets(
      'two fingers to one finger starts once after onEnd, and a fresh '
      'one-finger gesture restarts within 200ms', (tester) async {
    var oneFingerStart = 0;
    var twoFingerStart = 0;
    var twoFingerEnd = 0;
    final recognizer = CustomTouchGestureRecognizer();
    addTearDown(recognizer.dispose);
    recognizer
      ..onOneFingerPanStart = (d) {
        oneFingerStart++;
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

    recognizer.onEnd!.call(end(0));
    recognizer.onUpdate!.call(update(1));
    expect(oneFingerStart, 2);
  });
}
