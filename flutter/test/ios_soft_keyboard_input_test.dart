import 'package:flutter/services.dart';
import 'package:flutter_hbb/mobile/ios_soft_keyboard_input.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  const seed = '1111';

  test('handles a composition-only commit without leaking marked text', () {
    var value = seed;

    for (final editingValue in const [
      TextEditingValue(
          text: '${seed}n', composing: TextRange(start: 4, end: 5)),
      TextEditingValue(
          text: '${seed}nihao', composing: TextRange(start: 4, end: 9)),
      TextEditingValue(text: '$seed你', composing: TextRange(start: 4, end: 5)),
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
      editingValue: const TextEditingValue(text: '$seed你'),
    );
    expect(commit.backspaces, 0);
    expect(commit.insertText, '你');
    expect(commit.nextValue, '$seed你');
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

  test('removes only active composing ranges from committed text', () {
    final middleComposition = prepareIosSoftKeyboardUpdate(
      previousValue: '${seed}ab',
      editingValue: const TextEditingValue(
        text: '${seed}aXb',
        composing: TextRange(start: 5, end: 6),
      ),
    );
    expect(middleComposition.nextValue, '${seed}ab');
    expect(middleComposition.backspaces, 0);
    expect(middleComposition.insertText, isEmpty);

    final insertion = prepareIosSoftKeyboardUpdate(
      previousValue: '$seed你',
      editingValue: const TextEditingValue(
        text: '$seed你好',
        composing: TextRange.collapsed(4),
      ),
    );
    expect(insertion.backspaces, 0);
    expect(insertion.insertText, '好');

    final deletion = prepareIosSoftKeyboardUpdate(
      previousValue: insertion.nextValue,
      editingValue: const TextEditingValue(text: '$seed你'),
    );
    expect(deletion.backspaces, 1);
    expect(deletion.insertText, isEmpty);
  });
}
