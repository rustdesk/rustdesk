// Records what the handlers that used to live in `remote_page.dart` do, the
// failures included. These tests pin behavior; they do not endorse it.
//
// All sequences here are written by hand. They show what the code does with a
// given sequence, not that a keyboard produces that sequence.

import 'package:flutter_test/flutter_test.dart';

import 'soft_keyboard_test_support.dart';

void main() {
  group('legacy iOS handler', () {
    test('sends plain typing one key at a time', () {
      expect(
          typed(legacyIos(), ['a', 'ab', 'ab ', 'ab c']), ['a', 'b', ' ', 'c']);
    });

    test('replays a replaced tail', () {
      expect(typed(legacyIos(), ['t', 'te', 'teh', 'the ']),
          ['t', 'e', 'h', '<BS*2>', 'he ']);
    });

    test('sends Backspace for deleted text and for a deleted sentinel', () {
      final ios = legacyIos();
      expect(typed(ios, ['ab', 'a', '']), ['ab', '<BS>', '<BS>']);
      expect(sent(ios, [ev('', lead: 1023)]), ['<BS>']);
    });

    test('still sends the digit 1 after the sentinel got shorter', () {
      final ios = legacyIos();
      expect(
          sent(ios, [ev('', lead: 1023), ev('', lead: 1024)]), ['<BS>', '1']);
    });

    test('leaks the first letter of a composition (#16632)', () {
      expect(
          typed(
              legacyIos(), ['[n]', '[ni]', '[nih]', '[niha]', '[nihao]', '你好']),
          ['n', '<BS>', '你好']);
    });

    test('leaks the rest of a composition on a partial commit', () {
      expect(typed(legacyIos(), ['[n]', '[nihao]', '你[hao]', '你好']),
          ['n', '<BS>', '你hao', '<BS*3>', '好']);
    });

    test('leaks and retracts the first letter of a cancelled composition', () {
      expect(typed(legacyIos(), ['[n]', '[ni]', '']), ['n', '<BS>']);
    });

    test('misses a commit that does not change the text', () {
      // Only the first kana ever arrives.
      expect(typed(legacyIos(), ['[あ]', '[あり]', '[ありがとう]', 'ありがとう']), ['あ']);
    });

    test('sends two Backspaces for one deleted emoji', () {
      expect(typed(legacyIos(), ['😀', '']), ['😀', '<BS*2>']);
    });
  });

  group('legacy Android handler', () {
    test('sends plain typing one key at a time', () {
      expect(typed(legacyAndroid(), ['a', 'ab', 'ab ', 'ab c']),
          ['a', 'b', ' ', 'c']);
    });

    test('sends text committed at once as a string', () {
      expect(typed(legacyAndroid(), ['你好']), ['你好']);
    });

    test('sends one Backspace for any deletion', () {
      final android = legacyAndroid();
      expect(typed(android, ['ab', 'a']), ['ab', '<BS>']);
      expect(sent(android, [ev(''), ev('', lead: 1023)]), ['<BS>', '<BS>']);
    });

    test('ignores a replacement of equal length (#7727)', () {
      // The syllable never arrives, only its first jamo.
      expect(typed(legacyAndroid(), ['ㅁ', '마', '만']), ['ㅁ']);
    });

    test('sends one Backspace when a composition gets shorter on commit', () {
      // The converted text never arrives.
      expect(typed(legacyAndroid(), ['n', 'ni', 'nih', 'niha', 'nihao', '你好']),
          ['n', 'i', 'h', 'a', 'o', '<BS>']);
    });

    test('sends the wrong text for an insertion that is not at the end', () {
      expect(typed(legacyAndroid(), ['“|”', '“你好|”']), ['“”', '好”']);
    });

    test('resets the field after a pair of brackets', () {
      expect(sent(legacyAndroid(), [plain('a'), plain('a()', caret: 2)]),
          ['a', '()', '<RESET>']);
      expect(sent(legacyAndroid(), [plain('[]', caret: 1)]), ['[]', '<RESET>']);
    });

    test('sends the whole text when the sentinel is replaced', () {
      expect(
          sent(legacyAndroid(), [const TextEditingValue(text: 'pasted text')]),
          ['pasted text']);
    });
  });
}
