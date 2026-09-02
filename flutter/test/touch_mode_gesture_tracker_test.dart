import 'package:flutter_hbb/common/widgets/touch_mode_gesture_tracker.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  group('TouchModeGestureTracker', () {
    test('suppresses a late tap after the pan recognizer wins', () {
      final tracker = TouchModeGestureTracker();
      tracker.pointerDown(1);
      final sequence = tracker.sequence;
      tracker.recordTapDown();

      tracker.claimPan();
      tracker.pointerEnd(1);

      expect(tracker.takeCurrentTapDown(sequence), isTrue);
      expect(tracker.shouldHandleTap(sequence), isFalse);
    });

    test('a new physical touch invalidates cached gesture state', () {
      final tracker = TouchModeGestureTracker();
      tracker.pointerDown(1);
      final firstSequence = tracker.sequence;
      tracker.recordTapDown();
      tracker.recordLongPress();
      tracker.pointerEnd(1);

      tracker.pointerDown(2);
      final secondSequence = tracker.sequence;

      expect(secondSequence, firstSequence + 1);
      expect(tracker.takeCurrentTapDown(secondSequence), isFalse);
      expect(tracker.isCurrentLongPress(secondSequence), isFalse);
      expect(tracker.shouldHandleTap(secondSequence), isTrue);
    });

    test('additional pointers stay in the same touch sequence', () {
      final tracker = TouchModeGestureTracker();
      tracker.pointerDown(1);
      final sequence = tracker.sequence;

      tracker.pointerDown(2);

      expect(tracker.sequence, sequence);
    });
  });
}
