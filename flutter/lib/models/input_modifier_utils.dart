import 'package:flutter/services.dart';

/// Identifies where terminal input originated so paste data can bypass all
/// keyboard-only transformations.
enum TerminalInputSource {
  keyboard,
  paste,
}

/// Returns true when a stale mobile one-shot Shift state should be released
/// by replaying a tracked Shift key-down as a synthesized key-up.
///
/// This is only valid on mobile when Flutter's cached Shift state is still on
/// (`cachedShiftPressed == true`) but the current hardware/raw event reports
/// Shift as off (`actualShiftPressed == false`).
///
/// A tracked Shift key-down is required so the caller can safely synthesize the
/// matching key-up. Both `shiftLeft` and `shiftRight` are excluded because the
/// Shift key event itself must be processed first; otherwise we could release
/// the tracked key while still handling the original Shift press/release.
/// Callers should evaluate this only after their cached modifier state has been
/// updated for the current event.
///
/// When this returns true, the caller logs a line like:
/// `input: releasing stale mobile Shift before replaying tracked raw key-up`
/// immediately before calling `_releaseTrackedRawShiftKeyEventIfNeeded()`.
bool shouldReleaseStaleMobileShift({
  required bool isMobile,
  required bool cachedShiftPressed,
  required bool actualShiftPressed,
  required LogicalKeyboardKey logicalKey,
  required bool hasTrackedShiftKeyDown,
}) {
  if (!isMobile || !cachedShiftPressed || actualShiftPressed) {
    return false;
  }
  if (!hasTrackedShiftKeyDown) {
    return false;
  }
  if (logicalKey == LogicalKeyboardKey.shiftLeft ||
      logicalKey == LogicalKeyboardKey.shiftRight) {
    return false;
  }
  return true;
}

/// Applies the terminal Ctrl/Alt one-shot modifiers to a single input payload.
///
String applyTerminalInputModifiers(
  String data, {
  required bool ctrlLocked,
  required bool altLocked,
}) {
  var result = data;
  if (ctrlLocked) {
    result = _applyTerminalCtrlModifier(result);
  }
  if (altLocked) {
    result = '\x1B$result';
  }
  return result;
}

/// Builds the exact payload xterm sends for paste, without applying modifiers.
String terminalPastePayload(String text, {required bool bracketedPasteMode}) {
  if (!bracketedPasteMode) {
    return text;
  }
  return '\x1B[200~$text\x1B[201~';
}

/// Returns whether one-shot Ctrl/Alt may transform and consume this input.
///
/// xterm emits terminal control keys as either one control byte or a longer
/// escape sequence. Neither form is ordinary text input, so a pending modifier
/// must survive until the user enters a printable character.
bool shouldApplyTerminalInputModifiers(String data) {
  if (data.length != 1) return false;
  final codeUnit = data.codeUnitAt(0);
  return codeUnit >= 0x20 && codeUnit != 0x7F;
}

/// Builds the payload sent to the remote terminal for keyboard and paste input.
///
/// Keyboard input keeps the mobile Enter workaround and one-shot Ctrl/Alt
/// mapping. Paste input deliberately bypasses both transformations so even a
/// one-character clipboard payload is preserved exactly.
String prepareTerminalInputPayload(
  String data, {
  required TerminalInputSource source,
  required bool isMobileOrWebMobile,
  required bool bracketedPasteMode,
  required bool ctrlLocked,
  required bool altLocked,
}) {
  if (source == TerminalInputSource.paste) {
    return terminalPastePayload(
      data,
      bracketedPasteMode: bracketedPasteMode,
    );
  }

  var result = data;
  if (isMobileOrWebMobile && result == '\n') {
    result = '\r';
  }
  if (shouldApplyTerminalInputModifiers(result) &&
      (ctrlLocked || altLocked)) {
    result = applyTerminalInputModifiers(
      result,
      ctrlLocked: ctrlLocked,
      altLocked: altLocked,
    );
  }
  return result;
}

/// Returns true for the platform paste shortcuts handled by xterm by default.
/// Only key-down is accepted to prevent one clipboard operation from firing
/// again for the matching key-up event.
bool shouldHandleTerminalPasteShortcut({
  required LogicalKeyboardKey logicalKey,
  required bool isKeyDown,
  required bool controlPressed,
  required bool metaPressed,
}) {
  return isKeyDown &&
      logicalKey == LogicalKeyboardKey.keyV &&
      (controlPressed || metaPressed);
}

/// Returns true when collapsing Row3 should also clear hidden modifier state.
bool shouldClearTerminalModifiersWhenRow3Collapses({
  required bool wasExpanded,
  required bool willExpand,
  required bool ctrlLocked,
  required bool altLocked,
}) {
  return wasExpanded && !willExpand && (ctrlLocked || altLocked);
}

String _applyTerminalCtrlModifier(String data) {
  final result = StringBuffer();
  for (var i = 0; i < data.length; i++) {
    final code = data.codeUnitAt(i);
    if (code >= 0x61 && code <= 0x7A) {
      result.writeCharCode(code - 0x60);
    } else if (code >= 0x41 && code <= 0x5A) {
      result.writeCharCode(code - 0x40);
    } else if (code == 0x20) {
      result.writeCharCode(0);
    } else if (code == 0x5B) {
      result.writeCharCode(27);
    } else if (code == 0x5C) {
      result.writeCharCode(28);
    } else if (code == 0x5D) {
      result.writeCharCode(29);
    } else if (code == 0x5E) {
      result.writeCharCode(30);
    } else if (code == 0x5F || code == 0x2F) {
      result.writeCharCode(31);
    } else {
      result.writeCharCode(code);
    }
  }
  return result.toString();
}
