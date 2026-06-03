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

bool shouldForceCommitIOSComposingText({
  required String? previousControllerText,
  required TextRange? previousComposingRange,
  required TextEditingValue currentValue,
}) {
  final previousRange = previousComposingRange;
  if (previousControllerText == null || previousRange == null) return false;
  if (previousControllerText != currentValue.text) return false;
  if (!_isValidComposingRange(previousControllerText, previousRange)) {
    return false;
  }
  if (_isValidComposingRange(currentValue.text, currentValue.composing)) {
    return false;
  }

  final previousComposingText = previousControllerText.substring(
    previousRange.start,
    previousRange.end,
  );
  final kind = _compositionKind(previousComposingText);
  return kind != _CompositionKind.ascii && kind != _CompositionKind.koreanJamo;
}

IOSSoftKeyboardInputResult diffIOSSoftKeyboardInput({
  required String previousValue,
  required String currentValue,
  required TextRange composingRange,
  String? previousComposingValue,
  int? sentinelPrefixLength,
  bool forceCommitComposingText = false,
  String sentinel = '1',
}) {
  final tails = _sentinelTails(
    previousValue,
    currentValue,
    sentinel,
    sentinelPrefixLength,
  );

  if (_isOnlySentinelPrefixRestore(
    previousValue,
    currentValue,
    sentinel,
    sentinelPrefixLength,
  )) {
    return IOSSoftKeyboardInputResult(
      nextValue: currentValue,
      nextComposingValue: null,
      actions: const [],
    );
  }

  if (!forceCommitComposingText &&
      _shouldHoldComposingText(
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

  final continuedComposingValue = _continuedComposingValue(
    tails.previous,
    tails.current,
    previousComposingValue,
  );
  if (!forceCommitComposingText && continuedComposingValue != null) {
    return IOSSoftKeyboardInputResult(
      nextValue: previousValue,
      nextComposingValue: continuedComposingValue,
      actions: const [],
    );
  }

  final collapsedComposingValue = _collapsedComposingValue(
    tails.previous,
    tails.current,
    previousComposingValue,
  );
  if (!forceCommitComposingText && collapsedComposingValue != null) {
    return IOSSoftKeyboardInputResult(
      nextValue: previousValue,
      nextComposingValue: collapsedComposingValue,
      actions: const [],
    );
  }

  return IOSSoftKeyboardInputResult(
    nextValue: currentValue,
    nextComposingValue: null,
    actions: _inputActionsForTailChange(tails.previous, tails.current),
  );
}

({String previous, String current}) _sentinelTails(
  String previousValue,
  String currentValue,
  String sentinel,
  int? sentinelPrefixLength,
) {
  if (_shouldResetSentinelBaseline(
    previousValue,
    currentValue,
    sentinel,
    sentinelPrefixLength,
  )) {
    return (previous: '', current: currentValue);
  }

  final prefixLength = _effectiveSentinelPrefixLength(
    previousValue,
    currentValue,
    sentinel,
    sentinelPrefixLength,
  );
  return (
    previous: _tailAfterPrefix(previousValue, prefixLength),
    current: _tailAfterPrefix(currentValue, prefixLength),
  );
}

bool _shouldResetSentinelBaseline(
  String previousValue,
  String currentValue,
  String sentinel,
  int? sentinelPrefixLength,
) {
  if (sentinelPrefixLength == null || sentinel.isEmpty) return false;
  final previousPrefixLength = _leadingSentinelLength(previousValue, sentinel);
  if (previousPrefixLength == 0) return false;
  if (_leadingSentinelLength(currentValue, sentinel) > 0) return false;
  if (previousPrefixLength == sentinel.length &&
      previousValue.length == previousPrefixLength &&
      currentValue.isEmpty) {
    return false;
  }
  return true;
}

List<IOSSoftKeyboardInputAction> _inputActionsForTailChange(
  String previousTail,
  String currentTail,
) {
  final commonPrefixLength = _commonPrefixLengthByRunes(
    currentTail,
    previousTail,
  );
  final actions = <IOSSoftKeyboardInputAction>[];

  for (final _ in previousTail.substring(commonPrefixLength).runes) {
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
  return actions;
}

bool _isOnlySentinelPrefixRestore(
  String previousValue,
  String currentValue,
  String sentinel,
  int? sentinelPrefixLength,
) {
  final prefixLengthLimit = sentinelPrefixLength;
  if (prefixLengthLimit == null || sentinel.isEmpty) return false;
  final previousPrefixLength = _leadingSentinelLength(previousValue, sentinel);
  final currentPrefixLength = _leadingSentinelLength(currentValue, sentinel);
  if (currentPrefixLength <= previousPrefixLength) return false;
  if (currentPrefixLength > prefixLengthLimit) return false;
  return previousValue.length == previousPrefixLength &&
      currentValue.length == currentPrefixLength;
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

String? _continuedComposingValue(
  String previousTail,
  String currentTail,
  String? previousComposingValue,
) {
  if (previousComposingValue == null || previousComposingValue.isEmpty) {
    return null;
  }
  if (!currentTail.startsWith(previousTail)) return null;

  final continuedValue = currentTail.substring(previousTail.length);
  if (continuedValue.isEmpty) return null;

  final kind = _compositionKind(continuedValue);
  if (!_shouldHoldCollapsedKind(kind)) return null;

  final previousKind = _compositionKind(previousComposingValue);
  if (!_isComposingTransition(previousKind, kind)) return null;

  return continuedValue;
}

String? _collapsedComposingValue(
  String previousTail,
  String currentTail,
  String? previousComposingValue,
) {
  if (previousComposingValue == null || previousComposingValue.isEmpty) {
    return null;
  }
  if (!currentTail.startsWith(previousTail)) return null;

  final composingTail = currentTail.substring(previousTail.length);
  if (composingTail != previousComposingValue) return null;

  final kind = _compositionKind(composingTail);
  if (_shouldHoldCollapsedKind(kind)) return composingTail;
  return null;
}

bool _shouldHoldCollapsedKind(_CompositionKind kind) {
  return kind == _CompositionKind.ascii || kind == _CompositionKind.koreanJamo;
}

int _sentinelPrefixLength(
  String previousValue,
  String currentValue,
  String sentinel,
) {
  if (sentinel.isEmpty) return 0;
  final previousLength = _leadingSentinelLength(previousValue, sentinel);
  final currentLength = _leadingSentinelLength(currentValue, sentinel);
  return previousLength < currentLength ? previousLength : currentLength;
}

int _effectiveSentinelPrefixLength(
  String previousValue,
  String currentValue,
  String sentinel,
  int? prefixLengthLimit,
) {
  final prefixLength = _sentinelPrefixLength(
    previousValue,
    currentValue,
    sentinel,
  );
  if (prefixLengthLimit == null) return prefixLength;
  return prefixLength < prefixLengthLimit ? prefixLength : prefixLengthLimit;
}

int _leadingSentinelLength(String value, String sentinel) {
  var length = 0;
  while (value.startsWith(sentinel, length)) {
    length += sentinel.length;
  }
  return length;
}

String _tailAfterPrefix(String value, int prefixLength) {
  if (prefixLength <= 0) return value;
  if (prefixLength >= value.length) return '';
  return value.substring(prefixLength);
}

int _commonPrefixLengthByRunes(String a, String b) {
  final aIterator = RuneIterator(a);
  final bIterator = RuneIterator(b);
  var common = 0;
  while (aIterator.moveNext() && bIterator.moveNext()) {
    if (aIterator.current != bIterator.current) break;
    common += aIterator.currentSize;
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
  japaneseKanaAscii,
  koreanJamo,
  koreanHangul,
  committedText,
}

_CompositionKind _compositionKind(String value) {
  if (value.runes.every(_isAsciiComposingRune)) {
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
  if (value.runes.every(
    (rune) => rune <= 0x7F || _isJapaneseKanaRune(rune),
  )) {
    return _CompositionKind.japaneseKanaAscii;
  }
  if (value.runes.every(_isKoreanJamoRune)) {
    return _CompositionKind.koreanJamo;
  }
  if (value.runes.every(_isKoreanHangulRune)) {
    return _CompositionKind.koreanHangul;
  }
  return _CompositionKind.committedText;
}

bool _isAsciiComposingRune(int rune) {
  // iOS IME can emit 0x2006 during composing, so treat it as ASCII-like.
  return rune <= 0x7F || rune == 0x2006;
}

bool _isComposingTransition(
  _CompositionKind previous,
  _CompositionKind current,
) {
  if (previous == current) return true;
  if (previous == _CompositionKind.ascii &&
      (current == _CompositionKind.stroke ||
          current == _CompositionKind.bopomofo ||
          current == _CompositionKind.japaneseKana ||
          current == _CompositionKind.japaneseKanaAscii ||
          current == _CompositionKind.koreanJamo ||
          current == _CompositionKind.koreanHangul)) {
    return true;
  }
  if ((previous == _CompositionKind.japaneseKana ||
          previous == _CompositionKind.japaneseKanaAscii ||
          previous == _CompositionKind.ascii) &&
      (current == _CompositionKind.japaneseKana ||
          current == _CompositionKind.japaneseKanaAscii)) {
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
      (rune >= 0x31A0 && rune <= 0x31BF) ||
      rune == 0x02C7 ||
      rune == 0x02CA ||
      rune == 0x02CB ||
      rune == 0x02D9;
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
