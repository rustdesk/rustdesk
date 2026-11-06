import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/models/input_modifier_utils.dart';

void main() {
  group('shouldReleaseStaleMobileShift', () {
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
  });

  group('shouldApplyTerminalInputModifiers', () {
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

    test('keeps decomposed graphemes intact with locked keyboard modifiers',
        () {
      const decomposedEAcute = 'é';

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

  group('shouldHandleTerminalPasteShortcut', () {
    test('handles only Ctrl+Shift+V on Linux with a virtual lock', () {
      expect(
        shouldHandleTerminalPasteShortcut(
          platform: TargetPlatform.linux,
          logicalKey: LogicalKeyboardKey.keyV,
          isKeyDown: true,
          isKeyRepeat: false,
          controlPressed: true,
          metaPressed: false,
          altPressed: false,
          shiftPressed: true,
          modifierLockActive: true,
        ),
        isTrue,
      );
      expect(
        shouldHandleTerminalPasteShortcut(
          platform: TargetPlatform.linux,
          logicalKey: LogicalKeyboardKey.keyV,
          isKeyDown: true,
          isKeyRepeat: false,
          controlPressed: true,
          metaPressed: false,
          altPressed: false,
          shiftPressed: false,
          modifierLockActive: true,
        ),
        isFalse,
      );
    });

    test('handles Ctrl+V and Meta+V when a virtual modifier lock is active',
        () {
      expect(
        shouldHandleTerminalPasteShortcut(
          platform: TargetPlatform.windows,
          logicalKey: LogicalKeyboardKey.keyV,
          isKeyDown: true,
          isKeyRepeat: false,
          controlPressed: true,
          metaPressed: false,
          altPressed: false,
          shiftPressed: false,
          modifierLockActive: true,
        ),
        isTrue,
      );
      expect(
        shouldHandleTerminalPasteShortcut(
          platform: TargetPlatform.macOS,
          logicalKey: LogicalKeyboardKey.keyV,
          isKeyDown: true,
          isKeyRepeat: false,
          controlPressed: false,
          metaPressed: true,
          altPressed: false,
          shiftPressed: false,
          modifierLockActive: true,
        ),
        isTrue,
      );
    });
  });

  group('TerminalImeCommitFilter', () {
    test('keeps one copy of a commit the backend resends', () {
      // A wide window keeps the assertion independent of scheduler jitter.
      final filter = TerminalImeCommitFilter(
        enabled: true,
        window: const Duration(seconds: 1),
      );

      expect(filter.isRepeatedCommit('你好'), isFalse);
      expect(filter.isRepeatedCommit('你好'), isTrue);
      expect(filter.isRepeatedCommit('你好'), isTrue);
    });

    test('keeps a different commit and hardware input', () {
      final filter = TerminalImeCommitFilter(enabled: true);

      expect(filter.isRepeatedCommit('你好'), isFalse);
      expect(filter.isRepeatedCommit('世界'), isFalse);
      // Hardware keys arrive one character at a time, even when held down.
      expect(filter.isRepeatedCommit('l'), isFalse);
      expect(filter.isRepeatedCommit('l'), isFalse);
      expect(filter.isRepeatedCommit('s'), isFalse);
    });

    test('keeps one copy of an English word a Chinese IME commits', () {
      final filter = TerminalImeCommitFilter(
        enabled: true,
        window: const Duration(seconds: 1),
      );

      expect(filter.isRepeatedCommit('hello'), isFalse);
      expect(filter.isRepeatedCommit('hello'), isTrue);
      expect(filter.isRepeatedCommit('hello'), isTrue);
    });

    test('keeps a commit the user repeats after the resend window', () async {
      final filter = TerminalImeCommitFilter(
        enabled: true,
        window: const Duration(milliseconds: 10),
      );

      expect(filter.isRepeatedCommit('。'), isFalse);
      await Future.delayed(const Duration(milliseconds: 30));
      expect(filter.isRepeatedCommit('。'), isFalse);
    });

    test('keeps every repeated mouse report', () {
      final filter = TerminalImeCommitFilter(enabled: true);
      // Normal-encoding report for column 100, where the encoded coordinate is
      // no longer ASCII. Scrolling sends one such report per line.
      final report = '\x1B[M${String.fromCharCode(32)}'
          '${String.fromCharCode(132)}${String.fromCharCode(42)}';

      expect(filter.isRepeatedCommit(report), isFalse);
      expect(filter.isRepeatedCommit(report), isFalse);
      expect(filter.isRepeatedCommit(report), isFalse);
    });

    test('does not filter when the platform is not affected', () {
      final filter = TerminalImeCommitFilter(enabled: false);

      expect(filter.isRepeatedCommit('你好'), isFalse);
      expect(filter.isRepeatedCommit('你好'), isFalse);
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
  });
}
