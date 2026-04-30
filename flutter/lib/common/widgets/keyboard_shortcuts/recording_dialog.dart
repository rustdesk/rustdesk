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
import 'shortcut_utils.dart';

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

  // Human-readable label for the most recent press that we couldn't bind to
  // (e.g. F13, media keys). null when the last press was either supported or
  // a modifier-only press. Cleared whenever a supported key arrives, so a
  // user who hits an unsupported key after a valid capture sees the warning
  // until they press something else. Distinct from `_key == null` so the
  // status line can tell the user *why* their press was ignored instead of
  // silently doing nothing.
  String? _unsupportedKey;

  // Modifier LogicalKeyboardKeys we should *not* treat as "unsupported" when
  // they fail to map to a key name. A modifier-only press is normal during
  // combo capture (the user is building up their combo) — only non-modifier
  // unmapped keys deserve the warning.
  static final _modifierKeys = <LogicalKeyboardKey>{
    LogicalKeyboardKey.shift,
    LogicalKeyboardKey.shiftLeft,
    LogicalKeyboardKey.shiftRight,
    LogicalKeyboardKey.control,
    LogicalKeyboardKey.controlLeft,
    LogicalKeyboardKey.controlRight,
    LogicalKeyboardKey.alt,
    LogicalKeyboardKey.altLeft,
    LogicalKeyboardKey.altRight,
    LogicalKeyboardKey.meta,
    LogicalKeyboardKey.metaLeft,
    LogicalKeyboardKey.metaRight,
    LogicalKeyboardKey.capsLock,
    LogicalKeyboardKey.numLock,
    LogicalKeyboardKey.scrollLock,
    LogicalKeyboardKey.fn,
    LogicalKeyboardKey.fnLock,
  };

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

  /// True when the captured combo includes at least one modifier. Lower bound
  /// for any sensible binding — pure single-key bindings would swallow normal
  /// typing the moment shortcuts are enabled. Beyond one mod the user is on
  /// their own; the in-session pass-through toggle is the escape hatch when
  /// a chosen combo collides with something needed on the remote.
  bool get _hasRequiredPrefix => _mods.isNotEmpty;

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
    if (event is KeyDownEvent &&
        event.logicalKey == LogicalKeyboardKey.escape) {
      Navigator.of(context).pop();
      return KeyEventResult.handled;
    }
    if (event is! KeyDownEvent) return KeyEventResult.handled;

    // Ignore modifier-only KeyDowns: don't lock in a partial combo.
    final logical = event.logicalKey;
    final keyName = logicalKeyName(logical);

    // Mirror of `normalize_modifiers` in src/keyboard/shortcuts.rs:
    //   * macOS: Cmd → primary, Ctrl → ctrl (distinct).
    //   * Win/Linux: Ctrl → primary, no separate Ctrl modifier.
    // The two halves must agree on labels, otherwise saved bindings will not
    // match the events the matcher sees at runtime.
    final mods = <String>{};
    if (HardwareKeyboard.instance.isAltPressed) mods.add('alt');
    if (HardwareKeyboard.instance.isShiftPressed) mods.add('shift');
    if (_isMac) {
      if (HardwareKeyboard.instance.isMetaPressed) mods.add('primary');
      if (HardwareKeyboard.instance.isControlPressed) mods.add('ctrl');
    } else {
      if (HardwareKeyboard.instance.isControlPressed) mods.add('primary');
    }

    setState(() {
      _mods = mods;
      // Only lock in the key when it's a non-modifier we recognize.
      // Modifier-only KeyDowns (Shift, Ctrl, etc.) leave the captured key
      // untouched, so the user can adjust modifiers after the fact.
      if (keyName != null) {
        _key = keyName;
        _unsupportedKey = null;
      } else if (!_modifierKeys.contains(logical)) {
        // Non-modifier key we don't recognize (e.g. F13, media keys, IME
        // compose keys). Surface a warning instead of silently dropping the
        // press — the dialog otherwise looks unresponsive.
        final label = logical.keyLabel.isNotEmpty
            ? logical.keyLabel
            : (logical.debugName ?? 'this key');
        _unsupportedKey = label;
      }
    });
    return KeyEventResult.handled;
  }

  void _onSave() {
    if (_key == null || !_hasRequiredPrefix) return;
    final ordered = canonicalShortcutModsForSave(_mods);
    final binding = <String, dynamic>{
      'action': widget.actionId,
      'mods': ordered,
      'key': _key!,
    };
    Navigator.of(context).pop(RecordingResult(binding, _conflictActionId));
  }

  String _formatPrefix() {
    // Used in the "must include..." validation row; lists the modifier set
    // a binding can pick from. Localised modifier glyphs aren't used here so
    // the names stay greppable for users searching for "Option" / "Cmd".
    if (_isMac) return 'Cmd / Control / Option / Shift';
    return 'Ctrl / Alt / Shift';
  }

  String _formatCombo() {
    // Plain-text labels (see same rationale in display.dart::_keyDisplay).
    final parts = <String>[];
    for (final m in ['primary', 'ctrl', 'alt', 'shift']) {
      if (!_mods.contains(m)) continue;
      switch (m) {
        case 'primary':
          parts.add(_isMac ? 'Cmd' : 'Ctrl');
          break;
        case 'ctrl':
          parts.add(_isMac ? 'Control' : 'Ctrl');
          break;
        case 'alt':
          parts.add(_isMac ? 'Option' : 'Alt');
          break;
        case 'shift':
          parts.add('Shift');
          break;
      }
    }
    if (_key != null) {
      parts.add(_keyDisplay(_key!));
    }
    if (parts.isEmpty) return translate('shortcut-recording-press-keys-tip');
    return parts.join('+');
  }

  String _keyDisplay(String key) {
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
    return key.toUpperCase();
  }

  @override
  Widget build(BuildContext context) {
    final hasKey = _key != null;
    final conflictId = _conflictActionId;
    final hasConflict = conflictId != null;
    // The Save button still fires for the previously-captured combo even if
    // the user just hit an unsupported key — the captured state is what gets
    // saved, the warning is just feedback that the latest press was rejected.
    final canSave = hasKey && _hasRequiredPrefix;

    Widget statusLine;
    if (_unsupportedKey != null) {
      // Most recent press was unsupported. Take precedence over the
      // captured-combo states so the user gets explicit feedback that their
      // last keystroke was ignored, regardless of whether a previous combo
      // is still captured.
      statusLine = Row(
        children: [
          const Icon(Icons.close, size: 16, color: Colors.red),
          const SizedBox(width: 6),
          Flexible(
            child: Text(
              translate('shortcut-key-not-supported')
                  .replaceAll('{}', _unsupportedKey!),
              style: const TextStyle(color: Colors.red),
            ),
          ),
        ],
      );
    } else if (!hasKey) {
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
              translate('shortcut-must-include-modifiers')
                  .replaceAll('{}', _formatPrefix()),
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
          Text(translate('Valid'), style: const TextStyle(color: Colors.green)),
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
                padding:
                    const EdgeInsets.symmetric(vertical: 18, horizontal: 12),
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
            onPressed: () => Navigator.of(context).pop(), isOutline: true),
        dialogButton(saveLabel, onPressed: canSave ? _onSave : null),
      ],
    );
  }
}
