// flutter/lib/common/widgets/keyboard_shortcuts/display.dart
import 'dart:convert';
import 'package:flutter/foundation.dart';
import '../../../consts.dart';
import '../../../models/platform_model.dart';

/// Read the bindings JSON and produce a human-readable shortcut string for
/// `actionId`, formatted for the current OS. Returns null if unbound.
class ShortcutDisplay {
  static String? formatFor(String actionId) {
    final raw = bind.mainGetLocalOption(key: kShortcutLocalConfigKey);
    if (raw.isEmpty) return null;
    final Map<String, dynamic> parsed;
    try {
      parsed = jsonDecode(raw) as Map<String, dynamic>;
    } catch (_) {
      return null;
    }
    if (parsed['enabled'] != true) return null;
    final list = (parsed['bindings'] as List? ?? []).cast<Map<String, dynamic>>();
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
    final parts = <String>[];
    for (final m in ['primary', 'alt', 'shift']) {
      if (!mods.contains(m)) continue;
      switch (m) {
        case 'primary': parts.add(isMac ? '⌘' : 'Ctrl'); break;
        case 'alt':     parts.add(isMac ? '⌥' : 'Alt'); break;
        case 'shift':   parts.add(isMac ? '⇧' : 'Shift'); break;
      }
    }
    parts.add(_keyDisplay(keyValue, isMac));
    return isMac ? parts.join('') : parts.join('+');
  }

  static String _keyDisplay(String key, bool isMac) {
    switch (key) {
      case 'delete':     return isMac ? '⌫' : 'Del';
      case 'enter':      return isMac ? '⏎' : 'Enter';
      case 'arrow_left': return '←';
      case 'arrow_right':return '→';
      case 'arrow_up':   return '↑';
      case 'arrow_down': return '↓';
    }
    if (key.startsWith('digit')) return key.substring(5);
    return key.toUpperCase();
  }
}
