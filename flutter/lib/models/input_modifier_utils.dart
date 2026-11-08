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

enum ModifierKind { alt, ctrl, shift, command }

/// Resolve a release using its down event, keeping modifiers held by other keys.
ModifierKind? releasedModifier(
    LogicalKeyboardKey logical, PhysicalKeyboardKey physical,
    {Iterable<RawKeyEvent> pressedKeys = const [],
    RawKeyEventData? releaseData}) {
  // Mod3-Mod5 and virtual Super/Hyper/Meta can identify genuine Meta mappings.
  // Mod2 is NumLock, which is on in many sessions.
  const gtkNonAltModifiers = 0xe0 | (7 << 26);
  bool hasGtkMask(RawKeyEventData? data, int mask) =>
      data is RawKeyEventDataLinux &&
      data.keyHelper is GtkKeyHelper &&
      data.modifiers & (gtkNonAltModifiers | mask) == mask;
  final altMask = hasGtkMask(releaseData, GtkKeyHelper.modifierMod1);
  final down = pressedKeys.where((event) => event.physicalKey == physical);
  // GTK recognizes Super down from its keysym, before Mod4 enters the mask.
  final superWithAlt = down.isNotEmpty &&
      hasGtkMask(down.first.data, GtkKeyHelper.modifierMod1) &&
      down.first.data.isMetaPressed;
  final kind = down.isEmpty
      ? _modifierKind(logical, physical)
      : _rawModifierKind(down.first, logical, altMask);
  final stillPressed = pressedKeys.any((event) =>
      event.physicalKey != physical &&
      _rawModifierKind(event, event.logicalKey,
              altMask || (superWithAlt && hasGtkMask(event.data, 0))) ==
          kind);
  return stillPressed ? null : kind;
}

ModifierKind? _rawModifierKind(
    RawKeyEvent event, LogicalKeyboardKey logical, bool altMask) {
  const gtkMetaLeft = 0xffe7;
  const gtkMetaRight = 0xffe8;
  final data = event.data;
  // Use the saved Shift mask: RawKeyEvent modifier getters read current state.
  // GTK reports shifted Alt as Meta, including Alt at remapped positions.
  if ((logical == LogicalKeyboardKey.altLeft ||
          logical == LogicalKeyboardKey.altRight ||
          (data.isShiftPressed &&
              (altMask ||
                  _releasedPhysicalModifier(event.physicalKey) ==
                      ModifierKind.alt))) &&
      data is RawKeyEventDataLinux &&
      data.keyHelper is GtkKeyHelper &&
      (data.keyCode == gtkMetaLeft || data.keyCode == gtkMetaRight)) {
    return ModifierKind.alt;
  }
  return _modifierKind(event.logicalKey, event.physicalKey);
}

ModifierKind? _modifierKind(
    LogicalKeyboardKey logical, PhysicalKeyboardKey physical) {
  if (logical == LogicalKeyboardKey.altLeft ||
      logical == LogicalKeyboardKey.altRight) {
    return ModifierKind.alt;
  }
  if (logical == LogicalKeyboardKey.controlLeft ||
      logical == LogicalKeyboardKey.controlRight) {
    return ModifierKind.ctrl;
  }
  if (logical == LogicalKeyboardKey.shiftLeft ||
      logical == LogicalKeyboardKey.shiftRight) {
    return ModifierKind.shift;
  }
  if (logical == LogicalKeyboardKey.metaLeft ||
      logical == LogicalKeyboardKey.metaRight ||
      logical == LogicalKeyboardKey.superKey) {
    return ModifierKind.command;
  }
  return _isGroupSwitch(logical) ? _releasedPhysicalModifier(physical) : null;
}

bool _isGroupSwitch(LogicalKeyboardKey logical) =>
    logical == LogicalKeyboardKey.groupPrevious ||
    logical == LogicalKeyboardKey.groupNext ||
    logical == LogicalKeyboardKey.groupFirst ||
    logical == LogicalKeyboardKey.groupLast;

ModifierKind? _releasedPhysicalModifier(PhysicalKeyboardKey physical) {
  if (physical == PhysicalKeyboardKey.altLeft ||
      physical == PhysicalKeyboardKey.altRight) {
    return ModifierKind.alt;
  }
  if (physical == PhysicalKeyboardKey.controlLeft ||
      physical == PhysicalKeyboardKey.controlRight) {
    return ModifierKind.ctrl;
  }
  if (physical == PhysicalKeyboardKey.shiftLeft ||
      physical == PhysicalKeyboardKey.shiftRight) {
    return ModifierKind.shift;
  }
  if (physical == PhysicalKeyboardKey.metaLeft ||
      physical == PhysicalKeyboardKey.metaRight) {
    return ModifierKind.command;
  }
  return null;
}
