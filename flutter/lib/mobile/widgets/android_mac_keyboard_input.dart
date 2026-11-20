import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

class AndroidMacKeyboardInput extends TextInputFormatter {
  AndroidMacKeyboardInput({required this.onEdit});

  final void Function(int backspaces, String text) onEdit;
  static const _hangulCompositionDelay = Duration(milliseconds: 150);
  Timer? _pendingSyllable;
  String _sentText = '';

  // Keep a character for the IME to delete when the remote field has focus.
  static const _backspaceMarker = '\u200B';
  static const initialValue = TextEditingValue(
    text: _backspaceMarker,
    selection: TextSelection.collapsed(offset: _backspaceMarker.length),
  );

  void reset() {
    _pendingSyllable?.cancel();
    _sentText = '';
  }

  @override
  TextEditingValue formatEditUpdate(
      TextEditingValue oldValue, TextEditingValue newValue) {
    _pendingSyllable?.cancel();
    if (newValue.text.isEmpty && oldValue.text == _backspaceMarker) {
      onEdit(1, '');
      return initialValue;
    }

    final text = _inputText(newValue.text);
    if (text.isNotEmpty &&
        newValue.isComposingRangeValid &&
        !newValue.composing.isCollapsed &&
        newValue.composing.end == newValue.text.length) {
      final characters = text.characters;
      final lastCodePoint = characters.last.runes.first;
      final isHangulSyllable =
          lastCodePoint >= 0xAC00 && lastCodePoint <= 0xD7A3;
      final isHangulJamo = lastCodePoint >= 0x3131 && lastCodePoint <= 0x318E;
      if (isHangulSyllable || isHangulJamo) {
        // Gboard may mark the whole word as composing. Send the finished
        // prefix and give the last syllable time to settle before typing it.
        final prefix = characters.skipLast(1).toString();
        if (prefix.startsWith(_sentText)) _sendText(prefix);
        if (isHangulSyllable) {
          _pendingSyllable =
              Timer(_hangulCompositionDelay, () => _sendText(text));
        }
        return newValue;
      }
    }

    _sendText(text);
    return newValue.text.isEmpty ? initialValue : newValue;
  }

  void _sendText(String text) {
    if (text == _sentText) return;
    final previous = _sentText.characters;
    final current = text.characters;
    final oldChars = previous.iterator;
    final newChars = current.iterator;
    var common = 0;
    while (oldChars.moveNext() &&
        newChars.moveNext() &&
        oldChars.current == newChars.current) {
      common++;
    }

    final backspaces = previous.length - common;
    _sentText = text;
    onEdit(backspaces, current.skip(common).toString());
  }

  static String _inputText(String text) => text.startsWith(_backspaceMarker)
      ? text.substring(_backspaceMarker.length)
      : text;
}
