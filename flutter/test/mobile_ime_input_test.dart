import 'package:flutter/services.dart';
import 'package:flutter_hbb/mobile/utils/mobile_ime_input.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('middle composition does not discard a committed suffix', () {
    final input = MobileImeInput();
    input.update([replacement(oldText: ' ', start: 1, end: 1, text: 'abc')]);
    final edit = input.update([
      replacement(oldText: ' abc', start: 1, end: 2, text: 'A'),
      replacement(
          oldText: ' Abc',
          start: 3,
          end: 3,
          text: 'x',
          composing: const TextRange(start: 3, end: 4)),
    ]);
    expect((edit.moveBefore, edit.backspaces, edit.text, edit.moveAfter),
        (0, 0, '', 0));
    final committed = input.update([input.commit()]);
    expect((
      committed.moveBefore,
      committed.backspaces,
      committed.text,
      committed.moveAfter
    ), (
      -1,
      2,
      'Abx',
      0
    ));
  });

  test('batched Backspace counts the boundary, but selection deletion does not',
      () {
    for (final selection in [false, true]) {
      final input = MobileImeInput();
      input.update([replacement(oldText: ' ', start: 1, end: 1, text: 'x')]);
      input.update([
        const TextEditingDeltaNonTextUpdate(
          oldText: ' x',
          selection: TextSelection.collapsed(offset: 1),
          composing: TextRange.empty,
        ),
      ]);
      final edit = input.update(selection
          ? [replacement(oldText: ' x', start: 0, end: 2, text: '')]
          : [
              replacement(oldText: ' x', start: 1, end: 2, text: ''),
              replacement(oldText: ' ', start: 0, end: 1, text: '')
            ]);
      expect((edit.moveBefore, edit.backspaces), (1, selection ? 1 : 2));
    }
  });

  test(
      'native cursor movement preserves the suffix during insertion and deletion',
      () {
    final input = MobileImeInput(forwardComposing: true);
    input.update([replacement(oldText: ' ', start: 1, end: 1, text: 'hello')]);
    final move = input.update([
      const TextEditingDeltaNonTextUpdate(
        oldText: ' hello',
        selection: TextSelection.collapsed(offset: 3),
        composing: TextRange.empty,
      ),
    ]);
    expect((move.moveBefore, move.backspaces, move.text, move.moveAfter),
        (-3, 0, '', 0));
    final insert = input.update([
      replacement(oldText: ' hello', start: 3, end: 3, text: 'X'),
    ]);
    expect(
        (insert.moveBefore, insert.backspaces, insert.text, insert.moveAfter),
        (0, 0, 'X', 0));
    final delete = input.update([input.backspace()]);
    expect(
        (delete.moveBefore, delete.backspaces, delete.text, delete.moveAfter),
        (0, 1, '', 0));
    expect(input.value.text, ' hello');
  });

  test('paste preserves leading spaces when replacing the owned boundary', () {
    final input = MobileImeInput();
    input.update([
      const TextEditingDeltaNonTextUpdate(
        oldText: ' ',
        selection: TextSelection(baseOffset: 0, extentOffset: 1),
        composing: TextRange.empty,
      ),
    ]);
    final edit = input.update([
      replacement(oldText: ' ', start: 0, end: 1, text: ' first\r\nsecond'),
    ]);
    expect(edit.text, ' first\r\nsecond');
    expect(edit.backspaces, 0);
    expect(edit.syncPlatform, isTrue);
  });

  test('a native replacement batch emits only the final Korean syllable', () {
    final input = MobileImeInput();
    expect(
        input.update([
          replacement(oldText: ' ', start: 1, end: 1, text: 'ㅎ'),
        ]).text,
        'ㅎ');
    final edit = input.update([
      const TextEditingDeltaNonTextUpdate(
        oldText: ' ㅎ',
        selection: TextSelection(baseOffset: 0, extentOffset: 2),
        composing: TextRange.empty,
      ),
      replacement(oldText: ' ㅎ', start: 0, end: 2, text: ''),
      replacement(oldText: '', start: 0, end: 0, text: ' '),
      replacement(oldText: ' ', start: 1, end: 1, text: '하'),
    ]);
    expect(edit.backspaces, 1);
    expect(edit.text, '하');
    expect(edit.syncPlatform, isFalse);
    final next = input.update([
      replacement(oldText: ' 하', start: 1, end: 2, text: '한'),
    ]);
    expect(next.backspaces, 1);
    expect(next.text, '한');
  });

  test('repeated native deletion crosses the empty local buffer once per key',
      () {
    final input = MobileImeInput();
    for (var press = 0; press < 2; press++) {
      final edit = input.update([
        replacement(oldText: ' ', start: 0, end: 1, text: ''),
      ]);
      expect(edit.backspaces, 1);
      expect(edit.text, isEmpty);
    }
    final edit = input.update([
      replacement(oldText: ' ', start: 1, end: 1, text: 'a'),
    ]);
    expect(edit.text, 'a');
    expect(edit.backspaces, 0);
  });
}

TextEditingDelta replacement({
  required String oldText,
  required int start,
  required int end,
  required String text,
  TextRange composing = TextRange.empty,
}) =>
    TextEditingDelta.fromJSON({
      'oldText': oldText,
      'deltaStart': start,
      'deltaEnd': end,
      'deltaText': text,
      'selectionBase': start + text.length,
      'selectionExtent': start + text.length,
      'composingBase': composing.start,
      'composingExtent': composing.end,
    });
