import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

/// Forwards text that reaches Flutter only as *text input* to the remote side.
///
/// On Windows, input methods such as Unikey and EVKey (and AutoHotkey, on-screen
/// keyboards, ...) emit composed characters with `SendInput(KEYEVENTF_UNICODE)`,
/// i.e. `VK_PACKET`. Flutter's Windows embedder skips `VK_PACKET` key messages and
/// hands the resulting `WM_CHAR` to the text input plugin only, so these characters
/// never become key events. Without a text input client the remote page drops them,
/// while the Backspaces the input method sends to erase the raw letters do arrive:
/// typing "coong" (Telex for "công") ends up as "cng" on the remote side.
///
/// Mobile does not have this problem because it reads text from a hidden text field
/// and sends it with `sessionInputString`. This client does the same on desktop: it
/// is attached while the remote view has focus and forwards committed text.
class ImeTextBridge with TextInputClient {
  ImeTextBridge({required this.onText, this.maxBufferLength = 4096});

  /// Receives each piece of committed text, in order.
  final void Function(String text) onText;

  /// The engine-side buffer is cleared once it holds this many characters.
  final int maxBufferLength;

  TextInputConnection? _connection;
  TextEditingValue _value = TextEditingValue.empty;

  /// Text in the engine buffer that has already been forwarded.
  String _forwarded = '';

  /// Set after a buffer reset was requested, until an update shows the engine
  /// applied it. Updates still based on the old buffer must not be resent.
  String? _beforeReset;

  bool get isAttached => _connection?.attached ?? false;

  void attach() {
    if (isAttached) return;
    _value = TextEditingValue.empty;
    _forwarded = '';
    _beforeReset = null;
    _connection = TextInput.attach(
      this,
      const TextInputConfiguration(
        inputType: TextInputType.text,
        inputAction: TextInputAction.none,
        autocorrect: false,
        enableSuggestions: false,
        enableIMEPersonalizedLearning: false,
      ),
    );
    _connection!.setEditingState(TextEditingValue.empty);
  }

  void detach() {
    _connection?.close();
    _connection = null;
  }

  @override
  void updateEditingValue(TextEditingValue value) {
    _value = value;
    // An IME composition (e.g. Microsoft Vietnamese Telex) is sent once committed.
    if (value.composing.isValid && !value.composing.isCollapsed) return;

    final text = value.text;
    final pendingReset = _beforeReset;
    String delta;
    if (pendingReset != null && text.startsWith(pendingReset)) {
      // The engine has not applied the reset yet: its buffer still starts with
      // text that was already forwarded.
      delta = text.substring(pendingReset.length);
      _beforeReset = text;
    } else {
      _beforeReset = null;
      delta = text.startsWith(_forwarded)
          ? text.substring(_forwarded.length)
          : text;
      _forwarded = text;
      if (_forwarded.length >= maxBufferLength) {
        _beforeReset = _forwarded;
        _forwarded = '';
        _value = TextEditingValue.empty;
        _connection?.setEditingState(TextEditingValue.empty);
      }
    }
    if (delta.isNotEmpty) onText(delta);
  }

  @override
  TextEditingValue? get currentTextEditingValue => _value;

  @override
  AutofillScope? get currentAutofillScope => null;

  @override
  void performAction(TextInputAction action) {}

  @override
  void performPrivateCommand(String action, Map<String, dynamic> data) {}

  @override
  void updateFloatingCursor(RawFloatingCursorPoint point) {}

  @override
  void showAutocorrectionPromptRect(int start, int end) {}

  @override
  void connectionClosed() {
    _connection = null;
  }
}

/// Runs async tasks strictly one after another.
///
/// Key events and text go to Rust through separate async FFI calls, which may run
/// on different worker threads. An input method sends Backspaces immediately
/// followed by the replacement text, so both must reach the peer in that order.
class OrderedSender {
  Future<void> _tail = Future<void>.value();

  Future<void> run(Future<void> Function() task) {
    final next = _tail.then((_) => task()).catchError((Object e) {
      debugPrint('OrderedSender: task failed: $e');
    });
    _tail = next;
    return next;
  }
}

/// The bridge is used on Windows with Flutter as the input source ("Input
/// source 2"), where Flutter delivers input-method text, in Map (the default)
/// and Translate mode. Input source 1 grabs keys with a low-level hook before
/// the input method sees them, so there is nothing to forward there; legacy
/// mode has its own key path.
bool shouldUseImeTextBridge({
  required bool isWindows,
  required bool inputSourceFlutter,
  required String keyboardMode,
  required bool keyboardPerm,
  required bool viewOnly,
}) =>
    isWindows &&
    inputSourceFlutter &&
    (keyboardMode == 'map' || keyboardMode == 'translate') &&
    keyboardPerm &&
    !viewOnly;

/// Keys an input method may inject without a scan code (Windows `SendInput`
/// with `wScan = 0`). Flutter derives the physical key from the scan code only,
/// so these arrive as an unknown physical key; the logical key is still right.
final Map<LogicalKeyboardKey, PhysicalKeyboardKey> _physicalFallback = {
  LogicalKeyboardKey.backspace: PhysicalKeyboardKey.backspace,
  LogicalKeyboardKey.delete: PhysicalKeyboardKey.delete,
  LogicalKeyboardKey.enter: PhysicalKeyboardKey.enter,
  LogicalKeyboardKey.numpadEnter: PhysicalKeyboardKey.numpadEnter,
  LogicalKeyboardKey.tab: PhysicalKeyboardKey.tab,
  LogicalKeyboardKey.escape: PhysicalKeyboardKey.escape,
  LogicalKeyboardKey.space: PhysicalKeyboardKey.space,
  LogicalKeyboardKey.arrowLeft: PhysicalKeyboardKey.arrowLeft,
  LogicalKeyboardKey.arrowRight: PhysicalKeyboardKey.arrowRight,
  LogicalKeyboardKey.arrowUp: PhysicalKeyboardKey.arrowUp,
  LogicalKeyboardKey.arrowDown: PhysicalKeyboardKey.arrowDown,
  LogicalKeyboardKey.home: PhysicalKeyboardKey.home,
  LogicalKeyboardKey.end: PhysicalKeyboardKey.end,
};

/// The USB HID usage to forward for a key event (low 16 bits, as the Rust side
/// expects), recovering keys whose physical key Flutter could not identify.
int forwardingUsbHid(PhysicalKeyboardKey physical, LogicalKeyboardKey logical) {
  if (PhysicalKeyboardKey.findKeyByCode(physical.usbHidUsage) == null) {
    final fallback = _physicalFallback[logical];
    if (fallback != null) return fallback.usbHidUsage & 0xFFFF;
  }
  return physical.usbHidUsage & 0xFFFF;
}
