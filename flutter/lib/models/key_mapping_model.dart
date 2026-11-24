import 'dart:convert';

import 'package:flutter/services.dart';

import 'platform_model.dart';

/// LocalConfig key, read per key event by `src/keyboard/remap.rs`.
const kKeyMappingLocalConfigKey = 'key-mapping';

/// Remote systems a mapping set can target, as the Rust side normalizes
/// `peer_platform()`.
const kKeyMappingRemoteSystems = ['macos', 'windows', 'linux'];

/// Key ids shared with `KEYS` in `src/keyboard/remap.rs`.
const kKeyMappingKeyIds = [
  'ControlLeft',
  'ControlRight',
  'ShiftLeft',
  'ShiftRight',
  'Alt',
  'AltGr',
  'MetaLeft',
  'MetaRight',
  'Escape',
  'Tab',
  'Backspace',
  'Return',
  'Delete',
  'Insert',
  'Home',
  'End',
  'PageUp',
  'PageDown',
  'UpArrow',
  'DownArrow',
  'LeftArrow',
  'RightArrow',
  'F1',
  'F2',
  'F3',
  'F4',
  'F5',
  'F6',
  'F7',
  'F8',
  'F9',
  'F10',
  'F11',
  'F12',
];

final Map<PhysicalKeyboardKey, String> _physicalKeyIds = {
  PhysicalKeyboardKey.controlLeft: 'ControlLeft',
  PhysicalKeyboardKey.controlRight: 'ControlRight',
  PhysicalKeyboardKey.shiftLeft: 'ShiftLeft',
  PhysicalKeyboardKey.shiftRight: 'ShiftRight',
  PhysicalKeyboardKey.altLeft: 'Alt',
  PhysicalKeyboardKey.altRight: 'AltGr',
  PhysicalKeyboardKey.metaLeft: 'MetaLeft',
  PhysicalKeyboardKey.metaRight: 'MetaRight',
  PhysicalKeyboardKey.escape: 'Escape',
  PhysicalKeyboardKey.tab: 'Tab',
  PhysicalKeyboardKey.backspace: 'Backspace',
  PhysicalKeyboardKey.enter: 'Return',
  PhysicalKeyboardKey.delete: 'Delete',
  PhysicalKeyboardKey.insert: 'Insert',
  PhysicalKeyboardKey.home: 'Home',
  PhysicalKeyboardKey.end: 'End',
  PhysicalKeyboardKey.pageUp: 'PageUp',
  PhysicalKeyboardKey.pageDown: 'PageDown',
  PhysicalKeyboardKey.arrowUp: 'UpArrow',
  PhysicalKeyboardKey.arrowDown: 'DownArrow',
  PhysicalKeyboardKey.arrowLeft: 'LeftArrow',
  PhysicalKeyboardKey.arrowRight: 'RightArrow',
  PhysicalKeyboardKey.f1: 'F1',
  PhysicalKeyboardKey.f2: 'F2',
  PhysicalKeyboardKey.f3: 'F3',
  PhysicalKeyboardKey.f4: 'F4',
  PhysicalKeyboardKey.f5: 'F5',
  PhysicalKeyboardKey.f6: 'F6',
  PhysicalKeyboardKey.f7: 'F7',
  PhysicalKeyboardKey.f8: 'F8',
  PhysicalKeyboardKey.f9: 'F9',
  PhysicalKeyboardKey.f10: 'F10',
  PhysicalKeyboardKey.f11: 'F11',
  PhysicalKeyboardKey.f12: 'F12',
};

String? keyMappingIdFromPhysicalKey(PhysicalKeyboardKey key) =>
    _physicalKeyIds[key];

String keyMappingSystemLabel(String system) {
  switch (system) {
    case 'macos':
      return 'macOS';
    case 'windows':
      return 'Windows';
    default:
      return 'Linux';
  }
}

/// Name of key [id] as printed on a keyboard of [system].
String keyMappingKeyLabel(String id, String system) {
  final mac = system == 'macos';
  final meta = mac ? 'Command ⌘' : (system == 'windows' ? 'Win' : 'Super');
  switch (id) {
    case 'ControlLeft':
      return mac ? 'Left Control ⌃' : 'Left Ctrl';
    case 'ControlRight':
      return mac ? 'Right Control ⌃' : 'Right Ctrl';
    case 'ShiftLeft':
      return 'Left Shift';
    case 'ShiftRight':
      return 'Right Shift';
    case 'Alt':
      return mac ? 'Left Option ⌥' : 'Left Alt';
    case 'AltGr':
      return mac ? 'Right Option ⌥' : 'Right Alt / AltGr';
    case 'MetaLeft':
      return 'Left $meta';
    case 'MetaRight':
      return 'Right $meta';
    case 'Escape':
      return 'Esc';
    case 'Return':
      return mac ? 'Return' : 'Enter';
    case 'Backspace':
      return mac ? 'Delete ⌫' : 'Backspace';
    case 'Delete':
      return mac ? 'Forward Delete ⌦' : 'Delete';
    case 'PageUp':
      return 'Page Up';
    case 'PageDown':
      return 'Page Down';
    case 'UpArrow':
      return '↑';
    case 'DownArrow':
      return '↓';
    case 'LeftArrow':
      return '←';
    case 'RightArrow':
      return '→';
    default:
      return id;
  }
}

/// `{"enabled": bool, "maps": {system: {fromId: toId}}}`.
class KeyMappingConfig {
  bool enabled;
  final Map<String, Map<String, String>> maps;

  KeyMappingConfig(
      {this.enabled = false, Map<String, Map<String, String>>? maps})
      : maps = maps ?? {};

  factory KeyMappingConfig.parse(String raw) {
    if (raw.isEmpty) return KeyMappingConfig();
    try {
      final json = jsonDecode(raw) as Map<String, dynamic>;
      final maps = <String, Map<String, String>>{};
      final rawMaps = json['maps'];
      if (rawMaps is Map) {
        rawMaps.forEach((system, map) {
          if (map is Map) {
            maps[system.toString()] = {
              for (final e in map.entries)
                if (kKeyMappingKeyIds.contains(e.key) &&
                    kKeyMappingKeyIds.contains(e.value))
                  e.key.toString(): e.value.toString(),
            };
          }
        });
      }
      return KeyMappingConfig(enabled: json['enabled'] == true, maps: maps);
    } catch (_) {
      return KeyMappingConfig();
    }
  }

  static KeyMappingConfig load() => KeyMappingConfig.parse(
      bind.mainGetLocalOption(key: kKeyMappingLocalConfigKey));

  Future<void> save() =>
      bind.mainSetLocalOption(key: kKeyMappingLocalConfigKey, value: toJson());

  String toJson() => jsonEncode({'enabled': enabled, 'maps': maps});

  Map<String, String> mapsFor(String system) => maps[system] ?? {};

  void set(String system, String from, String to) {
    final map = maps.putIfAbsent(system, () => {});
    if (from == to) {
      map.remove(from);
    } else {
      map[from] = to;
    }
  }

  void remove(String system, String from) => maps[system]?.remove(from);

  /// Puts Alt where Command sits on a Mac keyboard, and the reverse.
  void applySwapAltCommand() {
    set('macos', 'Alt', 'MetaLeft');
    set('macos', 'MetaLeft', 'Alt');
    set('macos', 'AltGr', 'MetaRight');
    set('macos', 'MetaRight', 'AltGr');
  }
}
