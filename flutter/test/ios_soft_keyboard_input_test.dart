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

    test('does not send repeated stroke composition before commit', () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111',
        currentValue: '111一',
        composingRange: const TextRange(start: 3, end: 4),
        previousComposingValue: '一',
      );

      expect(result.nextValue, '111');
      expect(result.nextComposingValue, '一');
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

    test('does not send ascii composing text when range collapses briefly', () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111',
        currentValue: '111ni',
        composingRange: TextRange.empty,
        previousComposingValue: 'ni',
        sentinelPrefixLength: 3,
      );

      expect(result.nextValue, '111');
      expect(result.nextComposingValue, 'ni');
      expect(result.actions, isEmpty);
    });

    test('does not send ascii composing text after committed text collapses',
        () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111你',
        currentValue: '111你ni',
        composingRange: TextRange.empty,
        previousComposingValue: 'ni',
        sentinelPrefixLength: 3,
      );

      expect(result.nextValue, '111你');
      expect(result.nextComposingValue, 'ni');
      expect(result.actions, isEmpty);
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

    test('does not send mixed Japanese kana and ascii before commit', () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111',
        currentValue: '111にh',
        composingRange: const TextRange(start: 3, end: 5),
        previousComposingValue: 'ni',
      );

      expect(result.nextValue, '111');
      expect(result.nextComposingValue, 'にh');
      expect(result.actions, isEmpty);
    });

    test('sends committed mixed Japanese kana and ascii', () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111',
        currentValue: '111にh',
        composingRange: TextRange.empty,
        previousComposingValue: 'にh',
        sentinelPrefixLength: 3,
      );

      expect(result.nextValue, '111にh');
      expect(result.nextComposingValue, isNull);
      expect(result.actions, [
        const IOSSoftKeyboardInputAction.inputText('にh'),
      ]);
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

    test('sends ascii after committed Japanese text when composing collapses',
        () {
      var result = diffIOSSoftKeyboardInput(
        previousValue: '111に',
        currentValue: '111にa',
        composingRange: const TextRange(start: 4, end: 5),
        sentinelPrefixLength: 3,
      );

      expect(result.nextValue, '111に');
      expect(result.nextComposingValue, 'a');
      expect(result.actions, isEmpty);

      result = diffIOSSoftKeyboardInput(
        previousValue: result.nextValue,
        currentValue: '111にa',
        composingRange: TextRange.empty,
        previousComposingValue: result.nextComposingValue,
        sentinelPrefixLength: 3,
      );

      expect(result.nextValue, '111にa');
      expect(result.nextComposingValue, isNull);
      expect(result.actions, [
        const IOSSoftKeyboardInputAction.inputKey('a'),
      ]);
    });

    test('does not send pinyin composing after committed Japanese text', () {
      var previousValue = '111みau';
      String? previousComposingValue;

      IOSSoftKeyboardInputResult diff(
        String currentValue,
        TextRange composingRange,
      ) {
        final result = diffIOSSoftKeyboardInput(
          previousValue: previousValue,
          currentValue: currentValue,
          composingRange: composingRange,
          previousComposingValue: previousComposingValue,
          sentinelPrefixLength: 3,
        );
        previousValue = result.nextValue;
        previousComposingValue = result.nextComposingValue;
        return result;
      }

      var result = diff('111みaun', const TextRange(start: 6, end: 7));
      expect(result.nextValue, '111みau');
      expect(result.nextComposingValue, 'n');
      expect(result.actions, isEmpty);

      result = diff('111みauni', const TextRange(start: 6, end: 8));
      expect(result.nextValue, '111みau');
      expect(result.nextComposingValue, 'ni');
      expect(result.actions, isEmpty);

      result = diff('111みauni\u2006h', const TextRange(start: 6, end: 10));
      expect(result.nextValue, '111みau');
      expect(result.nextComposingValue, 'ni\u2006h');
      expect(result.actions, isEmpty);

      result = diff('111みauni\u2006hao', const TextRange(start: 6, end: 12));
      expect(result.nextValue, '111みau');
      expect(result.nextComposingValue, 'ni\u2006hao');
      expect(result.actions, isEmpty);

      result = diff('111みau你好', const TextRange(start: 6, end: 8));
      expect(result.nextValue, '111みau你好');
      expect(result.nextComposingValue, isNull);
      expect(result.actions, [
        const IOSSoftKeyboardInputAction.inputText('你好'),
      ]);
    });

    test('sends committed ascii after Japanese text composing collapses', () {
      var result = diffIOSSoftKeyboardInput(
        previousValue: '111みau',
        currentValue: '111みaua',
        composingRange: const TextRange(start: 6, end: 7),
        sentinelPrefixLength: 3,
      );

      expect(result.nextValue, '111みau');
      expect(result.nextComposingValue, 'a');
      expect(result.actions, isEmpty);

      result = diffIOSSoftKeyboardInput(
        previousValue: result.nextValue,
        currentValue: '111みaua',
        composingRange: TextRange.empty,
        previousComposingValue: result.nextComposingValue,
        sentinelPrefixLength: 3,
      );

      expect(result.nextValue, '111みaua');
      expect(result.nextComposingValue, isNull);
      expect(result.actions, [
        const IOSSoftKeyboardInputAction.inputKey('a'),
      ]);
    });

    test('sends Japanese IME ascii after space when composing collapses', () {
      var previousValue = '111 ';
      String? previousComposingValue;

      IOSSoftKeyboardInputResult diff(
        String currentValue,
        TextRange composingRange,
      ) {
        final result = diffIOSSoftKeyboardInput(
          previousValue: previousValue,
          currentValue: currentValue,
          composingRange: composingRange,
          previousComposingValue: previousComposingValue,
          sentinelPrefixLength: 3,
        );
        previousValue = result.nextValue;
        previousComposingValue = result.nextComposingValue;
        return result;
      }

      expect(diff('111 l', const TextRange(start: 4, end: 5)).actions, isEmpty);
      expect(
          diff('111 lm', const TextRange(start: 4, end: 6)).actions, isEmpty);
      expect(
          diff('111 lmw', const TextRange(start: 4, end: 7)).actions, isEmpty);
      expect(
          diff('111 lmww', const TextRange(start: 4, end: 8)).actions, isEmpty);
      expect(diff('111 lmwwm', const TextRange(start: 4, end: 9)).actions,
          isEmpty);

      final result = diff('111 lmwwm', TextRange.empty);

      expect(result.nextValue, '111 lmwwm');
      expect(result.nextComposingValue, isNull);
      expect(result.actions, [
        const IOSSoftKeyboardInputAction.inputText('lmwwm'),
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

    test('does not send Korean jamo without composing range', () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111',
        currentValue: '111ㅎ',
        composingRange: TextRange.empty,
        sentinelPrefixLength: 3,
      );

      expect(result.nextValue, '111');
      expect(result.nextComposingValue, 'ㅎ');
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

    test('replaces Korean hangul after sentinel reset during composition', () {
      var previousValue = '111';
      String? previousComposingValue;
      String? previousControllerText = '111';

      IOSSoftKeyboardInputResult diff(String currentValue) {
        final result = diffIOSSoftKeyboardInput(
          previousValue: previousValue,
          currentValue: currentValue,
          composingRange: TextRange.empty,
          previousComposingValue: previousComposingValue,
          previousControllerText: previousControllerText,
          sentinelPrefixLength: 3,
        );
        previousValue = result.nextValue;
        previousComposingValue = result.nextComposingValue;
        previousControllerText = currentValue;
        return result;
      }

      expect(diff('111ㅎ').actions, isEmpty);
      expect(diff('11').actions, isEmpty);
      expect(diff('111').actions, isEmpty);

      var result = diff('111하');
      expect(result.nextValue, '111하');
      expect(result.nextComposingValue, '하');
      expect(result.actions, [
        const IOSSoftKeyboardInputAction.inputText('하'),
      ]);

      expect(diff('111하').actions, isEmpty);
      expect(diff('11').actions, isEmpty);
      expect(diff('111').actions, isEmpty);

      result = diff('111한');
      expect(result.nextValue, '111한');
      expect(result.nextComposingValue, '한');
      expect(result.actions, [
        const IOSSoftKeyboardInputAction.backspace(),
        const IOSSoftKeyboardInputAction.inputText('한'),
      ]);
    });

    test('keeps backspace when deleting sent Korean composing text', () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111하',
        currentValue: '111',
        composingRange: TextRange.empty,
        previousComposingValue: '하',
        previousControllerText: '111하',
        sentinelPrefixLength: 3,
      );

      expect(result.nextValue, '111');
      expect(result.nextComposingValue, isNull);
      expect(result.actions, [
        const IOSSoftKeyboardInputAction.backspace(),
      ]);
    });

    test('does not buffer unrelated Korean consonants until newline', () {
      var previousValue = '111';
      String? previousComposingValue;

      IOSSoftKeyboardInputResult diff(String currentValue) {
        final result = diffIOSSoftKeyboardInput(
          previousValue: previousValue,
          currentValue: currentValue,
          composingRange: TextRange.empty,
          previousComposingValue: previousComposingValue,
          sentinelPrefixLength: 3,
        );
        previousValue = result.nextValue;
        previousComposingValue = result.nextComposingValue;
        return result;
      }

      var result = diff('111ㄹ');
      expect(result.nextValue, '111');
      expect(result.nextComposingValue, 'ㄹ');
      expect(result.actions, isEmpty);

      result = diff('111ㄹㅇ');
      expect(result.nextValue, '111ㄹ');
      expect(result.nextComposingValue, 'ㅇ');
      expect(result.actions, [
        const IOSSoftKeyboardInputAction.inputText('ㄹ'),
      ]);

      result = diff('111ㄹㅇㅎ');
      expect(result.nextValue, '111ㄹㅇ');
      expect(result.nextComposingValue, 'ㅎ');
      expect(result.actions, [
        const IOSSoftKeyboardInputAction.inputText('ㅇ'),
      ]);

      result = diff('111ㄹㅇㅎㅎ');
      expect(result.nextValue, '111ㄹㅇㅎ');
      expect(result.nextComposingValue, 'ㅎ');
      expect(result.actions, [
        const IOSSoftKeyboardInputAction.inputText('ㅎ'),
      ]);

      result = diff('111ㄹㅇㅎㅎㅇ');
      expect(result.nextValue, '111ㄹㅇㅎㅎ');
      expect(result.nextComposingValue, 'ㅇ');
      expect(result.actions, [
        const IOSSoftKeyboardInputAction.inputText('ㅎ'),
      ]);

      result = diff('111ㄹㅇㅎㅎㅇ\n');
      expect(result.nextValue, '111ㄹㅇㅎㅎㅇ\n');
      expect(result.nextComposingValue, isNull);
      expect(result.actions, [
        const IOSSoftKeyboardInputAction.inputText('ㅇ\n'),
      ]);
    });

    test('does not backspace while Korean composition shrinks sentinel only',
        () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111',
        currentValue: '11',
        composingRange: TextRange.empty,
        previousComposingValue: 'ㅎ',
        sentinelPrefixLength: 3,
      );

      expect(result.nextValue, '111');
      expect(result.nextComposingValue, 'ㅎ');
      expect(result.actions, isEmpty);
    });

    test('does not backspace while Korean hangul composition shrinks sentinel',
        () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111하',
        currentValue: '11',
        composingRange: TextRange.empty,
        previousComposingValue: '하',
        sentinelPrefixLength: 3,
      );

      expect(result.nextValue, '111하');
      expect(result.nextComposingValue, '하');
      expect(result.actions, isEmpty);
    });

    test(
        'does not send restored sentinel while Korean hangul composition resets',
        () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111하',
        currentValue: '111',
        composingRange: TextRange.empty,
        previousComposingValue: '하',
        previousControllerText: '11',
        sentinelPrefixLength: 3,
      );

      expect(result.nextValue, '111하');
      expect(result.nextComposingValue, '하');
      expect(result.actions, isEmpty);
    });

    test('does not send restored sentinel before committed Korean text', () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '11',
        currentValue: '111하',
        composingRange: TextRange.empty,
        sentinelPrefixLength: 3,
      );

      expect(result.nextValue, '111하');
      expect(result.nextComposingValue, isNull);
      expect(result.actions, [
        const IOSSoftKeyboardInputAction.inputText('하'),
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

    test('keeps typed sentinel as user input', () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111',
        currentValue: '1111',
        composingRange: TextRange.empty,
        sentinelPrefixLength: 3,
      );

      expect(result.nextValue, '1111');
      expect(result.nextComposingValue, isNull);
      expect(result.actions, [
        const IOSSoftKeyboardInputAction.inputKey('1'),
      ]);
    });

    test('does not delete preceding text when typed text contains sentinel',
        () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111a',
        currentValue: '111a1',
        composingRange: TextRange.empty,
        sentinelPrefixLength: 3,
      );

      expect(result.nextValue, '111a1');
      expect(result.nextComposingValue, isNull);
      expect(result.actions, [
        const IOSSoftKeyboardInputAction.inputKey('1'),
      ]);
    });

    test('does not backspace sentinel prefix when current text loses prefix',
        () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111abc',
        currentValue: 'xyz',
        composingRange: TextRange.empty,
        sentinelPrefixLength: 3,
      );

      expect(result.nextValue, 'xyz');
      expect(result.nextComposingValue, isNull);
      expect(result.actions, [
        const IOSSoftKeyboardInputAction.inputText('xyz'),
      ]);
    });

    test('does not backspace sentinel prefix when current text is cleared', () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111abc',
        currentValue: '',
        composingRange: TextRange.empty,
        sentinelPrefixLength: 3,
      );

      expect(result.nextValue, '');
      expect(result.nextComposingValue, isNull);
      expect(result.actions, isEmpty);
    });

    test('keeps input after the sentinel prefix is shortened', () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '11',
        currentValue: '11a',
        composingRange: TextRange.empty,
        sentinelPrefixLength: 3,
      );

      expect(result.nextValue, '11a');
      expect(result.nextComposingValue, isNull);
      expect(result.actions, [
        const IOSSoftKeyboardInputAction.inputKey('a'),
      ]);
    });

    test('sends typed sentinel after the prefix was shortened', () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '11',
        currentValue: '111',
        composingRange: TextRange.empty,
        sentinelPrefixLength: 3,
      );

      expect(result.nextValue, '111');
      expect(result.nextComposingValue, isNull);
      expect(result.actions, [
        const IOSSoftKeyboardInputAction.inputKey('1'),
      ]);
    });

    test('sends backspace when deleting from an empty sentinel tail', () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111',
        currentValue: '11',
        composingRange: TextRange.empty,
        sentinelPrefixLength: 3,
      );

      expect(result.nextValue, '11');
      expect(result.nextComposingValue, isNull);
      expect(result.actions, [
        const IOSSoftKeyboardInputAction.backspace(),
      ]);
    });

    test('keeps backspace when the last sentinel is deleted', () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '1',
        currentValue: '',
        composingRange: TextRange.empty,
        sentinelPrefixLength: 3,
      );

      expect(result.nextValue, '');
      expect(result.nextComposingValue, isNull);
      expect(result.actions, [
        const IOSSoftKeyboardInputAction.backspace(),
      ]);
    });

    test('does not send bopomofo tone-mark composition before commit', () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111',
        currentValue: '111ㄋㄧˇ',
        composingRange: const TextRange(start: 3, end: 6),
      );

      expect(result.nextValue, '111');
      expect(result.nextComposingValue, 'ㄋㄧˇ');
      expect(result.actions, isEmpty);
    });

    test('does not send bopomofo converted from ascii before commit', () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111',
        currentValue: '111ㄋㄧ',
        composingRange: const TextRange(start: 3, end: 5),
        previousComposingValue: 'ni',
      );

      expect(result.nextValue, '111');
      expect(result.nextComposingValue, 'ㄋㄧ');
      expect(result.actions, isEmpty);
    });

    test('does not send pinyin growth if composing range temporarily collapses',
        () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111',
        currentValue: '111ni hao',
        composingRange: TextRange.empty,
        previousComposingValue: 'ni',
        sentinelPrefixLength: 3,
      );

      expect(result.nextValue, '111');
      expect(result.nextComposingValue, 'ni hao');
      expect(result.actions, isEmpty);
    });

    test('does not send pinyin growth with iOS marked-text spacing', () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111',
        currentValue: '111ni\u2006h',
        composingRange: TextRange.empty,
        previousComposingValue: 'ni',
        sentinelPrefixLength: 3,
      );

      expect(result.nextValue, '111');
      expect(result.nextComposingValue, 'ni\u2006h');
      expect(result.actions, isEmpty);
    });

    test('does not send pinyin growth after committed text', () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111你',
        currentValue: '111你ni hao',
        composingRange: TextRange.empty,
        previousComposingValue: 'ni',
        sentinelPrefixLength: 3,
      );

      expect(result.nextValue, '111你');
      expect(result.nextComposingValue, 'ni hao');
      expect(result.actions, isEmpty);
    });

    test('sends committed bopomofo candidate text', () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111',
        currentValue: '111你',
        composingRange: const TextRange(start: 3, end: 4),
        previousComposingValue: 'ㄋㄧˇ',
      );

      expect(result.nextValue, '111你');
      expect(result.nextComposingValue, isNull);
      expect(result.actions, [
        const IOSSoftKeyboardInputAction.inputText('你'),
      ]);
    });

    test('does not send stroke converted from ascii before commit', () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111',
        currentValue: '111一',
        composingRange: const TextRange(start: 3, end: 4),
        previousComposingValue: 'h',
      );

      expect(result.nextValue, '111');
      expect(result.nextComposingValue, '一');
      expect(result.actions, isEmpty);
    });

    test('does not send Korean jamo growth if composing range collapses', () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111',
        currentValue: '111ㅎㅏ',
        composingRange: TextRange.empty,
        previousComposingValue: 'ㅎ',
        sentinelPrefixLength: 3,
      );

      expect(result.nextValue, '111');
      expect(result.nextComposingValue, 'ㅎㅏ');
      expect(result.actions, isEmpty);
    });

    test('does not send Korean jamo when composing range collapses briefly',
        () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111',
        currentValue: '111ㅎ',
        composingRange: TextRange.empty,
        previousComposingValue: 'ㅎ',
        sentinelPrefixLength: 3,
      );

      expect(result.nextValue, '111');
      expect(result.nextComposingValue, 'ㅎ');
      expect(result.actions, isEmpty);
    });

    test('sends committed stroke text if composing range is retained', () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111',
        currentValue: '111一',
        composingRange: const TextRange(start: 3, end: 4),
        previousComposingValue: '一',
        forceCommitComposingText: true,
      );

      expect(result.nextValue, '111一');
      expect(result.nextComposingValue, isNull);
      expect(result.actions, [
        const IOSSoftKeyboardInputAction.inputText('一'),
      ]);
    });

    test('keeps emoji replacement on rune boundaries', () {
      final result = diffIOSSoftKeyboardInput(
        previousValue: '111😀',
        currentValue: '111😃',
        composingRange: TextRange.empty,
        sentinelPrefixLength: 3,
      );

      expect(result.nextValue, '111😃');
      expect(result.nextComposingValue, isNull);
      expect(result.actions, [
        const IOSSoftKeyboardInputAction.backspace(),
        const IOSSoftKeyboardInputAction.inputText('😃'),
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

  group('shouldForceCommitIOSComposingText', () {
    test('forces commit when composing collapses without text change', () {
      expect(
        shouldForceCommitIOSComposingText(
          previousControllerText: '111一',
          previousComposingRange: const TextRange(start: 3, end: 4),
          currentValue: const TextEditingValue(
            text: '111一',
            composing: TextRange.collapsed(4),
          ),
        ),
        isTrue,
      );
    });

    test('does not force commit while composing range remains valid', () {
      expect(
        shouldForceCommitIOSComposingText(
          previousControllerText: '111一丨',
          previousComposingRange: const TextRange(start: 3, end: 4),
          currentValue: const TextEditingValue(
            text: '111一丨',
            composing: TextRange(start: 3, end: 5),
          ),
        ),
        isFalse,
      );
    });

    test('does not force commit when ascii composing collapses', () {
      expect(
        shouldForceCommitIOSComposingText(
          previousControllerText: '111ni',
          previousComposingRange: const TextRange(start: 3, end: 5),
          currentValue: const TextEditingValue(
            text: '111ni',
            composing: TextRange.empty,
          ),
        ),
        isFalse,
      );
    });

    test('does not force commit when Korean jamo composing collapses', () {
      expect(
        shouldForceCommitIOSComposingText(
          previousControllerText: '111ㅎ',
          previousComposingRange: const TextRange(start: 3, end: 4),
          currentValue: const TextEditingValue(
            text: '111ㅎ',
            composing: TextRange.empty,
          ),
        ),
        isFalse,
      );
    });

    test('does not force commit when text changes', () {
      expect(
        shouldForceCommitIOSComposingText(
          previousControllerText: '111一',
          previousComposingRange: const TextRange(start: 3, end: 4),
          currentValue: const TextEditingValue(
            text: '111一丨',
            composing: TextRange(start: 3, end: 5),
          ),
        ),
        isFalse,
      );
    });

    test('does not force commit without previous composing state', () {
      expect(
        shouldForceCommitIOSComposingText(
          previousControllerText: null,
          previousComposingRange: null,
          currentValue: const TextEditingValue(
            text: '111一',
            composing: TextRange.collapsed(4),
          ),
        ),
        isFalse,
      );
    });
  });
}
