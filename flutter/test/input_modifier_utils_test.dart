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

  group('shouldApplyTerminalInputModifiers', () {
    test('accepts ordinary single-character keyboard input', () {
      expect(shouldApplyTerminalInputModifiers('a'), isTrue);
      expect(shouldApplyTerminalInputModifiers(' '), isTrue);
      expect(shouldApplyTerminalInputModifiers('/'), isTrue);
    });

    test('accepts supplementary-plane single-character keyboard input', () {
      expect(shouldApplyTerminalInputModifiers('😀'), isTrue);
    });

    test('rejects terminal control bytes and multi-character sequences', () {
      for (final input in ['\x00', '\x03', '\t', '\n', '\r', '\x1B', '\x7F']) {
        expect(
          shouldApplyTerminalInputModifiers(input),
          isFalse,
          reason: '${input.codeUnits} must not consume a one-shot modifier',
        );
      }
      expect(shouldApplyTerminalInputModifiers('\x1B[A'), isFalse);
    });
  });

  group('applyTerminalInputModifiers', () {
    test('keeps decomposed graphemes intact under Ctrl', () {
      const decomposedEAcute = 'e\u0301';

      expect(
        applyTerminalInputModifiers(
          decomposedEAcute,
          ctrlLocked: true,
          altLocked: false,
        ),
        decomposedEAcute,
      );
    });

    test('keeps non-ASCII graphemes intact under Ctrl', () {
      for (final input in ['é', '😀']) {
        expect(
          applyTerminalInputModifiers(
            input,
            ctrlLocked: true,
            altLocked: false,
          ),
          input,
        );
      }
    });

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

    test('maps the complete Ctrl symbol range', () {
      const mappings = {
        '[': '\x1B',
        r'\': '\x1C',
        ']': '\x1D',
        '^': '\x1E',
        '_': '\x1F',
        '/': '\x1F',
      };

      for (final entry in mappings.entries) {
        expect(
          applyTerminalInputModifiers(
            entry.key,
            ctrlLocked: true,
            altLocked: false,
          ),
          entry.value,
          reason: 'Ctrl+${entry.key} should map to ${entry.value.codeUnits}',
        );
      }
    });

    test('applies Ctrl before Alt for combined modifiers', () {
      expect(
        applyTerminalInputModifiers(
          'b',
          ctrlLocked: true,
          altLocked: true,
        ),
        '\x1B\x02',
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

    test('keeps a lone newline unchanged when bracketed paste is disabled', () {
      expect(
        terminalPastePayload('\n', bracketedPasteMode: false),
        '\n',
      );
    });
  });

  group('prepareTerminalInputPayload', () {
    test('normalizes a mobile keyboard Enter to carriage return', () {
      expect(
        prepareTerminalInputPayload(
          '\n',
          source: TerminalInputSource.keyboard,
          isMobileOrWebMobile: true,
          bracketedPasteMode: false,
          ctrlLocked: false,
          altLocked: false,
        ),
        '\r',
      );
    });

    test('keeps Ctrl+J as line feed on mobile', () {
      expect(
        prepareTerminalInputPayload(
          'j',
          source: TerminalInputSource.keyboard,
          isMobileOrWebMobile: true,
          bracketedPasteMode: false,
          ctrlLocked: true,
          altLocked: false,
        ),
        '\n',
      );
    });

    test('does not apply Alt to a terminal control byte', () {
      expect(
        prepareTerminalInputPayload(
          '\x1B',
          source: TerminalInputSource.keyboard,
          isMobileOrWebMobile: true,
          bracketedPasteMode: false,
          ctrlLocked: false,
          altLocked: true,
        ),
        '\x1B',
      );
    });

    test('keeps large keyboard payloads unchanged when modifiers are inactive',
        () {
      final payload = 'd' * (1024 * 1024);

      expect(
        prepareTerminalInputPayload(
          payload,
          source: TerminalInputSource.keyboard,
          isMobileOrWebMobile: false,
          bracketedPasteMode: false,
          ctrlLocked: false,
          altLocked: false,
        ),
        payload,
      );
    });

    test('keeps decomposed graphemes intact with locked keyboard modifiers',
        () {
      const decomposedEAcute = 'e\u0301';

      expect(
        prepareTerminalInputPayload(
          decomposedEAcute,
          source: TerminalInputSource.keyboard,
          isMobileOrWebMobile: true,
          bracketedPasteMode: false,
          ctrlLocked: true,
          altLocked: false,
        ),
        decomposedEAcute,
      );
    });

    test('preserves a lone pasted newline when modifiers are locked', () {
      expect(
        prepareTerminalInputPayload(
          '\n',
          source: TerminalInputSource.paste,
          isMobileOrWebMobile: true,
          bracketedPasteMode: false,
          ctrlLocked: true,
          altLocked: true,
        ),
        '\n',
      );
    });

    test('wraps paste without applying locked modifiers', () {
      expect(
        prepareTerminalInputPayload(
          'd',
          source: TerminalInputSource.paste,
          isMobileOrWebMobile: true,
          bracketedPasteMode: true,
          ctrlLocked: true,
          altLocked: true,
        ),
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

    test('clears Alt state when expanded row is collapsed', () {
      expect(
        shouldClearTerminalModifiersWhenRow3Collapses(
          wasExpanded: true,
          willExpand: false,
          ctrlLocked: false,
          altLocked: true,
        ),
        isTrue,
      );
    });
  });
}
