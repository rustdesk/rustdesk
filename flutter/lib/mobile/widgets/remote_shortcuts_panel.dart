import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/remote_input_event_log.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/input_model.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/platform_model.dart';

class RemoteShortcut {
  RemoteShortcut({
    required this.id,
    required this.name,
    required this.keys,
  });

  final String id;
  final String name;
  final List<String> keys;

  factory RemoteShortcut.fromJson(Map<String, dynamic> json) {
    final id = (json['id'] ?? '').toString();
    final name = (json['name'] ?? '').toString();
    final rawKeys = json['keys'];
    final keys = (rawKeys is List)
        ? rawKeys.map((e) => e.toString()).where((e) => e.isNotEmpty).toList()
        : <String>[];
    return RemoteShortcut(id: id, name: name, keys: keys);
  }

  Map<String, dynamic> toJson() => {
        'id': id,
        'name': name,
        'keys': keys,
      };
}

class RemoteShortcutsStore {
  static List<RemoteShortcut> load() {
    final raw = bind.mainGetLocalOption(key: kAndroidRemoteShortcuts);
    if (raw.isEmpty) return <RemoteShortcut>[];
    try {
      final decoded = jsonDecode(raw);
      if (decoded is! List) return <RemoteShortcut>[];
      return decoded
          .whereType<Map>()
          .map((e) => RemoteShortcut.fromJson(Map<String, dynamic>.from(e)))
          .toList();
    } catch (_) {
      return <RemoteShortcut>[];
    }
  }

  static Future<void> save(List<RemoteShortcut> shortcuts) async {
    final raw = jsonEncode(shortcuts.map((e) => e.toJson()).toList());
    await bind.mainSetLocalOption(key: kAndroidRemoteShortcuts, value: raw);
  }
}

String formatShortcutKeys(List<String> keys, {required bool isMacPeer}) {
  String mapOne(String k) {
    switch (k) {
      case 'VK_CONTROL':
        return 'Ctrl';
      case 'VK_SHIFT':
        return 'Shift';
      case 'VK_MENU':
        return 'Alt';
      case 'Meta':
      case 'RWin':
        return isMacPeer ? 'Cmd' : 'Win';
      default:
        if (k.startsWith('VK_')) return k.substring(3);
        return k;
    }
  }

  return keys.map(mapOne).join(' + ');
}

Future<void> sendShortcutOnce(InputModel inputModel, List<String> keys,
    {required bool isMacPeer}) async {
  if (keys.isEmpty) return;
  final savedCtrl = inputModel.ctrl;
  final savedShift = inputModel.shift;
  final savedAlt = inputModel.alt;
  final savedCmd = inputModel.command;
  inputModel.resetModifiers();
  for (final k in keys) {
    inputModel.inputKey(k, down: true, press: false);
  }
  for (final k in keys.reversed) {
    inputModel.inputKey(k, down: false, press: false);
  }
  inputModel.ctrl = savedCtrl;
  inputModel.shift = savedShift;
  inputModel.alt = savedAlt;
  inputModel.command = savedCmd;

  RemoteInputEventLog.add('shortcut_key_press', data: {
    'keys': formatShortcutKeys(keys, isMacPeer: isMacPeer),
  });
}

Future<void> setShortcutHold(InputModel inputModel, List<String> keys,
    {required bool hold, required bool isMacPeer}) async {
  if (keys.isEmpty) return;
  final savedCtrl = inputModel.ctrl;
  final savedShift = inputModel.shift;
  final savedAlt = inputModel.alt;
  final savedCmd = inputModel.command;
  inputModel.resetModifiers();
  if (hold) {
    for (final k in keys) {
      inputModel.inputKey(k, down: true, press: false);
    }
    RemoteInputEventLog.add('shortcut_key_hold_on', data: {
      'keys': formatShortcutKeys(keys, isMacPeer: isMacPeer),
    });
  } else {
    for (final k in keys.reversed) {
      inputModel.inputKey(k, down: false, press: false);
    }
    RemoteInputEventLog.add('shortcut_key_hold_off', data: {
      'keys': formatShortcutKeys(keys, isMacPeer: isMacPeer),
    });
  }
  inputModel.ctrl = savedCtrl;
  inputModel.shift = savedShift;
  inputModel.alt = savedAlt;
  inputModel.command = savedCmd;
}

class RemoteShortcutsPanel extends StatefulWidget {
  const RemoteShortcutsPanel({
    super.key,
    required this.visible,
    required this.cursorModel,
    required this.inputModel,
    required this.shortcuts,
    required this.heldShortcutIds,
    required this.onDelete,
    required this.onToggleHold,
  });

  final bool visible;
  final CursorModel cursorModel;
  final InputModel inputModel;
  final List<RemoteShortcut> shortcuts;
  final Set<String> heldShortcutIds;
  final ValueChanged<RemoteShortcut> onDelete;
  final ValueChanged<RemoteShortcut> onToggleHold;

  @override
  State<RemoteShortcutsPanel> createState() => _RemoteShortcutsPanelState();
}

class _RemoteShortcutsPanelState extends State<RemoteShortcutsPanel> {
  static const double _right = 10;
  static const double _top = 86;
  static const double _width = 240;
  static const double _height = 280;
  static const double _btnW = 180;
  static const double _btnH = 42;

  Rect? _blockedRect;

  void _syncBlockedRect(Size screenSize) {
    final enabled = widget.visible;
    final left = (screenSize.width - _right - _width).clamp(0.0, screenSize.width);
    final newRect =
        enabled ? Rect.fromLTWH(left, _top, _width, _height) : null;

    if (_blockedRect != null) {
      widget.cursorModel.removeBlockedRect(_blockedRect!);
      _blockedRect = null;
    }
    if (newRect != null) {
      widget.cursorModel.addBlockedRect(newRect);
      _blockedRect = newRect;
    }
  }

  @override
  void dispose() {
    if (_blockedRect != null) {
      widget.cursorModel.removeBlockedRect(_blockedRect!);
      _blockedRect = null;
    }
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final screenSize = MediaQuery.of(context).size;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) _syncBlockedRect(screenSize);
    });

    if (!widget.visible) return const Offstage();

    final pi = gFFI.ffiModel.pi;
    final isMacPeer = pi.platform == kPeerPlatformMacOS;

    return Positioned(
      right: _right,
      top: _top,
      width: _width,
      height: _height,
      child: Semantics(
        label: 'u2_remote_shortcuts_panel',
        container: true,
        excludeSemantics: true,
        child: Material(
          color: Colors.transparent,
          child: Container(
            decoration: BoxDecoration(
              color: Colors.grey.withOpacity(0.18),
              borderRadius: BorderRadius.circular(12),
              border: Border.all(color: Colors.white24),
            ),
            child: Column(
              children: [
                const SizedBox(height: 10),
                const Text(
                  '快捷键',
                  style: TextStyle(
                    color: Colors.white,
                    fontSize: 12,
                    fontWeight: FontWeight.w600,
                  ),
                ),
                const SizedBox(height: 8),
                Expanded(
                  child: ListView.separated(
                    padding: const EdgeInsets.symmetric(
                      horizontal: 12,
                      vertical: 8,
                    ),
                    itemCount: widget.shortcuts.length,
                    separatorBuilder: (_, __) => const SizedBox(height: 8),
                    itemBuilder: (context, idx) {
                      final s = widget.shortcuts[idx];
                      final held = widget.heldShortcutIds.contains(s.id);
                      final label = s.name.isNotEmpty
                          ? s.name
                          : formatShortcutKeys(s.keys, isMacPeer: isMacPeer);

                      return Semantics(
                        label: 'u2_remote_shortcut_button',
                        button: true,
                        child: GestureDetector(
                          onTap: () => sendShortcutOnce(
                            widget.inputModel,
                            s.keys,
                            isMacPeer: isMacPeer,
                          ),
                          onDoubleTap: () => widget.onToggleHold(s),
                          onLongPress: () async {
                            final ok = await showDialog<bool>(
                                  context: context,
                                  builder: (ctx) => AlertDialog(
                                    title: const Text('删除快捷键？'),
                                    content: Text(label),
                                    actions: [
                                      TextButton(
                                        onPressed: () =>
                                            Navigator.of(ctx).pop(false),
                                        child: const Text('取消'),
                                      ),
                                      TextButton(
                                        onPressed: () =>
                                            Navigator.of(ctx).pop(true),
                                        child: const Text('删除'),
                                      ),
                                    ],
                                  ),
                                ) ??
                                false;
                            if (ok) widget.onDelete(s);
                          },
                          child: Container(
                            width: _btnW,
                            height: _btnH,
                            decoration: BoxDecoration(
                              color: Colors.grey.withOpacity(0.25),
                              borderRadius: BorderRadius.circular(10),
                              border: Border.all(
                                color: held ? Colors.white : Colors.white38,
                                width: held ? 2 : 1,
                              ),
                            ),
                            alignment: Alignment.center,
                            padding:
                                const EdgeInsets.symmetric(horizontal: 10),
                            child: Text(
                              label,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: const TextStyle(
                                color: Colors.white,
                                fontSize: 12,
                              ),
                            ),
                          ),
                        ),
                      );
                    },
                  ),
                ),
                if (isAndroid)
                  const Padding(
                    padding: EdgeInsets.only(bottom: 10),
                    child: Text(
                      '单击=触发；双击=按住/释放；长按=删除',
                      style: TextStyle(color: Colors.white70, fontSize: 10),
                    ),
                  ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

