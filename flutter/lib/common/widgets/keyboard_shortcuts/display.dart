// flutter/lib/common/widgets/keyboard_shortcuts/display.dart
import 'dart:convert';
import 'package:flutter/foundation.dart';
import '../../../consts.dart';
import '../../../models/platform_model.dart';
import 'shortcut_utils.dart';

/// Read the bindings JSON and produce a human-readable shortcut string for
/// `actionId`, formatted for the current OS. Returns null if unbound, or —
/// when [requireEnabled] is true (the default) — when the master toggle is
/// off. The configuration page passes `requireEnabled: false` so users still
/// see what they have bound while the feature is disabled.
class ShortcutDisplay {
  // Cache parsed JSON keyed by the raw string — called per visible action on
  // every menu rebuild, so the jsonDecode is the real cost. Invalidation is
  // automatic: a write changes the raw and we re-parse.
  static String? _cachedRaw;
  static Map<String, dynamic>? _cachedParsed;

  @visibleForTesting
  static void resetCache() {
    _cachedRaw = null;
    _cachedParsed = null;
  }

  static String? formatFor(String actionId, {bool requireEnabled = true}) {
    final raw = bind.mainGetLocalOption(key: kShortcutLocalConfigKey);
    if (raw.isEmpty) return null;
    Map<String, dynamic>? parsed;
    if (raw == _cachedRaw) {
      parsed = _cachedParsed;
    } else {
      try {
        parsed = jsonDecode(raw) as Map<String, dynamic>;
      } catch (_) {
        parsed = null;
      }
      _cachedRaw = raw;
      _cachedParsed = parsed;
    }
    if (parsed == null) return null;
    if (requireEnabled && parsed['enabled'] != true) return null;
    // When pass-through is on, the matcher returns early on every keystroke.
    // Showing the bound combo next to a menu item would lie to the user — they
    // would press it expecting the local action and instead the keys would go
    // to the remote. Treat as unbound for display purposes.
    if (requireEnabled && parsed['pass_through'] == true) return null;
    final list = shortcutBindingMapsFrom(parsed['bindings']);
    final found = list.firstWhere(
      (b) => b['action'] == actionId,
      orElse: () => {},
    );
    if (found.isEmpty) return null;

    // Guard against a hand-edited / corrupt config where `key` is missing or
    // not a string — silently treat the binding as unbound rather than
    // crashing the toolbar render.
    final keyValue = found['key'];
    if (keyValue is! String) return null;

    final isMac = defaultTargetPlatform == TargetPlatform.macOS ||
                  defaultTargetPlatform == TargetPlatform.iOS;
    // `mods` similarly may be malformed; treat a non-list as no modifiers.
    final modsRaw = found['mods'];
    final mods = modsRaw is List
        ? modsRaw.whereType<String>().toList()
        : const <String>[];
    // Plain-text labels (Cmd / Ctrl / Alt / Shift) instead of Unicode glyphs
    // (⌘ ⌃ ⌥ ⇧). Flutter Web's CanvasKit bundled fonts don't always carry the
    // macOS modifier symbols, which renders as garbled boxes on Mac browsers;
    // text is portable and readable on every platform.
    //
    // Order matches the canonical macOS order (Cmd, Control, Option, Shift)
    // so the rendered hint reads naturally. `ctrl` only ever appears in
    // saved bindings on macOS — Win/Linux collapses Ctrl into `primary`.
    final parts = <String>[];
    for (final m in ['primary', 'ctrl', 'alt', 'shift']) {
      if (!mods.contains(m)) continue;
      switch (m) {
        case 'primary': parts.add(isMac ? 'Cmd' : 'Ctrl'); break;
        case 'ctrl':    parts.add(isMac ? 'Control' : 'Ctrl'); break;
        case 'alt':     parts.add(isMac ? 'Option' : 'Alt'); break;
        case 'shift':   parts.add('Shift'); break;
      }
    }
    parts.add(_keyDisplay(keyValue));
    return parts.join('+');
  }

  static String _keyDisplay(String key) {
    switch (key) {
      case 'delete':     return 'Del';
      case 'backspace':  return 'Backspace';
      case 'enter':      return 'Enter';
      case 'tab':        return 'Tab';
      case 'space':      return 'Space';
      case 'arrow_left': return 'Left';
      case 'arrow_right':return 'Right';
      case 'arrow_up':   return 'Up';
      case 'arrow_down': return 'Down';
      case 'home':       return 'Home';
      case 'end':        return 'End';
      case 'page_up':    return 'PgUp';
      case 'page_down':  return 'PgDn';
      case 'insert':     return 'Ins';
    }
    if (key.startsWith('digit')) return key.substring(5);
    // F-keys ("f1".."f12") and single letters fall through to uppercase.
    return key.toUpperCase();
  }
}
