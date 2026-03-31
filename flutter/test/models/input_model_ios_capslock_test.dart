import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/models/ios_caps_lock_state_tracker.dart';

void main() {
  group('IosCapsLockStateTracker', () {
    test('preserves cached caps lock state for non-character events', () {
      final tracker = IosCapsLockStateTracker();

      expect(
        tracker.update(
          character: null,
          shiftPressed: false,
          logicalKey: LogicalKeyboardKey.capsLock,
          isKeyDown: true,
        ),
        isTrue,
      );

      expect(
        tracker.update(
          character: 'A',
          shiftPressed: false,
          logicalKey: LogicalKeyboardKey.keyA,
          isKeyDown: true,
        ),
        isTrue,
      );

      expect(
        tracker.update(
          character: null,
          shiftPressed: false,
          logicalKey: LogicalKeyboardKey.keyA,
          isKeyDown: false,
        ),
        isTrue,
      );
    });

    test('does not clear cached caps lock state when shift is pressed', () {
      final tracker = IosCapsLockStateTracker();

      tracker.update(
        character: null,
        shiftPressed: false,
        logicalKey: LogicalKeyboardKey.capsLock,
        isKeyDown: true,
      );

      expect(
        tracker.update(
          character: 'A',
          shiftPressed: true,
          logicalKey: LogicalKeyboardKey.keyA,
          isKeyDown: true,
        ),
        isTrue,
      );

      expect(
        tracker.update(
          character: null,
          shiftPressed: false,
          logicalKey: LogicalKeyboardKey.keyA,
          isKeyDown: false,
        ),
        isTrue,
      );
    });
  });
}
