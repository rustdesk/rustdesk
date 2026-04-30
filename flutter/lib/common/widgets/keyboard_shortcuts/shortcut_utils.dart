import 'package:flutter/services.dart';

import 'shortcut_constants.dart';

List<String> canonicalShortcutModsForSave(Set<String> mods) {
  return <String>[
    if (mods.contains('primary')) 'primary',
    if (mods.contains('ctrl')) 'ctrl',
    if (mods.contains('alt')) 'alt',
    if (mods.contains('shift')) 'shift',
  ];
}

bool isSwitchTabShortcutAction(String? actionId) {
  return actionId == kShortcutActionSwitchTabNext ||
      actionId == kShortcutActionSwitchTabPrev;
}

/// Map a [LogicalKeyboardKey] to the canonical key name used in saved
/// bindings, or `null` for keys we don't accept as shortcuts.
///
/// Mirror of `event_to_key_name` in `src/keyboard/shortcuts.rs` and
/// `logicalToKeyName` in `flutter/web/js/src/shortcut_matcher.ts` — keep
/// the three in lockstep. Cross-language parity is enforced by:
///   * `flutter/test/fixtures/supported_shortcut_keys.json` — the
///     authoritative list of names this function must produce.
///   * Dart `supported keys` test in `keyboard_shortcuts_test.dart` —
///     asserts the (LogicalKeyboardKey → name) mapping covers the fixture.
///   * Rust `supported_keys_match_fixture` test in `shortcuts.rs` — the
///     Rust-side mirror against the same fixture.
/// A drift in any of the three breaks one of the two tests.
String? logicalKeyName(LogicalKeyboardKey k) {
  // Singletons that map 1:1.
  if (k == LogicalKeyboardKey.delete) return 'delete';
  if (k == LogicalKeyboardKey.backspace) return 'backspace';
  // Numpad Enter shares the "enter" name with the main Return key — matches
  // the Rust matcher (`Return | KpReturn` → "enter") and matches user
  // expectation that the two physical Enters are interchangeable.
  if (k == LogicalKeyboardKey.enter || k == LogicalKeyboardKey.numpadEnter) {
    return 'enter';
  }
  if (k == LogicalKeyboardKey.tab) return 'tab';
  if (k == LogicalKeyboardKey.space) return 'space';
  if (k == LogicalKeyboardKey.arrowLeft) return 'arrow_left';
  if (k == LogicalKeyboardKey.arrowRight) return 'arrow_right';
  if (k == LogicalKeyboardKey.arrowUp) return 'arrow_up';
  if (k == LogicalKeyboardKey.arrowDown) return 'arrow_down';
  if (k == LogicalKeyboardKey.home) return 'home';
  if (k == LogicalKeyboardKey.end) return 'end';
  if (k == LogicalKeyboardKey.pageUp) return 'page_up';
  if (k == LogicalKeyboardKey.pageDown) return 'page_down';
  if (k == LogicalKeyboardKey.insert) return 'insert';

  // Letter / digit / F-key tables. `LogicalKeyboardKey` constants are
  // `static final` (not `const`), so the maps can't be `const` — but they
  // initialize once per process and the lookup is O(1).
  final letters = <LogicalKeyboardKey, String>{
    LogicalKeyboardKey.keyA: 'a', LogicalKeyboardKey.keyB: 'b',
    LogicalKeyboardKey.keyC: 'c', LogicalKeyboardKey.keyD: 'd',
    LogicalKeyboardKey.keyE: 'e', LogicalKeyboardKey.keyF: 'f',
    LogicalKeyboardKey.keyG: 'g', LogicalKeyboardKey.keyH: 'h',
    LogicalKeyboardKey.keyI: 'i', LogicalKeyboardKey.keyJ: 'j',
    LogicalKeyboardKey.keyK: 'k', LogicalKeyboardKey.keyL: 'l',
    LogicalKeyboardKey.keyM: 'm', LogicalKeyboardKey.keyN: 'n',
    LogicalKeyboardKey.keyO: 'o', LogicalKeyboardKey.keyP: 'p',
    LogicalKeyboardKey.keyQ: 'q', LogicalKeyboardKey.keyR: 'r',
    LogicalKeyboardKey.keyS: 's', LogicalKeyboardKey.keyT: 't',
    LogicalKeyboardKey.keyU: 'u', LogicalKeyboardKey.keyV: 'v',
    LogicalKeyboardKey.keyW: 'w', LogicalKeyboardKey.keyX: 'x',
    LogicalKeyboardKey.keyY: 'y', LogicalKeyboardKey.keyZ: 'z',
  };
  final letter = letters[k];
  if (letter != null) return letter;

  final digits = <LogicalKeyboardKey, String>{
    LogicalKeyboardKey.digit0: 'digit0',
    LogicalKeyboardKey.digit1: 'digit1',
    LogicalKeyboardKey.digit2: 'digit2',
    LogicalKeyboardKey.digit3: 'digit3',
    LogicalKeyboardKey.digit4: 'digit4',
    LogicalKeyboardKey.digit5: 'digit5',
    LogicalKeyboardKey.digit6: 'digit6',
    LogicalKeyboardKey.digit7: 'digit7',
    LogicalKeyboardKey.digit8: 'digit8',
    LogicalKeyboardKey.digit9: 'digit9',
  };
  final digit = digits[k];
  if (digit != null) return digit;

  final fkeys = <LogicalKeyboardKey, String>{
    LogicalKeyboardKey.f1: 'f1', LogicalKeyboardKey.f2: 'f2',
    LogicalKeyboardKey.f3: 'f3', LogicalKeyboardKey.f4: 'f4',
    LogicalKeyboardKey.f5: 'f5', LogicalKeyboardKey.f6: 'f6',
    LogicalKeyboardKey.f7: 'f7', LogicalKeyboardKey.f8: 'f8',
    LogicalKeyboardKey.f9: 'f9', LogicalKeyboardKey.f10: 'f10',
    LogicalKeyboardKey.f11: 'f11', LogicalKeyboardKey.f12: 'f12',
  };
  return fkeys[k];
}

/// Bundle of "is this shortcut available on the current platform" flags.
///
/// Production code reaches a single source of truth via
/// [ShortcutModel.currentPlatformCapabilities] (which encodes the per-runtime
/// rules in one place); tests construct one directly with whichever flags
/// they want to exercise. Two filter functions consume this:
/// [filterDefaultBindingsForPlatform] (for trimming default-binding JSON
/// before it hits LocalConfig) and [filterKeyboardShortcutActionGroupsForPlatform]
/// (for trimming the configuration UI's action list). Both must agree on the
/// same capability set, otherwise a default binding could be seeded for an
/// action the user has no UI to manage.
class ShortcutPlatformCapabilities {
  final bool includeFullscreenShortcut;
  final bool includeScreenshotShortcut;
  final bool includeTabShortcuts;
  final bool includeToolbarShortcut;
  final bool includeCloseTabShortcut;
  final bool includeSwitchSidesShortcut;
  final bool includeRecordingShortcut;
  final bool includeResetCanvasShortcut;
  final bool includePinToolbarShortcut;
  final bool includeViewModeShortcut;
  final bool includeInputSourceShortcut;
  final bool includeVoiceCallShortcut;

  const ShortcutPlatformCapabilities({
    required this.includeFullscreenShortcut,
    required this.includeScreenshotShortcut,
    required this.includeTabShortcuts,
    required this.includeToolbarShortcut,
    required this.includeCloseTabShortcut,
    required this.includeSwitchSidesShortcut,
    required this.includeRecordingShortcut,
    required this.includeResetCanvasShortcut,
    required this.includePinToolbarShortcut,
    required this.includeViewModeShortcut,
    required this.includeInputSourceShortcut,
    required this.includeVoiceCallShortcut,
  });
}

List<Map<String, dynamic>> filterDefaultBindingsForPlatform(
  Iterable<dynamic> bindings,
  ShortcutPlatformCapabilities cap,
) {
  final filtered = <Map<String, dynamic>>[];
  for (final raw in bindings) {
    if (raw is! Map) continue;
    final binding = Map<String, dynamic>.from(raw);
    final action = binding['action'] as String?;
    if (!cap.includeFullscreenShortcut &&
        action == kShortcutActionToggleFullscreen) {
      continue;
    }
    if (!cap.includeScreenshotShortcut && action == kShortcutActionScreenshot) {
      continue;
    }
    if (!cap.includeTabShortcuts && isSwitchTabShortcutAction(action)) {
      continue;
    }
    if (!cap.includeToolbarShortcut &&
        action == kShortcutActionToggleToolbar) {
      continue;
    }
    if (!cap.includeCloseTabShortcut && action == kShortcutActionCloseTab) {
      continue;
    }
    if (!cap.includeSwitchSidesShortcut &&
        action == kShortcutActionSwitchSides) {
      continue;
    }
    if (!cap.includeRecordingShortcut &&
        action == kShortcutActionToggleRecording) {
      continue;
    }
    if (!cap.includeResetCanvasShortcut &&
        action == kShortcutActionResetCanvas) {
      continue;
    }
    if (!cap.includePinToolbarShortcut && action == kShortcutActionPinToolbar) {
      continue;
    }
    if (!cap.includeViewModeShortcut &&
        (action == kShortcutActionViewModeOriginal ||
            action == kShortcutActionViewModeAdaptive ||
            action == kShortcutActionViewModeCustom)) {
      continue;
    }
    if (!cap.includeInputSourceShortcut &&
        action == kShortcutActionToggleInputSource) {
      continue;
    }
    if (!cap.includeVoiceCallShortcut &&
        action == kShortcutActionToggleVoiceCall) {
      continue;
    }
    filtered.add(binding);
  }
  return filtered;
}
