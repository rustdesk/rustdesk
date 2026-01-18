import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/input_model.dart';
import 'package:flutter_hbb/models/platform_model.dart';

class RemoteShortcut {
  RemoteShortcut({
    required this.id,
    required this.name,
    required this.mainKey,
    required this.ctrl,
    required this.shift,
    required this.alt,
    required this.win,
  });

  final String id;
  final String name;
  final String mainKey;
  final bool ctrl;
  final bool shift;
  final bool alt;
  final bool win;

  Map<String, dynamic> toJson() => {
        'id': id,
        'name': name,
        'mainKey': mainKey,
        'ctrl': ctrl,
        'shift': shift,
        'alt': alt,
        'win': win,
      };

  static RemoteShortcut? tryFromJson(dynamic json) {
    if (json is! Map) return null;
    final id = json['id'];
    final name = json['name'];
    final mainKey = json['mainKey'];
    if (id is! String || name is! String || mainKey is! String) return null;
    return RemoteShortcut(
      id: id,
      name: name,
      mainKey: mainKey,
      ctrl: json['ctrl'] == true,
      shift: json['shift'] == true,
      alt: json['alt'] == true,
      win: json['win'] == true,
    );
  }

  List<String> keysDownOrder() {
    final keys = <String>[];
    if (ctrl) keys.add('VK_CONTROL');
    if (shift) keys.add('VK_SHIFT');
    if (alt) keys.add('VK_MENU');
    if (win) keys.add('VK_LWIN');
    keys.add(mainKey);
    return keys;
  }

  String displayCombo({required bool peerIsMac}) {
    final parts = <String>[];
    if (ctrl) parts.add('Ctrl');
    if (shift) parts.add('Shift');
    if (alt) parts.add('Alt');
    if (win) parts.add(peerIsMac ? 'Cmd' : 'Win');
    parts.add(_vkToDisplay(mainKey));
    return parts.join(' + ');
  }
}

String _vkToDisplay(String vk) {
  const rename = {
    'VK_ESCAPE': 'Esc',
    'VK_PRIOR': 'PgUp',
    'VK_NEXT': 'PgDn',
    'VK_BACK': 'Back',
    'VK_DELETE': 'Del',
    'VK_RETURN': 'Enter',
  };
  final mapped = rename[vk];
  if (mapped != null) return mapped;
  if (vk.startsWith('VK_')) return vk.substring(3);
  return vk;
}

class RemoteShortcutStore {
  static List<RemoteShortcut> load() {
    final raw = bind.mainGetLocalOption(key: kAndroidRemoteShortcuts);
    if (raw.isEmpty) return [];
    try {
      final decoded = jsonDecode(raw);
      if (decoded is! List) return [];
      final out = <RemoteShortcut>[];
      for (final item in decoded) {
        final shortcut = RemoteShortcut.tryFromJson(item);
        if (shortcut != null) out.add(shortcut);
      }
      return out;
    } catch (_) {
      return [];
    }
  }

  static Future<void> save(List<RemoteShortcut> shortcuts) async {
    final raw = jsonEncode(shortcuts.map((e) => e.toJson()).toList());
    await bind.mainSetLocalOption(key: kAndroidRemoteShortcuts, value: raw);
  }
}

class RemoteShortcutSender {
  static void pressOnce(InputModel inputModel, RemoteShortcut shortcut) {
    final old = _ModifierSnapshot.take(inputModel);
    inputModel.resetModifiers();

    final keys = shortcut.keysDownOrder();
    for (final key in keys) {
      inputModel.inputKey(key, down: true, press: false);
    }
    for (final key in keys.reversed) {
      inputModel.inputKey(key, down: false, press: false);
    }

    old.restore(inputModel);
  }

  static void holdDown(InputModel inputModel, RemoteShortcut shortcut) {
    final old = _ModifierSnapshot.take(inputModel);
    inputModel.resetModifiers();

    final keys = shortcut.keysDownOrder();
    for (final key in keys) {
      inputModel.inputKey(key, down: true, press: false);
    }

    old.restore(inputModel);
  }

  static void holdUp(InputModel inputModel, RemoteShortcut shortcut) {
    final old = _ModifierSnapshot.take(inputModel);
    inputModel.resetModifiers();

    final keys = shortcut.keysDownOrder();
    for (final key in keys.reversed) {
      inputModel.inputKey(key, down: false, press: false);
    }

    old.restore(inputModel);
  }
}

class _ModifierSnapshot {
  _ModifierSnapshot(this.ctrl, this.shift, this.alt, this.command);
  final bool ctrl;
  final bool shift;
  final bool alt;
  final bool command;

  static _ModifierSnapshot take(InputModel inputModel) => _ModifierSnapshot(
        inputModel.ctrl,
        inputModel.shift,
        inputModel.alt,
        inputModel.command,
      );

  void restore(InputModel inputModel) {
    inputModel.ctrl = ctrl;
    inputModel.shift = shift;
    inputModel.alt = alt;
    inputModel.command = command;
  }
}

class RemoteShortcutPanel extends StatelessWidget {
  const RemoteShortcutPanel({
    super.key,
    required this.shortcuts,
    required this.heldIds,
    required this.peerIsMac,
    required this.onPress,
    required this.onToggleHold,
    required this.onClose,
  });

  final List<RemoteShortcut> shortcuts;
  final Set<String> heldIds;
  final bool peerIsMac;
  final ValueChanged<RemoteShortcut> onPress;
  final ValueChanged<RemoteShortcut> onToggleHold;
  final VoidCallback onClose;

  @override
  Widget build(BuildContext context) {
    return Material(
      color: Colors.transparent,
      child: Container(
        width: 240,
        constraints: const BoxConstraints(maxHeight: 340),
        padding: const EdgeInsets.all(10),
        decoration: BoxDecoration(
          color: const Color(0xCC000000),
          borderRadius: BorderRadius.circular(12),
          border: Border.all(color: Colors.white24),
        ),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Row(
              children: [
                Expanded(
                  child: Text(
                    translate('Shortcuts'),
                    style: const TextStyle(
                      color: Colors.white,
                      fontSize: 13,
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                ),
                IconButton(
                  visualDensity: VisualDensity.compact,
                  padding: EdgeInsets.zero,
                  icon: const Icon(Icons.close, size: 18, color: Colors.white),
                  onPressed: onClose,
                ),
              ],
            ),
            const SizedBox(height: 6),
            Flexible(
              child: Scrollbar(
                thumbVisibility: true,
                child: GridView.builder(
                  shrinkWrap: true,
                  gridDelegate: const SliverGridDelegateWithFixedCrossAxisCount(
                    crossAxisCount: 2,
                    mainAxisSpacing: 8,
                    crossAxisSpacing: 8,
                    childAspectRatio: 2.6,
                  ),
                  itemCount: shortcuts.length,
                  itemBuilder: (context, index) {
                    final shortcut = shortcuts[index];
                    final held = heldIds.contains(shortcut.id);
                    return GestureDetector(
                      behavior: HitTestBehavior.opaque,
                      onTap: () => onPress(shortcut),
                      onDoubleTap: () => onToggleHold(shortcut),
                      child: Container(
                        padding: const EdgeInsets.symmetric(horizontal: 10),
                        decoration: BoxDecoration(
                          color: held ? MyTheme.accent80 : Colors.white10,
                          borderRadius: BorderRadius.circular(10),
                          border: Border.all(
                              color: held ? Colors.white70 : Colors.white24),
                        ),
                        alignment: Alignment.centerLeft,
                        child: Column(
                          mainAxisAlignment: MainAxisAlignment.center,
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(
                              shortcut.name,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: TextStyle(
                                color: Colors.white.withOpacity(0.85),
                                fontSize: 11,
                                fontWeight: FontWeight.w600,
                              ),
                            ),
                            const SizedBox(height: 2),
                            Text(
                              shortcut.displayCombo(peerIsMac: peerIsMac),
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: const TextStyle(
                                color: Colors.white,
                                fontSize: 11,
                              ),
                            ),
                          ],
                        ),
                      ),
                    );
                  },
                ),
              ),
            ),
            const SizedBox(height: 6),
            Text(
              translate('Tip: tap to send, double tap to hold/release'),
              style:
                  TextStyle(color: Colors.white.withOpacity(0.7), fontSize: 11),
            ),
          ],
        ),
      ),
    );
  }
}
