import 'dart:async';

import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
import 'package:xterm/xterm.dart';

const _cellIndexOffset = 1;
const _legacyCodeOffset = 32;
const _leftButtonCode = 0;
const _motionButtonCode = 32;
const _releaseButtonCode = 3;
const _shiftModifierCode = 4;
const _metaModifierCode = 8;
const _controlModifierCode = 16;
const _modifierCodeMask =
    _shiftModifierCode | _metaModifierCode | _controlModifierCode;
const _normalCoordinateLimit = 223;
const _utfCoordinateLimit = 2015;

String encodeTerminalMouseReport(
  MouseReportMode mode,
  int button,
  CellOffset position, {
  bool release = false,
}) {
  final x = position.x + _cellIndexOffset;
  final y = position.y + _cellIndexOffset;
  final reportedButton =
      release ? _releaseButtonCode | (button & _modifierCodeMask) : button;
  switch (mode) {
    case MouseReportMode.normal:
    case MouseReportMode.utf:
      final limit = mode == MouseReportMode.normal
          ? _normalCoordinateLimit
          : _utfCoordinateLimit;
      final encodedButton =
          String.fromCharCode(_legacyCodeOffset + reportedButton);
      return '\x1b[M$encodedButton${_legacyCoordinate(x, limit)}'
          '${_legacyCoordinate(y, limit)}';
    case MouseReportMode.sgr:
      final suffix = release ? 'm' : 'M';
      return '\x1b[<$button;$x;$y$suffix';
    case MouseReportMode.urxvt:
      return '\x1b[${_legacyCodeOffset + reportedButton};$x;${y}M';
  }
}

String _legacyCoordinate(int value, int limit) =>
    value > limit ? '\x00' : String.fromCharCode(_legacyCodeOffset + value);

int _activeModifierCode() {
  final keyboard = HardwareKeyboard.instance;
  return (keyboard.isShiftPressed ? _shiftModifierCode : 0) |
      (keyboard.isAltPressed ? _metaModifierCode : 0) |
      (keyboard.isControlPressed ? _controlModifierCode : 0);
}

class TerminalMouseDragReporter {
  int? _pointerId;
  TerminalController? _controller;
  late CellOffset _lastReportedPosition;
  var _ownsControllerSuspension = false;
  var _releasePending = false;
  var _reporting = false;
  var _dragged = false;

  bool handleDown(
    PointerDownEvent event,
    Terminal terminal,
    TerminalViewState? terminalView, {
    bool reportTouchInput = false,
    bool deferReport = false,
  }) {
    if (!_isPrimaryPointer(event, reportTouchInput) ||
        !_reportsDrag(terminal.mouseMode)) {
      return false;
    }
    if (terminalView == null || terminalView.widget.readOnly) return false;
    final controller = terminalView.widget.controller;
    if (controller == null ||
        controller.suspendedPointerInputs ||
        !controller.pointerInput.inputs.contains(PointerInput.tap)) {
      return false;
    }

    cancel();
    _pointerId = event.pointer;
    _controller = controller;
    _ownsControllerSuspension = true;
    _releasePending = !deferReport;
    _reporting = !deferReport;
    _dragged = false;
    controller.setSuspendPointerInput(true);
    _clearSelection(controller);
    final position = _cellAt(event, terminalView);
    _lastReportedPosition = position;
    if (!deferReport) {
      terminal.textInput(
        _report(terminal.mouseReportMode, position),
      );
    }
    return true;
  }

  bool activateDeferredDown(Terminal terminal) {
    if (_pointerId == null ||
        _controller == null ||
        _releasePending ||
        !_reportsDrag(terminal.mouseMode)) {
      return false;
    }
    _releasePending = true;
    _reporting = true;
    _clearSelection(_controller);
    terminal.textInput(
      _report(terminal.mouseReportMode, _lastReportedPosition),
    );
    return true;
  }

  bool handleMove(
    PointerMoveEvent event,
    Terminal terminal,
    TerminalViewState? terminalView, {
    void Function(bool dragged)? beforeRelease,
    void Function()? onCancel,
  }) {
    if (event.pointer != _pointerId) return false;
    if (terminalView == null) {
      onCancel?.call();
      cancel();
      return true;
    }
    final reportsDrag = _reportsDrag(terminal.mouseMode);
    if (!_hasPrimaryButton(event)) {
      if (_releasePending && reportsDrag) {
        _finishRelease(
          event,
          terminal,
          terminalView,
          beforeRelease: beforeRelease,
        );
      } else {
        onCancel?.call();
      }
      cancel();
      return true;
    }
    if (!_reporting || !reportsDrag) {
      if (!reportsDrag && _releasePending) {
        _releasePending = false;
        onCancel?.call();
      }
      _reporting = false;
      // Keep ownership until the matching end event to suppress local selection.
      final controller = _controller;
      scheduleMicrotask(() => _clearSelection(controller));
      return true;
    }

    final position = _cellAt(event, terminalView);
    _recordPosition(position);
    terminal.textInput(
      _report(terminal.mouseReportMode, position, motion: true),
    );
    final controller = _controller;
    scheduleMicrotask(() => _clearSelection(controller));
    return true;
  }

  bool handleEnd(
    PointerEvent event,
    Terminal terminal,
    TerminalViewState? terminalView, {
    void Function(bool dragged)? beforeRelease,
    void Function()? onCancel,
  }) {
    if (event.pointer != _pointerId) return false;
    if (terminalView != null &&
        _releasePending &&
        _reportsDrag(terminal.mouseMode)) {
      _finishRelease(
        event,
        terminal,
        terminalView,
        beforeRelease: beforeRelease,
      );
    } else {
      onCancel?.call();
    }
    _clearSelection(_controller);
    final controller = _controller;
    _pointerId = null;
    // Keep xterm's tap recognizer suspended for this pointer event.
    scheduleMicrotask(() {
      if (_pointerId == null && identical(_controller, controller)) {
        _clearSelection(controller);
        cancel();
      }
    });
    return true;
  }

  void cancel() {
    final controller = _controller;
    if (_ownsControllerSuspension) {
      controller?.setSuspendPointerInput(false);
    }
    _pointerId = null;
    _controller = null;
    _ownsControllerSuspension = false;
    _releasePending = false;
    _reporting = false;
    _dragged = false;
  }

  void updateController(TerminalController controller) {
    final oldController = _controller;
    if (_pointerId == null || oldController == null) {
      cancel();
      return;
    }
    if (identical(oldController, controller)) return;
    if (_ownsControllerSuspension) {
      oldController.setSuspendPointerInput(false);
    }
    final acceptsPointerInput = !controller.suspendedPointerInputs &&
        controller.pointerInput.inputs.contains(PointerInput.tap);
    _controller = controller;
    _ownsControllerSuspension = acceptsPointerInput;
    _reporting = _reporting && acceptsPointerInput;
    if (_ownsControllerSuspension) controller.setSuspendPointerInput(true);
    _clearSelection(controller);
  }

  void _reportRelease(Terminal terminal, CellOffset position) {
    terminal.textInput(
      _report(
        terminal.mouseReportMode,
        position,
        release: true,
      ),
    );
  }

  void _finishRelease(
    PointerEvent event,
    Terminal terminal,
    TerminalViewState terminalView, {
    void Function(bool dragged)? beforeRelease,
  }) {
    final position =
        _reporting ? _cellAt(event, terminalView) : _lastReportedPosition;
    if (_reporting) _recordPosition(position);
    beforeRelease?.call(_dragged);
    _reportRelease(terminal, position);
  }

  void _recordPosition(CellOffset position) {
    _dragged = _dragged || position != _lastReportedPosition;
    _lastReportedPosition = position;
  }

  CellOffset _cellAt(PointerEvent event, TerminalViewState terminalView) {
    final renderTerminal = terminalView.renderTerminal;
    return renderTerminal.getCellOffset(
      renderTerminal.globalToLocal(event.position),
    );
  }

  bool _isPrimaryPointer(PointerEvent event, bool reportTouchInput) =>
      (event.kind == PointerDeviceKind.mouse ||
          reportTouchInput && event.kind == PointerDeviceKind.touch) &&
      _hasPrimaryButton(event);

  bool _hasPrimaryButton(PointerEvent event) =>
      (event.buttons & kPrimaryButton) == kPrimaryButton;

  bool _reportsDrag(MouseMode mode) =>
      mode == MouseMode.upDownScrollDrag || mode == MouseMode.upDownScrollMove;

  void _clearSelection(TerminalController? controller) {
    if (controller == null || controller.selection == null) return;
    controller.clearSelection();
  }

  String _report(
    MouseReportMode mode,
    CellOffset position, {
    bool release = false,
    bool motion = false,
  }) {
    final baseButton = motion ? _motionButtonCode : _leftButtonCode;
    final button = baseButton | _activeModifierCode();
    return encodeTerminalMouseReport(mode, button, position, release: release);
  }
}
