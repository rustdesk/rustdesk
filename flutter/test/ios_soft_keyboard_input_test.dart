import 'package:flutter/widgets.dart';
import 'package:flutter_hbb/mobile/utils/ios_soft_keyboard_input.dart';
import 'package:flutter_test/flutter_test.dart';

const sentinelLength = 1024;
final sentinel = ' ' * sentinelLength;

TextEditingValue editing(String text, {bool composing = false}) =>
    TextEditingValue(
      text: text,
      selection: TextSelection.collapsed(offset: text.length),
      composing: composing
          ? TextRange(start: sentinelLength, end: text.length)
          : TextRange.empty,
    );

String applyEdit(String text, IosKeyboardEdit edit) =>
    text.characters.skipLast(edit.backspaces).toString() + edit.text;

void main() {
  test('IME replacement cannot delete host text or insert a sentinel',
      verifyRecomposition);
  test('IME commits on the composing-only change without sending preedit',
      verifyComposingCommit);
  test('deletion and keyboard reopen preserve digits and Unicode text',
      verifyDeletion);
  test('replacement, partial composition and reset preserve committed text',
      verifyEditingAndReset);
  test('native selection cannot enter the hidden padding', verifyPaddingCaret);
  test('prefix deletion replenishes the native field after exhaustion',
      verifyPaddingExhaustion);
  test('native backspace selections are preserved before deletion',
      verifyDeletionSelection);
  test('backspace before committed text preserves the suffix',
      verifyDeletionBeforeCommittedText);
}

void verifyDeletionBeforeCommittedText() {
  const hostText = 'existing';
  const backspaceCount = 3;
  for (final suffix in ['abc', '你好', ' abc', ' 你好']) {
    final input = IosSoftKeyboardInput(sentinel);
    var remote =
        applyEdit(hostText, input.update(editing('$sentinel$suffix')));
    for (var i = 0; i < backspaceCount; i++) {
      final prefix = sentinel.substring(0, sentinelLength - i);
      final before = editing('$prefix$suffix');
      for (final value in [
        before.copyWith(
            selection: TextSelection.collapsed(offset: prefix.length)),
        before.copyWith(
            selection: TextSelection(
                baseOffset: prefix.length - 1, extentOffset: prefix.length)),
        editing('${prefix.substring(0, prefix.length - 1)}$suffix').copyWith(
            selection: TextSelection.collapsed(offset: prefix.length - 1)),
      ]) {
        remote = applyEdit(remote, input.update(value));
      }
      expect(remote, '${hostText.substring(0, hostText.length - i - 1)}$suffix');
    }
    final remaining = hostText.substring(0, hostText.length - backspaceCount);
    final replacement = suffix.trimLeft();
    remote = applyEdit(remote, input.update(editing(replacement)));
    expect(remote, '$remaining$replacement',
        reason: 'Replacing the field must not delete preceding host text');
    remote = applyEdit(
        remote,
        input.update(editing('x$replacement').copyWith(
            selection: const TextSelection.collapsed(offset: 1))));
    expect(remote, '${remaining}x$replacement');
  }
}

void verifyPaddingCaret() {
  final input = IosSoftKeyboardInput(sentinel);
  var remote =
      applyEdit('existing', input.update(editing('${sentinel}abcdef')));
  for (final selection in [
    const TextSelection.collapsed(offset: 0),
    const TextSelection(baseOffset: sentinelLength - 1, extentOffset: 0),
  ]) {
    final value = editing('${sentinel}abcdef').copyWith(selection: selection);
    final edit = input.update(value);
    remote = applyEdit(remote, edit);
    final corrected = edit.localValue ?? value;
    expect(corrected.selection.start, sentinelLength);
    expect(corrected.selection.end, sentinelLength);
    expect(remote, 'existingabcdef');
  }
  remote = applyEdit(remote, input.update(editing('${sentinel}xabcdef')));
  expect(remote, 'existingxabcdef');
}

void verifyDeletionSelection() {
  final input = IosSoftKeyboardInput(sentinel);
  var remote = 'abc';
  for (var i = 0; i < 3; i++) {
    final length = sentinelLength - i;
    final selected = editing(sentinel.substring(0, length)).copyWith(
      selection: TextSelection(baseOffset: length - 1, extentOffset: length),
    );
    final edit = input.update(selected);
    expect(edit.localValue, isNull,
        reason: 'iOS selects the previous character before deleting it');
    expect(applyEdit(remote, edit), remote);
    remote = applyEdit(
        remote, input.update(editing(sentinel.substring(0, length - 1))));
    expect(remote, 'abc'.substring(0, 2 - i));
  }
}

void verifyPaddingExhaustion() {
  final input = IosSoftKeyboardInput(sentinel);
  var value = editing(sentinel);
  var remote = 'x' * (sentinelLength + 3);
  for (var i = 0; i < sentinelLength + 2; i++) {
    value = editing(value.text.substring(0, value.text.length - 1));
    final edit = input.update(value);
    remote = applyEdit(remote, edit);
    value = edit.localValue ?? value;
    expect(remote.length, sentinelLength + 2 - i);
    expect(value.text, isNotEmpty, reason: 'Backspace must keep working');
    expect(value.selection.extentOffset, value.text.length);
    if (edit.localValue != null) {
      expect(applyEdit(remote, input.update(value)), remote,
          reason: 'Controller correction must not echo to the peer');
    }
  }
  remote = applyEdit(remote, input.update(editing('${value.text}a')));
  expect(remote, 'xa');
  input.reset();
  remote = applyEdit('existing', input.update(editing('a')));
  final deleted = input.update(editing(''));
  expect(applyEdit(remote, deleted), 'existing');
  expect(deleted.localValue?.text, sentinel,
      reason: 'Replacing all padding must not disable subsequent Backspace');
}

void verifyRecomposition() {
  for (final entry in {'a': 'â', 'u': 'ư', 'd': 'đ', 'ㅇ': '아'}.entries) {
    final input = IosSoftKeyboardInput(sentinel);
    var remote = 'existing';
    // Native iOS traces remove the preceding sentinel with the old letter,
    // then restore the sentinel before inserting the recomposed character.
    for (final text in [
      '$sentinel${entry.key}',
      sentinel.substring(0, sentinelLength - 1),
      sentinel,
      '$sentinel${entry.value}',
    ]) {
      remote = applyEdit(remote, input.update(editing(text)));
    }
    expect(remote, 'existing${entry.value}', reason: entry.toString());
  }
  final input = IosSoftKeyboardInput(sentinel);
  var remote = 'existing';
  for (final text in [' teh', ' ten', ' ten ', ' ten', ' te', ' text ']) {
    remote = applyEdit(remote, input.update(editing('$sentinel$text')));
    expect(remote, 'existing$text', reason: 'Native correction or deletion');
  }
}

void verifyComposingCommit() {
  final cases = {
    '你好': ['n', 'ni', 'ni\u2006h', 'ni\u2006ha', 'ni\u2006hao'],
    '女孩': ['n', 'n\u2006h'],
    '交通车上了': [
      'j',
      'j\u2006t',
      'j\u2006t\u2006c',
      'j\u2006t\u2006c\u2006s',
      'j\u2006t\u2006c\u2006s\u2006l'
    ],
    '妳好': ['ㄋ', 'ㄋㄧ', 'ㄋㄧˇ', 'ㄋㄧˇ\u2004ㄏㄠˇ', '妳ㄏㄠˇ'],
    '嗎': ['ㄇ', 'ㄇㄚ', 'ㄇㄚ˙'],
  };
  for (final entry in cases.entries) {
    final input = IosSoftKeyboardInput(sentinel);
    var remote = 'existing';
    for (final text in [...entry.value, entry.key]) {
      remote = applyEdit(
          remote, input.update(editing('$sentinel$text', composing: true)));
      expect(remote, 'existing', reason: 'Uncommitted $text');
    }
    remote = applyEdit(remote, input.update(editing('$sentinel${entry.key}')));
    expect(remote, 'existing${entry.key}');
  }
}

void verifyDeletion() {
  for (final text in [
    '1',
    'a1b',
    '아이폰',
    '你好',
    '😀',
    'e\u0301',
    '👨‍👩‍👧‍👦',
    '👍🏽',
    '🇹🇼'
  ]) {
    final input = IosSoftKeyboardInput(sentinel);
    var remote = 'existing';
    remote = applyEdit(remote, input.update(editing('$sentinel$text')));
    expect(remote, 'existing$text');
    remote = applyEdit(remote, input.update(editing(sentinel)));
    expect(remote, 'existing', reason: 'Delete $text');
    remote = applyEdit(remote,
        input.update(editing(sentinel.substring(0, sentinelLength - 1))));
    expect(remote, 'existin', reason: 'Backspace before this keyboard session');
    input.reset();
    remote = applyEdit(remote, input.update(editing('${sentinel}1')));
    expect(remote, 'existin1', reason: 'Reopen keyboard and type 1');
  }
}

void verifyEditingAndReset() {
  final input = IosSoftKeyboardInput(sentinel);
  var remote = 'existing';
  for (final text in ['abc', 'a中文c', 'abc', 'a  b\nc', '']) {
    remote = applyEdit(remote, input.update(editing('$sentinel$text')));
    expect(remote, 'existing$text', reason: 'Replacement or undo: $text');
    remote = applyEdit(
        remote,
        input.update(TextEditingValue(
          text: '$sentinel$text',
          selection: TextSelection(
              baseOffset: sentinelLength,
              extentOffset: sentinelLength + text.length),
        )));
    expect(remote, 'existing$text',
        reason: 'Selection alone must not edit text');
  }
  remote = applyEdit(
      remote,
      input.update(editing('$sentinel家庭出身l').copyWith(
        composing: TextRange(start: sentinelLength + 4, end: sentinelLength + 5),
      )));
  expect(remote, 'existing家庭出身', reason: 'Only confirmed candidates are sent');
  input.reset();
  remote = applyEdit(remote, input.update(editing('$sentinel你好')));
  expect(remote, 'existing家庭出身你好', reason: 'Reset discards pending preedit');
}
