import 'package:flutter/services.dart';

enum IOSSoftKeyboardInputActionType {
  backspace,
  inputKey,
  inputText,
}

class IOSSoftKeyboardInputAction {
  final IOSSoftKeyboardInputActionType type;
  final String value;

  const IOSSoftKeyboardInputAction.backspace()
      : type = IOSSoftKeyboardInputActionType.backspace,
        value = '';

  const IOSSoftKeyboardInputAction.inputKey(this.value)
      : type = IOSSoftKeyboardInputActionType.inputKey;

  const IOSSoftKeyboardInputAction.inputText(this.value)
      : type = IOSSoftKeyboardInputActionType.inputText;

  @override
  bool operator ==(Object other) {
    return other is IOSSoftKeyboardInputAction &&
        other.type == type &&
        other.value == value;
  }

  @override
  int get hashCode => Object.hash(type, value);

  @override
  String toString() => 'IOSSoftKeyboardInputAction($type, $value)';
}

class IOSSoftKeyboardInputResult {
  final String nextValue;
  final String? nextComposingValue;
  final List<IOSSoftKeyboardInputAction> actions;

  const IOSSoftKeyboardInputResult({
    required this.nextValue,
    required this.nextComposingValue,
    required this.actions,
  });
}

IOSSoftKeyboardInputResult diffIOSSoftKeyboardInput({
  required String previousValue,
  required String currentValue,
  required TextRange composingRange,
  String? previousComposingValue,
  String sentinel = '1',
}) {
  if (_shouldHoldComposingText(
    currentValue,
    composingRange,
    previousComposingValue,
  )) {
    return IOSSoftKeyboardInputResult(
      nextValue: previousValue,
      nextComposingValue: currentValue.substring(
        composingRange.start,
        composingRange.end,
      ),
      actions: const [],
    );
  }

  var currentSentinelIndex = _lastSentinelIndex(currentValue, sentinel);
  var previousSentinelIndex = _lastSentinelIndex(previousValue, sentinel);
  if (currentSentinelIndex < previousSentinelIndex) {
    previousSentinelIndex = currentSentinelIndex;
  }

  final currentTail = currentValue.substring(currentSentinelIndex + 1);
  final previousTail = previousValue.substring(previousSentinelIndex + 1);
  final commonPrefixLength = _commonPrefixLength(currentTail, previousTail);
  final actions = <IOSSoftKeyboardInputAction>[];

  final deleteCount = previousTail.length - commonPrefixLength;
  for (var i = 0; i < deleteCount; i++) {
    actions.add(const IOSSoftKeyboardInputAction.backspace());
  }

  final insertedText = currentTail.substring(commonPrefixLength);
  if (insertedText.isNotEmpty) {
    if (_shouldInputAsText(insertedText)) {
      actions.add(IOSSoftKeyboardInputAction.inputText(insertedText));
    } else {
      actions.add(IOSSoftKeyboardInputAction.inputKey(insertedText));
    }
  }

  return IOSSoftKeyboardInputResult(
    nextValue: currentValue,
    nextComposingValue: null,
    actions: actions,
  );
}

bool _isValidComposingRange(String value, TextRange range) {
  return range.isValid &&
      !range.isCollapsed &&
      range.isNormalized &&
      range.end <= value.length;
}

bool _shouldHoldComposingText(
  String value,
  TextRange range,
  String? previousComposingValue,
) {
  if (!_isValidComposingRange(value, range)) return false;

  final composingValue = value.substring(range.start, range.end);
  final kind = _compositionKind(composingValue);
  if (kind == _CompositionKind.committedText) return false;

  if (previousComposingValue == null || previousComposingValue.isEmpty) {
    return true;
  }

  return _isComposingTransition(_compositionKind(previousComposingValue), kind);
}

int _lastSentinelIndex(String value, String sentinel) {
  if (sentinel.isEmpty) return -1;
  return value.lastIndexOf(sentinel);
}

int _commonPrefixLength(String a, String b) {
  var common = 0;
  while (common < a.length && common < b.length && a[common] == b[common]) {
    common++;
  }
  return common;
}

bool _shouldInputAsText(String value) {
  if (value.length != 1) return true;
  return value.runes.any((rune) => rune > 0x7F);
}

enum _CompositionKind {
  ascii,
  stroke,
  bopomofo,
  japaneseKana,
  koreanJamo,
  koreanHangul,
  committedText,
}

_CompositionKind _compositionKind(String value) {
  if (value.runes.every((rune) => rune <= 0x7F)) {
    return _CompositionKind.ascii;
  }
  if (value.runes.every(_isChineseStrokeRune)) {
    return _CompositionKind.stroke;
  }
  if (value.runes.every(_isBopomofoRune)) {
    return _CompositionKind.bopomofo;
  }
  if (value.runes.every(_isJapaneseKanaRune)) {
    return _CompositionKind.japaneseKana;
  }
  if (value.runes.every(_isKoreanJamoRune)) {
    return _CompositionKind.koreanJamo;
  }
  if (value.runes.every(_isKoreanHangulRune)) {
    return _CompositionKind.koreanHangul;
  }
  return _CompositionKind.committedText;
}

bool _isComposingTransition(
  _CompositionKind previous,
  _CompositionKind current,
) {
  if (previous == current) return true;
  if (previous == _CompositionKind.ascii &&
      (current == _CompositionKind.japaneseKana ||
          current == _CompositionKind.koreanJamo ||
          current == _CompositionKind.koreanHangul)) {
    return true;
  }
  if ((previous == _CompositionKind.koreanJamo ||
          previous == _CompositionKind.koreanHangul) &&
      (current == _CompositionKind.koreanJamo ||
          current == _CompositionKind.koreanHangul)) {
    return true;
  }
  return false;
}

bool _isChineseStrokeRune(int rune) {
  return rune == 0x4E00 ||
      rune == 0x4E28 ||
      rune == 0x4E3F ||
      rune == 0x4E36 ||
      rune == 0x4E59 ||
      (rune >= 0x31C0 && rune <= 0x31EF);
}

bool _isBopomofoRune(int rune) {
  return (rune >= 0x3100 && rune <= 0x312F) ||
      (rune >= 0x31A0 && rune <= 0x31BF);
}

bool _isJapaneseKanaRune(int rune) {
  return (rune >= 0x3040 && rune <= 0x309F) ||
      (rune >= 0x30A0 && rune <= 0x30FF) ||
      (rune >= 0x31F0 && rune <= 0x31FF) ||
      (rune >= 0xFF66 && rune <= 0xFF9F);
}

bool _isKoreanJamoRune(int rune) {
  return (rune >= 0x1100 && rune <= 0x11FF) ||
      (rune >= 0x3130 && rune <= 0x318F) ||
      (rune >= 0xA960 && rune <= 0xA97F) ||
      (rune >= 0xD7B0 && rune <= 0xD7FF);
}

bool _isKoreanHangulRune(int rune) {
  return rune >= 0xAC00 && rune <= 0xD7AF;
}
