import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

import 'soft_keyboard_actions.dart';

/// How much of the soft keyboard input is written to the log.
enum SoftKeyboardTraceLevel {
  /// Nothing. The default.
  off,

  /// Structure only: lengths, ranges and action types. Never any text. It
  /// can show where an input sequence went wrong but cannot be replayed.
  meta,

  /// Everything, including the text that was typed. For development with
  /// made-up input only. A trace at this level can be replayed in a test.
  full,
}

/// Build with `--dart-define=SOFT_KEYBOARD_TRACE=meta` (or `full`) to turn
/// tracing on. There is deliberately no runtime switch for it.
const String _traceFromEnvironment =
    String.fromEnvironment('SOFT_KEYBOARD_TRACE');

SoftKeyboardTraceLevel parseSoftKeyboardTraceLevel(String value) {
  for (final level in SoftKeyboardTraceLevel.values) {
    if (level.name == value) return level;
  }
  return SoftKeyboardTraceLevel.off;
}

/// Writes one JSON object per line: a header whenever an interpreter starts,
/// then one line per editing value with the actions derived from it.
///
/// The log lines of a [SoftKeyboardTraceLevel.full] trace can be saved as a
/// fixture as they are. The replay test ignores whatever the log puts in
/// front of the JSON, and starts over at every header.
class SoftKeyboardTrace {
  SoftKeyboardTrace(
      {SoftKeyboardTraceLevel? level, void Function(String)? sink})
      : level = level ?? parseSoftKeyboardTraceLevel(_traceFromEnvironment),
        _sink = sink ?? _printLine;

  static const String tag = '[soft-keyboard]';

  final SoftKeyboardTraceLevel level;
  final void Function(String line) _sink;
  int _seq = 0;

  bool get enabled => level != SoftKeyboardTraceLevel.off;

  bool get _withText => level == SoftKeyboardTraceLevel.full;

  /// Records that [interpreter] starts from [initial].
  void start(SoftKeyboardInterpreter interpreter, TextEditingValue initial,
      {required String platform}) {
    if (!enabled) return;
    _seq = 0;
    _sink(jsonEncode({
      'trace': 'soft-keyboard',
      'v': 1,
      'level': level.name,
      'platform': platform,
      'interpreter': interpreter.name,
      'synthetic': false,
      'initial': encodeSoftKeyboardValue(initial, withText: _withText),
    }));
  }

  /// Records one editing value and what was derived from it.
  void record(TextEditingValue value, List<SoftKeyboardAction> actions,
      {String? note}) {
    if (!enabled) return;
    _sink(jsonEncode({
      'n': ++_seq,
      ...encodeSoftKeyboardValue(value, withText: _withText),
      'act': encodeSoftKeyboardActions(actions, withText: _withText),
      if (note != null) 'note': note,
    }));
  }

  static void _printLine(String line) => debugPrint('$tag $line');
}

/// Encodes an editing value.
///
/// With text, the sentinel is not spelled out: `lead` is the number of leading
/// sentinel characters and `rest` whatever follows them. Without text there
/// is only the length, because a typed sentinel character cannot be told from
/// the sentinel and splitting the two would give it away.
Map<String, Object?> encodeSoftKeyboardValue(TextEditingValue value,
    {required bool withText}) {
  final text = value.text;
  var lead = 0;
  while (lead < text.length && text[lead] == kSoftKeyboardSentinelChar) {
    lead++;
  }
  return {
    if (withText) ...{'lead': lead, 'rest': text.substring(lead)} else
      'len': text.length,
    'sel': [value.selection.baseOffset, value.selection.extentOffset],
    'comp': [value.composing.start, value.composing.end],
  };
}

/// Encodes actions as `["bs", count]`, `["text", text]`, `["reset"]` and
/// `["set", value]`. Without text, text is replaced by its length.
List<Object?> encodeSoftKeyboardActions(List<SoftKeyboardAction> actions,
    {required bool withText}) {
  return [
    for (final action in actions)
      switch (action) {
        SoftKeyboardBackspace(:final count) => ['bs', count],
        SoftKeyboardText(:final text) => [
            'text',
            withText ? text : text.length
          ],
        SoftKeyboardResetField() => ['reset'],
        SoftKeyboardSetField(:final value) => [
            'set',
            encodeSoftKeyboardValue(value, withText: withText)
          ],
      }
  ];
}
