import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/models/input_modifier_utils.dart';

void main() {
  group('shouldReleaseStaleMobileShift', () {
    test('does not release when cached shift is already false', () {
      expect(
        shouldReleaseStaleMobileShift(
          isMobile: true,
          cachedShiftPressed: false,
          actualShiftPressed: false,
          logicalKey: LogicalKeyboardKey.keyD,
          character: 'D',
          hasTrackedShiftKeyDown: true,
        ),
        isFalse,
      );
    });

    test('releases one-shot mobile shift after a text key', () {
      expect(
        shouldReleaseStaleMobileShift(
          isMobile: true,
          cachedShiftPressed: true,
          actualShiftPressed: false,
          logicalKey: LogicalKeyboardKey.keyD,
          character: 'D',
          hasTrackedShiftKeyDown: true,
        ),
        isTrue,
      );
    });

    test('does not release manually toggled shift without tracked key down', () {
      expect(
        shouldReleaseStaleMobileShift(
          isMobile: true,
          cachedShiftPressed: true,
          actualShiftPressed: false,
          logicalKey: LogicalKeyboardKey.keyD,
          character: 'D',
          hasTrackedShiftKeyDown: false,
        ),
        isFalse,
      );
    });

    test('does not release when shift is still physically pressed', () {
      expect(
        shouldReleaseStaleMobileShift(
          isMobile: true,
          cachedShiftPressed: true,
          actualShiftPressed: true,
          logicalKey: LogicalKeyboardKey.keyD,
          character: 'D',
          hasTrackedShiftKeyDown: true,
        ),
        isFalse,
      );
    });

    test('does not release on non-mobile platforms', () {
      expect(
        shouldReleaseStaleMobileShift(
          isMobile: false,
          cachedShiftPressed: true,
          actualShiftPressed: false,
          logicalKey: LogicalKeyboardKey.keyD,
          character: 'D',
          hasTrackedShiftKeyDown: true,
        ),
        isFalse,
      );
    });

    test('does not release with empty character', () {
      expect(
        shouldReleaseStaleMobileShift(
          isMobile: true,
          cachedShiftPressed: true,
          actualShiftPressed: false,
          logicalKey: LogicalKeyboardKey.keyD,
          character: '',
          hasTrackedShiftKeyDown: true,
        ),
        isFalse,
      );
    });

    test('does not release with null character on non-modifier keys', () {
      expect(
        shouldReleaseStaleMobileShift(
          isMobile: true,
          cachedShiftPressed: true,
          actualShiftPressed: false,
          logicalKey: LogicalKeyboardKey.keyD,
          character: null,
          hasTrackedShiftKeyDown: true,
        ),
        isFalse,
      );
    });

    test('does not release on modifier events', () {
      expect(
        shouldReleaseStaleMobileShift(
          isMobile: true,
          cachedShiftPressed: true,
          actualShiftPressed: false,
          logicalKey: LogicalKeyboardKey.shiftLeft,
          character: null,
          hasTrackedShiftKeyDown: true,
        ),
        isFalse,
      );
    });

    test('does not release on shiftRight modifier events', () {
      expect(
        shouldReleaseStaleMobileShift(
          isMobile: true,
          cachedShiftPressed: true,
          actualShiftPressed: false,
          logicalKey: LogicalKeyboardKey.shiftRight,
          character: null,
          hasTrackedShiftKeyDown: true,
        ),
        isFalse,
      );
    });
  });
}
