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
SoftKeyboardInterpreter newIos() => SoftKeyboardReducer.ios(ev(''));
SoftKeyboardInterpreter newAndroid() => SoftKeyboardReducer.android(ev(''));

/// Creates the interpreter a trace was recorded with, from the `interpreter`
/// field of its header.
SoftKeyboardInterpreter interpreterByName(
    String name, TextEditingValue initial) {
  switch (name) {
    case 'legacy-ios':
      return LegacyIosSoftKeyboard(initial);
    case 'legacy-android':
      return LegacyAndroidSoftKeyboard(initial);
    case 'hold+tail':
      return SoftKeyboardReducer.ios(initial);
    case 'forward+mirror':
      return SoftKeyboardReducer.android(initial);
    case 'hold+mirror':
      // Not used by the app. Holding composing text on Android needs a way
      // to tell a keyboard that only composes real compositions from one
      // that also composes plain words.
      return SoftKeyboardReducer(
        policy: ComposingPolicy.hold,
        diff: MirrorSoftKeyboardDiff(),
        initial: initial,
      );
  }
  throw ArgumentError.value(name, 'name', 'unknown soft keyboard interpreter');
}

/// The inverse of [encodeSoftKeyboardValue] for values encoded with text.
TextEditingValue decodeValue(Map<String, Object?> json) {
  final sel = (json['sel'] as List).cast<int>();
  final comp = (json['comp'] as List).cast<int>();
  return TextEditingValue(
    text: kSoftKeyboardSentinelChar * (json['lead'] as int) +
        (json['rest'] as String),
    selection: TextSelection(baseOffset: sel[0], extentOffset: sel[1]),
    composing: TextRange(start: comp[0], end: comp[1]),
  );
}

/// The inverse of [encodeSoftKeyboardActions] for actions encoded with text.
List<SoftKeyboardAction> decodeActions(List<Object?> json) {
  return [
    for (final action in json.cast<List>())
      switch (action[0]) {
        'bs' => SoftKeyboardBackspace(action[1] as int),
        'text' => SoftKeyboardText(action[1] as String),
        'reset' => const SoftKeyboardResetField(),
        'set' => SoftKeyboardSetField(
            decodeValue((action[1] as Map).cast<String, Object?>())),
        _ => throw FormatException('unknown soft keyboard action: $action'),
      }
  ];
}

/// A stand-in for the remote text field: Backspace deletes before the caret,
/// which is at the end, and text is appended.
class RemoteText {
  RemoteText({required this.backspaceDeletesCodePoint});

  /// Whether Backspace deletes a whole code point, or one UTF-16 code unit.
  /// The old iOS tail diff counts code units.
  final bool backspaceDeletesCodePoint;

  String text = '';

  /// Backspaces that found nothing to delete: they went past what was typed.
  int overrun = 0;

  void apply(List<SoftKeyboardAction> actions) {
    for (final action in actions) {
      switch (action) {
        case SoftKeyboardBackspace(:final count):
          for (var i = 0; i < count; i++) {
            if (text.isEmpty) {
              overrun++;
            } else if (backspaceDeletesCodePoint) {
              final runes = text.runes.toList()..removeLast();
              text = String.fromCharCodes(runes);
            } else {
              text = text.substring(0, text.length - 1);
            }
          }
        case SoftKeyboardText(:final text):
          this.text += text;
        case SoftKeyboardResetField():
        case SoftKeyboardSetField():
          break;
      }
    }
  }
}
