import 'package:flutter/services.dart';

import 'soft_keyboard_actions.dart';

/// The result of [legacyTailDiff].
class LegacyTailDiff {
  const LegacyTailDiff(this.backspaces, this.inserted);

  /// How many UTF-16 code units of the old tail have to be deleted.
  final int backspaces;

  /// What has to be typed after the deletion. May be empty.
  final String inserted;
}

/// The tail diff the iOS handler has always used.
///
/// Both values are cut after their last sentinel character, then compared by
/// common prefix. Lengths are UTF-16 code units.
LegacyTailDiff legacyTailDiff(String oldValue, String newValue) {
  var i = newValue.length - 1;
  for (; i >= 0 && newValue[i] != kSoftKeyboardSentinelChar; --i) {}
  var j = oldValue.length - 1;
  for (; j >= 0 && oldValue[j] != kSoftKeyboardSentinelChar; --j) {}
  if (i < j) j = i;
  final subNewValue = newValue.substring(j + 1);
  final subOldValue = oldValue.substring(j + 1);

  // get common prefix of subNewValue and subOldValue
  var common = 0;
  for (;
      common < subOldValue.length &&
          common < subNewValue.length &&
          subNewValue[common] == subOldValue[common];
      ++common) {}

  return LegacyTailDiff(
    subOldValue.length - common,
    subNewValue.length > common ? subNewValue.substring(common) : '',
  );
}

/// `_handleIOSSoftKeyboardInput` as it was in `remote_page.dart`, returning
/// actions instead of sending them.
///
/// Do not fix bugs here.
class LegacyIosSoftKeyboard implements SoftKeyboardInterpreter {
  LegacyIosSoftKeyboard(TextEditingValue initial)
      : _value = initial.text,
        _lastText = initial.text;

  String _value;
  String _lastText;

  @override
  String get name => 'legacy-ios';

  @override
  List<SoftKeyboardAction> update(TextEditingValue value) {
    final newValue = value.text;
    // This used to be driven by `onChanged`, which is only called when the
    // text changes.
    if (newValue == _lastText) return const [];
    _lastText = newValue;

    final oldValue = _value;
    _value = newValue;
    final diff = legacyTailDiff(oldValue, newValue);

    // Set the value to the old value and early return if is still composing. (1 && 2)
    // 1. The composing range is valid
    // 2. The new string is shorter than the composing range.
    if (value.isComposingRangeValid) {
      final composingLength = value.composing.end - value.composing.start;
      if (composingLength > diff.inserted.length) {
        _value = oldValue;
        return const [];
      }
    }

    return [
      // Delete the different part in the old value.
      if (diff.backspaces > 0) SoftKeyboardBackspace(diff.backspaces),
      // Input the new string.
      if (diff.inserted.isNotEmpty) SoftKeyboardText(diff.inserted),
    ];
  }
}

/// `_handleNonIOSSoftKeyboardInput` as it was in `remote_page.dart`, returning
/// actions instead of sending them.
///
/// Do not fix bugs here.
class LegacyAndroidSoftKeyboard implements SoftKeyboardInterpreter {
  LegacyAndroidSoftKeyboard(TextEditingValue initial) : _value = initial.text;

  String _value;

  @override
  String get name => 'legacy-android';

  @override
  List<SoftKeyboardAction> update(TextEditingValue value) {
    final newValue = value.text;
    // This used to be driven by `onChanged`, which is only called when the
    // text changes.
    if (newValue == _value) return const [];

    var oldValue = _value;
    _value = newValue;
    if (oldValue.isNotEmpty &&
        newValue.isNotEmpty &&
        oldValue[0] == kSoftKeyboardSentinelChar &&
        newValue[0] != kSoftKeyboardSentinelChar) {
      // clipboard
      oldValue = '';
    }
    // This length-only diff assumes the IME appends at the end. If an IME
    // leaves the caret inside paired punctuation, later insertions can replay
    // the closing symbol: "()" -> "(a)" sends ")" instead of "a". This is a
    // client text-diff issue, independent of the host keyboard layout.
    if (newValue.length == oldValue.length) {
      // ?
      return const [];
    } else if (newValue.length < oldValue.length) {
      return const [SoftKeyboardBackspace(1)];
    } else {
      final content = newValue.substring(oldValue.length);
      if (content.length > 1) {
        if (oldValue != '' &&
            content.length == 2 &&
            (content == '""' ||
                content == '()' ||
                content == '[]' ||
                content == '<>' ||
                content == "{}" ||
                content == '”“' ||
                content == '《》' ||
                content == '（）' ||
                content == '【】')) {
          // can not only input content[0], because when input ], [ are also auo insert, which cause ] never be input
          return [SoftKeyboardText(content), const SoftKeyboardResetField()];
        }
      }
      return [SoftKeyboardText(content)];
    }
  }
}
