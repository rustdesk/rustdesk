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
  if (defaultTargetPlatform != TargetPlatform.linux) return null;
  return {
    for (final entry in defaultTerminalShortcuts.entries)
      if (!_isControlVShortcut(entry.key)) entry.key: entry.value,
    _controlShiftVPasteShortcut:
        const PasteTextIntent(SelectionChangedCause.keyboard),
  };
}

bool _isControlVShortcut(ShortcutActivator shortcut) =>
    shortcut is SingleActivator &&
    shortcut.trigger == LogicalKeyboardKey.keyV &&
    shortcut.control &&
    !shortcut.shift &&
    !shortcut.alt &&
    !shortcut.meta;

FocusOnKeyEventCallback terminalCopyHandler(
  Terminal terminal,
  TerminalController controller,
) =>
    (_, event) {
      if (!_isWindowsCopyShortcut(event)) return KeyEventResult.ignored;
      final selection = controller.selection;
      if (selection == null || selection.isCollapsed) {
        return KeyEventResult.ignored;
      }
      if (event is KeyDownEvent) {
        final text = terminal.buffer.getText(selection);
        unawaited(writeTerminalClipboard(text));
      }
      return KeyEventResult.handled;
    };

bool _isWindowsCopyShortcut(KeyEvent event) {
  final keyboard = HardwareKeyboard.instance;
  return defaultTargetPlatform == TargetPlatform.windows &&
      (event is KeyDownEvent || event is KeyRepeatEvent) &&
      event.logicalKey == LogicalKeyboardKey.keyC &&
      keyboard.isControlPressed &&
      !keyboard.isShiftPressed &&
      !keyboard.isAltPressed &&
      !keyboard.isMetaPressed;
}
