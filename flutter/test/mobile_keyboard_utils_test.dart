import 'package:flutter_hbb/mobile/mobile_keyboard_utils.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  final init = '1' * 1024;

  group('resolveMobileSoftKeyboardDiff', () {
    test('returns null when IME is composing to suppress premature keystrokes', () {
      final diff = resolveMobileSoftKeyboardDiff(
        oldValue: init,
        newValue: '${init}nihao',
        isComposing: true,
      );
      expect(diff, isNull);
    });

    test('correctly commits Chinese candidate replacing composing pinyin', () {
      final diff = resolveMobileSoftKeyboardDiff(
        oldValue: '${init}nihao',
        newValue: '${init}你好',
        isComposing: false,
      );
      expect(diff, isNotNull);
      expect(diff!.backspaceCount, 5);
      expect(diff.newStr, '你好');
      expect(diff.isStringInput, isTrue);
    });

    test('treats single Chinese character as string sequence rather than physical key', () {
      final diff = resolveMobileSoftKeyboardDiff(
        oldValue: init,
        newValue: '${init}中',
        isComposing: false,
      );
      expect(diff, isNotNull);
      expect(diff!.backspaceCount, 0);
      expect(diff.newStr, '中');
      expect(diff.isStringInput, isTrue);
    });

    test('treats ASCII character as normal key event', () {
      final diff = resolveMobileSoftKeyboardDiff(
        oldValue: init,
        newValue: '${init}a',
        isComposing: false,
      );
      expect(diff, isNotNull);
      expect(diff!.backspaceCount, 0);
      expect(diff.newStr, 'a');
      expect(diff.isStringInput, isFalse);
    });

    test('handles backspace deletion correctly', () {
      final diff = resolveMobileSoftKeyboardDiff(
        oldValue: '${init}ab',
        newValue: '${init}a',
        isComposing: false,
      );
      expect(diff, isNotNull);
      expect(diff!.backspaceCount, 1);
      expect(diff.newStr, '');
      expect(diff.isStringInput, isFalse);
    });
  });
}
