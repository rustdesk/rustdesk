import 'dart:async';

import 'package:flutter_hbb/common/widgets/touch_mode_gesture_tracker.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  group('TouchModeGestureTracker', () {
    test('suppresses a late tap after the pan recognizer wins', () {
      final tracker = TouchModeGestureTracker();
      tracker.pointerDown(1);
      final sequence = tracker.recordTapDown();

      tracker.claimPan(sequence);
      tracker.pointerEnd(1);

      expect(tracker.takeNextTapDown(), sequence);
      expect(tracker.shouldHandleTap(sequence), isFalse);
    });

    test('a delayed callback does not consume a newer tap-down', () {
      final tracker = TouchModeGestureTracker();
      tracker.pointerDown(1);
      final firstSequence = tracker.recordTapDown();
      tracker.pointerEnd(1);

      tracker.pointerDown(2);
      final secondSequence = tracker.recordTapDown();

      expect(secondSequence, firstSequence + 1);
      expect(tracker.takeNextTapDown(), firstSequence);
      expect(tracker.shouldHandleTap(firstSequence), isFalse);
      expect(tracker.takeNextTapDown(), secondSequence);
      expect(tracker.shouldHandleTap(secondSequence), isTrue);
    });

    test('does not send a tap if a newer touch starts while move is pending',
        () async {
      final tracker = TouchModeGestureTracker();
      tracker.pointerDown(1);
      final firstSequence = tracker.recordTapDown();
      final started = Completer<void>();
      final resume = Completer<void>();
      var clicks = 0;

      final result = handleTrackedTap(
        tracker: tracker,
        sequence: firstSequence,
        move: () async {
          started.complete();
          await resume.future;
          return true;
        },
        sendTap: () async {
          clicks++;
        },
      );

      await started.future;
      tracker.pointerEnd(1);
      tracker.pointerDown(2);
      resume.complete();

      await result;
      expect(clicks, 0);
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
