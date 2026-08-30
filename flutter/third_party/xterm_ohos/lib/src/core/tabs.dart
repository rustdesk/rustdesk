import 'dart:math' show min;

const _kMaxColumns = 1024;

/// Manages the tab stop state for a terminal.
class TabStops {
  final _stops = List<bool>.filled(_kMaxColumns, false);

  TabStops() {
    _initialize();
  }

  /// Initializes the tab stops to the default 8 column intervals.
  void _initialize() {
    const interval = 8;
    for (var i = 0; i < _kMaxColumns; i += interval) {
      _stops[i] = true;
    }
  }

  /// Finds the next tab stop index, which satisfies [start] <= index < [end].
  int? find(int start, int end) {
    if (start >= end) {
      return null;
    }
    end = min(end, _stops.length);
    for (var i = start; i < end; i++) {
      if (_stops[i]) {
        return i;
      }
    }
    return null;
  }

  /// Sets the tab stop at [index]. If there is already a tab stop at [index],
  /// this method does nothing.
  ///
  /// See also:
  /// * [clearAt] which does the opposite.
  void setAt(int index) {
    assert(index >= 0 && index < _kMaxColumns);
    _stops[index] = true;
  }

  /// Clears the tab stop at [index]. If there is no tab stop at [index], this
  /// method does nothing.
  void clearAt(int index) {
    assert(index >= 0 && index < _kMaxColumns);
    _stops[index] = false;
  }

  /// Clears all tab stops without resetting them to the default 8 column
  /// intervals.
  void clearAll() {
    _stops.fillRange(0, _kMaxColumns, false);
  }

  /// Returns true if there is a tab stop at [index].
  bool isSetAt(int index) {
    return _stops[index];
  }

  /// Resets the tab stops to the default 8 column intervals.
  void reset() {
    clearAll();
    _initialize();
  }
}
