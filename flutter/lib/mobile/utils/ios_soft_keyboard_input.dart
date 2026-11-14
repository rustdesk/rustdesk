import 'dart:math' as math;

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart'
    show
        BuildContext,
        Offstage,
        SizedBox,
        StatelessWidget,
        StringCharacters,
        TextEditingController,
        Widget;

// This hidden field is a keyboard buffer, not a copy of the remote document.
// Diagram key: [P] = owned padding (normally 1024 spaces), {ab} = composing
// (not yet committed), <ab> = selected range, | = caret.
// [P:n] uses a small padding length n to make examples easier to read.
// iOS -> controller -> update() -> IosKeyboardEdit
//                                  |-> backspaces, then text: remote caret
//                                  +-> localValue: local controller only
// Padding lets Backspace produce a local edit even before any text is typed.
class IosKeyboardEdit {
  const IosKeyboardEdit({this.backspaces = 0, this.text = '', this.localValue});

  final int backspaces;
  final String text;
  // Local controller correction; this text must not be sent to the peer.
  final TextEditingValue? localValue;
}

class IosSoftKeyboardController extends TextEditingController {
  IosSoftKeyboardController({super.text});

  // Valid caret-only moves, including Space dragging, have two handling paths:
  //   controller.selection
  //     +-- composing active -> reject here; keep selection + composing
  //     +-- no composing ----> super.selection -> listener -> update()
  //                                                       -> _clampSelection()
  //                                                       -> caret at text end
  // The base setter can clear composing when a move leaves its range. Reject
  // first, before update() could mistake that for a commit of unfinished text.
  // _clampSelection() skips active composing, so it cannot replace this guard.
  // Notify even without a value change so the native selection can be resynced.
  // Keyboard text/commit updates still use the value setter and reach update().
  @override
  set selection(TextSelection newSelection) {
    final composing = value.composing;
    if (newSelection.isValid &&
        newSelection.isCollapsed &&
        newSelection.end <= text.length &&
        value.isComposingRangeValid &&
        !composing.isCollapsed &&
        newSelection != value.selection) {
      notifyListeners();
      return;
    }
    super.selection = newSelection;
  }
}

/// Converts the hidden iOS field into Backspace/text edits at the remote caret.
/// Padding and active composition are local; the field is not the remote document.
/// Native cursor navigation is ignored; selections needed by the IME are kept.
class IosSoftKeyboardInput {
  static const _singleCharacterLength = 1;

  // Start with local padding only: buffer = [P], committed suffix = "".
  // The existing remote document is unknown and is never copied into this model.
  IosSoftKeyboardInput(this._initialText)
      : _previousValue = TextEditingValue(text: _initialText),
        _prefixLength = _initialText.length;

  final String _initialText;
  // Last local snapshot, including corrections returned to the controller.
  TextEditingValue _previousValue;
  // Suffix represented by our remote edits, not the whole remote document.
  String _committedText = '';
  // UTF-16 padding boundary, retained across temporary IME padding removal.
  int _prefixLength;

  // Forget this keyboard session: model [P]ab{c} -> [P], committed suffix -> "".
  // This sends no remote edits; the caller resets the actual controller separately.
  void reset() {
    _previousValue = TextEditingValue(text: _initialText);
    _committedText = '';
    _prefixLength = _initialText.length;
  }

  // Snapshot -> track padding -> strip padding/composing -> diff committed text
  //                |                                            |
  //                +-> padding Backspaces                       +-> suffix edit
  // The returned remote edit combines both; local corrections stay on the iPad.
  //   old: abcd    new: abXY    shared: ab -> 2 Backspaces, then "XY"
  // Padding deletions add Backspaces into text already present on the peer.
  // Count suffix characters as graphemes; local selection ranges use UTF-16.
  // Cursor correction is computed here, not applied to the incoming snapshot.
  // A padding refill, if needed, takes priority over that local correction.
  IosKeyboardEdit update(TextEditingValue value) {
    final localValue = _clampSelection(value);
    final hadPrefix = _prefixLength != 0;
    final prefixBackspaces = _updatePrefix(value);
    final committed = _withoutComposingText(value);
    // Remember local selection/composition changes even if the suffix is unchanged.
    _previousValue = value;
    // Compare confirmed text after removing padding and marked composition:
    //   [P]ab{c} -> [P]ab{cd} leaves both committed strings equal to "ab".
    // No suffix diff is needed, but padding deletion can still affect the peer:
    //   [P:4] -> [P:3] leaves both strings empty yet needs one Backspace.
    if (committed == _committedText) {
      // text defaults to ''. Still return padding Backspaces and local corrections.
      // Prefer a local refill; otherwise keep the correction from _clampSelection.
      return IosKeyboardEdit(
          backspaces: prefixBackspaces,
          localValue: _replenishPrefix(value, hadPrefix) ?? localValue);
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
      localValue: _replenishPrefix(value, hadPrefix) ?? localValue,
    );
  }

  // Restore exhausted padding locally so Backspace can keep reaching the peer:
  //   empty: | -> [P]|       bulk replacement: ab| -> [P]ab|
  // Once the tracked prefix is zero:
  //   empty field ------------------------------------------> refill
  //   hadPrefix + no composition + (multiple graphemes/' ') -> refill
  //   anything else ----------------------------------------> wait
  // hadPrefix means padding was still owned at the start of this update.
  // Rewriting unmarked Korean/Telex text can reset its IME state; once the IME
  // owns the whole field, leave later keystrokes untouched until it is empty.
  // Shift selection/composing offsets by the inserted padding length, and cache
  // the corrected snapshot before the caller assigns it and reenters update().
  TextEditingValue? _replenishPrefix(TextEditingValue value, bool hadPrefix) {
    if (_prefixLength != 0) return null;
    if (value.text.isNotEmpty &&
        (!hadPrefix ||
            (value.isComposingRangeValid && !value.composing.isCollapsed) ||
            (value.text.characters.length == _singleCharacterLength &&
                value.text != ' '))) {
      return null;
    }
    _prefixLength = _initialText.length;
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

  // Handle the no-composition branch in the controller diagram above: the
  // selection setter lets the move through, then its listener calls update().
  // Here we compute a correction; we do not reject the setter call itself.
  //   same text: [P]a|bc -> [P]abc|       replacement range: [P]<abc> -> kept
  // iOS still recognizes the Space gesture; no cursor keys are sent to the peer.
  // Leave text changes, invalid selections and active composition untouched.
  // Otherwise, inspect the selection:
  //   caret already at end? ------------------------ yes --> keep
  //   nonempty range reaches/passes padding end? ---- yes --> keep
  //   anything else ---------------------------------------> collapse to end
  // Keep ranges reaching the boundary because iOS selects them before deleting
  // or replacing text. This includes the last pad alone and Select All.
  // null means no correction. Otherwise update() returns this as localValue,
  // and the remote page writes it back to controller.value to restore the caret.
  TextEditingValue? _clampSelection(TextEditingValue value) {
    if (value.text != _previousValue.text ||
        !value.selection.isValid ||
        (value.isComposingRangeValid && !value.composing.isCollapsed)) {
      return null;
    }
    final selection = value.selection;
    if (selection.isCollapsed && selection.extentOffset == value.text.length) {
      return null;
    }
    if (!selection.isCollapsed &&
        selection.end >= math.min(_prefixLength, value.text.length)) {
      return null;
    }
    return value.copyWith(
        selection: TextSelection.collapsed(offset: value.text.length));
  }

  // Track which leading characters are padding; return only padding Backspaces.
  //   [P:4] -> [P:3]             => 1 padding Backspace (both padding-only)
  //   [P:4]a -> [P:3] -> [P:4]A  => 0 padding Backspaces (IME recomposition)
  // In the second case update() still handles replacing the committed "a".
  // Retain ownership across temporary IME removal, even beyond current length.
  // Scan only the old owned prefix: user-typed leading spaces remain input.
  // Whole-field replacement/Undo is handled first because text alone is ambiguous.
  int _updatePrefix(TextEditingValue value) {
    final text = value.text;
    if (_updateReplacementPrefix(value)) return 0;
    final limit = math.min(_prefixLength, text.length);
    var prefix = 0;
    while (prefix < limit && text[prefix] == _initialText[prefix]) {
      prefix++;
    }
    if (prefix < limit) {
      _prefixLength = prefix;
      return 0;
    }
    if (text.length < _previousValue.text.length &&
        _previousValue.text.length <= _prefixLength) {
      final backspaces = _previousValue.text.length - text.length;
      _prefixLength = text.length;
      return backspaces;
    }
    return 0;
  }

  // Recognize ownership changes that cannot be inferred from leading spaces:
  //   select <[P]abc>, replace with " ab" -> prefix = 0; keep the typed space
  //   prefix = 0, Undo restores exactly [P] -> own the padding again
  // The latter recognizes the original padding, not arbitrary Undo history.
  // Exclude a one-character prefix: IMEs may select the last pad with a letter
  // during recomposition. true means ownership was handled, with no pad deletion.
  bool _updateReplacementPrefix(TextEditingValue value) {
    if (_prefixLength == 0 && value.text == _initialText) {
      _prefixLength = _initialText.length;
      return true;
    }
    final selection = _previousValue.selection;
    if (value.text != _previousValue.text &&
        selection.start == 0 &&
        selection.end == _previousValue.text.length &&
        _prefixLength > _singleCharacterLength) {
      _prefixLength = 0;
      return true;
    }
    return false;
  }

  // Remove padding and marked text; keep committed text on BOTH sides:
  //   [P]ab{cd}ef -> abef       composition ends: [P]abcdef -> abcdef
  // Partial candidate selection can commit text while the rest still composes.
  // Therefore a composing-range change alone can produce a new remote edit.
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

class HiddenKeyboardInput extends StatelessWidget {
  const HiddenKeyboardInput({
    super.key,
    required this.isIOS,
    required this.child,
  });

  final bool isIOS;
  final Widget child;

  // iOS: field -> Offstage -> real line layout, keyboard focus, no painting.
  // Floating-cursor bounds need that layout even though the field is hidden.
  // Other platforms: field -> zero-size box (the existing hiding behavior).
  @override
  Widget build(BuildContext context) {
    if (isIOS) return Offstage(child: child);
    return SizedBox(width: 0, height: 0, child: child);
  }
}
