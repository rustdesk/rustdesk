import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/models/input_focus_gate.dart';

void main() {
  group('shouldBlockUnfocusedMouseInput', () {
    test('never blocks on non-desktop (mobile/web), even when unfocused', () {
      var reads = 0;
      final blocked = shouldBlockUnfocusedMouseInput(
        isDesktop: false,
        isWindowFocused: false,
        isOptionEnabled: () {
          reads++;
          return true;
        },
      );
      expect(blocked, isFalse);
      expect(reads, 0, reason: 'option must not be read when not on desktop');
    });

    test('never blocks while the window is focused, and does not read option',
        () {
      var reads = 0;
      final blocked = shouldBlockUnfocusedMouseInput(
        isDesktop: true,
        isWindowFocused: true,
        isOptionEnabled: () {
          reads++;
          return true;
        },
      );
      expect(blocked, isFalse);
      expect(reads, 0,
          reason: 'focused short-circuit must skip the option read');
    });

    test('blocks only when desktop + unfocused + option enabled', () {
      expect(
        shouldBlockUnfocusedMouseInput(
            isDesktop: true,
            isWindowFocused: false,
            isOptionEnabled: () => true),
        isTrue,
      );
    });

    test('does NOT block when the option is disabled (the default)', () {
      expect(
        shouldBlockUnfocusedMouseInput(
            isDesktop: true,
            isWindowFocused: false,
            isOptionEnabled: () => false),
        isFalse,
      );
    });
  });
}
