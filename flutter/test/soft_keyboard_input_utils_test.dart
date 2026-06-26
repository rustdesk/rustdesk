import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/models/soft_keyboard_input_utils.dart';

void main() {
  group('computeSoftKeyboardEdit', () {
    test('1 in-place jamo build ㅁ->마 sends backspace+replacement', () {
      expect(computeSoftKeyboardEdit('1ㅁ', '1마'),
          (backspaces: 1, insert: '마'));
    });
    test('2 in-place jamo build 마->만', () {
      expect(computeSoftKeyboardEdit('1마', '1만'),
          (backspaces: 1, insert: '만'));
    });
    test('3 new syllable append ㄴ', () {
      expect(computeSoftKeyboardEdit('1만', '1만ㄴ'),
          (backspaces: 0, insert: 'ㄴ'));
    });
    test('4 syllable build 만ㄴ->만나', () {
      expect(computeSoftKeyboardEdit('1만ㄴ', '1만나'),
          (backspaces: 1, insert: '나'));
    });
    test('5 first jamo of buffer', () {
      expect(computeSoftKeyboardEdit('1', '1ㅇ'),
          (backspaces: 0, insert: 'ㅇ'));
    });
    test('6 batchim delete 만->마', () {
      expect(computeSoftKeyboardEdit('1만', '1마'),
          (backspaces: 1, insert: '마'));
    });
    test('7 multi-unit delete sends N backspaces', () {
      expect(computeSoftKeyboardEdit('1가나', '1'),
          (backspaces: 2, insert: ''));
    });
    test('8 single delete', () {
      expect(computeSoftKeyboardEdit('1가', '1'),
          (backspaces: 1, insert: ''));
    });
    test('9 bracket pair append', () {
      expect(computeSoftKeyboardEdit('1', '1()'),
          (backspaces: 0, insert: '()'));
    });
    test('10 non-pair multi-char append', () {
      expect(computeSoftKeyboardEdit('1', '1ab'),
          (backspaces: 0, insert: 'ab'));
    });
  });

  group('clipboardAdjustedOldValue', () {
    test('11 paste replaces sentinel -> empty old', () {
      expect(clipboardAdjustedOldValue('1111', 'hello'), '');
    });
    test('12 normal typing keeps old', () {
      expect(clipboardAdjustedOldValue('1만', '1마'), '1만');
    });
    test('13 empty old does not throw RangeError', () {
      expect(clipboardAdjustedOldValue('', '1ㅁ'), '');
    });
    test('14 empty new does not throw RangeError', () {
      expect(clipboardAdjustedOldValue('1111', ''), '1111');
    });
  });

  group('isAutoInsertedBracketPair', () {
    test('15 pair appended onto non-empty buffer', () {
      expect(isAutoInsertedBracketPair('1', 0, '()'), true);
    });
    test('16 not a known pair', () {
      expect(isAutoInsertedBracketPair('1', 0, 'ab'), false);
    });
    test('17 empty old (post-clipboard) is not auto-pair', () {
      expect(isAutoInsertedBracketPair('', 0, '()'), false);
    });
    test('18 backspaces present is not a host auto-pair', () {
      expect(isAutoInsertedBracketPair('1', 1, '()'), false);
    });
  });
}
