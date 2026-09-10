import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

const _sentinel = '1';
const _pinyinMarkedTextSpace = '\u2006';
const _textInputMarkedTextSpace = 0x2004;
const _pinyinMarkedTextSpaceRune = 0x2006;
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

IOSSoftKeyboardInputResult diffIOSSoftKeyboardInput({
  required String previousValue,
  required String currentValue,
  required TextRange composingRange,
  String? previousComposingValue,
  String? previousControllerText,
  TextRange? previousControllerComposingRange,
  int? sentinelPrefixLength,
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

  // Some IMEs commit only the selected prefix and leave the rest composing.
  // Emit the committed prefix first, otherwise the remaining composing text may
  // be replayed as normal input.
  final partialPinyinCommitResult = _partialPinyinCommitResult(
    currentValue: normalizedCurrentValue,
    composingRange: composingRange,
    previousComposingValue: previousComposingValue,
    tails: tails,
  );
  if (partialPinyinCommitResult != null) {
    return partialPinyinCommitResult;
  }
  final partialBopomofoCommitResult = _partialBopomofoCommitResult(
    currentValue: normalizedCurrentValue,
    composingRange: composingRange,
    previousComposingValue: previousComposingValue,
    tails: tails,
  );
  if (partialBopomofoCommitResult != null) {
    return partialBopomofoCommitResult;
  }

  if (_shouldHoldComposingText(
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

  // Korean IME can briefly expose only the sentinel prefix while recomposing.
  // Treat that as a transient controller state, not as remote backspaces.
  final koreanComposingResult = _invalidKoreanComposingResult(
    previousValue: previousValue,
    currentValue: normalizedCurrentValue,
    previousControllerText: previousControllerText,
    sentinelPrefixLength: sentinelPrefixLength,
  );
  if (koreanComposingResult != null) {
    return koreanComposingResult;
  }

  // iOS may clear the composing range without changing controller text. In
  // that case the previous composing span is the committed text to send.
  final collapsedComposingCommitResult =
      _collapsedControllerComposingCommitResult(
    previousValue: previousValue,
    currentValue: normalizedCurrentValue,
    composingRange: composingRange,
    previousControllerText: previousControllerText,
    previousControllerComposingRange: previousControllerComposingRange,
  );
  if (collapsedComposingCommitResult != null) {
    return collapsedComposingCommitResult;
  }

  return IOSSoftKeyboardInputResult(
    nextValue: normalizedCurrentValue,
    nextComposingValue: null,
    actions: _inputActionsForTailChange(tails.previous, tails.current),
  );
}

IOSSoftKeyboardInputResult? _collapsedControllerComposingCommitResult({
  required String previousValue,
  required String currentValue,
  required TextRange composingRange,
  required String? previousControllerText,
  required TextRange? previousControllerComposingRange,
}) {
  final previousText = previousControllerText;
  final previousRange = previousControllerComposingRange;
  if (previousText == null || previousRange == null) return null;
  if (previousText != currentValue) return null;
  if (!_isValidComposingRange(previousText, previousRange)) return null;
  if (_isValidComposingRange(currentValue, composingRange)) return null;

  final composingText = previousText.substring(
    previousRange.start,
    previousRange.end,
  );
  if (composingText.isEmpty) return null;

  final expectedPreviousValue = previousText.replaceRange(
    previousRange.start,
    previousRange.end,
    '',
  );
  if (previousValue != expectedPreviousValue) return null;

  return IOSSoftKeyboardInputResult(
    nextValue: currentValue,
    nextComposingValue: null,
    actions: _inputActionsForTailChange('', composingText),
  );
}

IOSSoftKeyboardInputResult? _partialBopomofoCommitResult({
  required String currentValue,
  required TextRange composingRange,
  required String? previousComposingValue,
  required ({String previous, String current}) tails,
}) {
  if (!_isHeldBopomofoComposingText(previousComposingValue)) return null;
  if (!tails.current.startsWith(tails.previous)) return null;

  final heldText = _heldBopomofoTextAfterPartialCommit(
    currentValue: currentValue,
    composingRange: composingRange,
    previousComposingValue: previousComposingValue!,
    previousTail: tails.previous,
    currentTail: tails.current,
  );
  if (heldText == null || !tails.current.endsWith(heldText)) return null;

  final committedTail = tails.current.substring(
    0,
    tails.current.length - heldText.length,
  );
  final nextValue = currentValue.substring(
    0,
    currentValue.length - heldText.length,
  );
  return IOSSoftKeyboardInputResult(
    nextValue: nextValue,
    nextComposingValue: heldText,
    actions: _inputActionsForTailChange(tails.previous, committedTail),
  );
}

String? _heldBopomofoTextAfterPartialCommit({
  required String currentValue,
  required TextRange composingRange,
  required String previousComposingValue,
  required String previousTail,
  required String currentTail,
}) {
  if (_isValidComposingRange(currentValue, composingRange) &&
      composingRange.end == currentValue.length) {
    final composingText = currentValue.substring(
      composingRange.start,
      composingRange.end,
    );
    if (_isHeldBopomofoComposingText(composingText)) return composingText;
    return _bopomofoSuffixAfterPartialCommit(
      value: composingText,
      previousComposingValue: previousComposingValue,
    );
  }

  final delta = currentTail.substring(previousTail.length);
  return _bopomofoSuffixAfterPartialCommit(
    value: delta,
    previousComposingValue: previousComposingValue,
  );
}

IOSSoftKeyboardInputResult? _partialPinyinCommitResult({
  required String currentValue,
  required TextRange composingRange,
  required String? previousComposingValue,
  required ({String previous, String current}) tails,
}) {
  if (!_isHeldPinyinComposingText(previousComposingValue)) return null;
  if (!tails.current.startsWith(tails.previous)) return null;

  final heldTail = _heldPinyinTailAfterPartialCommit(
    currentValue: currentValue,
    composingRange: composingRange,
    previousComposingValue: previousComposingValue!,
    previousTail: tails.previous,
    currentTail: tails.current,
  );
  if (heldTail == null || !tails.current.endsWith(heldTail.currentSuffix)) {
    return null;
  }

  final committedTail = tails.current.substring(
    0,
    tails.current.length - heldTail.currentSuffix.length,
  );
  final nextValue = currentValue.substring(
    0,
    currentValue.length - heldTail.currentSuffix.length,
  );
  return IOSSoftKeyboardInputResult(
    nextValue: nextValue,
    nextComposingValue: heldTail.composingValue,
    actions: _inputActionsForTailChange(tails.previous, committedTail),
  );
}

({String currentSuffix, String composingValue})?
    _heldPinyinTailAfterPartialCommit({
  required String currentValue,
  required TextRange composingRange,
  required String previousComposingValue,
  required String previousTail,
  required String currentTail,
}) {
  if (_isValidComposingRange(currentValue, composingRange) &&
      composingRange.end == currentValue.length) {
    final composingText = currentValue.substring(
      composingRange.start,
      composingRange.end,
    );
    if (_isHeldPinyinComposingText(composingText)) {
      return (currentSuffix: composingText, composingValue: composingText);
    }
    return _pinyinSuffixAfterPartialCommit(
      value: composingText,
      previousComposingValue: previousComposingValue,
    );
  }

  final delta = currentTail.substring(previousTail.length);
  return _pinyinSuffixAfterPartialCommit(
    value: delta,
    previousComposingValue: previousComposingValue,
  );
}

String _normalizedShortenedKoreanHangulValue({
  required String previousValue,
  required String currentValue,
  required TextRange composingRange,
  required int? sentinelPrefixLength,
}) {
  if (sentinelPrefixLength == null) return currentValue;
  if (_isValidComposingRange(currentValue, composingRange)) return currentValue;

  // Korean composition can temporarily shorten or over-restore the sentinel
  // prefix. Normalize those controller artifacts before calculating the remote
  // input diff.
  final previousPrefixLength = _leadingSentinelLength(previousValue);
  final currentPrefixLength = _leadingSentinelLength(currentValue);
  if (currentPrefixLength <= 0) return currentValue;
  if (currentValue.length == currentPrefixLength) {
    final previousTail = _tailAfterPrefix(previousValue, previousPrefixLength);
    final isShortenedKoreanSentinelReset =
        previousPrefixLength < sentinelPrefixLength &&
            currentPrefixLength < previousPrefixLength &&
            _endsWithKoreanText(previousTail);
    if (isShortenedKoreanSentinelReset) {
      return _sentinel * previousPrefixLength;
    }
    return currentValue;
  }
  if (previousPrefixLength < sentinelPrefixLength) return currentValue;

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
  // The sentinel prefix is local TextField state. Remote actions are based only
  // on the text after the effective sentinel prefix.
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

  // When a shortened sentinel prefix is restored before committed CJK text, do
  // not treat the restored sentinel characters as user input.
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

bool _isHeldPinyinComposingText(String? value) {
  if (value == null || value.isEmpty) return false;
  if (_compositionKind(value) != _CompositionKind.ascii) return false;
  return _isLikelyPinyinComposingText(value);
}

bool _isHeldBopomofoComposingText(String? value) {
  if (value == null || value.isEmpty) return false;
  return _compositionKind(value) == _CompositionKind.bopomofo;
}

String? _bopomofoSuffixAfterPartialCommit({
  required String value,
  required String previousComposingValue,
}) {
  final previous = _normalizedBopomofoComposingText(previousComposingValue);
  if (previous.isEmpty) return null;

  // Mixed text means the candidate prefix was committed while the suffix is
  // still composing. Keep holding the suffix and send only the committed prefix.
  //
  // Example: previous composing text is "ㄆㄨˊㄊㄠˊㄍㄢ". After selecting a
  // candidate, iOS may expose "葡萄ㄍㄢ", where "葡萄" is committed text and
  // "ㄍㄢ" is still marked text. The split point must be on a Unicode scalar
  // boundary so the committed prefix can be sent without replaying the suffix.
  for (final offset in _runeBoundaryOffsets(value)) {
    final committedPrefix = value.substring(0, offset);
    if (!_containsNonBopomofoComposingText(committedPrefix)) continue;

    final suffix = value.substring(offset);
    if (!_isHeldBopomofoComposingText(suffix)) continue;
    if (!previous.endsWith(_normalizedBopomofoComposingText(suffix))) continue;
    return suffix;
  }
  return null;
}

String _normalizedBopomofoComposingText(String value) {
  return value
      .replaceAll(String.fromCharCode(_textInputMarkedTextSpace), '')
      .replaceAll(String.fromCharCode(_pinyinMarkedTextSpaceRune), '')
      .replaceAll(RegExp(r'\s+'), '');
}

bool _containsNonBopomofoComposingText(String value) {
  return value.runes.any((rune) => !_isBopomofoComposingRune(rune));
}

({String currentSuffix, String composingValue})?
    _pinyinSuffixAfterPartialCommit({
  required String value,
  required String previousComposingValue,
}) {
  final previous = _normalizedPinyinComposingText(previousComposingValue);
  if (previous.isEmpty) return null;

  // Pinyin partial selection can return committed text followed by the
  // remaining spelling. Preserve the original spelling separators when possible.
  //
  // Example: previous composing text is "shen me". After selecting a candidate,
  // iOS may expose "什么 shen me" or "什么shme". The committed prefix "什么"
  // should be sent immediately, while the remaining spelling stays held as
  // composing text until the IME commits or clears it.
  for (final offset in _runeBoundaryOffsets(value)) {
    final committedPrefix = value.substring(0, offset);
    if (!_containsNonAscii(committedPrefix)) continue;

    final suffix = value.substring(offset);
    final normalizedSuffix = _normalizedPinyinComposingText(suffix);
    if (!previous.endsWith(normalizedSuffix)) continue;
    if (_isHeldPinyinComposingText(suffix)) {
      return (currentSuffix: suffix, composingValue: suffix);
    }

    final previousSuffix = _previousPinyinComposingSuffix(
      normalizedSuffix: normalizedSuffix,
      previousComposingValue: previousComposingValue,
    );
    if (previousSuffix != null) {
      return (currentSuffix: suffix, composingValue: previousSuffix);
    }
  }
  return null;
}

String? _previousPinyinComposingSuffix({
  required String normalizedSuffix,
  required String previousComposingValue,
}) {
  // When iOS normalizes the visible suffix, recover the original suffix from
  // the previous composing text. This keeps separators such as marked spaces
  // intact for future comparisons.
  //
  // Example: previous "shen\u2006me", current mixed text "什么me". The current
  // suffix normalizes to "me", but the value we keep should be the original
  // previous suffix after the split, not a newly synthesized one.
  for (final offset in _runeBoundaryOffsets(previousComposingValue)) {
    final suffix = previousComposingValue.substring(offset);
    if (_startsWithPinyinSeparator(suffix)) continue;
    if (!_isHeldPinyinComposingText(suffix)) continue;
    if (_normalizedPinyinComposingText(suffix) == normalizedSuffix) {
      return suffix;
    }
  }
  return null;
}

bool _startsWithPinyinSeparator(String value) {
  if (value.isEmpty) return false;
  final rune = value.runes.first;
  return rune == _pinyinMarkedTextSpaceRune ||
      String.fromCharCode(rune).trim().isEmpty;
}

Iterable<int> _runeBoundaryOffsets(String value) sync* {
  // Return substring offsets after each Unicode scalar value, excluding the
  // final string length. This lets callers test every prefix/suffix split
  // without cutting a surrogate pair in half.
  //
  // Examples:
  // - "abc" yields 1, 2 -> "a|bc", "ab|c".
  // - "你hao" yields 1, 2, 3 -> "你|hao", "你h|ao", "你ha|o".
  // - "😀a" yields 2, not 1, because the emoji is a UTF-16 surrogate pair.
  //
  // This is rune-safe, not grapheme-cluster-safe. It protects surrogate pairs,
  // but it does not keep combining-mark sequences such as "e\u0301" together.
  var offset = 0;
  for (final rune in value.runes) {
    offset += String.fromCharCode(rune).length;
    if (offset < value.length) yield offset;
  }
}

bool _containsNonAscii(String value) {
  return value.runes.any((rune) => rune > 0x7F);
}

String _normalizedPinyinComposingText(String value) {
  return value
      .toLowerCase()
      .replaceAll(_pinyinMarkedTextSpace, '')
      .replaceAll(RegExp(r'\s+'), '');
}

bool _isLikelyPinyinComposingText(String value) {
  // ASCII marked text can be normal Latin input or an intermediate Pinyin
  // spelling. We only hold values that look like Pinyin syllable prefixes or a
  // short sequence of initials, so normal words are still sent when composing
  // collapses.
  //
  // Examples held while composing: "ni", "zhong", "lm".
  // Example sent after composing clears: a normal candidate such as "cat".
  final normalized =
      value.toLowerCase().replaceAll(_pinyinMarkedTextSpace, ' ').trim();
  if (normalized.isEmpty) return false;

  final tokens = normalized.split(RegExp(r'\s+'));
  return tokens.every(_isLikelyPinyinToken);
}

bool _isLikelyPinyinToken(String token) {
  return _isPinyinSyllablePrefix(token) ||
      _isPinyinSyllableSequencePrefix(token) ||
      _isShortPinyinInitialSequence(token);
}

bool _isPinyinSyllableSequencePrefix(String token) {
  for (var index = 1; index < token.length; index++) {
    if (!_isCompletePinyinSyllable(token.substring(0, index))) continue;

    final remaining = token.substring(index);
    if (_isPinyinSyllablePrefixWithFinal(remaining) ||
        _isPinyinSyllableSequencePrefix(remaining)) {
      return true;
    }
  }
  return false;
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

bool _isPinyinSyllablePrefixWithFinal(String token) {
  for (final initial in _pinyinInitials) {
    if (!token.startsWith(initial)) continue;

    final finalPrefix = token.substring(initial.length);
    if (finalPrefix.isEmpty) return false;
    return _pinyinFinals
        .any((finalValue) => finalValue.startsWith(finalPrefix));
  }
  return _pinyinFinals.any((finalValue) => finalValue.startsWith(token));
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
  // Classify the marked text by the script shape exposed by iOS. CJK IMEs use
  // different transient forms before the user commits a candidate:
  // - Pinyin and some Japanese romaji stages are ASCII.
  // - Chinese stroke input uses stroke runes such as "一丨".
  // - Traditional Chinese Bopomofo uses "ㄋㄧˇ".
  // - Japanese conversion uses kana such as "にほん".
  // - Korean composition may expose jamo "ㅎㅏ" or a Hangul syllable "한".
  // Anything mixed outside these forms is treated as committed text.
  if (value.runes.every(_isAsciiComposingRune)) {
    return _CompositionKind.ascii;
  }
  if (value.runes.every(_isChineseStrokeRune)) {
    return _CompositionKind.stroke;
  }
  if (value.runes.every(_isBopomofoComposingRune)) {
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
  return rune <= 0x7F || rune == _pinyinMarkedTextSpaceRune;
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

bool _isBopomofoComposingRune(int rune) {
  return _isBopomofoRune(rune) ||
      rune == _textInputMarkedTextSpace ||
      rune == _pinyinMarkedTextSpaceRune;
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

bool _endsWithKoreanText(String value) {
  if (value.isEmpty) return false;
  final lastRune = value.runes.last;
  return _isKoreanJamoRune(lastRune) || _isKoreanHangulRune(lastRune);
}

bool _isKoreanHangulRune(int rune) {
  return rune >= 0xAC00 && rune <= 0xD7AF;
}
