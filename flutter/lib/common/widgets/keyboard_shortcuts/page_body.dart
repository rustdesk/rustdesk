// flutter/lib/common/widgets/keyboard_shortcuts/page_body.dart
//
// Shared body widget for the Keyboard Shortcuts configuration page. Both the
// desktop (`desktop/pages/desktop_keyboard_shortcuts_page.dart`) and mobile
// (`mobile/pages/mobile_keyboard_shortcuts_page.dart`) pages render this
// widget inside their own platform-styled Scaffold + AppBar shell.
//
// The body owns:
//   * the top-level enable/disable toggle (mirrors the General-tab toggle —
//     same JSON key, same semantics);
//   * a grouped list of actions, each with its current binding plus
//     edit / clear icons;
//   * the JSON read/write helpers under [kShortcutLocalConfigKey] in the
//     canonical {enabled, bindings:[{action,mods,key}]} shape;
//   * the recording-dialog round-trip and conflict-replace bookkeeping;
//   * "Reset to defaults" (called from the platform AppBar).
//
// Platform shells supply only:
//   * the AppBar (with a "Reset to defaults" action that calls
//     [KeyboardShortcutsPageBodyState.resetToDefaultsWithConfirm]);
//   * surrounding padding / list-tile vs. dense-row visuals via the
//     [compact] flag.

import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';

import '../../../common.dart';
import '../../../consts.dart';
import '../../../models/platform_model.dart';
import '../../../models/shortcut_model.dart';
import 'recording_dialog.dart';

/// One configurable action — id + i18n key for its label.
class KeyboardShortcutActionEntry {
  final String id;
  final String labelKey;
  const KeyboardShortcutActionEntry(this.id, this.labelKey);
}

/// A named group of actions (e.g. "Session Control").
class KeyboardShortcutActionGroup {
  final String titleKey;
  final List<KeyboardShortcutActionEntry> actions;
  const KeyboardShortcutActionGroup(this.titleKey, this.actions);
}

/// Canonical action group definitions used by both the desktop and mobile
/// configuration pages. The order of groups and entries here is the order
/// the user sees in the UI. (Not `const` because the per-tab ids come from
/// the `kShortcutActionSwitchTab(n)` helper in `consts.dart`.)
final List<KeyboardShortcutActionGroup> kKeyboardShortcutActionGroups = [
  KeyboardShortcutActionGroup('Session Control', [
    KeyboardShortcutActionEntry(
        kShortcutActionSendCtrlAltDel, 'Insert Ctrl + Alt + Del'),
    KeyboardShortcutActionEntry(kShortcutActionInsertLock, 'Insert Lock'),
    KeyboardShortcutActionEntry(kShortcutActionRefresh, 'Refresh'),
    KeyboardShortcutActionEntry(kShortcutActionSwitchSides, 'Switch Sides'),
    KeyboardShortcutActionEntry(
        kShortcutActionToggleRecording, 'Toggle Recording'),
    KeyboardShortcutActionEntry(
        kShortcutActionToggleBlockInput, 'Toggle Block User Input'),
  ]),
  KeyboardShortcutActionGroup('Display', [
    KeyboardShortcutActionEntry(
        kShortcutActionToggleFullscreen, 'Toggle Fullscreen'),
    KeyboardShortcutActionEntry(
        kShortcutActionSwitchDisplayNext, 'Switch to next display'),
    KeyboardShortcutActionEntry(
        kShortcutActionSwitchDisplayPrev, 'Switch to previous display'),
    KeyboardShortcutActionEntry(kShortcutActionViewMode1to1, 'View Mode 1:1'),
    KeyboardShortcutActionEntry(
        kShortcutActionViewModeShrink, 'View Mode Shrink'),
    KeyboardShortcutActionEntry(
        kShortcutActionViewModeStretch, 'View Mode Stretch'),
  ]),
  KeyboardShortcutActionGroup('Other', [
    KeyboardShortcutActionEntry(kShortcutActionScreenshot, 'Take Screenshot'),
    KeyboardShortcutActionEntry(kShortcutActionToggleAudio, 'Toggle Audio'),
    KeyboardShortcutActionEntry(
        kShortcutActionTogglePrivacyMode, 'Toggle Privacy Mode'),
    for (var n = 1; n <= 9; n++)
      KeyboardShortcutActionEntry(
          kShortcutActionSwitchTab(n), 'Switch Tab $n'),
  ]),
];

/// The shared body widget. Render this inside a platform-styled Scaffold.
///
/// [compact] toggles the desktop dense-row layout (`true`) versus the mobile
/// touch-friendly ListTile layout (`false`).
///
/// [editButtonHint] is shown as the tooltip on the Edit icon. Mobile shells
/// use this to clarify that recording requires a physical keyboard.
///
/// [headerBanner] is an optional widget rendered above the toggle. Mobile
/// uses this to show the "Recording requires a physical keyboard" hint.
class KeyboardShortcutsPageBody extends StatefulWidget {
  final bool compact;
  final String? editButtonHint;
  final Widget? headerBanner;

  const KeyboardShortcutsPageBody({
    Key? key,
    this.compact = true,
    this.editButtonHint,
    this.headerBanner,
  }) : super(key: key);

  @override
  State<KeyboardShortcutsPageBody> createState() =>
      KeyboardShortcutsPageBodyState();
}

/// Public state so platform shells can call [resetToDefaultsWithConfirm] from
/// their AppBar action.
class KeyboardShortcutsPageBodyState extends State<KeyboardShortcutsPageBody> {
  // ----- Persistence helpers -----

  Map<String, dynamic> _readJson() {
    final raw = bind.mainGetLocalOption(key: kShortcutLocalConfigKey);
    if (raw.isEmpty) return {'enabled': false, 'bindings': <dynamic>[]};
    try {
      final parsed = jsonDecode(raw) as Map<String, dynamic>;
      parsed['bindings'] ??= <dynamic>[];
      parsed['enabled'] ??= false;
      return parsed;
    } catch (_) {
      return {'enabled': false, 'bindings': <dynamic>[]};
    }
  }

  Future<void> _writeJson(Map<String, dynamic> json) async {
    await bind.mainSetLocalOption(
        key: kShortcutLocalConfigKey, value: jsonEncode(json));
    // Refresh the matcher cache so writes take effect immediately. On native
    // this hits the Rust matcher; on Web the bridge forwards to the JS-side
    // matcher in flutter/web/js/.
    bind.mainReloadKeyboardShortcuts();
    if (mounted) setState(() {});
  }

  /// Replace the bindings entry for [actionId] with [binding]. If [binding]
  /// is null, removes the existing entry. If the user is replacing a
  /// conflicting binding, [clearActionId] points at the action whose
  /// (now-stale) binding should be removed in the same write.
  Future<void> _setBinding(
    String actionId, {
    Map<String, dynamic>? binding,
    String? clearActionId,
  }) async {
    final json = _readJson();
    final list = ((json['bindings'] as List?) ?? <dynamic>[])
        .cast<Map<String, dynamic>>()
        .toList();
    list.removeWhere((b) {
      final a = b['action'];
      return a == actionId || (clearActionId != null && a == clearActionId);
    });
    if (binding != null) {
      list.add(binding);
    }
    json['bindings'] = list;
    await _writeJson(json);
  }

  Future<void> _setEnabled(bool v) async {
    final json = _readJson();
    json['enabled'] = v;
    // First-time enable: seed defaults if the user has never bound anything.
    final list = (json['bindings'] as List?) ?? const [];
    if (v && list.isEmpty) {
      json['bindings'] = jsonDecode(bind.mainGetDefaultKeyboardShortcuts());
    }
    await _writeJson(json);
  }

  Future<void> _resetToDefaults() async {
    final json = _readJson();
    json['bindings'] = jsonDecode(bind.mainGetDefaultKeyboardShortcuts());
    await _writeJson(json);
  }

  String _labelFor(String actionId) {
    for (final g in kKeyboardShortcutActionGroups) {
      for (final a in g.actions) {
        if (a.id == actionId) return translate(a.labelKey);
      }
    }
    return actionId;
  }

  // ----- UI handlers -----

  Future<void> _onEdit(KeyboardShortcutActionEntry entry) async {
    final json = _readJson();
    final bindings = ((json['bindings'] as List?) ?? <dynamic>[])
        .cast<Map<String, dynamic>>();
    final result = await showRecordingDialog(
      context: context,
      actionId: entry.id,
      actionLabel: translate(entry.labelKey),
      existingBindings: bindings,
      actionLabelLookup: _labelFor,
    );
    if (result == null) return;
    await _setBinding(
      entry.id,
      binding: result.binding,
      clearActionId: result.clearActionId,
    );
  }

  Future<void> _onClear(KeyboardShortcutActionEntry entry) async {
    await _setBinding(entry.id, binding: null);
  }

  /// Public — invoked from the platform AppBar action.
  Future<void> resetToDefaultsWithConfirm() async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: Text(translate('Reset to defaults')),
        content: Text(translate('shortcut-reset-confirm-tip')),
        actions: [
          dialogButton('Cancel',
              onPressed: () => Navigator.of(ctx).pop(false),
              isOutline: true),
          dialogButton('OK', onPressed: () => Navigator.of(ctx).pop(true)),
        ],
      ),
    );
    if (confirmed == true) {
      await _resetToDefaults();
    }
  }

  // ----- Build -----

  @override
  Widget build(BuildContext context) {
    final enabled = ShortcutModel.isEnabled();
    final theme = Theme.of(context);

    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        if (widget.headerBanner != null) ...[
          widget.headerBanner!,
          const SizedBox(height: 12),
        ],
        // Top toggle — mirrors the General-tab _OptionCheckBox semantics.
        Row(
          children: [
            Checkbox(
              value: enabled,
              onChanged: (v) async {
                if (v == null) return;
                await _setEnabled(v);
              },
            ),
            const SizedBox(width: 4),
            Expanded(
              child: GestureDetector(
                behavior: HitTestBehavior.opaque,
                onTap: () => _setEnabled(!enabled),
                child: Text(
                  translate('Enable keyboard shortcuts in remote session'),
                ),
              ),
            ),
          ],
        ),
        const SizedBox(height: 8),
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 8),
          child: Text(
            translate('shortcut-page-description'),
            style: TextStyle(color: theme.hintColor),
          ),
        ),
        const SizedBox(height: 16),
        // Disabled visual state when toggle is off — but still scrollable.
        Opacity(
          opacity: enabled ? 1.0 : 0.5,
          child: AbsorbPointer(
            absorbing: !enabled,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                for (final group in kKeyboardShortcutActionGroups)
                  _buildGroup(context, group),
              ],
            ),
          ),
        ),
      ],
    );
  }

  Widget _buildGroup(BuildContext context, KeyboardShortcutActionGroup group) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        const SizedBox(height: 12),
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 8),
          child: Row(
            children: [
              Text(
                translate(group.titleKey),
                style: TextStyle(
                  fontWeight: FontWeight.w600,
                  color: Theme.of(context).colorScheme.primary,
                ),
              ),
              const SizedBox(width: 8),
              const Expanded(
                child: Divider(thickness: 1),
              ),
            ],
          ),
        ),
        const SizedBox(height: 4),
        for (final action in group.actions)
          widget.compact
              ? _buildCompactRow(context, action)
              : _buildTouchRow(context, action),
      ],
    );
  }

  /// Desktop dense row: label | shortcut | edit | clear, all in one Row.
  Widget _buildCompactRow(
      BuildContext context, KeyboardShortcutActionEntry entry) {
    final shortcut = ShortcutDisplayForActionId.format(entry.id);
    final hasBinding = shortcut != null;
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
      child: Row(
        children: [
          Expanded(
            flex: 5,
            child: Text(translate(entry.labelKey)),
          ),
          Expanded(
            flex: 4,
            child: Text(
              shortcut ?? '—',
              style: TextStyle(
                fontFamily: defaultTargetPlatform == TargetPlatform.windows
                    ? 'Consolas'
                    : 'monospace',
                color: hasBinding ? null : Theme.of(context).hintColor,
              ),
            ),
          ),
          IconButton(
            tooltip: widget.editButtonHint ?? translate('Edit'),
            onPressed: () => _onEdit(entry),
            icon: const Icon(Icons.edit_outlined, size: 18),
          ),
          SizedBox(
            width: 40,
            child: hasBinding
                ? IconButton(
                    tooltip: translate('Clear'),
                    onPressed: () => _onClear(entry),
                    icon: const Icon(Icons.close, size: 18),
                  )
                : const SizedBox.shrink(),
          ),
        ],
      ),
    );
  }

  /// Mobile touch row: ListTile with title + subtitle + trailing icons.
  Widget _buildTouchRow(
      BuildContext context, KeyboardShortcutActionEntry entry) {
    final shortcut = ShortcutDisplayForActionId.format(entry.id);
    final hasBinding = shortcut != null;
    return ListTile(
      dense: false,
      contentPadding: const EdgeInsets.symmetric(horizontal: 8),
      title: Text(translate(entry.labelKey)),
      subtitle: Text(
        shortcut ?? '—',
        style: TextStyle(
          fontFamily: defaultTargetPlatform == TargetPlatform.windows
              ? 'Consolas'
              : 'monospace',
          color: hasBinding ? null : Theme.of(context).hintColor,
        ),
      ),
      trailing: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          IconButton(
            tooltip: widget.editButtonHint ?? translate('Edit'),
            onPressed: () => _onEdit(entry),
            icon: const Icon(Icons.edit_outlined),
          ),
          if (hasBinding)
            IconButton(
              tooltip: translate('Clear'),
              onPressed: () => _onClear(entry),
              icon: const Icon(Icons.close),
            )
          else
            const SizedBox(width: 48),
        ],
      ),
    );
  }
}

/// Thin wrapper around [ShortcutDisplay.formatFor] that ignores the
/// `enabled` flag so the configuration page can always show the user what
/// they have bound, even when the feature is currently disabled.
class ShortcutDisplayForActionId {
  static String? format(String actionId) {
    final raw = bind.mainGetLocalOption(key: kShortcutLocalConfigKey);
    if (raw.isEmpty) return null;
    final Map<String, dynamic> parsed;
    try {
      parsed = jsonDecode(raw) as Map<String, dynamic>;
    } catch (_) {
      return null;
    }
    final list = (parsed['bindings'] as List? ?? const [])
        .cast<Map<String, dynamic>>();
    final found = list.firstWhere(
      (b) => b['action'] == actionId,
      orElse: () => {},
    );
    if (found.isEmpty) return null;

    // Guard against a hand-edited / corrupt config where `key` is missing or
    // not a string — render the row as unbound instead of crashing the
    // settings page.
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
        case 'primary':
          parts.add(isMac ? '⌘' : 'Ctrl');
          break;
        case 'alt':
          parts.add(isMac ? '⌥' : 'Alt');
          break;
        case 'shift':
          parts.add(isMac ? '⇧' : 'Shift');
          break;
      }
    }
    parts.add(_keyDisplay(keyValue, isMac));
    return isMac ? parts.join('') : parts.join('+');
  }

  static String _keyDisplay(String key, bool isMac) {
    switch (key) {
      case 'delete':
        return isMac ? '⌫' : 'Del';
      case 'enter':
        return isMac ? '⏎' : 'Enter';
      case 'arrow_left':
        return '←';
      case 'arrow_right':
        return '→';
      case 'arrow_up':
        return '↑';
      case 'arrow_down':
        return '↓';
    }
    if (key.startsWith('digit')) return key.substring(5);
    return key.toUpperCase();
  }
}
