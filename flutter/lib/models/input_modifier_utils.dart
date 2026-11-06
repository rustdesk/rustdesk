import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

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
  if (data.characters.length != 1) return false;
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
  if ((ctrlLocked || altLocked) && shouldApplyTerminalInputModifiers(result)) {
    result = applyTerminalInputModifiers(
      result,
      ctrlLocked: ctrlLocked,
      altLocked: altLocked,
    );
  }
  return result;
}

/// Suppresses the duplicate IME commits produced by Flutter's GTK backend.
///
/// That backend reports a commit by resending the whole text model, and the
/// `preedit-changed` / `preedit-end` signals that follow the commit resend the
/// same text before the framework's state reset reaches the platform. xterm
/// treats every update as fresh input, so one IME commit reaches `onOutput`
/// two or three times. Hardware keys do not take this path: each one arrives
/// as a single character, so single-character ASCII payloads are never filtered.
/// A commit whose preedit collapses repeats the same way whatever its content
/// is, which is why text longer than one character is filtered even when it is
/// pure ASCII (an English word committed by a Chinese IME).
///
/// Escape-prefixed payloads are never IME text: they are key sequences, mouse
/// reports, bracketed pastes or terminal query answers. Mouse reports repeat
/// identically on purpose (one report per scrolled line) and their coordinates
/// can be non-ASCII in the normal/UTF-8 encodings, so they must not be merged.
///
/// The resends are posted back to back from the same GTK event, so [window]
/// only has to cover the gap between platform messages. It must stay below the
/// key repeat interval, otherwise a held-down IME key (full-width punctuation,
/// for example) would be read as a resend and its repeats dropped.
class TerminalImeCommitFilter {
  TerminalImeCommitFilter({
    required this.enabled,
    this.window = const Duration(milliseconds: 20),
  });

  final bool enabled;
  final Duration window;

  // Monotonic, so a system clock adjustment cannot turn a later commit into a
  // negative elapsed time that looks like a resend.
  final _clock = Stopwatch()..start();
  String? _lastCommit;
  Duration? _lastCommitAt;

  /// Returns true when [data] repeats the previous IME commit within [window].
  bool isRepeatedCommit(String data) {
    if (!enabled) return false;
    if (data.startsWith('\x1B')) return false;
    // A single ASCII character is a hardware key, which repeats at the key
    // repeat interval. Text can only be longer, or non-ASCII, when an IME or a
    // paste produced it.
    if (data.length < 2 && data.codeUnits.every((unit) => unit < 0x80)) {
      return false;
    }

    final now = _clock.elapsed;
    final previousAt = _lastCommitAt;
    final repeated = data == _lastCommit &&
        previousAt != null &&
        now - previousAt < window;
    _lastCommit = data;
    _lastCommitAt = now;
    return repeated;
  }
}

/// Returns true when a hardware paste shortcut must bypass keyboard modifiers.
///
/// xterm already handles each platform's paste shortcut in the common case.
/// Only intercept while a virtual Ctrl/Alt lock is active, because xterm can
/// emit a one-character paste as normal text when bracketed paste mode is off.
bool shouldHandleTerminalPasteShortcut({
  required TargetPlatform platform,
  required LogicalKeyboardKey logicalKey,
  required bool isKeyDown,
  required bool isKeyRepeat,
  required bool controlPressed,
  required bool metaPressed,
  required bool altPressed,
  required bool shiftPressed,
  required bool modifierLockActive,
}) {
  if (!modifierLockActive) return false;
  if (!isKeyDown && !isKeyRepeat) return false;
  if (logicalKey != LogicalKeyboardKey.keyV) return false;
  if (altPressed) return false;
  switch (platform) {
    case TargetPlatform.linux:
      return controlPressed && !metaPressed && shiftPressed;
    case TargetPlatform.iOS:
    case TargetPlatform.macOS:
      return !controlPressed && metaPressed && !shiftPressed;
    case TargetPlatform.android:
    case TargetPlatform.fuchsia:
    case TargetPlatform.windows:
      return controlPressed && !metaPressed && !shiftPressed;
  }
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
  // Ctrl mappings are defined only for ASCII scalars. A visible character can
  // be multiple scalars (for example, a decomposed accent), so leave those
  // graphemes untouched instead of rewriting only their ASCII base letter.
  final graphemes = data.characters.toList(growable: false);
  if (graphemes.length != 1) {
    return data;
  }

  final runes = graphemes.single.runes.toList(growable: false);
  if (runes.length != 1) {
    return data;
  }

  final code = runes.single;
  if (code >= 0x61 && code <= 0x7A) {
    return String.fromCharCode(code - 0x60);
  }
  if (code >= 0x41 && code <= 0x5A) {
    return String.fromCharCode(code - 0x40);
  }
  if (code == 0x20) {
    return String.fromCharCode(0);
  }
  if (code == 0x5B) {
    return String.fromCharCode(27);
  }
  if (code == 0x5C) {
    return String.fromCharCode(28);
  }
  if (code == 0x5D) {
    return String.fromCharCode(29);
  }
  if (code == 0x5E) {
    return String.fromCharCode(30);
  }
  if (code == 0x5F || code == 0x2F) {
    return String.fromCharCode(31);
  }
  return data;
}
