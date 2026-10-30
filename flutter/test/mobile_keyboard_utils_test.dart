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

    test('correctly commits Chinese candidate after composing ends', () {
      // During composing, oldValue remains init because premature keystrokes are suppressed.
      // When composing ends, newValue contains the committed Chinese string.
      final diff = resolveMobileSoftKeyboardDiff(
        oldValue: init,
        newValue: '${init}你好',
        isComposing: false,
      );
      expect(diff, isNotNull);
      expect(diff!.backspaceCount, 0);
      expect(diff.newStr, '你好');
      expect(diff.isStringInput, isTrue);
    });

    test('correctly handles candidate word replacing previous text', () {
      final diff = resolveMobileSoftKeyboardDiff(
        oldValue: '${init}苹果',
        newValue: '${init}香蕉',
        isComposing: false,
      );
      expect(diff, isNotNull);
      expect(diff!.backspaceCount, 2);
      expect(diff.newStr, '香蕉');
      expect(diff.isStringInput, isTrue);
    });

    test('avoids sending backspaces when clipboard paste replaces sentinel buffer', () {
      final diff = resolveMobileSoftKeyboardDiff(
        oldValue: '${init}x',
        newValue: 'paste from clipboard',
        isComposing: false,
      );
      expect(diff, isNotNull);
      expect(diff!.backspaceCount, 0);
      expect(diff.newStr, 'paste from clipboard');
      expect(diff.isStringInput, isTrue);
    });

    test('counts single emoji deletion as one backspace', () {
      final diff = resolveMobileSoftKeyboardDiff(
        oldValue: '${init}a😀',
        newValue: '${init}a',
        isComposing: false,
      );
      expect(diff, isNotNull);
      expect(diff!.backspaceCount, 1);
      expect(diff.newStr, '');
      expect(diff.isStringInput, isFalse);
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
