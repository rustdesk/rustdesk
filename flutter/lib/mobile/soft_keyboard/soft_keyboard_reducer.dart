import 'dart:math' as math;

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' show CharacterRange, StringCharacters;

import 'legacy_soft_keyboard.dart';
import 'soft_keyboard_actions.dart';

/// What to do with text the keyboard still marks as composing.
enum ComposingPolicy {
  /// Keep composing text local and send only what has been committed.
  ///
  /// Nothing provisional reaches the remote side. Right when the composing
  /// range is only used for real compositions, as on iOS.
  hold,

  /// Ignore the composing range and mirror the field as it is typed.
  ///
  /// Provisional text is sent and corrected afterwards. Needed where the
  /// keyboard also marks ordinary words as composing, as Gboard does for
  /// English.
  forward,
}

/// Turns one change of the mirrored text into remote operations.
abstract class SoftKeyboardDiff {
  /// Diagnostic identifier.
  String get name;

  /// Called once, with the text the interpretation starts from.
  void start(String text);

  List<SoftKeyboardAction> apply(String oldText, String newText);

  /// Why the last [apply] did not mirror the edit, if it did not.
  String? get lastNote;
}

/// The mobile soft keyboard interpreter.
///
/// It keeps a baseline, the text the remote side has been told about, and
/// derives a target from each editing value. The difference between the two
/// is handed to a [SoftKeyboardDiff]. With [ComposingPolicy.hold] the target
/// excludes the composing range, so a composition produces nothing until it is
/// committed, and a partial commit produces exactly the committed part.
class SoftKeyboardReducer implements SoftKeyboardInterpreter {
  SoftKeyboardReducer({
    required this.policy,
    required SoftKeyboardDiff diff,
    required TextEditingValue initial,
  })  : _diff = diff,
        _last = initial,
        _baseline = initial.text {
    _diff.start(initial.text);
  }

  /// iOS: hold composing text, and keep the tail diff the iOS handler has
  /// always used so that everything typed without a composing range is
  /// handled exactly as before.
  factory SoftKeyboardReducer.ios(TextEditingValue initial) =>
      SoftKeyboardReducer(
        policy: ComposingPolicy.hold,
        diff: LegacyTailSoftKeyboardDiff(),
        initial: initial,
      );

  /// Android: mirror the field as it is typed, with a diff that also handles
  /// replacements and edits that are not at the end.
  factory SoftKeyboardReducer.android(TextEditingValue initial) =>
      SoftKeyboardReducer(
        policy: ComposingPolicy.forward,
        diff: MirrorSoftKeyboardDiff(),
        initial: initial,
      );

  final ComposingPolicy policy;
  final SoftKeyboardDiff _diff;

  String _baseline;
  TextEditingValue _last;
  String? _note;

  @override
  String get name => '${policy.name}+${_diff.name}';

  @override
  String? get lastNote => _note;

  @override
  List<SoftKeyboardAction> update(TextEditingValue value) {
    _note = null;
    if (value == _last) return const [];
    _last = value;

    var target = value.text;
    String? note;
    final composing = _activeComposing(value);
    if (policy == ComposingPolicy.hold && composing != null) {
      final committed =
          value.text.replaceRange(composing.start, composing.end, '');
      if (_composingCoversSentText(committed, composing.start)) {
        // The keyboard marked text that is already on the remote side as
        // composing. It cannot be held back any more, and deleting it now
        // would only be undone by the commit, so follow it as it changes.
        note = 'composing covers sent text';
      } else {
        target = committed;
      }
    }
    _note = note;
    if (target == _baseline) return const [];

    final actions = _diff.apply(_baseline, target);
    _baseline = target;
    _note = _diff.lastNote ?? note;
    return actions;
  }

  /// The composing range, if the keyboard is in the middle of a composition.
  static TextRange? _activeComposing(TextEditingValue value) {
    if (!value.isComposingRangeValid || value.composing.isCollapsed) {
      return null;
    }
    return value.composing;
  }

  /// Whether [committed], the text without its composing range, is the
  /// baseline with a piece cut out exactly where the composing range starts.
  bool _composingCoversSentText(String committed, int composingStart) {
    if (committed.length >= _baseline.length) return false;
    return _baseline.startsWith(committed.substring(0, composingStart)) &&
        _baseline.endsWith(committed.substring(composingStart));
  }
}

/// [legacyTailDiff] as a [SoftKeyboardDiff].
class LegacyTailSoftKeyboardDiff implements SoftKeyboardDiff {
  @override
  String get name => 'tail';

  @override
  String? get lastNote => null;

  @override
  void start(String text) {}

  @override
  List<SoftKeyboardAction> apply(String oldText, String newText) {
    final diff = legacyTailDiff(oldText, newText);
    return [
      if (diff.backspaces > 0) SoftKeyboardBackspace(diff.backspaces),
      if (diff.inserted.isNotEmpty) SoftKeyboardText(diff.inserted),
    ];
  }
}

/// A diff that keeps the remote text equal to what was typed since the
/// keyboard was opened.
///
/// The only thing the client can do to remote text is press Backspace at the
/// remote caret and type. So a local edit can be reproduced only if it lies in
/// text this client typed, which is assumed to still end at the remote caret:
/// delete back to where the edit starts, then type the rest again.
///
/// That replay is limited to [maxReplay] characters, to text in which one
/// Backspace is known to delete one character, and it never crosses a line
/// break. An edit outside those limits is not guessed: typing and Backspace
/// are applied at the caret, a replacement is dropped, and a deletion is cut
/// off at the limit. [lastNote] says which, and the text up to that point is
/// never replayed afterwards.
class MirrorSoftKeyboardDiff implements SoftKeyboardDiff {
  MirrorSoftKeyboardDiff({this.maxReplay = 64});

  /// The most Backspaces a single edit may produce.
  final int maxReplay;

  /// Number of leading sentinel characters. They were never sent, and
  /// deleting any number of them at once is a single Backspace.
  int _sentinel = 0;

  /// Text before this index is not known to be on the remote side, so it is
  /// never replayed. Starts at the end of the sentinel.
  int _floor = 0;

  String? _note;

  @override
  String get name => 'mirror';

  @override
  String? get lastNote => _note;

  @override
  void start(String text) {
    var n = 0;
    while (n < text.length && text[n] == kSoftKeyboardSentinelChar) {
      n++;
    }
    _sentinel = n;
    _floor = n;
  }

  @override
  List<SoftKeyboardAction> apply(String oldText, String newText) {
    _note = null;
    if (oldText == newText) return const [];

    final prefix = _commonPrefix(oldText, newText);
    final suffix = _commonSuffix(oldText, newText, prefix);
    final removedEnd = oldText.length - suffix;
    final removed = oldText.substring(prefix, removedEnd);
    final inserted = newText.substring(prefix, newText.length - suffix);

    if (prefix >= _floor) {
      if (suffix == 0 && removed.isEmpty) {
        return _type(inserted);
      }
      if (suffix == 0 && inserted.isEmpty) {
        // The end of the text was deleted: Backspace was pressed.
        return [_backspace(removed.characters.length, newText.length)];
      }
      // A replacement, or an edit with text after it. Reproduce it by
      // deleting back to where it starts and typing everything after that
      // again.
      final count = _replayCount(oldText, prefix);
      if (count != null) {
        return [
          if (count > 0) SoftKeyboardBackspace(count),
          ..._type(inserted, oldText.substring(removedEnd)),
        ];
      }
    }

    // From here on the edit touches text that cannot be replayed.
    final inSentinel = prefix < _sentinel;
    var backspaces = 0;
    if (inSentinel) {
      backspaces = 1;
      if (removedEnd > _sentinel) {
        backspaces +=
            oldText.substring(_sentinel, removedEnd).characters.length;
      }
    } else if (removed.isNotEmpty) {
      backspaces = removed.characters.length;
    }
    _sentinel = math.min(_sentinel, prefix);

    if (suffix == 0 && inserted.isEmpty) {
      // Backspace reached past what is mirrored. It is still just Backspace.
      _floor = math.min(_floor, prefix);
      return [_backspace(backspaces, newText.length)];
    }

    _floor = newText.length;
    if (inSentinel && inserted.isNotEmpty) {
      // The sentinel was replaced or typed into. Nothing on the remote side
      // corresponds to it, so the new text is all that can be sent.
      _note = 'at caret: edit inside the sentinel';
      return _type(inserted);
    }
    if (removed.isNotEmpty && inserted.isNotEmpty) {
      // Backspace cannot be trusted to remove exactly the replaced text, and
      // typing the replacement next to it would only add to the damage.
      _note = 'dropped: replacement cannot be replayed';
      return const [];
    }
    // Typing or Backspace with text after the caret. They mean what they
    // always mean, at the remote caret.
    _note = 'at caret: text after the edit cannot be replayed';
    return inserted.isEmpty
        ? [_backspace(backspaces, newText.length)]
        : _type(inserted);
  }

  /// [inserted] and then [after], the text that follows it, as text to type.
  ///
  /// A line break at the end of what was inserted is Return being pressed,
  /// possibly in the same edit as a correction of the word before it. It is
  /// sent on its own, so that it goes out as the key and not as a character
  /// in a string.
  static List<SoftKeyboardAction> _type(String inserted, [String after = '']) {
    if (!inserted.endsWith('\n') || inserted.length + after.length == 1) {
      return [SoftKeyboardText(inserted + after)];
    }
    final before = inserted.substring(0, inserted.length - 1);
    return [
      if (before.isNotEmpty) SoftKeyboardText(before),
      const SoftKeyboardText('\n'),
      if (after.isNotEmpty) SoftKeyboardText(after),
    ];
  }

  /// [count] Backspaces, or as many as are allowed.
  ///
  /// When that is fewer, the remote text is left longer than the local text,
  /// which has [newLength] characters now. Nothing up to there is replayed
  /// afterwards.
  SoftKeyboardBackspace _backspace(int count, int newLength) {
    if (count <= maxReplay) return SoftKeyboardBackspace(count);
    _floor = newLength;
    _note = 'clamped: more Backspaces than the limit';
    return SoftKeyboardBackspace(maxReplay);
  }

  /// How many Backspaces delete [text] from [from] to its end, or null if
  /// that is not known well enough to replay.
  int? _replayCount(String text, int from) {
    // Every character has to be a single code point. For longer clusters,
    // such as emoji sequences, applications disagree on what one Backspace
    // deletes.
    var count = 0;
    for (final char in text.substring(from).characters) {
      if (char.runes.length != 1) return null;
      // Return is a key press, not text. Deleting a line break does not undo
      // it, and typing it again would press Return again.
      if (char.codeUnitAt(0) < 0x20) return null;
      if (++count > maxReplay) return null;
    }
    // And the edit must not start inside a cluster.
    if (CharacterRange.at(text, from).stringBeforeLength != from) return null;
    return count;
  }

  static int _commonPrefix(String a, String b) {
    final max = math.min(a.length, b.length);
    var n = 0;
    while (n < max && a.codeUnitAt(n) == b.codeUnitAt(n)) {
      n++;
    }
    // Do not split a surrogate pair.
    if (n > 0 && _isHighSurrogate(a.codeUnitAt(n - 1))) n--;
    return n;
  }

  static int _commonSuffix(String a, String b, int prefix) {
    final max = math.min(a.length, b.length) - prefix;
    var n = 0;
    while (n < max &&
        a.codeUnitAt(a.length - 1 - n) == b.codeUnitAt(b.length - 1 - n)) {
      n++;
    }
    // Do not split a surrogate pair.
    if (n > 0 && _isLowSurrogate(a.codeUnitAt(a.length - n))) n--;
    return n;
  }

  static bool _isHighSurrogate(int unit) => (unit & 0xFC00) == 0xD800;

  static bool _isLowSurrogate(int unit) => (unit & 0xFC00) == 0xDC00;
}
