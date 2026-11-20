import 'package:flutter/services.dart';

class MobileFloatingCursor {
  // Relative navigation cannot use the remote editor's unavailable font metrics.
  static const characterStep = 12.0;
  static const lineStep = 24.0;
  Offset? _origin;

  void reset() => _origin = null;

  Iterable<String> update(RawFloatingCursorPoint point) sync* {
    if (point.state == FloatingCursorDragState.Start) {
      _origin = point.offset;
      return;
    }
    if (point.state == FloatingCursorDragState.End) {
      reset();
      return;
    }
    final origin = _origin;
    if (origin == null) return;
    final delta = point.offset! - origin;
    final columns = (delta.dx / characterStep).truncate();
    final lines = (delta.dy / lineStep).truncate();
    _origin = origin + Offset(columns * characterStep, lines * lineStep);
    for (var index = 0; index < columns.abs(); index++) {
      yield columns < 0 ? 'VK_LEFT' : 'VK_RIGHT';
    }
    for (var index = 0; index < lines.abs(); index++) {
      yield lines < 0 ? 'VK_UP' : 'VK_DOWN';
    }
  }
}
