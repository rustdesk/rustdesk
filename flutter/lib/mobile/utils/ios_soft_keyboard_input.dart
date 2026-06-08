import 'package:flutter/services.dart';

const _sentinel = '1';
const _pinyinMarkedTextSpace = '\u2006';
const _maxHeldPinyinInitials = 2;
const _pinyinInitials = <String>[
  'zh',
  'ch',
  'sh',
  'b',
  'p',
  'm',
  'f',
  'd',
  't',
  'n',
  'l',
  'g',
  'k',
  'h',
  'j',
  'q',
  'x',
  'r',
  'z',
  'c',
  's',
  'y',
  'w',
];
const _pinyinFinals = <String>[
  'a',
  'ai',
  'an',
  'ang',
  'ao',
  'e',
  'ei',
  'en',
  'eng',
  'er',
  'i',
  'ia',
  'ian',
  'iang',
  'iao',
  'ie',
  'in',
  'ing',
  'iong',
  'iu',
  'o',
  'ong',
  'ou',
  'u',
  'ua',
  'uai',
  'uan',
  'uang',
  'ue',
  'ui',
  'un',
  'uo',
  'v',
  've',
  'vn',
];

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
  String? previousControllerText,
  int? sentinelPrefixLength,
  bool forceCommitComposingText = false,
}) {
  final normalizedCurrentValue = _normalizedShortenedKoreanHangulValue(
    previousValue: previousValue,
    currentValue: currentValue,
    composingRange: composingRange,
    sentinelPrefixLength: sentinelPrefixLength,
  );
  final tails = _sentinelTails(
    previousValue,
    normalizedCurrentValue,
    sentinelPrefixLength,
  );
  if (!forceCommitComposingText &&
      _shouldHoldComposingText(
        normalizedCurrentValue,
        composingRange,
        previousComposingValue,
      )) {
    final nextComposingValue = normalizedCurrentValue.substring(
      composingRange.start,
      composingRange.end,
    );
    return IOSSoftKeyboardInputResult(
      nextValue: previousValue,
      nextComposingValue: nextComposingValue,
      actions: const [],
    );
  }

  final koreanComposingResult = _invalidKoreanComposingResult(
    previousValue: previousValue,
    currentValue: normalizedCurrentValue,
    previousControllerText: previousControllerText,
    sentinelPrefixLength: sentinelPrefixLength,
  );
  if (!forceCommitComposingText && koreanComposingResult != null) {
    return koreanComposingResult;
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

  final result = IOSSoftKeyboardInputResult(
    nextValue: normalizedCurrentValue,
    nextComposingValue: null,
    actions: _inputActionsForTailChange(tails.previous, tails.current),
  );
  return result;
}

String _normalizedShortenedKoreanHangulValue({
  required String previousValue,
  required String currentValue,
  required TextRange composingRange,
  required int? sentinelPrefixLength,
}) {
  if (sentinelPrefixLength == null) return currentValue;
  if (_isValidComposingRange(currentValue, composingRange)) return currentValue;

  final previousPrefixLength = _leadingSentinelLength(previousValue);
  final currentPrefixLength = _leadingSentinelLength(currentValue);
  if (previousPrefixLength < sentinelPrefixLength) return currentValue;
  if (currentPrefixLength <= 0) return currentValue;
  if (currentValue.length == currentPrefixLength) return currentValue;

  final currentTail = currentValue.substring(currentPrefixLength);
  if (_compositionKind(currentTail) != _CompositionKind.koreanHangul) {
    return currentValue;
  }
  if (currentPrefixLength > sentinelPrefixLength) {
    if (previousPrefixLength == sentinelPrefixLength &&
        previousValue.length == previousPrefixLength) {
      return '${_sentinel * sentinelPrefixLength}$currentTail';
    }
    return currentValue;
  }
  if (currentPrefixLength == sentinelPrefixLength) return currentValue;
  return '${_sentinel * sentinelPrefixLength}$currentTail';
}

({String previous, String current}) _sentinelTails(
  String previousValue,
  String currentValue,
  int? sentinelPrefixLength,
) {
  if (_shouldResetSentinelBaseline(
    previousValue,
    currentValue,
  )) {
    return (previous: '', current: currentValue);
  }

  final restoredTail = _tailAfterRestoredSentinelPrefix(
    previousValue,
    currentValue,
    sentinelPrefixLength,
  );
  if (restoredTail != null) {
    return (previous: '', current: restoredTail);
  }

  final prefixLength = _effectiveSentinelPrefixLength(
    previousValue,
    currentValue,
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
) {
  final previousPrefixLength = _leadingSentinelLength(previousValue);
  if (previousPrefixLength == 0) return false;
  if (_leadingSentinelLength(currentValue) > 0) return false;
  if (previousPrefixLength == _sentinel.length &&
      previousValue.length == previousPrefixLength &&
      currentValue.isEmpty) {
    return false;
  }
  return true;
}

String? _tailAfterRestoredSentinelPrefix(
  String previousValue,
  String currentValue,
  int? sentinelPrefixLength,
) {
  if (sentinelPrefixLength == null) return null;
  final previousPrefixLength = _leadingSentinelLength(previousValue);
  final currentPrefixLength = _leadingSentinelLength(currentValue);
  if (previousPrefixLength == 0) return null;
  if (previousPrefixLength >= currentPrefixLength) return null;
  if (currentPrefixLength > sentinelPrefixLength) return null;
  if (previousValue.length != previousPrefixLength) return null;
  if (currentValue.length <= currentPrefixLength) return null;

  final tail = currentValue.substring(currentPrefixLength);
  if (tail.runes.any((rune) => rune > 0x7F)) return tail;
  return null;
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
  if (kind == _CompositionKind.koreanJamo ||
      kind == _CompositionKind.koreanHangul) {
    return false;
  }

  if (previousComposingValue == null || previousComposingValue.isEmpty) {
    return true;
  }

  return _isComposingTransition(_compositionKind(previousComposingValue), kind);
}

IOSSoftKeyboardInputResult? _invalidKoreanComposingResult({
  required String previousValue,
  required String currentValue,
  required String? previousControllerText,
  required int? sentinelPrefixLength,
}) {
  if (sentinelPrefixLength == null) {
    return null;
  }

  final currentPrefixLength = _leadingSentinelLength(currentValue);
  if (currentValue.length != currentPrefixLength) {
    return null;
  }

  final isShortened = currentPrefixLength < sentinelPrefixLength;
  final isRestoredAfterShortened =
      currentPrefixLength == sentinelPrefixLength &&
          _isShortenedSentinelOnlyValue(
              previousControllerText, sentinelPrefixLength);
  if (!isShortened && !isRestoredAfterShortened) return null;

  final previousSentinelTail =
      _tailAfterPrefix(previousValue, sentinelPrefixLength);
  if (_endsWithKoreanText(previousSentinelTail)) {
    return IOSSoftKeyboardInputResult(
      nextValue: previousValue,
      nextComposingValue: null,
      actions: const [],
    );
  }

  return null;
}

bool _isShortenedSentinelOnlyValue(
  String? value,
  int sentinelPrefixLength,
) {
  if (value == null) return false;
  final prefixLength = _leadingSentinelLength(value);
  return value.length == prefixLength && prefixLength < sentinelPrefixLength;
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
  if (kind != _CompositionKind.ascii) {
    return null;
  }
  if (!_shouldHoldCollapsedAsciiComposingText(
    previousTail: previousTail,
    composingText: continuedValue,
  )) {
    return null;
  }

  final previousKind = _compositionKind(previousComposingValue);
  if (!_isComposingTransition(previousKind, kind)) return null;

  return continuedValue;
}

bool _shouldHoldCollapsedAsciiComposingText({
  required String previousTail,
  required String composingText,
}) {
  if (_containsJapaneseKana(previousTail)) return false;
  return _isLikelyPinyinComposingText(composingText);
}

bool _isLikelyPinyinComposingText(String value) {
  final normalized =
      value.toLowerCase().replaceAll(_pinyinMarkedTextSpace, ' ').trim();
  if (normalized.isEmpty) return false;

  final tokens = normalized.split(RegExp(r'\s+'));
  return tokens.every(_isLikelyPinyinToken);
}

bool _isLikelyPinyinToken(String token) {
  return _isPinyinSyllablePrefix(token) ||
      _isPinyinSyllableWithTrailingInitial(token) ||
      _isShortPinyinInitialSequence(token);
}

bool _isPinyinSyllablePrefix(String token) {
  for (final initial in _pinyinInitials) {
    if (initial.startsWith(token)) return true;
    if (!token.startsWith(initial)) continue;

    final finalPrefix = token.substring(initial.length);
    if (finalPrefix.isEmpty) return true;
    return _pinyinFinals
        .any((finalValue) => finalValue.startsWith(finalPrefix));
  }
  return _pinyinFinals.any((finalValue) => finalValue.startsWith(token));
}

bool _isPinyinSyllableWithTrailingInitial(String token) {
  for (var index = 1; index < token.length; index++) {
    if (!_isCompletePinyinSyllable(token.substring(0, index))) continue;

    final trailing = token.substring(index);
    if (_pinyinInitials.any((initial) => initial.startsWith(trailing))) {
      return true;
    }
  }
  return false;
}

bool _isCompletePinyinSyllable(String token) {
  for (final initial in _pinyinInitials) {
    if (!token.startsWith(initial)) continue;
    return _pinyinFinals.contains(token.substring(initial.length));
  }
  return _pinyinFinals.contains(token);
}

bool _isShortPinyinInitialSequence(String token) {
  var offset = 0;
  var count = 0;
  while (offset < token.length) {
    final initial = _pinyinInitialAt(token, offset);
    if (initial == null) return false;
    offset += initial.length;
    count++;
  }
  return count <= _maxHeldPinyinInitials;
}

String? _pinyinInitialAt(String token, int offset) {
  for (final initial in _pinyinInitials) {
    if (token.startsWith(initial, offset)) return initial;
  }
  return null;
}

int _sentinelPrefixLength(
  String previousValue,
  String currentValue,
) {
  final previousLength = _leadingSentinelLength(previousValue);
  final currentLength = _leadingSentinelLength(currentValue);
  return previousLength < currentLength ? previousLength : currentLength;
}

int _effectiveSentinelPrefixLength(
  String previousValue,
  String currentValue,
  int? prefixLengthLimit,
) {
  final prefixLength = _sentinelPrefixLength(previousValue, currentValue);
  if (prefixLengthLimit == null) return prefixLength;
  return prefixLength < prefixLengthLimit ? prefixLength : prefixLengthLimit;
}

int _leadingSentinelLength(String value) {
  var length = 0;
  while (value.startsWith(_sentinel, length)) {
    length += _sentinel.length;
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
          previous == _CompositionKind.japaneseKanaAscii) &&
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

bool _containsJapaneseKana(String value) {
  return value.runes.any(_isJapaneseKanaRune);
}

bool _isKoreanJamoRune(int rune) {
  return (rune >= 0x1100 && rune <= 0x11FF) ||
      (rune >= 0x3130 && rune <= 0x318F) ||
      (rune >= 0xA960 && rune <= 0xA97F) ||
      (rune >= 0xD7B0 && rune <= 0xD7FF);
}

bool _endsWithKoreanText(String value) {
  if (value.isEmpty) return false;
  final lastRune = value.runes.last;
  return _isKoreanJamoRune(lastRune) || _isKoreanHangulRune(lastRune);
}

bool _isKoreanHangulRune(int rune) {
  return rune >= 0xAC00 && rune <= 0xD7AF;
}
