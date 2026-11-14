import 'package:flutter/services.dart';
import 'package:flutter_hbb/mobile/soft_keyboard/soft_keyboard.dart';

/// The hidden field as it is when the keyboard opens.
final String sentinel = kSoftKeyboardSentinelChar * 1024;

/// Builds an editing value from a compact description of what follows the
/// sentinel.
///
/// `[` and `]` enclose the composing range and `|` marks the caret, which is
/// at the end when not given. `ev('你[hao]')` is the committed `你` followed by
/// `hao` still being composed.
TextEditingValue ev(String spec, {int lead = 1024}) {
  final text = StringBuffer(kSoftKeyboardSentinelChar * lead);
  int? composingStart;
  int? composingEnd;
  int? caret;
  for (final rune in spec.runes) {
    final char = String.fromCharCode(rune);
    if (char == '[') {
      composingStart = text.length;
    } else if (char == ']') {
      composingEnd = text.length;
    } else if (char == '|') {
      caret = text.length;
    } else {
      text.write(char);
    }
  }
  return TextEditingValue(
    text: text.toString(),
    selection: TextSelection.collapsed(offset: caret ?? text.length),
    composing: composingStart == null
        ? TextRange.empty
        : TextRange(start: composingStart, end: composingEnd!),
  );
}

/// Like [ev], for text that contains `[`, `]` or `|` itself.
TextEditingValue plain(String rest, {int lead = 1024, int? caret}) {
  final text = kSoftKeyboardSentinelChar * lead + rest;
  return TextEditingValue(
    text: text,
    selection: TextSelection.collapsed(
        offset: caret == null ? text.length : lead + caret),
  );
}

/// Renders actions as short tokens: the text itself, `<BS>` for one Backspace
/// and `<BS*n>` for several, `<RESET>` for a field reset, `<SET>` for a
/// correction of the field.
List<String> tokens(List<SoftKeyboardAction> actions) => [
      for (final action in actions)
        switch (action) {
          SoftKeyboardBackspace(:final count) =>
            count == 1 ? '<BS>' : '<BS*$count>',
          SoftKeyboardText(:final text) => text,
          SoftKeyboardResetField() => '<RESET>',
          SoftKeyboardSetField() => '<SET>',
        }
    ];

/// Feeds [values] to [interpreter] and returns everything it sent, in order.
List<String> sent(
    SoftKeyboardInterpreter interpreter, List<TextEditingValue> values) {
  return [for (final value in values) ...tokens(interpreter.update(value))];
}

/// Like [sent], for values written as [ev] descriptions.
List<String> typed(SoftKeyboardInterpreter interpreter, List<String> specs) =>
    sent(interpreter, [for (final spec in specs) ev(spec)]);

SoftKeyboardInterpreter legacyIos() => LegacyIosSoftKeyboard(ev(''));
SoftKeyboardInterpreter legacyAndroid() => LegacyAndroidSoftKeyboard(ev(''));
