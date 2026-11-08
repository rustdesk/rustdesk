import 'dart:math' as math;

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' show StringCharacters;

class IosKeyboardEdit {
  const IosKeyboardEdit({this.backspaces = 0, this.text = '', this.localValue});

  final int backspaces;
  final String text;
  final TextEditingValue? localValue;
}

class IosSoftKeyboardInput {
  IosSoftKeyboardInput(this._initialText)
      : _previousText = _initialText,
        _prefixLength = _initialText.length;

  final String _initialText;
  String _previousText;
  String _committedText = '';
  int _prefixLength;

  void reset() {
    _previousText = _initialText;
    _committedText = '';
    _prefixLength = _initialText.length;
  }

  IosKeyboardEdit update(TextEditingValue value) {
    final localValue = _clampSelection(value);
    final prefixBackspaces = _updatePrefix(value);
    final committed = _withoutComposingText(value);
    _previousText = value.text;
    if (committed == _committedText) {
      return IosKeyboardEdit(
          backspaces: prefixBackspaces +
              (prefixBackspaces == 0 ? 0 : committed.characters.length),
          text: prefixBackspaces == 0 ? '' : committed,
          localValue: _replenishPrefix(value) ?? localValue);
    }
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
      localValue: _replenishPrefix(value) ?? localValue,
    );
  }

  TextEditingValue? _replenishPrefix(TextEditingValue value) {
    if (_prefixLength != 0 || value.text.isNotEmpty) return null;
    reset();
    return TextEditingValue(
      text: _initialText,
      selection: TextSelection.collapsed(offset: _prefixLength),
    );
  }

  TextEditingValue? _clampSelection(TextEditingValue value) {
    if (value.text != _previousText ||
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
    // A deletion containing real input may temporarily remove padding too.
    // Keep its boundary so restoring that padding cannot type it remotely.
    if (text.length < _previousText.length &&
        _previousText.length <= _prefixLength) {
      final backspaces = _previousText.length - text.length;
      _prefixLength = text.length;
      return backspaces;
    }
    return 0;
  }

  int _backspacesBeforeCommittedText(
      TextEditingValue value, int matchingPrefixLength) {
    final prefix = value.selection.extentOffset;
    final removed = _prefixLength - prefix;
    if (!value.selection.isCollapsed ||
        prefix < 0 ||
        prefix > matchingPrefixLength ||
        (value.isComposingRangeValid && !value.composing.isCollapsed) ||
        value.text.length + removed != _previousText.length ||
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
    final composingStart = math.max(start, value.composing.start);
    final composingEnd = math.max(start, value.composing.end);
    return value.text.substring(start, composingStart) +
        value.text.substring(composingEnd);
  }
}
