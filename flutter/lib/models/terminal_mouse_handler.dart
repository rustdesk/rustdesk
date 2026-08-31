import 'dart:async';

import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';
import 'package:xterm/xterm.dart';

import 'terminal_copy_shortcut.dart';
import 'terminal_mouse_drag_reporter.dart';

/// xterm 4.0.0 encodes wheel buttons as 68..71; the extra bit reads as a Shift
/// modifier, so strict full-screen apps ignore the report and never scroll.
/// Upstream fix: TerminalStudio/xterm.dart#238.
class WheelButtonFixMouseHandler implements TerminalMouseHandler {
  const WheelButtonFixMouseHandler({this.positionProvider});

  final CellOffset? Function()? positionProvider;

  @override
  String? call(TerminalMouseEvent event) {
    if (!event.button.isWheel) {
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

class TerminalMouseInteraction extends StatefulWidget {
  const TerminalMouseInteraction(
    this.terminal, {
    super.key,
    required this.controller,
    this.focusNode,
    this.backgroundOpacity = 1,
    this.padding,
    this.onSecondaryTapDown,
  });

  final Terminal terminal;
  final TerminalController controller;
  final FocusNode? focusNode;
  final double backgroundOpacity;
  final EdgeInsets? padding;
  final void Function(TapDownDetails, CellOffset)? onSecondaryTapDown;

  @override
  State<TerminalMouseInteraction> createState() =>
      _TerminalMouseInteractionState();
}

class _TerminalMouseInteractionState extends State<TerminalMouseInteraction> {
  static const _selectionScrollInterval = Duration(milliseconds: 50);
  static const _noScroll = 0;
  static const _scrollUp = -1;
  static const _scrollDown = 1;

  final _terminalViewKey = GlobalKey<TerminalViewState>();
  final _scrollController = ScrollController();
  final _mouseDrag = TerminalMouseDragReporter();
  late final WheelButtonFixMouseHandler _mouseHandler;
  TerminalMouseHandler? _previousMouseHandler;
  Offset? _pointerPosition;
  Offset? _selectionPointer;
  CellAnchor? _selectionBase;
  Buffer? _selectionBuffer;
  int? _selectionPointerId;
  Timer? _selectionScrollTimer;
  var _selectionHasScrolled = false;
  var _scrollDirection = _noScroll;
  TerminalViewState? get _terminalView => _terminalViewKey.currentState;

  @override
  void initState() {
    super.initState();
    _mouseHandler = WheelButtonFixMouseHandler(
      positionProvider: _cellAtPointer,
    );
    _installMouseHandler(widget.terminal);
  }

  @override
  void didUpdateWidget(TerminalMouseInteraction oldWidget) {
    super.didUpdateWidget(oldWidget);
    final terminalChanged = !identical(oldWidget.terminal, widget.terminal);
    final controllerChanged =
        !identical(oldWidget.controller, widget.controller);
    if (!terminalChanged && !controllerChanged) return;
    if (controllerChanged && !terminalChanged) {
      _mouseDrag.updateController(widget.controller);
    } else {
      _mouseDrag.cancel();
    }
    _clearSelectionDrag();
    if (!terminalChanged) return;
    _restoreMouseHandler(oldWidget.terminal);
    _installMouseHandler(widget.terminal);
  }

  void _installMouseHandler(Terminal terminal) {
    _previousMouseHandler = terminal.mouseHandler;
    terminal.mouseHandler = _mouseHandler;
  }

  void _restoreMouseHandler(Terminal terminal) {
    if (identical(terminal.mouseHandler, _mouseHandler)) {
      terminal.mouseHandler = _previousMouseHandler;
    }
  }

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
    if (_mouseDrag.handleDown(event, widget.terminal, _terminalView)) {
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

  void _handlePointerMove(PointerMoveEvent event) {
    _updatePointerPosition(event);
    if (_mouseDrag.handleMove(event, widget.terminal, _terminalView)) return;
    if (event.pointer != _selectionPointerId) return;
    if (event.kind != PointerDeviceKind.mouse ||
        (event.buttons & kPrimaryMouseButton) != kPrimaryMouseButton) {
      _clearSelectionDrag();
      return;
    }
    final terminalView = _terminalView;
    if (terminalView == null || _selectionBase == null) return;
    final renderTerminal = terminalView.renderTerminal;
    final localPosition = renderTerminal.globalToLocal(event.position);
    _selectionPointer = localPosition;
    _setScrollDirection(
      _directionFor(localPosition, renderTerminal.paintBounds),
    );
    if (_selectionHasScrolled) {
      scheduleMicrotask(() => _scrollSelection(scroll: false));
    }
  }

  int _directionFor(Offset position, Rect bounds) {
    if (position.dy < bounds.top) return _scrollUp;
    if (position.dy >= bounds.bottom) return _scrollDown;
    return _noScroll;
  }

  void _setScrollDirection(int direction) {
    if (_scrollDirection == direction) return;
    _stopAutoScroll();
    _scrollDirection = direction;
    if (direction == _noScroll) return;
    _scrollSelection();
    if (_scrollDirection != _noScroll) {
      _selectionScrollTimer = Timer.periodic(
        _selectionScrollInterval,
        (_) => _scrollSelection(),
      );
    }
  }

  void _scrollSelection({bool scroll = true}) {
    final terminalView = _terminalView;
    final selectionBase = _selectionBase;
    final selectionBuffer = _selectionBuffer;
    final selectionPointer = _selectionPointer;
    if (terminalView == null ||
        selectionBase == null ||
        selectionBuffer == null ||
        selectionPointer == null ||
        !_scrollController.hasClients) {
      return;
    }
    if (!identical(selectionBuffer, widget.terminal.buffer) ||
        !selectionBase.attached) {
      _clearSelectionDrag();
      return;
    }
    final renderTerminal = terminalView.renderTerminal;
    if (scroll) {
      final position = _scrollController.position;
      final target =
          (position.pixels + renderTerminal.lineHeight * _scrollDirection)
              .clamp(position.minScrollExtent, position.maxScrollExtent)
              .toDouble();
      if (target == position.pixels) {
        _stopAutoScroll();
      } else {
        position.jumpTo(target);
        _selectionHasScrolled = true;
      }
    }
    renderTerminal.selectCharacters(
      renderTerminal.getOffset(selectionBase.offset),
      selectionPointer,
    );
  }

  void _handlePointerEnd(PointerEvent event) {
    _updatePointerPosition(event);
    if (!_mouseDrag.handleEnd(event, widget.terminal, _terminalView) &&
        event.pointer != _selectionPointerId) return;
    if (_selectionHasScrolled) _scrollSelection(scroll: false);
    _clearSelectionDrag();
  }

  void _clearSelectionDrag() {
    _selectionPointerId = null;
    _selectionBase?.dispose();
    _selectionBase = null;
    _selectionBuffer = null;
    _selectionPointer = null;
    _selectionHasScrolled = false;
    _stopAutoScroll();
  }

  void _stopAutoScroll() {
    _selectionScrollTimer?.cancel();
    _selectionScrollTimer = null;
    _scrollDirection = _noScroll;
  }

  @override
  void dispose() {
    _mouseDrag.cancel();
    _clearSelectionDrag();
    _restoreMouseHandler(widget.terminal);
    _scrollController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Listener(
      onPointerDown: _handlePointerDown,
      onPointerMove: _handlePointerMove,
      onPointerUp: _handlePointerEnd,
      onPointerHover: _updatePointerPosition,
      onPointerCancel: _handlePointerEnd,
      onPointerSignal: _updatePointerPosition,
      onPointerPanZoomStart: _updatePointerPosition,
      onPointerPanZoomUpdate: _updatePointerPosition,
      onPointerPanZoomEnd: _updatePointerPosition,
      child: TerminalView(
        widget.terminal,
        key: _terminalViewKey,
        controller: widget.controller,
        scrollController: _scrollController,
        focusNode: widget.focusNode,
        backgroundOpacity: widget.backgroundOpacity,
        padding: widget.padding,
        shortcuts: platformTerminalShortcuts(),
        onKeyEvent: terminalCopyHandler(widget.terminal, widget.controller),
        onSecondaryTapDown: widget.onSecondaryTapDown,
      ),
    );
  }
}
