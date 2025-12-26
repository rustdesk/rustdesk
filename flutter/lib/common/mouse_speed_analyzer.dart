import 'dart:collection';
import 'dart:math';

final class MouseSpeedAnalyzer {
  final Queue<MousePosition> _events = Queue<MousePosition>();
  final Duration _window;
  final double _threshold;

  double _currentSum = 0.0;

  MouseSpeedAnalyzer({int windowMilliseconds = 250, double threshold = 60})
    : _window = Duration(milliseconds: windowMilliseconds),
      _threshold = threshold;

  void addEvent(double x, double y) {
    pruneEvents();

    var newEvent = MousePosition(x, y);

    if (_events.isNotEmpty) {
      _currentSum += newEvent - _events.last;
    }

    _events.add(newEvent);
  }

  void pruneEvents() {
    while (_events.isNotEmpty && (_events.first.age > _window)) {
      if (_events.length > 1) {
        _currentSum -= _events.elementAt(1) - _events.first;
      }
      _events.removeFirst();
    }

    if (_events.isEmpty) {
      // Reset sum to prevent any possibility of error accumulation
      _currentSum = 0.0;
    }
  }

  bool get hasExceededThreshold => _currentSum >= _threshold;
}

final class MousePosition {
  final DateTime timestamp = DateTime.now();
  final double x, y;

  MousePosition(this.x, this.y);

  Duration get age => DateTime.now().difference(timestamp);

  double operator -(MousePosition other) {
    double dx = other.x - x;
    double dy = other.y - y;

    return sqrt(dx * dx + dy * dy);
  }
}