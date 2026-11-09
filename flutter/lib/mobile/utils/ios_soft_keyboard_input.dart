import 'dart:math' as math;

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' show StringCharacters;

class IosKeyboardEdit {
  const IosKeyboardEdit({this.backspaces = 0, this.text = '', this.localValue});

  final int backspaces;
  final String text;
  // Local controller correction; this text must not be sent to the peer.
  final TextEditingValue? localValue;
}

/// Converts the hidden iOS field into Backspace/text edits at the remote caret.
/// Padding and active composition are local; the field is not the remote document.
class IosSoftKeyboardInput {
  static const _singleCharacterLength = 1;

  IosSoftKeyboardInput(this._initialText)
      : _previousValue = TextEditingValue(text: _initialText),
        _prefixLength = _initialText.length;

  final String _initialText;
  TextEditingValue _previousValue;
  String _committedText = '';
  // UTF-16 padding boundary, retained across temporary IME padding removal.
  int _prefixLength;

  void reset() {
    _previousValue = TextEditingValue(text: _initialText);
    _committedText = '';
    _prefixLength = _initialText.length;
  }

  IosKeyboardEdit update(TextEditingValue value) {
    final localValue = _clampSelection(value);
    final hadPrefix = _prefixLength != 0;
    final prefixBackspaces = _updatePrefix(value);
    final committed = _withoutComposingText(value);
    _previousValue = value;
    if (committed == _committedText) {
      // The remote caret stays after the suffix. Deleting before it requires
      // removing the suffix, deleting preceding text, then replaying the suffix.
      return IosKeyboardEdit(
          backspaces: prefixBackspaces +
              (prefixBackspaces == 0 ? 0 : committed.characters.length),
          text: prefixBackspaces == 0 ? '' : committed,
          localValue: _replenishPrefix(value, hadPrefix) ?? localValue);
    }
    // Count graphemes, not UTF-16 units, to avoid extra Backspaces for emoji.
    final previous = _committedText.characters.toList();
    final current = committed.characters.toList();
    var common = 0;
    while (common < previous.length &&
        common < current.length &&
        previous[common] == current[common]) {
      common++;
    }
    _committedText = committed;
    return IosKeyboardEdit(
      backspaces: prefixBackspaces + previous.length - common,
      text: current.skip(common).join(),
      localValue: _replenishPrefix(value, hadPrefix) ?? localValue,
    );
  }

  TextEditingValue? _replenishPrefix(TextEditingValue value, bool hadPrefix) {
    if (_prefixLength != 0) return null;
    // Rewriting a typed character resets unmarked Korean/Telex composition.
    // Once the IME takes over the whole field, leave later keystrokes untouched
    // until it is empty; a bulk replacement can be padded while ownership is known.
    if (value.text.isNotEmpty &&
        (!hadPrefix ||
            (value.isComposingRangeValid && !value.composing.isCollapsed) ||
            (value.text.characters.length == _singleCharacterLength &&
                value.text != ' '))) {
      return null;
    }
    _prefixLength = _initialText.length;
    // Cache the refill before the controller assignment reenters update().
    _previousValue = value.copyWith(
      text: _initialText + value.text,
      selection: value.selection.isValid
          ? value.selection.copyWith(
              baseOffset: value.selection.baseOffset + _prefixLength,
              extentOffset: value.selection.extentOffset + _prefixLength)
          : TextSelection.collapsed(offset: _prefixLength + value.text.length),
      composing: value.isComposingRangeValid
          ? TextRange(
              start: value.composing.start + _prefixLength,
              end: value.composing.end + _prefixLength)
          : TextRange.empty,
    );
    return _previousValue;
  }

  TextEditingValue? _clampSelection(TextEditingValue value) {
    // Only constrain navigation; rewriting an IME edit can cancel composition.
    if (value.text != _previousValue.text ||
        !value.selection.isValid ||
        (value.isComposingRangeValid && !value.composing.isCollapsed)) {
      return null;
    }
    final boundary = math.min(_prefixLength, value.text.length);
    if (value.selection.start >= boundary) return null;
    // iOS selects the deletion range before removing text. Do not cancel it.
    if (!value.selection.isCollapsed && value.selection.end >= boundary) {
      return null;
    }
    return value.copyWith(
      selection: value.selection.copyWith(
        baseOffset: math.max(boundary, value.selection.baseOffset),
        extentOffset: math.max(boundary, value.selection.extentOffset),
      ),
    );
  }

  int _updatePrefix(TextEditingValue value) {
    final text = value.text;
    if (_updateReplacementPrefix(value)) return 0;
    // User-typed leading spaces must not become part of the owned padding.
    final limit = math.min(_prefixLength, text.length);
    var prefix = 0;
    while (prefix < limit && text[prefix] == _initialText[prefix]) {
      prefix++;
    }
    final backspaces = _committedText.isEmpty
        ? 0
        : _backspacesBeforeCommittedText(value, prefix);
    if (prefix < limit || backspaces > 0) {
      _prefixLength = backspaces > 0 ? _prefixLength - backspaces : prefix;
      return backspaces;
    }
    // Korean/Telex may remove padding with a letter, then restore it. Only count
    // this length decrease for padding-only values, or recomposition deletes host text.
    if (text.length < _previousValue.text.length &&
        _previousValue.text.length <= _prefixLength) {
      final backspaces = _previousValue.text.length - text.length;
      _prefixLength = text.length;
      return backspaces;
    }
    return 0;
  }

  bool _updateReplacementPrefix(TextEditingValue value) {
    // Undo may restore the original local padding; it is not committed input.
    if (_prefixLength == 0 && value.text == _initialText) {
      _prefixLength = _initialText.length;
      return true;
    }
    final selection = _previousValue.selection;
    // Whole-field replacement owns its leading spaces as input, not padding.
    // Exclude the last pad: IMEs may select it together with recomposed text.
    if (value.text != _previousValue.text &&
        selection.start == 0 &&
        selection.end == _previousValue.text.length &&
        _prefixLength > _singleCharacterLength) {
      _prefixLength = 0;
      return true;
    }
    return false;
  }

  int _backspacesBeforeCommittedText(
      TextEditingValue value, int matchingPrefixLength) {
    // The suffix may start with spaces, so locate the boundary using the caret.
    // An unchanged suffix distinguishes this deletion from text replacement.
    final prefix = value.selection.extentOffset;
    final removed = _prefixLength - prefix;
    if (!value.selection.isCollapsed ||
        prefix < 0 ||
        prefix > matchingPrefixLength ||
        (value.isComposingRangeValid && !value.composing.isCollapsed) ||
        value.text.length + removed != _previousValue.text.length ||
        value.text.substring(prefix) != _committedText) {
      return 0;
    }
    return removed;
  }

  String _withoutComposingText(TextEditingValue value) {
    final start = math.min(_prefixLength, value.text.length);
    if (!value.isComposingRangeValid || value.composing.isCollapsed) {
      return value.text.substring(start);
    }
    // Partial candidate selection commits text while the rest still composes.
    final composingStart = math.max(start, value.composing.start);
    final composingEnd = math.max(start, value.composing.end);
    return value.text.substring(start, composingStart) +
        value.text.substring(composingEnd);
  }
}
