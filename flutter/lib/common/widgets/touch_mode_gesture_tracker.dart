/// Distinguishes callbacks from one physical touch sequence from stale ones.
class TouchModeGestureTracker {
  final Set<int> _activePointers = <int>{};

  int _sequence = 0;
  int? _tapDownSequence;
  int? _longPressSequence;
  bool _panHandled = false;

  int get sequence => _sequence;

  void pointerDown(int pointer) {
    if (_activePointers.isEmpty) {
      _sequence++;
      _panHandled = false;
    }
    _activePointers.add(pointer);
  }

  void pointerEnd(int pointer) {
    _activePointers.remove(pointer);
  }

  void recordTapDown() {
    _tapDownSequence = _sequence;
  }

  bool takeCurrentTapDown(int sequence) {
    final isCurrent = _tapDownSequence == sequence;
    _tapDownSequence = null;
    return isCurrent;
  }

  void clearTapDown() {
    _tapDownSequence = null;
  }

  void recordLongPress() {
    _longPressSequence = _sequence;
  }

  bool isCurrentLongPress(int sequence) => _longPressSequence == sequence;

  void claimPan() {
    _panHandled = true;
  }

  bool shouldHandleTap(int sequence) =>
      sequence == _sequence && !_panHandled;
}
