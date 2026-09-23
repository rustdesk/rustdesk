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

List<Map<String, dynamic>> shortcutBindingMapsFrom(dynamic rawBindings) {
  if (rawBindings is! Iterable) return <Map<String, dynamic>>[];
  final bindings = <Map<String, dynamic>>[];
  for (final raw in rawBindings) {
    if (raw is! Map) continue;
    final mods = raw['mods'];
    if (raw['action'] is! String ||
        raw['key'] is! String ||
        mods is! List ||
        mods.any((mod) =>
            !const {'primary', 'ctrl', 'alt', 'shift'}.contains(mod))) {
      continue;
    }
    final binding = <String, dynamic>{};
    for (final entry in raw.entries) {
      final key = entry.key;
      if (key is String) {
        binding[key] = entry.value;
      }
    }
    if (binding.isNotEmpty) {
      bindings.add(binding);
    }
  }
  return bindings;
}

Set<String> shortcutModSetFrom(dynamic rawMods) {
  if (rawMods is! Iterable) return <String>{};
  return rawMods.whereType<String>().toSet();
}

/// Whether enabling shortcuts should seed the default bindings. Only a config
/// that has never held a bindings list is seeded; an empty list means the
/// user cleared every binding and must stay empty.
bool shouldSeedDefaultShortcutBindings(Map<String, dynamic> config) =>
    !config.containsKey('bindings');

bool isSwitchTabShortcutAction(String? actionId) {
  return actionId == kShortcutActionSwitchTabNext ||
      actionId == kShortcutActionSwitchTabPrev;
}

/// USB HID usage (keyboard page 0x07) of every key accepted as a shortcut,
/// mapped to the canonical key name used in saved bindings.
const Map<int, String> _kUsbHidKeyNames = {
  0x04: 'a', 0x05: 'b', 0x06: 'c', 0x07: 'd', 0x08: 'e', 0x09: 'f',
  0x0A: 'g', 0x0B: 'h', 0x0C: 'i', 0x0D: 'j', 0x0E: 'k', 0x0F: 'l',
  0x10: 'm', 0x11: 'n', 0x12: 'o', 0x13: 'p', 0x14: 'q', 0x15: 'r',
  0x16: 's', 0x17: 't', 0x18: 'u', 0x19: 'v', 0x1A: 'w', 0x1B: 'x',
  0x1C: 'y', 0x1D: 'z',
  0x1E: 'digit1', 0x1F: 'digit2', 0x20: 'digit3', 0x21: 'digit4',
  0x22: 'digit5', 0x23: 'digit6', 0x24: 'digit7', 0x25: 'digit8',
  0x26: 'digit9', 0x27: 'digit0',
  0x28: 'enter', 0x2A: 'backspace', 0x2B: 'tab', 0x2C: 'space',
  0x3A: 'f1', 0x3B: 'f2', 0x3C: 'f3', 0x3D: 'f4', 0x3E: 'f5', 0x3F: 'f6',
  0x40: 'f7', 0x41: 'f8', 0x42: 'f9', 0x43: 'f10', 0x44: 'f11', 0x45: 'f12',
  0x49: 'insert', 0x4A: 'home', 0x4B: 'page_up', 0x4C: 'delete',
  0x4D: 'end', 0x4E: 'page_down', 0x4F: 'arrow_right', 0x50: 'arrow_left',
  0x51: 'arrow_down', 0x52: 'arrow_up',
  // Numpad Enter shares the "enter" name with the main Return key, as in the
  // Rust matcher (`Return | KpReturn`) and the Web matcher (`NumpadEnter`).
  0x58: 'enter',
};

/// Map a [PhysicalKeyboardKey] to the canonical key name used in saved
/// bindings, or `null` for keys we don't accept as shortcuts.
///
/// Bindings name physical key positions (US layout names), whatever the
/// active keyboard layout: the native matcher sees the key through its USB
/// HID usage (`rdev::usb_hid_key_from_code` -> `event_to_key_name` in
/// `src/keyboard/shortcuts.rs`) and the Web matcher through
/// `KeyboardEvent.code` (`flutter/web/js/src/shortcut_matcher.ts`). Parity
/// is enforced against `flutter/test/fixtures/shortcut_key_usb_hid.json` by
/// a Dart test and the Rust `usb_hid_keys_match_fixture` test.
String? physicalKeyName(PhysicalKeyboardKey k) {
  final usage = k.usbHidUsage;
  if (usage >> 16 != 0x07) return null;
  return _kUsbHidKeyNames[usage & 0xFFFF];
}

/// The key name a [KeyEvent] records or matches as.
String? shortcutKeyNameForEvent(KeyEvent e) => physicalKeyName(e.physicalKey);

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
  for (final binding in shortcutBindingMapsFrom(bindings)) {
    final action = binding['action'] as String?;
    if (!cap.includeFullscreenShortcut &&
        action == kShortcutActionToggleFullscreen) {
      continue;
    }
    if (!cap.includeScreenshotShortcut && action == kShortcutActionScreenshot) {
      continue;
    }
    if (!cap.includeScreenshotShortcut &&
        action == kShortcutActionToggleRelativeMouseMode) {
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
