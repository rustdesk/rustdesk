import 'package:flutter/services.dart';
import 'package:flutter_hbb/mobile/ios_soft_keyboard_input.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  final sentinel = '1' * 1024;

  TextEditingValue value(String suffix, {bool composing = false}) {
    final text = '$sentinel$suffix';
    return TextEditingValue(
      text: text,
      composing: composing
          ? TextRange(start: sentinel.length, end: text.length)
          : TextRange.empty,
    );
  }

  test('commits text when only composing state changes', () {
    expect(
        getIOSSoftKeyboardEdit(sentinel, value('n', composing: true)), isNull);
    expect(
        getIOSSoftKeyboardEdit(sentinel, value('ni', composing: true)), isNull);
    expect(getIOSSoftKeyboardEdit(sentinel, value('ni')),
        (backspaces: 0, text: 'ni'));
  });

  test('commits a selected Chinese candidate without forwarding Pinyin', () {
    expect(
        getIOSSoftKeyboardEdit(sentinel, value('ni', composing: true)), isNull);
    expect(getIOSSoftKeyboardEdit(sentinel, value('你')),
        (backspaces: 0, text: '你'));
  });

  test('commits a multi-character candidate together', () {
    expect(getIOSSoftKeyboardEdit(sentinel, value('nihao', composing: true)),
        isNull);
    expect(getIOSSoftKeyboardEdit(sentinel, value('你好')),
        (backspaces: 0, text: '你好'));
  });

  test('deletes one committed character', () {
    expect(getIOSSoftKeyboardEdit('$sentinel你好', value('你')),
        (backspaces: 1, text: ''));
  });

  test('sends ordinary English text without waiting for composition', () {
    expect(getIOSSoftKeyboardEdit(sentinel, value('a')),
        (backspaces: 0, text: 'a'));
    expect(getIOSSoftKeyboardEdit('${sentinel}a', value('ab')),
        (backspaces: 0, text: 'b'));
  });

  test('does not send a duplicate edit for unchanged text', () {
    expect(getIOSSoftKeyboardEdit('$sentinel你', value('你')), isNull);
  });
}
