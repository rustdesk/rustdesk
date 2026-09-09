import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';
import 'package:xterm/xterm.dart';

import 'platform_model.dart';
import 'rustdesk_terminal.dart';
import 'terminal_copy_shortcut.dart';
import 'terminal_mouse_drag_reporter.dart';

part 'terminal_mouse_handler_input.dart';
part 'terminal_web_clipboard_gesture.dart';

class TerminalMouseInteraction extends StatefulWidget {
  const TerminalMouseInteraction(
    this.terminal, {
    super.key,
    required this.controller,
    this.focusNode,
    this.autofocus = false,
    this.textStyle = const TerminalStyle(),
    this.deleteDetection = false,
    this.reportTouchInput = false,
    this.shortcuts,
    this.onKeyEvent,
    this.backgroundOpacity = 1,
    this.padding,
    this.onSecondaryTapDown,
  });

  final Terminal terminal;
  final TerminalController controller;
  final FocusNode? focusNode;
  final bool autofocus;
  final TerminalStyle textStyle;
  final bool deleteDetection;
  final bool reportTouchInput;
  final Map<ShortcutActivator, Intent>? shortcuts;
  final FocusOnKeyEventCallback? onKeyEvent;
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
  Timer? _pendingTouchMouseTimer;
  PointerDownEvent? _pendingTouchMouseDown;
  var _selectionHasScrolled = false;
  var _scrollDirection = _noScroll;
  // xterm can finish its tap callbacks after the raw drag was reported.
  var _suppressXtermLeftButton = false;
  var _terminalClipboardGesturePrepared = false;
  TerminalViewState? get _terminalView => _terminalViewKey.currentState;

  @override
  void initState() {
    super.initState();
    _mouseHandler = WheelButtonFixMouseHandler(
      positionProvider: _cellAtPointer,
      suppressLeftButton: kIsWeb ? _consumeXtermLeftButtonSuppression : null,
    );
    _installMouseHandler(widget.terminal);
  }

  @override
  void didUpdateWidget(TerminalMouseInteraction oldWidget) {
    super.didUpdateWidget(oldWidget);
    final terminalChanged = !identical(oldWidget.terminal, widget.terminal);
    final controllerChanged =
        !identical(oldWidget.controller, widget.controller);
    final touchInputChanged =
        oldWidget.reportTouchInput != widget.reportTouchInput;
    if (!terminalChanged && !controllerChanged && !touchInputChanged) return;
    _cancelPendingTouchMouseDrag();
    if (!terminalChanged && !controllerChanged) return;
    if (controllerChanged && !terminalChanged) {
      _mouseDrag.updateController(widget.controller);
    } else {
      _discardPendingTerminalClipboardWrites();
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

  void _handlePointerMove(PointerMoveEvent event) {
    _updatePointerPosition(event);
    if (_handlePendingTouchMove(event)) return;
    if (_mouseDrag.handleMove(
      event,
      widget.terminal,
      _terminalView,
      beforeRelease: _finishTerminalClipboardWrite,
      onCancel: _cancelTerminalClipboardWrite,
    )) {
      return;
    }
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
    final pendingTouch = _pendingTouchMouseDown;
    if (pendingTouch != null && pendingTouch.pointer == event.pointer) {
      final movedBeyondSlop =
          (event.position - pendingTouch.position).distance > kTouchSlop;
      if (event is PointerUpEvent && !movedBeyondSlop) {
        _activatePendingTouchMouseDrag(cancelOnFailure: false);
      } else {
        _takePendingTouchMouseDrag(pointer: event.pointer);
      }
    }
    final handledByMouseDrag = _mouseDrag.handleEnd(
      event,
      widget.terminal,
      _terminalView,
      beforeRelease: event is PointerUpEvent
          ? _finishTerminalClipboardWrite
          : (_) => _cancelTerminalClipboardWrite(),
      onCancel: _cancelTerminalClipboardWrite,
    );
    if (!handledByMouseDrag && event.pointer != _selectionPointerId) {
      return;
    }
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
    _discardPendingTerminalClipboardWrites();
    _cancelPendingTouchMouseDrag();
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
        autofocus: widget.autofocus,
        textStyle: widget.textStyle,
        deleteDetection: widget.deleteDetection,
        backgroundOpacity: widget.backgroundOpacity,
        padding: widget.padding,
        shortcuts: widget.shortcuts ?? platformTerminalShortcuts(),
        onKeyEvent: widget.onKeyEvent ??
            terminalCopyHandler(widget.terminal, widget.controller),
        onSecondaryTapDown: widget.onSecondaryTapDown,
      ),
    );
  }
}
