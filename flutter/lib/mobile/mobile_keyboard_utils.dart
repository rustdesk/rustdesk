import 'package:characters/characters.dart';

class SoftKeyboardDiff {
  final int backspaceCount;
  final String newStr;
  final bool isStringInput;
  final bool shouldReset;

  const SoftKeyboardDiff({
    required this.backspaceCount,
    required this.newStr,
    required this.isStringInput,
    this.shouldReset = false,
  });
}

/// Resolves the diff between old and new soft keyboard input buffer values.
/// Returns null if the IME is currently in a composing (pre-edit) state,
/// in which case no events should be dispatched to the remote host.
SoftKeyboardDiff? resolveMobileSoftKeyboardDiff({
  required String oldValue,
  required String newValue,
  required bool isComposing,
}) {
  if (isComposing) {
    return null;
  }

  // If oldValue has sentinel padding and newValue was completely replaced
  // (e.g. paste from clipboard or select all), reset oldValue to prevent
  // emitting backspaces for the sentinel padding.
  if (oldValue.isNotEmpty &&
      oldValue[0] == '1' &&
      newValue.isNotEmpty &&
      newValue[0] != '1') {
    oldValue = '';
  }

  var i = newValue.length - 1;
  for (; i >= 0 && newValue[i] != '1'; --i) {}
  var j = oldValue.length - 1;
  for (; j >= 0 && oldValue[j] != '1'; --j) {}
  if (i < j) j = i;
  var subNewValue = newValue.substring(j + 1);
  var subOldValue = oldValue.substring(j + 1);

  var common = 0;
  for (;
      common < subOldValue.length &&
          common < subNewValue.length &&
          subNewValue[common] == subOldValue[common];
      ++common) {}

  var newStr = "";
  if (subNewValue.length > common) {
    newStr = subNewValue.substring(common);
  }

  final backspaceCount = subOldValue.substring(common).characters.length;
  final isNonAscii = newStr.codeUnits.any((c) => c > 127);
  final isStringInput = newStr.length > 1 || isNonAscii;

  bool shouldReset = false;
  if (isStringInput &&
      oldValue != '' &&
      newStr.length == 2 &&
      (newStr == '""' ||
          newStr == '()' ||
          newStr == '[]' ||
          newStr == '<>' ||
          newStr == "{}" ||
          newStr == '”“' ||
          newStr == '《》' ||
          newStr == '（）' ||
          newStr == '【】')) {
    shouldReset = true;
  }

  return SoftKeyboardDiff(
    backspaceCount: backspaceCount,
    newStr: newStr,
    isStringInput: isStringInput,
    shouldReset: shouldReset,
  );
}
