import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:xterm/xterm.dart';

const _controlShiftVPasteShortcut = SingleActivator(
  LogicalKeyboardKey.keyV,
  control: true,
  shift: true,
);

Future<void> writeTerminalClipboard(String text) async {
  try {
    await Clipboard.setData(ClipboardData(text: text));
  } catch (error) {
    debugPrint('[Terminal] Failed to write clipboard: $error');
  }
}

Map<ShortcutActivator, Intent>? platformTerminalShortcuts() {
  final platform = defaultTargetPlatform;
  if (platform == TargetPlatform.linux) {
    return {
      for (final entry in defaultTerminalShortcuts.entries)
        if (!_isControlShortcut(entry.key, LogicalKeyboardKey.keyV))
          entry.key: entry.value,
      _controlShiftVPasteShortcut:
          const PasteTextIntent(SelectionChangedCause.keyboard),
    };
  }
  if (platform != TargetPlatform.windows &&
      platform != TargetPlatform.android) {
    return null;
  }
  return {
    for (final entry in defaultTerminalShortcuts.entries)
      if (!_isControlShortcut(
        entry.key,
        LogicalKeyboardKey.keyC,
        shift: true,
      ))
        entry.key: entry.value,
  };
}

bool _isControlShortcut(
  ShortcutActivator shortcut,
  LogicalKeyboardKey key, {
  bool shift = false,
}) =>
    shortcut is SingleActivator &&
    shortcut.trigger == key &&
    shortcut.control &&
    shortcut.shift == shift &&
    !shortcut.alt &&
    !shortcut.meta;

FocusOnKeyEventCallback terminalCopyHandler(
  Terminal terminal,
  TerminalController controller, {
  FocusOnKeyEventCallback? fallback,
}) =>
    (focusNode, event) {
      if (_isSelectionCopyShortcut(event)) {
        final selection = controller.selection;
        if (selection != null && !selection.isCollapsed) {
          if (event is KeyDownEvent) {
            final text = terminal.buffer.getText(selection);
            unawaited(writeTerminalClipboard(text));
          }
          return KeyEventResult.handled;
        }
      }
      return fallback?.call(focusNode, event) ?? KeyEventResult.ignored;
    };

bool _isSelectionCopyShortcut(KeyEvent event) {
  final keyboard = HardwareKeyboard.instance;
  final platform = defaultTargetPlatform;
  final usesControlCopy =
      platform == TargetPlatform.windows || platform == TargetPlatform.android;
  return usesControlCopy &&
      (event is KeyDownEvent || event is KeyRepeatEvent) &&
      event.logicalKey == LogicalKeyboardKey.keyC &&
      keyboard.isControlPressed &&
      !keyboard.isShiftPressed &&
      !keyboard.isAltPressed &&
      !keyboard.isMetaPressed;
}
