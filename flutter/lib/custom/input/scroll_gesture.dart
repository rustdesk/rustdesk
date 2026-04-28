import 'package:flutter/material.dart';

import 'input_bridge.dart';

class TwoFingerScrollDetector extends StatefulWidget {
  final InputBridge inputBridge;
  final Widget child;
  final double sensitivity;
  final bool inverted;

  const TwoFingerScrollDetector({
    super.key,
    required this.inputBridge,
    required this.child,
    this.sensitivity = 1.0,
    this.inverted = false,
  });

  @override
  State<TwoFingerScrollDetector> createState() =>
      _TwoFingerScrollDetectorState();
}

class _TwoFingerScrollDetectorState extends State<TwoFingerScrollDetector> {
  late final _ScrollAccumulator _acc;

  @override
  void initState() {
    super.initState();
    _acc = _ScrollAccumulator(widget.inputBridge);
  }

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      behavior: HitTestBehavior.translucent,
      onScaleUpdate: (details) {
        if (details.pointerCount != 2) return;
        // Reject pinch-to-zoom
        if ((details.scale - 1.0).abs() > 0.05) return;
        final dx = details.focalPointDelta.dx * widget.sensitivity;
        final dy = details.focalPointDelta.dy *
            widget.sensitivity *
            (widget.inverted ? -1 : 1);
        _acc.add(dx, dy);
      },
      child: widget.child,
    );
  }
}

class _ScrollAccumulator {
  final InputBridge bridge;
  double _x = 0, _y = 0;
  DateTime _last = DateTime.now();

  _ScrollAccumulator(this.bridge);

  void add(double dx, double dy) {
    _x += dx;
    _y += dy;
    final now = DateTime.now();
    if (now.difference(_last).inMilliseconds > 16 &&
        (_x.abs() > 2 || _y.abs() > 2)) {
      bridge.scroll(_x.round(), _y.round());
      _x = 0;
      _y = 0;
      _last = now;
    }
  }
}
