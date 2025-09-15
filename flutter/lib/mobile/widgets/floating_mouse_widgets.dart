// This floating mouse widgets are used to simulate a physical mouse
// when "mobile" -> "desktop" in mouse mode.
// This file does not contain a whole mouse widgets, it only contains
// parts that help to control, such as wheel scroll and wheel button.

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/mobile/widgets/floating_mouse.dart';
import 'package:flutter_hbb/models/input_model.dart';
import 'package:flutter_hbb/models/model.dart';

const double _kDragHandleSize = 38;
const double _kSpaceBetweenHandleAndWheel = 2;
const double _mouseWidth = 50;
const double _mouseHeight = 162;
final Color _kDefaultColor = Colors.grey.withOpacity(0.7);
final Color _kTapDownColor = Colors.blue.withOpacity(0.7);

class FloatingMouseWidgets extends StatefulWidget {
  final FFI ffi;
  const FloatingMouseWidgets({
    super.key,
    required this.ffi,
  });

  @override
  State<FloatingMouseWidgets> createState() => _FloatingMouseWidgetsState();
}

class _FloatingMouseWidgetsState extends State<FloatingMouseWidgets> {
  Offset _position = Offset.zero;
  Rect? _lastBlockedRect;

  InputModel get _inputModel => widget.ffi.inputModel;
  CursorModel get _cursorModel => widget.ffi.cursorModel;

  @override
  void initState() {
    super.initState();
    _cursorModel.blockEvents = false;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      final size = MediaQuery.of(context).size;
      setState(() {
        _position = Offset(
          size.width -
              _mouseWidth -
              _kDragHandleSize -
              _kSpaceBetweenHandleAndWheel,
          (size.height - _mouseHeight) / 2,
        );
        _updateBlockedRect();
      });
    });
  }

  @override
  void dispose() {
    _cursorModel.blockEvents = false;
    if (_lastBlockedRect != null) {
      _cursorModel.removeBlockedRect(_lastBlockedRect!);
    }
    super.dispose();
  }

  void _onMoveUpdateDelta(Offset delta) {
    final context = this.context;
    final size = MediaQuery.of(context).size;
    Offset newPosition = _position + delta;
    double minX = 0;
    double minY = 0;
    double maxX = size.width -
        _mouseWidth -
        _kDragHandleSize -
        _kSpaceBetweenHandleAndWheel;
    double maxY = size.height - _mouseHeight;
    newPosition = Offset(
      newPosition.dx.clamp(minX, maxX),
      newPosition.dy.clamp(minY, maxY),
    );
    setState(() {
      final isPositionChanged = !(isDoubleEqual(newPosition.dx, _position.dx) &&
          isDoubleEqual(newPosition.dy, _position.dy));
      _position = newPosition;
      if (isPositionChanged) {
        _updateBlockedRect();
      }
    });
  }

  void _onBodyPointerMoveUpdate(PointerMoveEvent event) =>
      _onMoveUpdateDelta(event.delta);

  void _updateBlockedRect() {
    if (_lastBlockedRect != null) {
      _cursorModel.removeBlockedRect(_lastBlockedRect!);
    }
    final newRect =
        Rect.fromLTWH(_position.dx, _position.dy, _mouseWidth, _mouseHeight);
    _cursorModel.addBlockedRect(newRect);
    _lastBlockedRect = newRect;
  }

  @override
  Widget build(BuildContext context) {
    return Positioned(
      left: _position.dx,
      top: _position.dy,
      child: Row(
        children: [
          FloatingWheel(
            inputModel: _inputModel,
          ),
          SizedBox(width: _kSpaceBetweenHandleAndWheel),
          Listener(
            onPointerMove: _onBodyPointerMoveUpdate,
            onPointerDown: (event) => _cursorModel.blockEvents = true,
            onPointerUp: (event) => _cursorModel.blockEvents = false,
            onPointerCancel: (event) => _cursorModel.blockEvents = false,
            child: Container(
              width: _kDragHandleSize,
              child: Container(
                width: _kDragHandleSize,
                height: _kDragHandleSize,
                child: Icon(Icons.open_with, color: Colors.grey, size: 22),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class FloatingWheel extends StatefulWidget {
  final InputModel inputModel;
  const FloatingWheel({super.key, required this.inputModel});

  @override
  State<FloatingWheel> createState() => _FloatingWheelState();
}

class _FloatingWheelState extends State<FloatingWheel> {
  bool _isUpDown = false;
  bool _isMidDown = false;
  bool _isDownDown = false;

  Timer? _scrollTimer;

  @override
  void initState() {
    super.initState();
  }

  @override
  void dispose() {
    _scrollTimer?.cancel();
    super.dispose();
  }

  Widget _buildUpDownButton(
      void Function(PointerDownEvent) onPointerDown,
      void Function(PointerUpEvent) onPointerUp,
      void Function(PointerCancelEvent) onPointerCancel,
      bool Function() flagGetter,
      BorderRadiusGeometry borderRadius,
      IconData iconData) {
    return Listener(
      onPointerDown: onPointerDown,
      onPointerUp: onPointerUp,
      onPointerCancel: onPointerCancel,
      child: Container(
        width: _mouseWidth,
        height: 55,
        alignment: Alignment.center,
        decoration: BoxDecoration(
          border: Border.all(
              color: flagGetter() ? _kTapDownColor : _kDefaultColor, width: 2),
          borderRadius: borderRadius,
        ),
        child: Icon(iconData, color: Colors.grey, size: 32),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Container(
      width: _mouseWidth,
      height: _mouseHeight,
      child: Column(
        children: [
          _buildUpDownButton(
            (event) {
              setState(() {
                _isUpDown = true;
              });
              _startScrollTimer(1);
            },
            (event) {
              setState(() {
                _isUpDown = false;
              });
              _stopScrollTimer();
            },
            (event) {
              setState(() {
                _isUpDown = false;
              });
              _stopScrollTimer();
            },
            () => _isUpDown,
            BorderRadius.vertical(top: Radius.circular(_mouseWidth * 0.5)),
            Icons.keyboard_arrow_up,
          ),
          Listener(
            onPointerDown: (event) {
              setState(() {
                _isMidDown = true;
              });
              widget.inputModel.tapDown(MouseButtons.wheel);
            },
            onPointerUp: (event) {
              setState(() {
                _isMidDown = false;
              });
              widget.inputModel.tapUp(MouseButtons.wheel);
            },
            onPointerCancel: (event) {
              setState(() {
                _isMidDown = false;
              });
              widget.inputModel.tapUp(MouseButtons.wheel);
            },
            child: Container(
              width: _mouseWidth,
              height: 52,
              decoration: BoxDecoration(
                border: Border.symmetric(
                    vertical: BorderSide(
                        color: _isMidDown ? _kTapDownColor : _kDefaultColor,
                        width: 2)),
              ),
              child: Center(
                child: Container(
                  width: _mouseWidth - 10,
                  height: _mouseWidth - 10,
                  decoration: BoxDecoration(
                    border: Border.all(
                        color: _isMidDown ? _kTapDownColor : _kDefaultColor,
                        width: 2),
                    shape: BoxShape.circle,
                  ),
                  child: CustomPaint(
                    painter: FourArrowsPainter(2.5,
                        color: _isMidDown ? _kTapDownColor : Colors.grey),
                    size: Size(_mouseWidth, 52),
                  ),
                ),
              ),
            ),
          ),
          _buildUpDownButton(
            (event) {
              setState(() {
                _isDownDown = true;
              });
              _startScrollTimer(-1);
            },
            (event) {
              setState(() {
                _isDownDown = false;
              });
              _stopScrollTimer();
            },
            (event) {
              setState(() {
                _isDownDown = false;
              });
              _stopScrollTimer();
            },
            () => _isDownDown,
            BorderRadius.vertical(bottom: Radius.circular(_mouseWidth * 0.5)),
            Icons.keyboard_arrow_down,
          ),
        ],
      ),
    );
  }

  void _startScrollTimer(int direction) {
    _scrollTimer?.cancel();
    widget.inputModel.scroll(direction);
    _scrollTimer = Timer.periodic(Duration(milliseconds: 100), (timer) {
      widget.inputModel.scroll(direction);
    });
  }

  void _stopScrollTimer() {
    _scrollTimer?.cancel();
    _scrollTimer = null;
  }
}
