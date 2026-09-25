import 'package:flutter/services.dart';

({int backspaces, String text})? getIOSSoftKeyboardEdit(
    String oldValue, TextEditingValue newValue) {
  if (newValue.isComposingRangeValid || oldValue == newValue.text) {
    return null;
  }

  final newText = newValue.text;
  final lastNewSentinel = newText.lastIndexOf('1');
  var lastOldSentinel = oldValue.lastIndexOf('1');
  if (lastNewSentinel < lastOldSentinel) {
    lastOldSentinel = lastNewSentinel;
  }
  final newSuffix = newText.substring(lastOldSentinel + 1);
  final oldSuffix = oldValue.substring(lastOldSentinel + 1);

  var common = 0;
  while (common < oldSuffix.length &&
      common < newSuffix.length &&
      oldSuffix[common] == newSuffix[common]) {
    common++;
  }

  return (
    backspaces: oldSuffix.length - common,
    text: newSuffix.substring(common),
  );
}
