import 'package:flutter/widgets.dart';
import 'package:flutter_hbb/mobile/utils/ios_soft_keyboard_input.dart';
import 'package:flutter_test/flutter_test.dart';

import 'ios_soft_keyboard_input_test.dart'
    show applyEdit, editing, sentinel, sentinelLength;

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

void main() {
  test('whole-field replacement preserves host text and leading whitespace',
      verifyReplacement);
  test('Undo and Redo after replacing only padding keep it local',
      verifyUndoRedo);
  test('clearing padding and text keeps the next Backspace usable',
      verifyClearAndDelete);
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
