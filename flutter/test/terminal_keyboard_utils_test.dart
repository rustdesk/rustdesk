import 'package:flutter_hbb/mobile/terminal_keyboard_utils.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  group('mobile terminal keyboard layout', () {
    test('keeps the latest key order from the reviewed PR layout', () {
      expect(
        terminalKeyboardRow1Keys,
        ['Esc', '/', '|', 'Home', '↑', 'End', r'\'],
      );
      expect(
        terminalKeyboardRow2Keys,
        ['Tab', 'Ctrl+C', '~', '←', '↓', '→'],
      );
      expect(
        terminalKeyboardRow3Keys,
        ['Ctrl', 'Alt', '-', 'PgUp', 'PgDn'],
      );
    });

    test('keeps two trailing Row3 placeholders for row alignment', () {
      expect(terminalKeyboardRow3TrailingPlaceholderCount, 2);
    });

    test('keeps every expanded row aligned at 348dp', () {
      final rowWidths = [
        terminalKeyboardRowWidth(terminalKeyboardRow1Keys.length),
        terminalKeyboardRowWidth(terminalKeyboardRow2Keys.length + 1),
        terminalKeyboardRowWidth(
          terminalKeyboardRow3Keys.length +
              terminalKeyboardRow3TrailingPlaceholderCount,
        ),
      ];

      expect(terminalKeyboardKeyWidth, 48);
      expect(terminalKeyboardKeySpacing, 2);
      expect(rowWidths, everyElement(348));
    });
  });
}
