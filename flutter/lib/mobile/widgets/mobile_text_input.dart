import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

import '../utils/mobile_ime_input.dart';
import '../utils/mobile_floating_cursor.dart';

class MobileTextInputController extends ChangeNotifier {
  void resetContext() => notifyListeners();
}

class MobileTextInput extends StatefulWidget {
  const MobileTextInput({
    super.key,
    required this.controller,
    required this.focusNode,
    required this.onSubmit,
    required this.onRemoteBackspace,
    this.onClose,
    required this.onError,
    this.onRemoteKey,
    this.onRemoteArrow,
    this.enabled = true,
    this.commandMode = false,
  });

  final MobileTextInputController controller;
  final FocusNode focusNode;
  final Future<void> Function(String text, bool sendReturn) onSubmit;
  final Future<void> Function() onRemoteBackspace;
  final VoidCallback? onClose;
  final void Function(Object, StackTrace) onError;
  final KeyEventResult Function(KeyEvent)? onRemoteKey;
  final Future<void> Function(String key)? onRemoteArrow;
  final bool enabled;
  final bool commandMode;

  @override
  State<MobileTextInput> createState() => _MobileTextInputState();
}

class _MobileTextInputState extends State<MobileTextInput>
    with TextInputClient, DeltaTextInputClient, WidgetsBindingObserver {
  // Android IMEs also mark ordinary predictive English words as composing.
  final _ime = MobileImeInput(
      forwardComposing: defaultTargetPlatform == TargetPlatform.android);
  final _commandKeys = <PhysicalKeyboardKey>{};
  final _floatingCursor = MobileFloatingCursor();
  TextInputConnection? _connection;

  @override
  TextEditingValue get currentTextEditingValue => _ime.value;

  @override
  AutofillScope? get currentAutofillScope => null;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    widget.focusNode.addListener(_focusChanged);
    widget.controller.addListener(_reset);
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (defaultTargetPlatform != TargetPlatform.iOS ||
        state != AppLifecycleState.inactive ||
        _connection == null) {
      return;
    }
    // iOS may commit marked text as the app loses foreground focus.
    _connection?.close();
    connectionClosed();
  }

  @override
  void didUpdateWidget(MobileTextInput oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.focusNode != widget.focusNode) {
      oldWidget.focusNode.removeListener(_focusChanged);
      widget.focusNode.addListener(_focusChanged);
    }
    if (oldWidget.controller != widget.controller) {
      oldWidget.controller.removeListener(_reset);
      widget.controller.addListener(_reset);
    }
    if (oldWidget.commandMode != widget.commandMode &&
        widget.enabled &&
        widget.focusNode.hasFocus) {
      _reset();
    }
    _focusChanged();
  }

  void _focusChanged() {
    if (!widget.enabled || !widget.focusNode.hasFocus) {
      _connection?.close();
      _connection = null;
      _commandKeys.clear();
      _reset();
      return;
    }
    if (_connection?.attached == true) return;
    _reset();
    _attach();
  }

  void _attach() {
    _connection = TextInput.attach(
      this,
      TextInputConfiguration(
        inputType: widget.commandMode
            ? TextInputType.visiblePassword
            : TextInputType.text,
        enableSuggestions: !widget.commandMode,
        inputAction: TextInputAction.done,
        enableDeltaModel: true,
        autocorrect: false,
        smartDashesType: SmartDashesType.disabled,
        smartQuotesType: SmartQuotesType.disabled,
      ),
    );
    _connection!.setEditingState(_ime.value);
    _connection!.show();
  }

  void _reset() {
    _floatingCursor.reset();
    _ime.reset();
    if (_connection?.attached != true) return;
    // A new native client lets Flutter reject callbacks queued before reset.
    // Clearing the client first gives Gboard a null editor and hides the IME.
    _attach();
  }

  @override
  void updateEditingValue(TextEditingValue value) => _reportError(
      StateError('Expected an IME delta update'), StackTrace.current);

  @override
  void updateEditingValueWithDeltas(List<TextEditingDelta> deltas) =>
      _edit(deltas, fromPlatform: true);

  void _edit(List<TextEditingDelta> deltas, {bool fromPlatform = false}) {
    if (!widget.enabled || !widget.focusNode.hasFocus) return;
    final onError = _contextErrorHandler();
    try {
      final edit = _ime.update(deltas);
      if (edit.resetContext) {
        _reset();
      } else if (!fromPlatform || edit.syncPlatform) {
        _connection?.setEditingState(_ime.value);
      }
      final pending = <Future<void>>[
        ..._moveCursor(edit.moveBefore),
        for (var index = 0; index < edit.backspaces; index++)
          widget.onRemoteBackspace(),
        if (edit.text.isNotEmpty) widget.onSubmit(edit.text, false),
        ..._moveCursor(edit.moveAfter),
      ];
      unawaited(Future.wait(pending).then<void>((_) {}).catchError(onError));
    } catch (error, stack) {
      _reportError(error, stack);
    }
  }

  Iterable<Future<void>> _moveCursor(int steps) sync* {
    for (var index = 0; index < steps.abs(); index++) {
      yield _arrow(steps < 0 ? 'VK_LEFT' : 'VK_RIGHT');
    }
  }

  Future<void> _arrow(String key) =>
      widget.onRemoteArrow?.call(key) ??
      Future.error(StateError('Remote cursor input is unavailable'));

  void _reportError(Object error, StackTrace stack,
      {bool resetContext = true}) {
    if (!mounted) return;
    if (resetContext) _reset();
    widget.onError(error, stack);
  }

  void Function(Object, StackTrace) _contextErrorHandler() {
    // Reset attaches a new client; a previous send must not clear its input.
    final connection = _connection;
    return (error, stack) => _reportError(error, stack,
        resetContext: identical(connection, _connection));
  }

  void _return() {
    if (!widget.enabled) return;
    if (_ime.composing) {
      _edit([_ime.commit()]);
      if (!_ime.forwardComposing) return;
    }
    final onError = _contextErrorHandler();
    unawaited(widget.onSubmit('', true).catchError(onError));
    _reset();
  }

  @override
  void performAction(TextInputAction action) {
    switch (action) {
      case TextInputAction.done:
      case TextInputAction.go:
      case TextInputAction.send:
      case TextInputAction.search:
        _return();
        break;
      default:
        break;
    }
  }

  KeyEventResult _keyEvent(FocusNode node, KeyEvent event) {
    if (!widget.enabled) return KeyEventResult.skipRemainingHandlers;
    final remote = widget.onRemoteKey;
    final modifier =
        event.logicalKey.keyId >= LogicalKeyboardKey.controlLeft.keyId &&
            event.logicalKey.keyId <= LogicalKeyboardKey.metaRight.keyId;
    // A modifier can go down on the canvas before this input gains focus.
    if (event is KeyUpEvent &&
        (_commandKeys.remove(event.physicalKey) || modifier)) {
      return remote?.call(event) ?? KeyEventResult.handled;
    }
    if (!_ime.composing && remote != null && event is! KeyUpEvent) {
      final result = remote(event);
      if (result == KeyEventResult.handled) {
        _commandKeys.add(event.physicalKey);
        return result;
      }
    }
    final android = defaultTargetPlatform == TargetPlatform.android;
    if (android && event.logicalKey == LogicalKeyboardKey.backspace) {
      if (event is! KeyUpEvent) _edit([_ime.backspace()]);
      return KeyEventResult.handled;
    }
    if (android &&
        (event.logicalKey == LogicalKeyboardKey.enter ||
            event.logicalKey == LogicalKeyboardKey.numpadEnter)) {
      if (event is! KeyUpEvent) _return();
      return KeyEventResult.handled;
    }
    return KeyEventResult.skipRemainingHandlers;
  }

  @override
  void connectionClosed() {
    if (_connection?.attached == true) {
      _connection?.connectionClosedReceived();
    }
    _connection = null;
    _reset();
    widget.focusNode.unfocus();
    widget.onClose?.call();
  }

  @override
  void performPrivateCommand(String action, Map<String, dynamic> data) {}

  @override
  void updateFloatingCursor(RawFloatingCursorPoint point) {
    if (!widget.enabled || !widget.focusNode.hasFocus || _ime.composing) return;
    final onError = _contextErrorHandler();
    final pending = [
      for (final key in _floatingCursor.update(point)) _arrow(key)
    ];
    unawaited(Future.wait(pending).then<void>((_) {}).catchError(onError));
    if (point.state == FloatingCursorDragState.End) _reset();
  }

  @override
  void showAutocorrectionPromptRect(int start, int end) {}

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    widget.focusNode.removeListener(_focusChanged);
    widget.controller.removeListener(_reset);
    _connection?.close();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => Focus(
        focusNode: widget.focusNode,
        onKeyEvent: _keyEvent,
        child: const SizedBox.shrink(),
      );
}
