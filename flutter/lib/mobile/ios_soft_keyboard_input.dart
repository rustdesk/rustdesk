import 'package:flutter/services.dart';

class IosSoftKeyboardUpdate {
  const IosSoftKeyboardUpdate({
    required this.nextValue,
    required this.backspaces,
    required this.insertText,
  });

  final String nextValue;
  final int backspaces;
  final String insertText;
}

IosSoftKeyboardUpdate prepareIosSoftKeyboardUpdate({
  required String previousValue,
  required TextEditingValue editingValue,
}) {
  var committedValue = editingValue.text;
  if (editingValue.isComposingRangeValid) {
    // iOS includes the uncommitted marked text in the controller value.
    committedValue = committedValue.substring(0, editingValue.composing.start);
  }

  var newAnchor = committedValue.length - 1;
  for (; newAnchor >= 0 && committedValue[newAnchor] != '1'; --newAnchor) {}
  var oldAnchor = previousValue.length - 1;
  for (; oldAnchor >= 0 && previousValue[oldAnchor] != '1'; --oldAnchor) {}
  if (newAnchor < oldAnchor) oldAnchor = newAnchor;

  final newSuffix = committedValue.substring(oldAnchor + 1);
  final oldSuffix = previousValue.substring(oldAnchor + 1);
  var common = 0;
  while (common < oldSuffix.length &&
      common < newSuffix.length &&
      newSuffix[common] == oldSuffix[common]) {
    ++common;
  }

  return IosSoftKeyboardUpdate(
    nextValue: committedValue,
    backspaces: oldSuffix.length - common,
    insertText: newSuffix.substring(common),
  );
}
