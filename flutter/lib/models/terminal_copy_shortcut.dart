import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:xterm/xterm.dart';

import 'terminal_clipboard_writer.dart'
    if (dart.library.html) 'terminal_clipboard_writer_web.dart';

const _controlShiftVPasteShortcut = SingleActivator(
  LogicalKeyboardKey.keyV,
  control: true,
  shift: true,
);

typedef TerminalClipboardWriter = Future<bool> Function(
  String text, {
  required bool userInitiated,
});

class TerminalClipboardNoticeRequest<T> {
  const TerminalClipboardNoticeRequest({
    required this.source,
    required this.text,
    required this.persistAllowed,
  });

  final T source;
  final String text;
  final bool persistAllowed;

  String get actionKey => persistAllowed ? 'Enable' : 'Copy to clipboard';

  String get negativeActionKey => persistAllowed ? 'Decline' : 'Dismiss';
}

const kTerminalClipboardNoticeMessageKey = 'terminal-clipboard-write-tip';

class TerminalClipboardNoticeCoordinator<T> extends ChangeNotifier {
  TerminalClipboardNoticeRequest<T>? _current;
  bool _noticeVisible = false;
  bool _actionInProgress = false;

  TerminalClipboardNoticeRequest<T>? get current => _current;
  bool get canClaimAction =>
      _noticeVisible && !_actionInProgress && _current != null;

  TerminalClipboardNoticeRequest<T>? currentForSource(T source) {
    final current = _current;
    if (current == null || current.source != source) return null;
    return current;
  }

  TerminalClipboardNoticeRequest<T>? recordBlocked({
    required T source,
    required String text,
    required String option,
    required bool Function(T source) canWrite,
  }) {
    if (!canWrite(source)) return null;
    final requestAllowsPersistence =
        option == kTerminalClipboardWriteUnconfigured;
    if (option != kTerminalClipboardWriteAllowed && !requestAllowsPersistence) {
      return null;
    }
    if (_noticeVisible && _actionInProgress) return null;
    final wasVisible = _noticeVisible;
    final persistAllowed =
        wasVisible ? _current?.persistAllowed : requestAllowsPersistence;
    final request = TerminalClipboardNoticeRequest(
      source: source,
      text: text,
      persistAllowed: persistAllowed ?? requestAllowsPersistence,
    );
    _current = request;
    if (wasVisible) return null;
    _noticeVisible = true;
    return request;
  }

  TerminalClipboardNoticeRequest<T>? claimCurrentAction() {
    if (!canClaimAction) return null;
    final current = _current;
    if (current == null) return null;
    _actionInProgress = true;
    notifyListeners();
    return current;
  }

  void releaseAction() {
    if (!_actionInProgress) return;
    _actionInProgress = false;
    notifyListeners();
  }

  bool beginClose() {
    if (!_noticeVisible) return false;
    _actionInProgress = true;
    notifyListeners();
    return true;
  }

  void noticeClosed() => clear();

  void clear() {
    _current = null;
    _noticeVisible = false;
    _actionInProgress = false;
  }
}

Future<bool> writeTerminalClipboard(
  String text, {
  bool userInitiated = false,
}) =>
    writeTerminalClipboardPlatform(text, userInitiated: userInitiated);

Future<bool> completeTerminalClipboardWrite({
  required String clipboardText,
  required bool Function() canWrite,
  required TerminalClipboardWriter writeClipboard,
  Future<void> Function()? persistAllowed,
}) async {
  if (!canWrite()) return false;
  if (!await writeClipboard(clipboardText, userInitiated: true)) return false;
  await persistAllowed?.call();
  return true;
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
            unawaited(writeTerminalClipboard(text, userInitiated: true));
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
