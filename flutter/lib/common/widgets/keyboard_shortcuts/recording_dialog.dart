// flutter/lib/common/widgets/keyboard_shortcuts/recording_dialog.dart
//
// Modal dialog used by the Keyboard Shortcuts settings page to capture a new
// key combination for a given action. The dialog listens for KeyDown events,
// extracts the modifier set + non-modifier key, validates against the
// "must include Ctrl+Alt+Shift (Cmd+Option+Shift on macOS)" rule, and reports
// any conflict with another already-bound action.
//
// On Save, returns the new binding map ({action, mods, key}) plus the
// optional id of the action whose binding should be cleared (the conflict
// "Replace" path). On Cancel, returns null.

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../../common.dart';

/// Result of the recording dialog.
class RecordingResult {
  /// The new binding map to write: {action, mods, key}.
  final Map<String, dynamic> binding;

  /// If the chosen combo conflicted with another action, the user chose
  /// "Replace" — the caller must clear this action's binding before writing
  /// the new one.
  final String? clearActionId;

  RecordingResult(this.binding, this.clearActionId);
}

/// Show the recording dialog.
///
/// [actionId] is the action being edited (used for the title and to detect
/// "binding to itself" — that's not a conflict).
/// [actionLabel] is the translated, user-facing action name.
/// [existingBindings] is the current bindings list (used for conflict detection).
/// [actionLabelLookup] resolves an actionId to its translated label, used in
/// the conflict warning.
Future<RecordingResult?> showRecordingDialog({
  required BuildContext context,
  required String actionId,
  required String actionLabel,
  required List<Map<String, dynamic>> existingBindings,
  required String Function(String) actionLabelLookup,
}) {
  return showDialog<RecordingResult>(
    context: context,
    barrierDismissible: false,
    builder: (ctx) => _RecordingDialog(
      actionId: actionId,
      actionLabel: actionLabel,
      existingBindings: existingBindings,
      actionLabelLookup: actionLabelLookup,
    ),
  );
}

class _RecordingDialog extends StatefulWidget {
  final String actionId;
  final String actionLabel;
  final List<Map<String, dynamic>> existingBindings;
  final String Function(String) actionLabelLookup;

  const _RecordingDialog({
    required this.actionId,
    required this.actionLabel,
    required this.existingBindings,
    required this.actionLabelLookup,
  });

  @override
  State<_RecordingDialog> createState() => _RecordingDialogState();
}

class _RecordingDialogState extends State<_RecordingDialog> {
  final FocusNode _focusNode = FocusNode();

  // Captured combo. null until the user presses something with a non-modifier.
  Set<String> _mods = {};
  String? _key;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _focusNode.requestFocus();
    });
  }

  @override
  void dispose() {
    _focusNode.dispose();
    super.dispose();
  }

  bool get _isMac =>
      defaultTargetPlatform == TargetPlatform.macOS ||
      defaultTargetPlatform == TargetPlatform.iOS;

  /// True when the captured combo includes the required Ctrl+Alt+Shift
  /// (Cmd+Option+Shift on macOS) prefix and a non-modifier key.
  bool get _hasRequiredPrefix =>
      _mods.contains('primary') &&
      _mods.contains('alt') &&
      _mods.contains('shift');

  /// Return the actionId that this combo currently conflicts with, or null.
  /// The action being edited is not a conflict with itself.
  String? get _conflictActionId {
    if (_key == null || !_hasRequiredPrefix) return null;
    for (final b in widget.existingBindings) {
      final otherAction = b['action'] as String?;
      if (otherAction == null || otherAction == widget.actionId) continue;
      final otherKey = b['key'] as String?;
      final otherMods =
          ((b['mods'] as List?) ?? const []).cast<String>().toSet();
      if (otherKey == _key &&
          otherMods.length == _mods.length &&
          otherMods.containsAll(_mods)) {
        return otherAction;
      }
    }
    return null;
  }

  KeyEventResult _onKeyEvent(FocusNode node, KeyEvent event) {
    if (event is KeyDownEvent && event.logicalKey == LogicalKeyboardKey.escape) {
      Navigator.of(context).pop();
      return KeyEventResult.handled;
    }
    if (event is! KeyDownEvent) return KeyEventResult.handled;

    // Ignore modifier-only KeyDowns: don't lock in a partial combo.
    final logical = event.logicalKey;
    final keyName = _logicalToKeyName(logical);

    final mods = <String>{};
    if (HardwareKeyboard.instance.isAltPressed) mods.add('alt');
    if (HardwareKeyboard.instance.isShiftPressed) mods.add('shift');
    final primary = _isMac
        ? HardwareKeyboard.instance.isMetaPressed
        : HardwareKeyboard.instance.isControlPressed;
    if (primary) mods.add('primary');

    setState(() {
      _mods = mods;
      // Only lock in the key when it's a non-modifier we recognize.
      // Modifier-only KeyDowns (Shift, Ctrl, etc.) leave the captured key
      // untouched, so the user can adjust modifiers after the fact.
      if (keyName != null) {
        _key = keyName;
      }
    });
    return KeyEventResult.handled;
  }

  void _onSave() {
    if (_key == null || !_hasRequiredPrefix) return;
    // Sort mods to match the canonical order used by Rust default_bindings:
    // primary, alt, shift.
    final ordered = <String>[
      if (_mods.contains('primary')) 'primary',
      if (_mods.contains('alt')) 'alt',
      if (_mods.contains('shift')) 'shift',
    ];
    final binding = <String, dynamic>{
      'action': widget.actionId,
      'mods': ordered,
      'key': _key!,
    };
    Navigator.of(context).pop(RecordingResult(binding, _conflictActionId));
  }

  String _formatPrefix() {
    if (_isMac) return 'Cmd+Option+Shift';
    return 'Ctrl+Alt+Shift';
  }

  String _formatCombo() {
    final parts = <String>[];
    for (final m in ['primary', 'alt', 'shift']) {
      if (!_mods.contains(m)) continue;
      switch (m) {
        case 'primary':
          parts.add(_isMac ? '⌘' : 'Ctrl');
          break;
        case 'alt':
          parts.add(_isMac ? '⌥' : 'Alt');
          break;
        case 'shift':
          parts.add(_isMac ? '⇧' : 'Shift');
          break;
      }
    }
    if (_key != null) {
      parts.add(_keyDisplay(_key!));
    }
    if (parts.isEmpty) return translate('shortcut-recording-press-keys-tip');
    return _isMac ? parts.join('') : parts.join('+');
  }

  String _keyDisplay(String key) {
    switch (key) {
      case 'delete':
        return _isMac ? '⌫' : 'Del';
      case 'enter':
        return _isMac ? '⏎' : 'Enter';
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

  @override
  Widget build(BuildContext context) {
    final hasKey = _key != null;
    final conflictId = _conflictActionId;
    final hasConflict = conflictId != null;
    final canSave = hasKey && _hasRequiredPrefix;

    Widget statusLine;
    if (!hasKey) {
      statusLine = Text(
        translate('shortcut-recording-press-keys-tip'),
        style: TextStyle(color: Theme.of(context).hintColor),
      );
    } else if (!_hasRequiredPrefix) {
      statusLine = Row(
        children: [
          Icon(Icons.close, size: 16, color: Colors.red),
          const SizedBox(width: 6),
          Flexible(
            child: Text(
              '${translate('shortcut-must-include-prefix')} ${_formatPrefix()}',
              style: const TextStyle(color: Colors.red),
            ),
          ),
        ],
      );
    } else if (hasConflict) {
      final otherLabel = widget.actionLabelLookup(conflictId);
      statusLine = Row(
        children: [
          Icon(Icons.warning_amber_outlined,
              size: 16, color: Colors.orange.shade700),
          const SizedBox(width: 6),
          Flexible(
            child: Text(
              '${translate('shortcut-already-bound-to')} "$otherLabel"',
              style: TextStyle(color: Colors.orange.shade700),
            ),
          ),
        ],
      );
    } else {
      statusLine = Row(
        children: [
          const Icon(Icons.check, size: 16, color: Colors.green),
          const SizedBox(width: 6),
          Text(translate('Valid'),
              style: const TextStyle(color: Colors.green)),
        ],
      );
    }

    final saveLabel = hasConflict ? 'Replace' : 'Save';

    return AlertDialog(
      title: Text(
        '${translate('Set Shortcut')}: ${widget.actionLabel}',
      ),
      content: Focus(
        focusNode: _focusNode,
        autofocus: true,
        onKeyEvent: _onKeyEvent,
        child: ConstrainedBox(
          constraints: const BoxConstraints(minWidth: 380),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(translate('shortcut-recording-instruction')),
              const SizedBox(height: 12),
              Container(
                width: double.infinity,
                padding: const EdgeInsets.symmetric(
                    vertical: 18, horizontal: 12),
                decoration: BoxDecoration(
                  border: Border.all(color: Theme.of(context).dividerColor),
                  borderRadius: BorderRadius.circular(4),
                ),
                child: Text(
                  _formatCombo(),
                  textAlign: TextAlign.center,
                  style: TextStyle(
                    fontSize: 18,
                    fontWeight: FontWeight.w600,
                    color: hasKey
                        ? Theme.of(context).textTheme.titleLarge?.color
                        : Theme.of(context).hintColor,
                  ),
                ),
              ),
              const SizedBox(height: 12),
              statusLine,
            ],
          ),
        ),
      ),
      actions: [
        dialogButton('Cancel',
            onPressed: () => Navigator.of(context).pop(),
            isOutline: true),
        dialogButton(saveLabel, onPressed: canSave ? _onSave : null),
      ],
    );
  }

  /// Mirror of `event_to_key_name` in `src/keyboard/shortcuts.rs` and
  /// `logicalToKeyName` in `flutter/web/js/src/shortcut_matcher.ts` — keep
  /// the three in lockstep. Returns null for modifier-only or unsupported keys.
  static String? _logicalToKeyName(LogicalKeyboardKey k) {
    if (k == LogicalKeyboardKey.delete) return 'delete';
    if (k == LogicalKeyboardKey.enter ||
        k == LogicalKeyboardKey.numpadEnter) return 'enter';
    if (k == LogicalKeyboardKey.arrowLeft) return 'arrow_left';
    if (k == LogicalKeyboardKey.arrowRight) return 'arrow_right';
    if (k == LogicalKeyboardKey.arrowUp) return 'arrow_up';
    if (k == LogicalKeyboardKey.arrowDown) return 'arrow_down';

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
    if (letters.containsKey(k)) return letters[k];

    final digits = <LogicalKeyboardKey, String>{
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
    if (digits.containsKey(k)) return digits[k];

    return null;
  }
}
