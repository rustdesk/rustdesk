import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/mobile/utils/ios_soft_keyboard_input.dart';

void main() {
  group('diffIOSSoftKeyboardInput', () {
    test('does not send composing pinyin before commit', () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111',
        currentValue: '111ni',
        composingRange: const TextRange(start: 3, end: 5),
      );

      expect(result.nextValue, '111');
      expect(result.nextComposingValue, 'ni');
      expect(result.actions, isEmpty);
    });

    test('does not send a single composing stroke before commit', () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111',
        currentValue: '111一',
        composingRange: const TextRange(start: 3, end: 4),
      );

      expect(result.nextValue, '111');
      expect(result.nextComposingValue, '一');
      expect(result.actions, isEmpty);
    });

    test('does not send composing stroke input before commit', () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111',
        currentValue: '111一丨',
        composingRange: const TextRange(start: 3, end: 5),
        previousComposingValue: '一',
      );

      expect(result.nextValue, '111');
      expect(result.nextComposingValue, '一丨');
      expect(result.actions, isEmpty);
    });

    test('sends committed Chinese text even if iOS keeps composing active', () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111',
        currentValue: '111你',
        composingRange: const TextRange(start: 3, end: 4),
        previousComposingValue: 'ni',
      );

      expect(result.nextValue, '111你');
      expect(result.nextComposingValue, isNull);
      expect(result.actions, [
        const IOSSoftKeyboardInputAction.inputText('你'),
      ]);
    });

    test('sends committed Chinese text after stroke input replacement', () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111',
        currentValue: '111你',
        composingRange: const TextRange(start: 3, end: 4),
        previousComposingValue: '一丨',
      );

      expect(result.nextValue, '111你');
      expect(result.nextComposingValue, isNull);
      expect(result.actions, [
        const IOSSoftKeyboardInputAction.inputText('你'),
      ]);
    });

    test('sends committed Chinese text as direct text input', () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111',
        currentValue: '111你',
        composingRange: TextRange.empty,
      );

      expect(result.nextValue, '111你');
      expect(result.nextComposingValue, isNull);
      expect(result.actions, [
        const IOSSoftKeyboardInputAction.inputText('你'),
      ]);
    });

    test('does not send Japanese kana converted from romaji before commit', () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111',
        currentValue: '111にほん',
        composingRange: const TextRange(start: 3, end: 6),
        previousComposingValue: 'nihon',
      );

      expect(result.nextValue, '111');
      expect(result.nextComposingValue, 'にほん');
      expect(result.actions, isEmpty);
    });

    test('sends committed Japanese kanji after kana composition', () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111',
        currentValue: '111日本',
        composingRange: const TextRange(start: 3, end: 5),
        previousComposingValue: 'にほん',
      );

      expect(result.nextValue, '111日本');
      expect(result.nextComposingValue, isNull);
      expect(result.actions, [
        const IOSSoftKeyboardInputAction.inputText('日本'),
      ]);
    });

    test('sends committed Japanese kana when composing collapses', () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111',
        currentValue: '111にほん',
        composingRange: TextRange.empty,
        previousComposingValue: 'にほん',
      );

      expect(result.nextValue, '111にほん');
      expect(result.nextComposingValue, isNull);
      expect(result.actions, [
        const IOSSoftKeyboardInputAction.inputText('にほん'),
      ]);
    });

    test('does not send Korean jamo composition before commit', () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111',
        currentValue: '111ㅎㅏ',
        composingRange: const TextRange(start: 3, end: 5),
      );

      expect(result.nextValue, '111');
      expect(result.nextComposingValue, 'ㅎㅏ');
      expect(result.actions, isEmpty);
    });

    test('does not send composing Korean hangul syllable before commit', () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111',
        currentValue: '111한',
        composingRange: const TextRange(start: 3, end: 4),
        previousComposingValue: 'ㅎㅏ',
      );

      expect(result.nextValue, '111');
      expect(result.nextComposingValue, '한');
      expect(result.actions, isEmpty);
    });

    test('sends committed Korean hangul when composing collapses', () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111',
        currentValue: '111한',
        composingRange: TextRange.empty,
        previousComposingValue: '한',
      );

      expect(result.nextValue, '111한');
      expect(result.nextComposingValue, isNull);
      expect(result.actions, [
        const IOSSoftKeyboardInputAction.inputText('한'),
      ]);
    });

    test('keeps ascii single character input as a key stroke', () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111',
        currentValue: '111a',
        composingRange: TextRange.empty,
      );

      expect(result.nextValue, '111a');
      expect(result.nextComposingValue, isNull);
      expect(result.actions, [
        const IOSSoftKeyboardInputAction.inputKey('a'),
      ]);
    });

    test('sends backspace when committed text is deleted', () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111你',
        currentValue: '111',
        composingRange: TextRange.empty,
      );

      expect(result.nextValue, '111');
      expect(result.nextComposingValue, isNull);
      expect(result.actions, [
        const IOSSoftKeyboardInputAction.backspace(),
      ]);
    });
  });
}
