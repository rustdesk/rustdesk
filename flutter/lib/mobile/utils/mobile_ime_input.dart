import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' show StringCharacters;

class MobileImeEdit {
  const MobileImeEdit(
      {this.text = '',
      this.backspaces = 0,
      this.moveBefore = 0,
      this.moveAfter = 0,
      this.syncPlatform = false,
      this.resetContext = false});

  final String text;
  final int backspaces;
  final int moveBefore;
  final int moveAfter;
  final bool syncPlatform;
  final bool resetContext;
}

class MobileImeInput {
  MobileImeInput({this.forwardComposing = false});

  final bool forwardComposing;

  // iOS needs nonempty input for Backspace. A word separator keeps this owned
  // character outside Korean recomposition; a zero-width character does not.
  static const boundary = ' ';
  static const initialValue = TextEditingValue(
    text: boundary,
    selection: TextSelection.collapsed(offset: boundary.length),
  );

  TextEditingValue value = initialValue;
  String _sent = '';
  int _sentCursor = 0;
  int? _boundaryOffset = 0;

  bool get composing =>
      value.isComposingRangeValid && !value.composing.isCollapsed;

  void reset() {
    value = initialValue;
    _sent = '';
    _sentCursor = 0;
    _boundaryOffset = 0;
  }

  MobileImeEdit update(List<TextEditingDelta> deltas) {
    var deletedBoundary = false;
    final hadBoundary = _boundaryOffset == 0;
    for (var index = 0; index < deltas.length; index++) {
      final delta = deltas[index];
      _alignPlatformText(delta.oldText);
      final wasEmpty = value.text == boundary && _boundaryOffset == 0;
      _trackBoundary(delta);
      if (hadBoundary &&
          _boundaryOffset == null &&
          _restoresBoundary(deltas, index)) {
        _boundaryOffset = 0;
      }
      value = delta.apply(value);
      deletedBoundary |= wasEmpty && value.text.isEmpty;
    }
    final hold = composing && !forwardComposing;
    if (hold && value.composing.end < value.text.length) {
      return const MobileImeEdit();
    }
    var target =
        hold ? value.text.substring(0, value.composing.start) : value.text;
    final boundaryOffset = _boundaryOffset;
    if (boundaryOffset != null && boundaryOffset < target.length) {
      target = target.replaceRange(
          boundaryOffset, boundaryOffset + boundary.length, '');
    }
    if (hold && _sent.startsWith(target)) return const MobileImeEdit();
    final sync = !composing && _boundaryOffset == null;
    final cursor = _cursor(target);
    final edit = _difference(target,
        cursor: cursor,
        deletedBoundary: deletedBoundary && value.text.isEmpty && !composing,
        sync: sync);
    _sent = target;
    _sentCursor = cursor;
    if (sync) {
      value = _withBoundary(value);
      _boundaryOffset = 0;
    }
    return edit;
  }

  bool _restoresBoundary(List<TextEditingDelta> deltas, int index) {
    if (index + 1 >= deltas.length) return false;
    final delta = deltas[index];
    final next = deltas[index + 1];
    // UIKit recomposition can delete the whole buffer, then reinsert its word
    // separator and the edited word separately in the same native transaction.
    return delta is TextEditingDeltaInsertion &&
        delta.oldText.isEmpty &&
        delta.textInserted == boundary &&
        next is TextEditingDeltaInsertion &&
        next.oldText == boundary &&
        next.insertionOffset == boundary.length;
  }

  void _alignPlatformText(String oldText) {
    if (oldText == value.text) return;
    // An already queued native edit can precede the restored boundary.
    if (_boundaryOffset == 0 && value.text == boundary + oldText) {
      _boundaryOffset = null;
      return;
    }
    throw StateError('IME edit does not match the current input context');
  }

  void _trackBoundary(TextEditingDelta delta) {
    final offset = _boundaryOffset;
    if (offset == null || delta is TextEditingDeltaNonTextUpdate) return;
    final (range, inserted) = switch (delta) {
      TextEditingDeltaInsertion d => (
          TextRange.collapsed(d.insertionOffset),
          d.textInserted.length
        ),
      TextEditingDeltaDeletion d => (d.deletedRange, 0),
      TextEditingDeltaReplacement d => (
          d.replacedRange,
          d.replacementText.length
        ),
      _ => throw StateError('Unsupported IME edit'),
    };
    // Flutter can minimize a replacement into an insertion when the selected
    // prefix is identical. It still replaces the selected owned boundary.
    final selection = value.selection;
    final selectedBoundary = selection.isValid &&
        selection.start <= offset &&
        selection.end > offset;
    if (selectedBoundary || (range.start <= offset && range.end > offset)) {
      _boundaryOffset = null;
    } else if (range.end <= offset) {
      _boundaryOffset = offset + inserted - range.end + range.start;
    }
  }

  int _cursor(String target) {
    final selection = value.selection;
    if ((composing && !forwardComposing) ||
        !selection.isValid ||
        !selection.isCollapsed) {
      return target.characters.length;
    }
    var offset = selection.extentOffset;
    final boundaryOffset = _boundaryOffset;
    if (boundaryOffset != null && offset > boundaryOffset) {
      offset -= boundary.length;
    }
    return target.substring(0, offset).characters.length;
  }

  MobileImeEdit _difference(String target,
      {required int cursor,
      required bool deletedBoundary,
      required bool sync}) {
    final before = _sent.characters.toList();
    final after = target.characters.toList();
    var common = 0;
    while (common < before.length &&
        common < after.length &&
        before[common] == after[common]) {
      common++;
    }
    // Native Backspace may remove only part of a grapheme. Its remaining text
    // cannot safely be mirrored or corrected by this local editor afterward.
    if (!composing &&
        _sent.startsWith(target) &&
        before.skip(common).any((grapheme) => grapheme.runes.length > 1)) {
      return MobileImeEdit(
          moveBefore: before.length - _sentCursor,
          backspaces: before.length - common,
          resetContext: true);
    }
    if (!deletedBoundary &&
        (_sentCursor != before.length || cursor != after.length)) {
      return _differenceAtCursor(before, after,
          common: common, cursor: cursor, sync: sync);
    }
    return MobileImeEdit(
      text: after.skip(common).join(),
      moveBefore: deletedBoundary ? before.length - _sentCursor : 0,
      backspaces: before.length - common + (deletedBoundary ? 1 : 0),
      syncPlatform: sync,
    );
  }

  MobileImeEdit _differenceAtCursor(List<String> before, List<String> after,
      {required int common, required int cursor, required bool sync}) {
    if (common == before.length && common == after.length) {
      return MobileImeEdit(
          moveBefore: cursor - _sentCursor, syncPlatform: sync);
    }
    var oldEnd = before.length;
    var newEnd = after.length;
    while (oldEnd > common &&
        newEnd > common &&
        before[oldEnd - 1] == after[newEnd - 1]) {
      oldEnd--;
      newEnd--;
    }
    return MobileImeEdit(
      moveBefore: oldEnd - _sentCursor,
      backspaces: oldEnd - common,
      text: after.sublist(common, newEnd).join(),
      moveAfter: cursor - newEnd,
      syncPlatform: sync,
    );
  }

  TextEditingValue _withBoundary(TextEditingValue next) {
    final selection = next.selection;
    return next.copyWith(
      text: boundary + next.text,
      selection: selection.isValid
          ? selection.copyWith(
              baseOffset: selection.baseOffset + boundary.length,
              extentOffset: selection.extentOffset + boundary.length)
          : TextSelection.collapsed(offset: boundary.length + next.text.length),
      composing: TextRange.empty,
    );
  }

  TextEditingDelta backspace() {
    final selection = value.selection;
    final end = selection.isValid ? selection.end : value.text.length;
    var start = selection.isValid ? selection.start : end;
    if (start == end && start > 0) {
      start -= value.text.substring(0, start).characters.last.length;
    }
    final range = TextRange(start: start, end: end);
    return TextEditingDeltaDeletion(
      oldText: value.text,
      deletedRange: range,
      selection: TextSelection.collapsed(offset: start),
      composing: value.replaced(range, '').composing,
    );
  }

  TextEditingDelta commit() => TextEditingDeltaNonTextUpdate(
      oldText: value.text,
      selection: value.selection,
      composing: TextRange.empty);
}
