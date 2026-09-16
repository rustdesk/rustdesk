/// Distinguishes callbacks from one physical touch sequence from stale ones.
class TouchModeGestureTracker {
  final Set<int> _activePointers = <int>{};
  final List<int> _pendingTapDowns = <int>[];

  int _sequence = 0;
  int? _panSequence;

  int get sequence => _sequence;

  void pointerDown(int pointer) {
    if (_activePointers.isEmpty) {
      _sequence++;
      _panSequence = null;
    }
    _activePointers.add(pointer);
  }

  void pointerEnd(int pointer) {
    _activePointers.remove(pointer);
  }

  int recordTapDown() {
    _pendingTapDowns.add(_sequence);
    return _sequence;
  }

  int? takeNextTapDown() {
    if (_pendingTapDowns.isEmpty) {
      return null;
    }
    return _pendingTapDowns.removeAt(0);
  }

  void clearTapDowns() {
    _pendingTapDowns.clear();
  }

  void claimPan(int sequence) {
    if (sequence == _sequence) {
      _panSequence = sequence;
    }
  }

  bool shouldHandleTap(int sequence) =>
      sequence == _sequence && sequence != _panSequence;
}

/// Keeps tap delivery tied to the same physical touch across cursor movement.
Future<void> handleTrackedTap({
  required TouchModeGestureTracker tracker,
  required int sequence,
  required Future<bool> Function() move,
  required Future<void> Function() sendTap,
}) async {
  if (!tracker.shouldHandleTap(sequence)) {
    return;
  }
  if (!await move()) {
    return;
  }
  if (!tracker.shouldHandleTap(sequence)) {
    return;
  }
  await sendTap();
}
