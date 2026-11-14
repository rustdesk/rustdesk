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

// Models emitted edits; actual Backspace behavior still depends on the host.
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
  test('prefix deletion replenishes the native field after exhaustion',
      verifyPaddingExhaustion);
  test('typing ignores native cursor movement inside the hidden field',
      verifyInputAfterPaddingNavigation);
  test('backspace ignores native cursor movement inside the hidden field',
      verifyBackspaceAfterPaddingNavigation);
  test('whole-field replacement preserves host text and leading whitespace',
      verifyReplacement);
  test('Undo and Redo after replacing only padding keep it local',
      verifyUndoRedo);
  test('clearing padding and text keeps the next Backspace usable',
      verifyClearAndDelete);
  test('rejected input preserves accepted text when permission returns', () {
    final input = IosSoftKeyboardInput(sentinel);
    final initial = input.update(editing('$sentinelㅎ'), inputAllowed: false);
    expect(initial.localValue, editing(sentinel));
    final accepted = editing('${sentinel}abc');
    var remote = applyEdit('existing', input.update(accepted));
    for (final text in ['${sentinel}abcㅎ', '${sentinel}ab']) {
      final rejected = input.update(editing(text), inputAllowed: false);
      expect((rejected.backspaces, rejected.text), (0, ''));
      expect(rejected.localValue, accepted);
    }
    remote = applyEdit(remote, input.update(editing('${accepted.text}ㅗ')));
    expect(remote, 'existingabcㅗ');
    remote = applyEdit(remote, input.update(accepted));
    expect(remote, 'existingabc');
  });
}

void verifyInputAfterPaddingNavigation() {
  const typed = 'abcdef';
  for (final offset in [0, sentinelLength + typed.length ~/ 2]) {
    final input = IosSoftKeyboardInput(sentinel);
    var value = editing('$sentinel$typed');
    var remote = applyEdit('existing', input.update(value));
    value = value.copyWith(selection: TextSelection.collapsed(offset: offset));
    final navigation = input.update(value);
    remote = applyEdit(remote, navigation);
    if (navigation.localValue != null) {
      value = navigation.localValue!;
      remote = applyEdit(remote, input.update(value));
    }
    final caret = value.selection.extentOffset;
    value = value.copyWith(
      text: value.text.replaceRange(caret, caret, 'x'),
      selection: TextSelection.collapsed(offset: caret + 1),
    );
    remote = applyEdit(remote, input.update(value));
    expect(remote, 'existing${typed}x');
  }
}

void verifyBackspaceAfterPaddingNavigation() {
  final input = IosSoftKeyboardInput(sentinel);
  var value = editing('${sentinel}abc');
  var remote = applyEdit('existing', input.update(value));
  value = value.copyWith(
      selection: const TextSelection.collapsed(offset: sentinelLength));
  final navigation = input.update(value);
  remote = applyEdit(remote, navigation);
  if (navigation.localValue != null) {
    value = navigation.localValue!;
    remote = applyEdit(remote, input.update(value));
  }
  final caret = value.selection.extentOffset;
  final selected = value.copyWith(
      selection: TextSelection(baseOffset: caret - 1, extentOffset: caret));
  final selectionEdit = input.update(selected);
  expect(selectionEdit.localValue, isNull);
  remote = applyEdit(remote, selectionEdit);
  final deleted = value.copyWith(
      text: value.text.replaceRange(caret - 1, caret, ''),
      selection: TextSelection.collapsed(offset: caret - 1));
  remote = applyEdit(remote, input.update(deleted));
  expect(remote, 'existingab');
}

void verifyPaddingExhaustion() {
  final input = IosSoftKeyboardInput(sentinel);
  var value = editing(sentinel);
  var remote = 'x' * (sentinelLength + 3);
  for (var i = 0; i < sentinelLength + 2; i++) {
    final selected = value.copyWith(
        selection: TextSelection(
            baseOffset: value.text.length - 1,
            extentOffset: value.text.length));
    final selectionEdit = input.update(selected);
    expect(selectionEdit.localValue, isNull,
        reason: 'iOS selects the previous character before deleting it');
    expect(applyEdit(remote, selectionEdit), remote);
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
  final input = IosSoftKeyboardInput(sentinel);
  var remote = 'existing';
  // Native iOS traces remove the preceding sentinel with the old letter,
  // then restore the sentinel before inserting the recomposed character.
  // Reduced sequence: editing() reconstructs collapsed selections at text end.
  for (final (text, backspaces, inserted) in [
    ('$sentinelㅇ', 0, 'ㅇ'),
    (sentinel.substring(0, sentinelLength - 1), 1, ''),
    (sentinel, 0, ''),
    ('$sentinel아', 0, '아'),
  ]) {
    final edit = input.update(editing(text));
    expect((edit.backspaces, edit.text), (backspaces, inserted));
    remote = applyEdit(remote, edit);
  }
  expect(remote, 'existing아');
}

void verifyComposingCommit() {
  final input = IosSoftKeyboardInput(sentinel);
  var remote = 'existing';
  for (final text in ['n', 'ni', 'ni\u2006hao', '你好']) {
    remote = applyEdit(
        remote, input.update(editing('$sentinel$text', composing: true)));
    expect(remote, 'existing', reason: 'Uncommitted $text');
  }
  remote = applyEdit(remote, input.update(editing('$sentinel你好')));
  expect(remote, 'existing你好');
}

void verifyDeletion() {
  for (final text in ['a1b', '😀']) {
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
  // Editing before a real '1' must not treat it as the padding boundary.
  for (final text in ['a1b', 'x1b', 'a中文c', '']) {
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

// Select the hidden local field, not the remote document.
TextEditingValue selectAll(String text) => editing(text).copyWith(
    selection: TextSelection(baseOffset: 0, extentOffset: text.length));

({String remote, TextEditingValue value}) dispatch(
    IosSoftKeyboardInput input, String remote, TextEditingValue value) {
  final edit = input.update(value);
  final corrected = edit.localValue ?? value;
  if (edit.localValue != null) {
    final echo = input.update(corrected);
    expect([echo.backspaces, echo.text], [0, ''],
        reason: 'Restoring local padding must not send remote input');
  }
  return (remote: applyEdit(remote, edit), value: corrected);
}

void verifyReplacement() {
  final actual = <Object>[];
  final expected = <Object>[];
  for (final text in ['', ' ', ' hello', 'ㄱ']) {
    final input = IosSoftKeyboardInput(sentinel);
    final selected = dispatch(input, 'existing', selectAll(sentinel));
    final result = dispatch(input, selected.remote, editing(text));
    actual.add([result.remote, result.value.text]);
    expected.add(['existing$text', text == 'ㄱ' ? text : '$sentinel$text']);
  }
  expect(actual, expected);
}

void verifyUndoRedo() {
  for (final text in ['abc', ' hello', ' ', 'ㄱ']) {
    final input = IosSoftKeyboardInput(sentinel);
    var result = dispatch(input, 'existing', selectAll(sentinel));
    result = dispatch(input, result.remote, editing(text));
    expect(result.remote, 'existing$text');
    final replacement = result.value;
    result = dispatch(input, result.remote, editing(sentinel));
    expect(result.remote, 'existing', reason: 'Undo replacement');
    result = dispatch(input, result.remote, replacement);
    expect(result.remote, 'existing$text', reason: 'Redo replacement');
    expect(result.value, replacement);
  }
}

void verifyClearAndDelete() {
  final input = IosSoftKeyboardInput(sentinel);
  var result = dispatch(input, 'existing', editing('${sentinel}abc'));
  result = dispatch(input, result.remote, selectAll(result.value.text));
  result = dispatch(input, result.remote, editing(''));
  expect(result.remote, 'existing');
  expect(result.value.text, sentinel);
  result = dispatch(
      input,
      result.remote,
      result.value.copyWith(
          selection: const TextSelection(
              baseOffset: sentinelLength - 1, extentOffset: sentinelLength)));
  result = dispatch(
      input, result.remote, editing(sentinel.substring(0, sentinelLength - 1)));
  expect(result.remote, 'existin');

  final depleted = IosSoftKeyboardInput(sentinel);
  result =
      dispatch(depleted, 'existing${'x' * (sentinelLength - 1)}', editing(' '));
  for (final text in [' ㄱ', '', ' ', ' 가']) {
    if (text.isEmpty) {
      result = dispatch(depleted, result.remote, selectAll(result.value.text));
    }
    result = dispatch(depleted, result.remote, editing(text));
  }
  expect(result.remote, 'existing가',
      reason: 'IME recomposition can temporarily consume the last padding');
}
