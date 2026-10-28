import 'package:flutter/services.dart';
import 'package:flutter_hbb/mobile/ios_soft_keyboard_input.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  const seed = '1111';

  test('keeps an entire Pinyin composition local until it is committed', () {
    var value = seed;

    for (final editingValue in const [
      TextEditingValue(
          text: '${seed}n', composing: TextRange(start: 4, end: 5)),
      TextEditingValue(
          text: '${seed}nihao', composing: TextRange(start: 4, end: 9)),
    ]) {
      final update = prepareIosSoftKeyboardUpdate(
        previousValue: value,
        editingValue: editingValue,
      );
      expect(update.backspaces, 0);
      expect(update.insertText, isEmpty);
      value = update.nextValue;
    }

    final commit = prepareIosSoftKeyboardUpdate(
      previousValue: value,
      editingValue: const TextEditingValue(text: '$seed你好'),
    );
    expect(commit.backspaces, 0);
    expect(commit.insertText, '你好');
  });

  test('sends only the committed prefix during a partial commit', () {
    final partialCommit = prepareIosSoftKeyboardUpdate(
      previousValue: seed,
      editingValue: const TextEditingValue(
        text: '$seed你hao',
        composing: TextRange(start: 5, end: 8),
      ),
    );
    expect(partialCommit.backspaces, 0);
    expect(partialCommit.insertText, '你');

    final fullCommit = prepareIosSoftKeyboardUpdate(
      previousValue: partialCommit.nextValue,
      editingValue: const TextEditingValue(text: '$seed你好'),
    );
    expect(fullCommit.backspaces, 0);
    expect(fullCommit.insertText, '好');
  });

  test('preserves deletion after committed text', () {
    final update = prepareIosSoftKeyboardUpdate(
      previousValue: '$seed你好',
      editingValue: const TextEditingValue(text: '$seed你'),
    );

    expect(update.backspaces, 1);
    expect(update.insertText, isEmpty);
  });
}
