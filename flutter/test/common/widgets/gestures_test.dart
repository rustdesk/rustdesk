import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/common/widgets/gestures.dart';

/// Records the callbacks fired by [CustomTouchGestureRecognizer].
class _Recorder {
  int oneFingerStart = 0;
  int oneFingerUpdate = 0;
  int oneFingerEnd = 0;
  int twoFingerStart = 0;
  int twoFingerEnd = 0;
  int threeFingerStart = 0;
  int threeFingerUpdate = 0;
  int threeFingerEnd = 0;
  Offset? lastOneFingerStartPos;
}

CustomTouchGestureRecognizer _makeRecognizer(_Recorder recorder) {
  final recognizer = CustomTouchGestureRecognizer();
  recognizer
    ..onOneFingerPanStart = (d) {
      recorder.oneFingerStart++;
      recorder.lastOneFingerStartPos = d.globalPosition;
    }
    ..onOneFingerPanUpdate = (d) {
      recorder.oneFingerUpdate++;
    }
    ..onOneFingerPanEnd = (d) {
      recorder.oneFingerEnd++;
    }
    ..onTwoFingerScaleStart = (d) {
      recorder.twoFingerStart++;
    }
    ..onTwoFingerScaleEnd = (d) {
      recorder.twoFingerEnd++;
    }
    ..onThreeFingerVerticalDragStart = (d) {
      recorder.threeFingerStart++;
    }
    ..onThreeFingerVerticalDragUpdate = (d) {
      recorder.threeFingerUpdate++;
    }
    ..onThreeFingerVerticalDragEnd = (d) {
      recorder.threeFingerEnd++;
    };
  return recognizer;
}

ScaleUpdateDetails _updateDetails({
  required int pointerCount,
  Offset focalPoint = Offset.zero,
}) {
  return ScaleUpdateDetails(
    focalPoint: focalPoint,
    localFocalPoint: focalPoint,
    focalPointDelta: Offset.zero,
    pointerCount: pointerCount,
  );
}

ScaleEndDetails _endDetails({required int pointerCount}) {
  return ScaleEndDetails(pointerCount: pointerCount);
}

void main() {
  group('CustomTouchGestureRecognizer pointer-count transitions', () {
    testWidgets(
        'two fingers to one finger fires the start exactly once during '
        'continuous moves', (tester) async {
      final recorder = _Recorder();
      final recognizer = _makeRecognizer(recorder);

      // Two-finger scale is established.
      recognizer.onUpdate!.call(_updateDetails(pointerCount: 2));
      expect(recorder.twoFingerStart, 1);

      // One finger lifts: the scale recognizer ends with the new count.
      recognizer.onEnd!.call(_endDetails(pointerCount: 1));
      expect(recorder.twoFingerEnd, 1);

      // The remaining finger keeps moving below the 200ms debounce.
      final pos1 = const Offset(10, 10);
      final pos2 = const Offset(20, 20);
      final pos3 = const Offset(30, 30);
      recognizer.onUpdate!
          .call(_updateDetails(pointerCount: 1, focalPoint: pos1));
      await tester.pump(const Duration(milliseconds: 50));
      recognizer.onUpdate!
          .call(_updateDetails(pointerCount: 1, focalPoint: pos2));
      await tester.pump(const Duration(milliseconds: 50));
      recognizer.onUpdate!
          .call(_updateDetails(pointerCount: 1, focalPoint: pos3));
      await tester.pump(const Duration(milliseconds: 50));
      // The debounce deadline is still running (150ms < 200ms).
      expect(recorder.oneFingerStart, 0);

      // The deadline is measured from the first count change, not refreshed.
      await tester.pump(const Duration(milliseconds: 100));
      expect(recorder.oneFingerStart, 1);
      expect(recorder.lastOneFingerStartPos, const Offset(30, 30));

      // It must not fire a second time.
      await tester.pump(const Duration(milliseconds: 200));
      expect(recorder.oneFingerStart, 1);

      // Subsequent moves flow to the one-finger state.
      recognizer.onUpdate!.call(_updateDetails(pointerCount: 1));
      expect(recorder.oneFingerUpdate, 1);

      recognizer.dispose();
    });

    testWidgets(
        'three fingers to two fingers fires the start exactly once during '
        'continuous moves', (tester) async {
      final recorder = _Recorder();
      final recognizer = _makeRecognizer(recorder);

      // Three-finger drag is established.
      recognizer.onUpdate!.call(_updateDetails(pointerCount: 3));
      expect(recorder.threeFingerStart, 1);

      // One finger lifts.
      recognizer.onEnd!.call(_endDetails(pointerCount: 2));
      expect(recorder.threeFingerEnd, 1);

      // The remaining two fingers keep moving below the debounce.
      recognizer.onUpdate!.call(_updateDetails(pointerCount: 2));
      await tester.pump(const Duration(milliseconds: 50));
      recognizer.onUpdate!.call(_updateDetails(pointerCount: 2));
      await tester.pump(const Duration(milliseconds: 50));
      recognizer.onUpdate!.call(_updateDetails(pointerCount: 2));
      await tester.pump(const Duration(milliseconds: 50));
      expect(recorder.twoFingerStart, 0);

      await tester.pump(const Duration(milliseconds: 100));
      expect(recorder.twoFingerStart, 1);
      await tester.pump(const Duration(milliseconds: 200));
      expect(recorder.twoFingerStart, 1);

      recognizer.dispose();
    });

    testWidgets(
        'a pending one-finger start is cancelled when two fingers return',
        (tester) async {
      final recorder = _Recorder();
      final recognizer = _makeRecognizer(recorder);

      recognizer.onUpdate!.call(_updateDetails(pointerCount: 2));
      recognizer.onEnd!.call(_endDetails(pointerCount: 1));
      recognizer.onUpdate!.call(_updateDetails(pointerCount: 1));
      await tester.pump(const Duration(milliseconds: 50));

      // The second finger returns before the debounce elapses; the ended
      // two-finger gesture restarts instead of resuming silently.
      recognizer.onUpdate!.call(_updateDetails(pointerCount: 2));
      await tester.pump(const Duration(milliseconds: 300));

      expect(recorder.oneFingerStart, 0);
      expect(recorder.twoFingerStart, 2);

      recognizer.dispose();
    });

    testWidgets('onEnd cancels a pending start so no ghost gesture fires',
        (tester) async {
      final recorder = _Recorder();
      final recognizer = _makeRecognizer(recorder);

      recognizer.onUpdate!.call(_updateDetails(pointerCount: 2));
      recognizer.onEnd!.call(_endDetails(pointerCount: 1));
      recognizer.onUpdate!.call(_updateDetails(pointerCount: 1));

      // All pointers are gone before the debounce elapses.
      recognizer.onEnd!.call(_endDetails(pointerCount: 0));
      await tester.pump(const Duration(milliseconds: 300));

      expect(recorder.oneFingerStart, 0);
      expect(recorder.oneFingerEnd, 0);
      expect(recorder.twoFingerEnd, 1);

      recognizer.dispose();
    });

    testWidgets('four or more fingers are treated as the three-finger drag',
        (tester) async {
      final recorder = _Recorder();
      final recognizer = _makeRecognizer(recorder);

      // From none, four fingers start the three-finger drag immediately.
      recognizer.onUpdate!.call(_updateDetails(pointerCount: 4));
      expect(recorder.threeFingerStart, 1);

      // Adding a fifth finger must not restart the gesture.
      recognizer.onUpdate!.call(_updateDetails(pointerCount: 5));
      expect(recorder.threeFingerStart, 1);
      expect(recorder.threeFingerUpdate, 2);

      recognizer.dispose();
    });

    testWidgets('one finger from none starts immediately and ends',
        (tester) async {
      final recorder = _Recorder();
      final recognizer = _makeRecognizer(recorder);

      final pos = const Offset(5, 5);
      recognizer.onUpdate!
          .call(_updateDetails(pointerCount: 1, focalPoint: pos));
      expect(recorder.oneFingerStart, 1);
      expect(recorder.lastOneFingerStartPos, pos);

      recognizer.onUpdate!.call(_updateDetails(pointerCount: 1));
      expect(recorder.oneFingerUpdate, 2);

      recognizer.onEnd!.call(_endDetails(pointerCount: 0));
      expect(recorder.oneFingerEnd, 1);

      recognizer.dispose();
    });

    testWidgets(
        'a second one-finger gesture within 200ms starts again and ends '
        'normally', (tester) async {
      final recorder = _Recorder();
      final recognizer = _makeRecognizer(recorder);

      recognizer.onUpdate!.call(_updateDetails(pointerCount: 1));
      expect(recorder.oneFingerStart, 1);

      // All fingers up; the state reset is deferred by 200ms.
      recognizer.onEnd!.call(_endDetails(pointerCount: 0));
      expect(recorder.oneFingerEnd, 1);

      // A fresh one-finger gesture begins before the reset deadline elapses.
      recognizer.onUpdate!.call(_updateDetails(pointerCount: 1));
      expect(recorder.oneFingerStart, 2);
      // The start frame also delivers an update.
      expect(recorder.oneFingerUpdate, 2);

      recognizer.onEnd!.call(_endDetails(pointerCount: 0));
      expect(recorder.oneFingerEnd, 2);

      await tester.pump(const Duration(milliseconds: 300));
      recognizer.dispose();
    });

    testWidgets(
        'a second onEnd replaces the reset deadline instead of leaving a '
        'stale timer', (tester) async {
      final recorder = _Recorder();
      final recognizer = _makeRecognizer(recorder);

      // First gesture: start and end.
      recognizer.onUpdate!.call(_updateDetails(pointerCount: 1));
      expect(recorder.oneFingerStart, 1);
      recognizer.onEnd!.call(_endDetails(pointerCount: 0));
      expect(recorder.oneFingerEnd, 1);

      // A second onEnd 50ms later must not double-fire the end callback and
      // re-arms the reset timer (deadline t=250) instead of stacking a second
      // one with the original deadline (t=200).
      await tester.pump(const Duration(milliseconds: 50));
      recognizer.onEnd!.call(_endDetails(pointerCount: 0));
      expect(recorder.oneFingerEnd, 1);

      // The gesture restarts 50ms later, before either deadline.
      await tester.pump(const Duration(milliseconds: 50));
      recognizer.onUpdate!.call(_updateDetails(pointerCount: 1));
      expect(recorder.oneFingerStart, 2);

      // Past the first timer's original deadline (t=200) but before the
      // replacement deadline (t=250): the stale timer must not fire.
      await tester.pump(const Duration(milliseconds: 110));
      recognizer.onUpdate!.call(_updateDetails(pointerCount: 1));
      expect(recorder.oneFingerStart, 2);
      expect(recorder.oneFingerEnd, 1);

      // The restarted gesture ends normally.
      recognizer.onEnd!.call(_endDetails(pointerCount: 0));
      expect(recorder.oneFingerEnd, 2);

      await tester.pump(const Duration(milliseconds: 300));
      recognizer.dispose();
    });
  });
}
