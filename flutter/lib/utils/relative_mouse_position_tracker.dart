import 'dart:ui';

class RelativeMousePositionTracker {
  Offset? _lastPosition;
  Offset? _recenterTarget;

  Offset? get lastPosition => _lastPosition;

  void recenterTo(Offset? target) {
    _recenterTarget = target;
  }

  Offset? update(Offset position) {
    final previous = _lastPosition;
    _lastPosition = position;
    if (previous == null) {
      _recenterTarget = null;
      return Offset.zero;
    }

    final delta = position - previous;
    final target = _recenterTarget;
    // A queued move near the old position can arrive before the warp, and
    // the first sample after the warp can already be away from the target.
    if (target != null &&
        (position - target).distanceSquared <= delta.distanceSquared) {
      _recenterTarget = null;
      return null;
    }
    return delta;
  }

  void reset() {
    _lastPosition = null;
    _recenterTarget = null;
  }
}
