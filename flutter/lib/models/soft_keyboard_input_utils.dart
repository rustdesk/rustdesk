// Pure soft-keyboard text-diff helpers for the Android (non-iOS) controller input path.
//
// ponytail: the iOS handler (_handleIOSSoftKeyboardInput, remote_page.dart ~281-318) keeps its
// own inline copy of this diff; it is intentionally not rewired through here. Touching the
// already-correct iOS path purely to deduplicate would add regression risk for no behavioral
// gain. If the diff ever has to change for both platforms, unify then.

/// Computes the edit to push to the remote host given the previous (`oldValue`) and current
/// (`newValue`) soft-keyboard text-field contents, both seeded with the `'1' * N` sentinel.
///
/// Returns the number of `VK_BACK` presses to send (`backspaces`) followed by the literal text
/// to type (`insert`). Mirrors the iOS algorithm at remote_page.dart ~281-318 (minus the
/// iOS-only composing-hold). Diffs on CONTENT, not length, so Hangul/CJK in-place composing
/// mutation (ㅁ→마→만 — same UTF-16 length) is handled instead of silently dropped.
({int backspaces, String insert}) computeSoftKeyboardEdit(
    String oldValue, String newValue) {
  // Align the diff window to the text after the last sentinel '1' in each string.
  var i = newValue.length - 1;
  for (; i >= 0 && newValue[i] != '1'; --i) {}
  var j = oldValue.length - 1;
  for (; j >= 0 && oldValue[j] != '1'; --j) {}
  if (i < j) j = i;
  final subNewValue = newValue.substring(j + 1);
  final subOldValue = oldValue.substring(j + 1);

  // Longest common prefix of the two windows.
  var common = 0;
  for (;
      common < subOldValue.length &&
          common < subNewValue.length &&
          subNewValue[common] == subOldValue[common];
      ++common) {}

  final insert =
      subNewValue.length > common ? subNewValue.substring(common) : '';
  final backspaces = subOldValue.length - common;
  return (backspaces: backspaces, insert: insert);
}

/// Reproduces the Android clipboard guard (remote_page.dart ~331-337): when the sentinel-prefixed
/// buffer is replaced by pasted text (old starts with '1', new does not), treat the old value as
/// empty so the whole new text is sent.
///
/// Both `isNotEmpty` checks are required: `_value` starts empty and seeding the controller text
/// does not fire `onChanged`, so the first keystroke can arrive with an empty `oldValue`; without
/// the guards `oldValue[0]` would throw a RangeError.
String clipboardAdjustedOldValue(String oldValue, String newValue) {
  if (oldValue.isNotEmpty &&
      newValue.isNotEmpty &&
      oldValue[0] == '1' &&
      newValue[0] != '1') {
    return '';
  }
  return oldValue;
}

const _autoInsertedBracketPairs = <String>{
  '""', '()', '[]', '<>', '{}', '”“', '《》', '（）', '【】'
};

/// True when `insert` is a two-character bracket pair appended (no backspaces) onto a non-empty
/// buffer — the case where a host editor auto-inserts the closing bracket, so the whole pair must
/// be sent as a string (sending only the opener lets the host auto-close swallow later input).
/// Mirrors remote_page.dart ~346-356.
bool isAutoInsertedBracketPair(
    String effectiveOldValue, int backspaces, String insert) {
  return effectiveOldValue != '' &&
      backspaces == 0 &&
      _autoInsertedBracketPairs.contains(insert);
}
