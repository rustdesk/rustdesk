import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/mobile/soft_keyboard_input.dart';

void main() {
  final prefix = '1' * 1024;

  test('replaces an equal-length pinyin initial with a selected Hanzi', () {
    expect(
      getNonIOSSoftKeyboardEdit('${prefix}w', '$prefix我'),
      (backspaces: 1, text: '我'),
    );
  });

  test('removes every pinyin letter before inserting a selected Hanzi', () {
    expect(
      getNonIOSSoftKeyboardEdit('${prefix}ni', '$prefix你'),
      (backspaces: 2, text: '你'),
    );
  });

  test('removes every deleted character', () {
    expect(
      getNonIOSSoftKeyboardEdit('${prefix}abc', '${prefix}a'),
      (backspaces: 2, text: ''),
    );
  });

  test('replaces an emoji as one character', () {
    expect(
      getNonIOSSoftKeyboardEdit('$prefix😀', '$prefix😁'),
      (backspaces: 1, text: '😁'),
    );
  });

  test('appends ordinary input without deleting existing text', () {
    expect(
      getNonIOSSoftKeyboardEdit('${prefix}a', '${prefix}ab'),
      (backspaces: 0, text: 'b'),
    );
  });

  test('does nothing when the input method repeats the same value', () {
    expect(
      getNonIOSSoftKeyboardEdit('${prefix}a', '${prefix}a'),
      (backspaces: 0, text: ''),
    );
  });
}
