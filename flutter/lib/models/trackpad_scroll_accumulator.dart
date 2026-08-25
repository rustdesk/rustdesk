import 'dart:ui';

enum LinuxTrackpadScrollMode {
  legacy(true),
  highResolutionWheel(true),
  smooth(true);

  const LinuxTrackpadScrollMode(this.usesClientFling);

  final bool usesClientFling;
}

bool shouldStartTrackpadFling({
  required LinuxTrackpadScrollMode scrollMode,
  required bool isViewOnly,
  required Offset delta,
  required double minimumDelta,
}) =>
    !isViewOnly &&
    scrollMode.usesClientFling &&
    (delta.dx.abs() > minimumDelta || delta.dy.abs() > minimumDelta);

class TrackpadScrollAccumulator {
  static const _integerPrecisionTolerance = 1e-9;
  static const _flingStopDelta = 1.0;

  Offset _remainder = Offset.zero;

  Offset takeFling(Offset delta, double unitsPerPoint) {
    if (delta.dx.abs() < _flingStopDelta && delta.dy.abs() < _flingStopDelta) {
      return Offset.zero;
    }
    return take(delta, unitsPerPoint);
  }

  Offset take(Offset delta, double unitsPerPoint) {
    final total = _remainder + delta;
    final scaled = total * unitsPerPoint;
    final emitted = Offset(
      _truncateToInteger(scaled.dx),
      _truncateToInteger(scaled.dy),
    );
    _remainder = total - emitted / unitsPerPoint;
    return emitted;
  }

  static double _truncateToInteger(double value) {
    final nearestInteger = value.roundToDouble();
    if ((value - nearestInteger).abs() <= _integerPrecisionTolerance) {
      return nearestInteger;
    }
    return value.truncateToDouble();
  }
}
