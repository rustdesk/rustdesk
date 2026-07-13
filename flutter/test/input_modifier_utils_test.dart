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
          hasTrackedShiftKeyDown: true,
        ),
        isTrue,
      );
    });

    test('does not release manually toggled shift without tracked key down',
        () {
      expect(
        shouldReleaseStaleMobileShift(
          isMobile: true,
          cachedShiftPressed: true,
          actualShiftPressed: false,
          logicalKey: LogicalKeyboardKey.keyD,
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
          hasTrackedShiftKeyDown: true,
        ),
        isFalse,
      );
    });

    test('releases on enter key', () {
      expect(
        shouldReleaseStaleMobileShift(
          isMobile: true,
          cachedShiftPressed: true,
          actualShiftPressed: false,
          logicalKey: LogicalKeyboardKey.enter,
          hasTrackedShiftKeyDown: true,
        ),
        isTrue,
      );
    });

    test('releases on arrow key', () {
      expect(
        shouldReleaseStaleMobileShift(
          isMobile: true,
          cachedShiftPressed: true,
          actualShiftPressed: false,
          logicalKey: LogicalKeyboardKey.arrowLeft,
          hasTrackedShiftKeyDown: true,
        ),
        isTrue,
      );
    });

    test('does not release on modifier events', () {
      expect(
        shouldReleaseStaleMobileShift(
          isMobile: true,
          cachedShiftPressed: true,
          actualShiftPressed: false,
          logicalKey: LogicalKeyboardKey.shiftLeft,
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
          hasTrackedShiftKeyDown: true,
        ),
        isFalse,
      );
    });
  });

  group('applyTerminalInputModifiers', () {
    test('maps Ctrl underscore to unit separator', () {
      expect(
        applyTerminalInputModifiers(
          '_',
          ctrlLocked: true,
          altLocked: false,
        ),
        '\x1F',
      );
    });

    test('keeps pasted single characters unchanged when modifiers are skipped',
        () {
      expect(
        applyTerminalInputModifiers(
          'd',
          ctrlLocked: true,
          altLocked: true,
          applyModifiers: false,
        ),
        'd',
      );
    });
  });

  group('terminalPastePayload', () {
    test('wraps paste text when bracketed paste mode is active', () {
      expect(
        terminalPastePayload('d', bracketedPasteMode: true),
        '\x1B[200~d\x1B[201~',
      );
    });
  });

  group('shouldClearTerminalModifiersWhenRow3Collapses', () {
    test('clears visible modifier state when expanded row is collapsed', () {
      expect(
        shouldClearTerminalModifiersWhenRow3Collapses(
          wasExpanded: true,
          willExpand: false,
          ctrlLocked: true,
          altLocked: false,
        ),
        isTrue,
      );
    });

    test('does not clear modifiers when row expands', () {
      expect(
        shouldClearTerminalModifiersWhenRow3Collapses(
          wasExpanded: false,
          willExpand: true,
          ctrlLocked: true,
          altLocked: true,
        ),
        isFalse,
      );
    });
  });
}
