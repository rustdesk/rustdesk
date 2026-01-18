import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/remote_input_event_log.dart';
import 'package:flutter_hbb/common/widgets/overlay.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/input_model.dart';
import 'package:flutter_hbb/models/model.dart';

class RemoteWheelSlider extends StatefulWidget {
  const RemoteWheelSlider({
    super.key,
    required this.inputModel,
    required this.cursorModel,
    required this.position,
    this.width = 56,
    this.height = 230,
  });

  final InputModel inputModel;
  final CursorModel cursorModel;
  final DraggableKeyPosition position;
  final double width;
  final double height;

  @override
  State<RemoteWheelSlider> createState() => _RemoteWheelSliderState();
}

class _RemoteWheelSliderState extends State<RemoteWheelSlider> {
  bool _moveMode = false;
  double _thumbOffset = 0.0; // px, +down / -up
  double _scrollIntegral = 0.0;
  Offset? _lastDoubleTapDownLocal;
  Rect? _blockedRect;

  double _getSensitivity() {
    final raw =
        bind.mainGetLocalOption(key: kAndroidTwoFingerScrollSensitivity);
    final parsed = double.tryParse(raw);
    final v = parsed ?? 1.0;
    if (v.isNaN || v.isInfinite) return 1.0;
    return v.clamp(0.5, 3.0);
  }

  void _ensureDefaultPosition(Size screenSize) {
    if (!widget.position.isInvalid()) {
      widget.position.tryAdjust(widget.width, widget.height, 1);
      _updateBlockedRect();
      return;
    }
    final maxX = (screenSize.width - widget.width).clamp(0.0, screenSize.width);
    final maxY =
        (screenSize.height - widget.height).clamp(0.0, screenSize.height);
    final x = (screenSize.width - widget.width - 10).clamp(0.0, maxX);
    final y = ((screenSize.height - widget.height) / 2).clamp(0.0, maxY);
    widget.position.update(Offset(x.toDouble(), y.toDouble()));
    _updateBlockedRect();
  }

  void _moveBy(Offset delta, Size screenSize) {
    final pos = widget.position.pos;
    var x = pos.dx + delta.dx;
    var y = pos.dy + delta.dy;
    x = x.clamp(0.0, screenSize.width - widget.width);
    y = y.clamp(0.0, screenSize.height - widget.height);
    widget.position.update(Offset(x, y));
    _updateBlockedRect();
    setState(() {});
  }

  void _updateBlockedRect() {
    final newRect = Rect.fromLTWH(
      widget.position.pos.dx,
      widget.position.pos.dy,
      widget.width,
      widget.height,
    );
    if (_blockedRect != null) {
      widget.cursorModel.removeBlockedRect(_blockedRect!);
    }
    widget.cursorModel.addBlockedRect(newRect);
    _blockedRect = newRect;
  }

  @override
  void dispose() {
    if (_blockedRect != null) {
      widget.cursorModel.removeBlockedRect(_blockedRect!);
      _blockedRect = null;
    }
    super.dispose();
  }

  void _scrollByDelta(double deltaDy) {
    final sensitivity = _getSensitivity();
    _scrollIntegral += (-deltaDy) / 4 * sensitivity;
    while (_scrollIntegral >= 1) {
      widget.inputModel.scroll(1);
      _scrollIntegral -= 1;
      RemoteInputEventLog.add('wheel_v', data: {'dir': 'down', 'step': 1});
    }
    while (_scrollIntegral <= -1) {
      widget.inputModel.scroll(-1);
      _scrollIntegral += 1;
      RemoteInputEventLog.add('wheel_v', data: {'dir': 'up', 'step': -1});
    }
  }

  bool _isDoubleTapCenter() {
    final p = _lastDoubleTapDownLocal;
    if (p == null) return false;
    final centerY = widget.height / 2;
    return (p.dy - centerY).abs() <= 18;
  }

  @override
  Widget build(BuildContext context) {
    final screenSize = MediaQuery.of(context).size;
    _ensureDefaultPosition(screenSize);

    return Positioned(
      left: widget.position.pos.dx,
      top: widget.position.pos.dy,
      width: widget.width,
      height: widget.height,
      child: Semantics(
        label: 'u2_remote_wheel_slider',
        container: true,
        excludeSemantics: true,
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onDoubleTapDown: (d) => _lastDoubleTapDownLocal = d.localPosition,
          onDoubleTap: () {
            if (_moveMode) {
              setState(() => _moveMode = false);
              return;
            }
            if (_isDoubleTapCenter()) {
              widget.inputModel.tap(MouseButtons.wheel);
              RemoteInputEventLog.add('middle_click');
              return;
            }
            setState(() => _moveMode = true);
          },
          onPanUpdate: (details) {
            if (_moveMode) {
              _moveBy(details.delta, screenSize);
              return;
            }
            setState(() {
              _thumbOffset = (_thumbOffset + details.delta.dy).clamp(
                -(widget.height / 2 - 24),
                (widget.height / 2 - 24),
              );
            });
            _scrollByDelta(details.delta.dy);
          },
          onPanEnd: (_) => setState(() => _thumbOffset = 0),
          onPanCancel: () => setState(() => _thumbOffset = 0),
          child: Container(
            decoration: BoxDecoration(
              color: const Color(0xCC000000),
              borderRadius: BorderRadius.circular(16),
              border: Border.all(
                color: _moveMode ? MyTheme.accent : Colors.white24,
                width: _moveMode ? 1.5 : 1,
              ),
            ),
            child: Stack(
              alignment: Alignment.center,
              children: [
                Positioned(
                  top: 8,
                  child: Icon(
                    _moveMode ? Icons.open_with : Icons.swap_vert,
                    size: 14,
                    color: _moveMode ? Colors.white : Colors.white70,
                  ),
                ),
                AnimatedAlign(
                  alignment:
                      Alignment(0, _thumbOffset / (widget.height / 2 - 24)),
                  duration: const Duration(milliseconds: 260),
                  curve: Curves.elasticOut,
                  child: Container(
                    width: widget.width - 16,
                    height: 36,
                    decoration: BoxDecoration(
                      color: _moveMode
                          ? Colors.white24
                          : Colors.white.withOpacity(0.14),
                      borderRadius: BorderRadius.circular(12),
                      border: Border.all(color: Colors.white24),
                    ),
                  ),
                ),
                const Positioned(
                  bottom: 8,
                  child: Icon(
                    Icons.mouse,
                    size: 14,
                    color: Colors.white70,
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
