class MacOSFullScreenFocusRecovery {
  int _generation = 0;
  int? _pendingGeneration;

  int? get pendingGeneration => _pendingGeneration;

  int queue() {
    _generation += 1;
    _pendingGeneration = _generation;
    return _generation;
  }

  void cancel() {
    _pendingGeneration = null;
  }

  bool isCurrent(int generation) => _pendingGeneration == generation;

  bool consume(int generation) {
    if (!isCurrent(generation)) return false;
    _pendingGeneration = null;
    return true;
  }
}
