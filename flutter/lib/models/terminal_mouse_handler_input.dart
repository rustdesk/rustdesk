part of 'terminal_mouse_handler.dart';

/// xterm 4.0.0 encodes wheel buttons as 68..71; the extra bit reads as a Shift
/// modifier, so strict full-screen apps ignore the report and never scroll.
/// Upstream fix: TerminalStudio/xterm.dart#238.
class WheelButtonFixMouseHandler implements TerminalMouseHandler {
  const WheelButtonFixMouseHandler({
    this.positionProvider,
    this.suppressLeftButton,
  });

  final CellOffset? Function()? positionProvider;
  final bool Function(TerminalMouseButtonState)? suppressLeftButton;

  @override
  String? call(TerminalMouseEvent event) {
    if (!event.button.isWheel) {
      if (event.button == TerminalMouseButton.left &&
          suppressLeftButton?.call(event.buttonState) == true) {
        return null;
      }
      return defaultMouseHandler(event);
    }
    // Same gate as UpDownMouseHandler: only the scroll modes report a wheel,
    // and a wheel release is never reported, so the report is always a press.
    if (!event.state.mouseMode.reportScroll ||
        event.buttonState == TerminalMouseButtonState.up) {
      return null;
    }
    return _reportWheel(event);
  }

  String _reportWheel(TerminalMouseEvent event) {
    // Wheel buttons 4..7 go on the wire as 64..67, but `id` is 64 + 4..7.
    final button = event.button.id - 4;
    final position = positionProvider?.call() ?? event.position;
    return encodeTerminalMouseReport(
      event.state.mouseReportMode,
      button,
      position,
    );
  }
}

extension _TerminalMouseInput on _TerminalMouseInteractionState {
  CellOffset? _cellAtPointer() {
    final terminalView = _terminalView;
    final pointerPosition = _pointerPosition;
    if (terminalView == null || pointerPosition == null) return null;
    final renderTerminal = terminalView.renderTerminal;
    return renderTerminal.getCellOffset(
      renderTerminal.globalToLocal(pointerPosition),
    );
  }

  void _updatePointerPosition(PointerEvent event) =>
      _pointerPosition = event.position;

  void _handlePointerDown(PointerDownEvent event) {
    _updatePointerPosition(event);
    _suppressXtermLeftButton = false;
    if (_startPendingTouchMouseDrag(event)) return;
    if (_mouseDrag.handleDown(event, widget.terminal, _terminalView)) {
      _prepareTerminalClipboardWrite();
      if (kIsWeb) _suppressXtermLeftButton = true;
      _clearSelectionDrag();
      return;
    }
    if (event.kind != PointerDeviceKind.mouse ||
        (event.buttons & kPrimaryMouseButton) != kPrimaryMouseButton) {
      return;
    }
    _clearSelectionDrag();
    final terminalView = _terminalView;
    if (terminalView == null) return;
    final renderTerminal = terminalView.renderTerminal;
    final localPosition = renderTerminal.globalToLocal(event.position);
    final selectionBuffer = widget.terminal.buffer;
    _selectionPointerId = event.pointer;
    _selectionBase = selectionBuffer.createAnchorFromOffset(
      renderTerminal.getCellOffset(localPosition),
    );
    _selectionBuffer = selectionBuffer;
    _selectionPointer = localPosition;
  }

  bool _startPendingTouchMouseDrag(PointerDownEvent event) {
    if (!widget.reportTouchInput ||
        event.kind != PointerDeviceKind.touch ||
        !_mouseDrag.handleDown(
          event,
          widget.terminal,
          _terminalView,
          reportTouchInput: true,
          deferReport: true,
        )) {
      return false;
    }
    _pendingTouchMouseDown = event;
    _pendingTouchMouseTimer = Timer(
      kLongPressTimeout,
      _activatePendingTouchMouseDrag,
    );
    return true;
  }

  bool _activatePendingTouchMouseDrag({
    bool cancelOnFailure = true,
  }) {
    if (_takePendingTouchMouseDrag() == null) return false;
    if (_mouseDrag.activateDeferredDown(widget.terminal)) {
      _prepareTerminalClipboardWrite();
      _clearSelectionDrag();
      return true;
    }
    if (cancelOnFailure) _mouseDrag.cancel();
    return false;
  }

  PointerDownEvent? _takePendingTouchMouseDrag({int? pointer}) {
    final pending = _pendingTouchMouseDown;
    if (pending == null || pointer != null && pointer != pending.pointer) {
      return null;
    }
    _pendingTouchMouseTimer?.cancel();
    _pendingTouchMouseTimer = null;
    _pendingTouchMouseDown = null;
    return pending;
  }

  void _cancelPendingTouchMouseDrag({
    int? pointer,
    bool deferCancel = false,
  }) {
    if (_takePendingTouchMouseDrag(pointer: pointer) == null) return;
    if (deferCancel) {
      scheduleMicrotask(_mouseDrag.cancel);
    } else {
      _mouseDrag.cancel();
    }
  }

  bool _handlePendingTouchMove(PointerMoveEvent event) {
    final pending = _pendingTouchMouseDown;
    if (pending == null || pending.pointer != event.pointer) return false;
    if ((event.position - pending.position).distance > kTouchSlop) {
      _cancelPendingTouchMouseDrag(
        pointer: event.pointer,
        deferCancel: true,
      );
    }
    return true;
  }

  bool _consumeXtermLeftButtonSuppression(TerminalMouseButtonState state) {
    final suppress = _suppressXtermLeftButton;
    if (state == TerminalMouseButtonState.up) {
      _suppressXtermLeftButton = false;
    }
    return suppress;
  }
}
