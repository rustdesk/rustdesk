// This floating mouse widgets are used to simulate a physical mouse
// when "mobile" -> "desktop" in mouse mode.
// This file does not contain a whole mouse widgets, it only contains
// parts that help to control, such as wheel scroll and wheel button.

import 'dart:async';
import 'dart:convert';
import 'dart:math';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/widgets/remote_input.dart';
import 'package:flutter_hbb/models/input_model.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/platform_model.dart';

// Used for the wheel button and wheel scroll widgets
const double _kSpaceToHorizontalEdge = 25;
const double _wheelWidth = 50;
const double _wheelHeight = 162;
// Used for the left/right button widgets
const double _kSpaceToVerticalEdge = 15;
const double _kSpaceBetweenLeftRightButtons = 40;
const double _kLeftRightButtonWidth = 55;
const double _kLeftRightButtonHeight = 40;
const double _kBoarderWidth = 1;
final Color _kDefaultBorderColor = Colors.white.withOpacity(0.7);
final Color _kDefaultColor = Colors.black.withOpacity(0.4);
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
  InputModel get _inputModel => widget.ffi.inputModel;
  CursorModel get _cursorModel => widget.ffi.cursorModel;

  @override
  void initState() {
    super.initState();
    _cursorModel.blockEvents = false;
    isSpecialHoldDragActive = false;
  }

  @override
  void dispose() {
    super.dispose();
    _cursorModel.blockEvents = false;
    isSpecialHoldDragActive = false;
  }

  @override
  Widget build(BuildContext context) {
    return Stack(
      children: [
        FloatingWheel(
          inputModel: _inputModel,
          cursorModel: _cursorModel,
        ),
        FloatingLeftRightButton(
          isLeft: true,
          inputModel: _inputModel,
          cursorModel: _cursorModel,
        ),
        FloatingLeftRightButton(
          isLeft: false,
          inputModel: _inputModel,
          cursorModel: _cursorModel,
        ),
      ],
    );
  }
}

class FloatingWheel extends StatefulWidget {
  final InputModel inputModel;
  final CursorModel cursorModel;
  const FloatingWheel(
      {super.key, required this.inputModel, required this.cursorModel});

  @override
  State<FloatingWheel> createState() => _FloatingWheelState();
}

class _FloatingWheelState extends State<FloatingWheel> {
  Offset _position = Offset.zero;
  Rect? _lastBlockedRect;

  bool _isUpDown = false;
  bool _isMidDown = false;
  bool _isDownDown = false;

  Orientation? _previousOrientation;

  Timer? _scrollTimer;

  InputModel get _inputModel => widget.inputModel;
  CursorModel get _cursorModel => widget.cursorModel;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _resetPosition();
    });
  }

  void _resetPosition() {
    final size = MediaQuery.of(context).size;
    setState(() {
      _position = Offset(
        size.width - _wheelWidth - _kSpaceToHorizontalEdge,
        (size.height - _wheelHeight) / 2,
      );
    });
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) _updateBlockedRect();
    });
  }

  void _updateBlockedRect() {
    if (_lastBlockedRect != null) {
      _cursorModel.removeBlockedRect(_lastBlockedRect!);
    }
    final newRect =
        Rect.fromLTWH(_position.dx, _position.dy, _wheelWidth, _wheelHeight);
    _cursorModel.addBlockedRect(newRect);
    _lastBlockedRect = newRect;
  }

  @override
  void dispose() {
    _scrollTimer?.cancel();
    super.dispose();
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final currentOrientation = MediaQuery.of(context).orientation;
    if (_previousOrientation != null &&
        _previousOrientation != currentOrientation) {
      _resetPosition();
    }
    _previousOrientation = currentOrientation;
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
        width: _wheelWidth,
        height: 55,
        alignment: Alignment.center,
        decoration: BoxDecoration(
          color: _kDefaultColor,
          border: Border.all(
              color: flagGetter() ? _kTapDownColor : _kDefaultBorderColor,
              width: 1),
          borderRadius: borderRadius,
        ),
        child: Icon(iconData, color: _kDefaultBorderColor, size: 32),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Positioned(
      left: _position.dx,
      top: _position.dy,
      child: _buildWidget(context),
    );
  }

  Widget _buildWidget(BuildContext context) {
    return Container(
      width: _wheelWidth,
      height: _wheelHeight,
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
            BorderRadius.vertical(top: Radius.circular(_wheelWidth * 0.5)),
            Icons.keyboard_arrow_up,
          ),
          Listener(
            onPointerDown: (event) {
              setState(() {
                _isMidDown = true;
              });
              _inputModel.tapDown(MouseButtons.wheel);
            },
            onPointerUp: (event) {
              setState(() {
                _isMidDown = false;
              });
              _inputModel.tapUp(MouseButtons.wheel);
            },
            onPointerCancel: (event) {
              setState(() {
                _isMidDown = false;
              });
              _inputModel.tapUp(MouseButtons.wheel);
            },
            child: Container(
              width: _wheelWidth,
              height: 52,
              decoration: BoxDecoration(
                color: _kDefaultColor,
                border: Border.symmetric(
                    vertical: BorderSide(
                        color:
                            _isMidDown ? _kTapDownColor : _kDefaultBorderColor,
                        width: _kBoarderWidth)),
              ),
              child: Center(
                child: Container(
                  width: _wheelWidth - 10,
                  height: _wheelWidth - 10,
                  child: Center(
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Container(
                          width: 18,
                          height: 2,
                          color: _kDefaultBorderColor,
                        ),
                        SizedBox(height: 6),
                        Container(
                          width: 24,
                          height: 2,
                          color: _kDefaultBorderColor,
                        ),
                        SizedBox(height: 6),
                        Container(
                          width: 18,
                          height: 2,
                          color: _kDefaultBorderColor,
                        ),
                      ],
                    ),
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
            BorderRadius.vertical(bottom: Radius.circular(_wheelWidth * 0.5)),
            Icons.keyboard_arrow_down,
          ),
        ],
      ),
    );
  }

  void _startScrollTimer(int direction) {
    _scrollTimer?.cancel();
    _inputModel.scroll(direction);
    _scrollTimer = Timer.periodic(Duration(milliseconds: 100), (timer) {
      _inputModel.scroll(direction);
    });
  }

  void _stopScrollTimer() {
    _scrollTimer?.cancel();
    _scrollTimer = null;
  }
}

class FloatingLeftRightButton extends StatefulWidget {
  final bool isLeft;
  final InputModel inputModel;
  final CursorModel cursorModel;
  const FloatingLeftRightButton(
      {super.key,
      required this.isLeft,
      required this.inputModel,
      required this.cursorModel});

  @override
  State<FloatingLeftRightButton> createState() =>
      _FloatingLeftRightButtonState();
}

class _FloatingLeftRightButtonState extends State<FloatingLeftRightButton> {
  Offset _position = Offset.zero;
  bool _isDown = false;
  Rect? _lastBlockedRect;

  Orientation? _previousOrientation;
  Offset _preSavedPos = Offset.zero;

  bool get _isLeft => widget.isLeft;
  InputModel get _inputModel => widget.inputModel;
  CursorModel get _cursorModel => widget.cursorModel;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      final currentOrientation = MediaQuery.of(context).orientation;
      _previousOrientation = currentOrientation;
      _resetPosition(currentOrientation);
    });
  }

  @override
  void dispose() {
    if (_lastBlockedRect != null) {
      _cursorModel.removeBlockedRect(_lastBlockedRect!);
    }
    _trySavePosition();
    super.dispose();
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final currentOrientation = MediaQuery.of(context).orientation;
    if (_previousOrientation == null ||
        _previousOrientation != currentOrientation) {
      _resetPosition(currentOrientation);
    }
    _previousOrientation = currentOrientation;
  }

  double _getOffsetX(double w) {
    if (_isLeft) {
      return (w - _kLeftRightButtonWidth * 2 - _kSpaceBetweenLeftRightButtons) *
          0.5;
    } else {
      return (w + _kSpaceBetweenLeftRightButtons) * 0.5;
    }
  }

  String _getPositionKey(Orientation ori) {
    final strLeftRight = _isLeft ? 'l' : 'r';
    final strOri = ori == Orientation.landscape ? 'l' : 'p';
    return '$strLeftRight$strOri-mouse-btn-pos';
  }

  static Offset? _loadPositionFromString(String s) {
    if (s.isEmpty) {
      return null;
    }
    try {
      final m = jsonDecode(s);
      return Offset(m['x'], m['y']);
    } catch (e) {
      debugPrintStack(label: 'Failed to load position "$s" ${e.toString()}');
      return null;
    }
  }

  void _trySavePosition() {
    if (_previousOrientation == null) return;
    if ((Offset(_position.dx - _preSavedPos.dx, _position.dy - _preSavedPos.dy))
            .distanceSquared <
        0.1) return;
    final pos = jsonEncode({
      'x': _position.dx,
      'y': _position.dy,
    });
    bind.setLocalFlutterOption(
        k: _getPositionKey(_previousOrientation!), v: pos);
    _preSavedPos = _position;
  }

  void _restorePosition(Orientation ori) {
    final ps = bind.getLocalFlutterOption(k: _getPositionKey(ori));
    final pos = _loadPositionFromString(ps);
    if (pos == null) {
      final size = MediaQuery.of(context).size;
      _position = Offset(_getOffsetX(size.width),
          size.height - _kSpaceToVerticalEdge - _kLeftRightButtonHeight);
    } else {
      _position = pos;
      _preSavedPos = pos;
    }
  }

  void _resetPosition(Orientation ori) {
    setState(() {
      _restorePosition(ori);
    });
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) _updateBlockedRect();
    });
  }

  void _updateBlockedRect() {
    if (_lastBlockedRect != null) {
      _cursorModel.removeBlockedRect(_lastBlockedRect!);
    }
    final newRect = Rect.fromLTWH(_position.dx, _position.dy,
        _kLeftRightButtonWidth, _kLeftRightButtonHeight);
    _cursorModel.addBlockedRect(newRect);
    _lastBlockedRect = newRect;
  }

  void _onMoveUpdateDelta(Offset delta) {
    final context = this.context;
    final size = MediaQuery.of(context).size;
    Offset newPosition = _position + delta;
    double minX = _kSpaceToHorizontalEdge;
    double minY = _kSpaceToVerticalEdge;
    double maxX = size.width - _kLeftRightButtonWidth - _kSpaceToHorizontalEdge;
    double maxY = size.height - _kLeftRightButtonHeight - _kSpaceToVerticalEdge;
    newPosition = Offset(
      newPosition.dx.clamp(minX, maxX),
      newPosition.dy.clamp(minY, maxY),
    );
    final isPositionChanged = !(isDoubleEqual(newPosition.dx, _position.dx) &&
        isDoubleEqual(newPosition.dy, _position.dy));
    setState(() {
      _position = newPosition;
    });
    if (isPositionChanged) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) _updateBlockedRect();
      });
    }
  }

  void _onBodyPointerMoveUpdate(PointerMoveEvent event) =>
      _onMoveUpdateDelta(event.delta);

  Widget _buildButtonIcon() {
    final double w = _kLeftRightButtonWidth * 0.45;
    final double h = _kLeftRightButtonHeight * 0.75;
    final double borderRadius = w * 0.5;
    final double quarterCircleRadius = borderRadius * 0.9;
    return Stack(
      children: [
        Container(
          width: w,
          height: h,
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(_kLeftRightButtonWidth * 0.225),
            color: Colors.white,
          ),
        ),
        Positioned(
          left: _isLeft ? quarterCircleRadius * 0.25 : null,
          right: _isLeft ? null : quarterCircleRadius * 0.25,
          top: quarterCircleRadius * 0.25,
          child: CustomPaint(
            size: Size(quarterCircleRadius * 2, quarterCircleRadius * 2),
            painter: _QuarterCirclePainter(
              color: _kDefaultColor,
              isLeft: _isLeft,
              radius: quarterCircleRadius,
            ),
          ),
        ),
      ],
    );
  }

  @override
  Widget build(BuildContext context) {
    return Positioned(
      left: _position.dx,
      top: _position.dy,
      child: Listener(
        onPointerMove: _onBodyPointerMoveUpdate,
        onPointerDown: (event) async {
          setState(() {
            _isDown = true;
          });
          isSpecialHoldDragActive = true;
          // Sync cursor position to avoid the jumpy behavior.
          await _cursorModel.syncCursorPosition();
          await _inputModel
              .tapDown(_isLeft ? MouseButtons.left : MouseButtons.right);
        },
        onPointerUp: (event) {
          setState(() {
            _isDown = false;
          });
          isSpecialHoldDragActive = false;
          _inputModel.tapUp(_isLeft ? MouseButtons.left : MouseButtons.right);
          _trySavePosition();
        },
        onPointerCancel: (event) {
          setState(() {
            _isDown = false;
          });
          isSpecialHoldDragActive = false;
          _inputModel.tapUp(_isLeft ? MouseButtons.left : MouseButtons.right);
          _trySavePosition();
        },
        child: Container(
          width: _kLeftRightButtonWidth,
          height: _kLeftRightButtonHeight,
          alignment: Alignment.center,
          decoration: BoxDecoration(
            color: _kDefaultColor,
            border: Border.all(
                color: _isDown ? _kTapDownColor : _kDefaultBorderColor,
                width: _kBoarderWidth),
            borderRadius: _isLeft
                ? BorderRadius.horizontal(
                    left: Radius.circular(_kLeftRightButtonHeight * 0.5))
                : BorderRadius.horizontal(
                    right: Radius.circular(_kLeftRightButtonHeight * 0.5)),
          ),
          child: _buildButtonIcon(),
        ),
      ),
    );
  }
}

class _QuarterCirclePainter extends CustomPainter {
  final Color color;
  final bool isLeft;
  final double radius;
  _QuarterCirclePainter(
      {required this.color, required this.isLeft, required this.radius});

  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()
      ..color = color
      ..style = PaintingStyle.fill;
    final rect = Rect.fromLTWH(0, 0, radius * 2, radius * 2);
    if (isLeft) {
      canvas.drawArc(rect, -pi, pi / 2, true, paint);
    } else {
      canvas.drawArc(rect, -pi / 2, pi / 2, true, paint);
    }
  }

  @override
  bool shouldRepaint(CustomPainter oldDelegate) => false;
}
