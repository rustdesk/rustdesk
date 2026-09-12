import 'dart:async';

import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';
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

    testWidgets('single taps work after a double tap wins the gesture arena',
        (tester) async {
      final tracker = TouchModeGestureTracker();
      final callbacks = <String>[];
      var clicks = 0;
      await tester.pumpWidget(Directionality(
        textDirection: TextDirection.ltr,
        child: Listener(
          onPointerDown: (event) => tracker.pointerDown(event.pointer),
          onPointerUp: (event) => tracker.pointerEnd(event.pointer),
          child: GestureDetector(
            behavior: HitTestBehavior.opaque,
            onTapDown: (_) {
              callbacks.add('down');
              tracker.recordTapDown();
            },
            onTapCancel: () {
              callbacks.add('cancel');
              tracker.takeNextTapDown();
            },
            onTapUp: (_) async {
              final sequence = tracker.takeNextTapDown();
              if (sequence == null) return;
              await handleTrackedTap(
                tracker: tracker,
                sequence: sequence,
                move: () async => true,
                sendTap: () async => clicks++,
              );
            },
            onDoubleTap: () {
              callbacks.add('double');
              tracker.clearTapDowns();
            },
            child: const SizedBox.expand(),
          ),
        ),
      ));

      final position = tester.getCenter(find.byType(GestureDetector));
      // Hold each contact past the tap-down deadline. Flutter can then emit
      // both tap-downs before the double-tap recognizer cancels only one tap.
      for (var i = 0; i < 2; i++) {
        final gesture = await tester.startGesture(position);
        await tester.pump(kPressTimeout + const Duration(milliseconds: 1));
        await gesture.up();
        await tester.pump(kDoubleTapMinTime);
      }
      expect(callbacks, ['down', 'down', 'cancel', 'double']);
      expect(clicks, 0);

      // Both subsequent singles must be delivered; one successful tap is not
      // enough to detect a queue that remains permanently one sequence behind.
      for (var i = 1; i <= 2; i++) {
        await tester.tapAt(position);
        await tester.pump(kDoubleTapTimeout + const Duration(milliseconds: 1));
        expect(clicks, i);
      }
    });
  });
}
