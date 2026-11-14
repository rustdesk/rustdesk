import 'package:flutter/services.dart';

/// The character the hidden soft keyboard field is pre-filled with.
///
/// An empty editable field cannot report a Backspace, so the field always
/// holds a run of these for the keyboard to delete.
const String kSoftKeyboardSentinelChar = '1';

/// What has to happen in response to a local editing change.
///
/// Interpreters only describe it. Sending is done by the dispatcher and the
/// field is written by its owner, so the interpretation logic stays free of
/// FFI and can be executed in unit tests.
sealed class SoftKeyboardAction {
  const SoftKeyboardAction();

  /// Whether this is sent to the remote side. The others act on the local
  /// field only.
  bool get isRemote;
}

/// Press Backspace [count] times.
class SoftKeyboardBackspace extends SoftKeyboardAction {
  const SoftKeyboardBackspace(this.count) : assert(count > 0);

  final int count;

  @override
  bool get isRemote => true;

  @override
  bool operator ==(Object other) =>
      other is SoftKeyboardBackspace && other.count == count;

  @override
  int get hashCode => Object.hash(SoftKeyboardBackspace, count);

  @override
  String toString() => 'Backspace($count)';
}

/// Send [text] to the remote side.
///
/// Whether it goes out as a single key or as a text sequence is decided by the
/// dispatcher from the text length, the same way it always was.
class SoftKeyboardText extends SoftKeyboardAction {
  const SoftKeyboardText(this.text) : assert(text != '');

  final String text;

  @override
  bool get isRemote => true;

  @override
  bool operator ==(Object other) =>
      other is SoftKeyboardText && other.text == text;

  @override
  int get hashCode => Object.hash(SoftKeyboardText, text);

  @override
  String toString() => 'Text(${_escape(text)})';
}

/// Re-create the hidden field. Only the legacy Android handler asks for this,
/// after it has sent a pair of brackets or quotes.
class SoftKeyboardResetField extends SoftKeyboardAction {
  const SoftKeyboardResetField();

  @override
  bool get isRemote => false;

  @override
  bool operator ==(Object other) => other is SoftKeyboardResetField;

  @override
  int get hashCode => (SoftKeyboardResetField).hashCode;

  @override
  String toString() => 'ResetField';
}

/// Write [value] to the hidden field. Nothing is sent.
///
/// For an interpreter that has to correct the field itself, for instance to
/// put back padding the keyboard used up, or to move the caret to the end.
/// The interpreter is given the corrected value like any other and must not
/// answer it with remote operations.
class SoftKeyboardSetField extends SoftKeyboardAction {
  const SoftKeyboardSetField(this.value);

  final TextEditingValue value;

  @override
  bool get isRemote => false;

  @override
  bool operator ==(Object other) =>
      other is SoftKeyboardSetField && other.value == value;

  @override
  int get hashCode => Object.hash(SoftKeyboardSetField, value);

  // Without the value: it holds what was typed.
  @override
  String toString() => 'SetField';
}

/// Turns the editing values of the hidden field into remote operations.
///
/// An interpreter starts from the value the field has when it is created.
/// When the app changes the field, a new one is created.
abstract class SoftKeyboardInterpreter {
  /// Short identifier, recorded in traces.
  String get name;

  /// Consumes one editing value and returns what has to be sent for it.
  ///
  /// Must be called for every value the controller reports, including the
  /// ones where only the selection or the composing range changed.
  List<SoftKeyboardAction> update(TextEditingValue value);
}

String _escape(String s) => s.replaceAll('\n', r'\n');
