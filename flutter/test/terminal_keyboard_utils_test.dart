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
  });
}
